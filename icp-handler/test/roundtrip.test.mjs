// End-to-end test: build a real signed Intent, POST it to the running
// handler, accept the returned Quote, fulfill the escrow, retrieve the
// SettlementReceipt. Uses node's built-in test runner — no deps.
//
// Run: node --test test/roundtrip.test.mjs

import { test, after, before } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync } from 'node:crypto';

import {
  canonicalJson,
  signEd25519,
  publicKeyToRaw,
  newId,
  newNonceHex,
  base58btcEncode,
} from '../src/codec.mjs';
import { server } from '../src/server.mjs';
import { createHash } from 'node:crypto';

let baseUrl;
const buyerKp = generateKeyPairSync('ed25519');
const buyerXkp = generateKeyPairSync('x25519');
const buyerEdPubRaw = publicKeyToRaw(buyerKp.publicKey);
const buyerXPubRaw = publicKeyToRaw(buyerXkp.publicKey);

const buyerAid = (() => {
  const buf = Buffer.concat([buyerEdPubRaw, Buffer.from([0x00]), buyerXPubRaw]);
  const digest = createHash('sha256').update(buf).digest();
  return `aid:v1:z${base58btcEncode(digest)}`;
})();

before(async () => {
  await new Promise((resolve) => {
    if (server.listening) return resolve();
    server.once('listening', resolve);
  });
  const addr = server.address();
  baseUrl = `http://127.0.0.1:${addr.port}`;
});

after(() => server.close());

function buildSignedIntent() {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantPlaceholder',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 2, unit_price: { amount: '29.99', currency: 'USDC' } }],
    max_total: { amount: '70.00', currency: 'USDC' },
    expiry: exp.toISOString(),
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '500', currency: 'USDC' }, verbs: ['purchase.create'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, buyerKp.privateKey);
  return {
    intent,
    signature: { alg: 'ed25519', kid: buyerAid, sig },
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
  };
}

test('GET /healthz returns ok', async () => {
  const r = await fetch(`${baseUrl}/healthz`);
  assert.equal(r.status, 200);
  const j = await r.json();
  assert.equal(j.ok, true);
});

test('GET /icp/v1/.well-known/icp advertises capabilities', async () => {
  const r = await fetch(`${baseUrl}/icp/v1/.well-known/icp`);
  assert.equal(r.status, 200);
  const j = await r.json();
  assert.equal(j.spec, 'icp-1.0');
  assert.ok(j.merchant_aid?.startsWith('aid:v1:'));
  assert.ok(Array.isArray(j.settler_allowlist) && j.settler_allowlist.length > 0);
});

test('Intent → Quote → Accept → Fulfill → SettlementReceipt', async () => {
  const body = buildSignedIntent();

  // 1. Submit Intent → get Quote
  const r1 = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const r1Body = await r1.json();
  assert.equal(r1.status, 200, JSON.stringify(r1Body));
  const { quote, signature: merchantSig } = r1Body;
  assert.equal(quote.v, 'icp-1.0');
  assert.equal(quote.intent_id, body.intent.intent_id);
  assert.ok(merchantSig.sig);
  // Total = 2 * 29.99 * 1.05 = 62.979 → 62.98
  assert.equal(quote.total.amount, '62.98');

  // 2. Accept Quote → get funding instructions + escrow_id
  const r2 = await fetch(`${baseUrl}/icp/v1/quotes/${quote.quote_id}/accept`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({}),
  });
  assert.equal(r2.status, 200);
  const { funding } = await r2.json();
  assert.ok(funding.escrow_id.startsWith('0x'));
  assert.equal(funding.chain, 'base-sepolia');

  // 3. Fulfill (stub auto-funds + auto-releases for demo)
  const r3 = await fetch(`${baseUrl}/icp/v1/escrows/${funding.escrow_id}/fulfill`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ evidence_id: 'icp_ful_TEST' }),
  });
  assert.equal(r3.status, 200);
  const { receipt } = await r3.json();
  assert.equal(receipt.final_state, 'released');
  assert.equal(receipt.amount.amount, '62.98');
  assert.ok(receipt.merchant_signature?.sig);
  assert.ok(receipt.settler_signature?.sig);

  // 4. Re-fetch the SettlementReceipt by ID
  const r4 = await fetch(`${baseUrl}/icp/v1/settlements/${receipt.settlement_id}`);
  assert.equal(r4.status, 200);
  const fetched = await r4.json();
  assert.equal(fetched.settlement_id, receipt.settlement_id);
});

test('Intent with bad signature is rejected', async () => {
  const body = buildSignedIntent();
  body.signature.sig = '00'.repeat(64);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(r.status, 401);
  const j = await r.json();
  assert.equal(j.code, 'signature.invalid');
});

test('Intent with disallowed Settler is rejected', async () => {
  const body = buildSignedIntent();
  body.intent.settler = 'settler:evil.usdc.fake';
  // Re-sign because we changed the payload.
  body.signature.sig = signEd25519(canonicalJson(body.intent), buyerKp.privateKey);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(r.status, 400);
  const j = await r.json();
  assert.equal(j.code, 'policy.settler.not_allowed');
});

test('subscription.create Intent → signed SubscriptionAuthorization', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const firstCharge = new Date(now.getTime() + 86400 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'subscription.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantSaaS',
    settler: 'settler:stateset.usdc.base-sepolia',
    service_id: 'premium-monthly',
    cadence: '30d',
    max_total_per_period: { amount: '29.99', currency: 'USDC' },
    max_occurrences: 12,
    first_charge_at: firstCharge.toISOString(),
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: {
        max_per_intent: { amount: '500', currency: 'USDC' },
        verbs: ['purchase.create', 'subscription.create'],
      },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, buyerKp.privateKey);
  const body = {
    intent,
    signature: { alg: 'ed25519', kid: buyerAid, sig },
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
  };

  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  const rBody = await r.json();
  assert.equal(r.status, 200, JSON.stringify(rBody));
  const { authorization, signature: merchantSig } = rBody;

  assert.equal(authorization.type, 'subscription.authorization');
  assert.equal(authorization.v, 'icp-1.0');
  assert.equal(authorization.intent_id, intent.intent_id);
  assert.equal(authorization.cadence, '30d');
  assert.equal(authorization.max_total_per_period.amount, '29.99');
  assert.equal(authorization.max_occurrences, 12);
  assert.equal(authorization.merchant_terms.refund_policy, 'pro-rated');
  assert.ok(authorization.subscription_id.startsWith('icp_sub_'));
  assert.equal(merchantSig.alg, 'ed25519');
  assert.equal(Buffer.from(merchantSig.sig, 'hex').length, 64);

  // Capabilities endpoint now advertises subscription.create
  const caps = await (await fetch(`${baseUrl}/icp/v1/.well-known/icp`)).json();
  assert.ok(caps.capabilities.verbs.includes('subscription.create'));
});

test('subscription with per-period cap above demo policy is rejected', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'subscription.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantSaaS',
    settler: 'settler:stateset.usdc.base-sepolia',
    service_id: 'enterprise',
    cadence: '30d',
    max_total_per_period: { amount: '5000.00', currency: 'USDC' }, // > $1000 cap
    max_occurrences: null,
    first_charge_at: new Date(now.getTime() + 86400 * 1000).toISOString(),
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: {
        max_per_intent: { amount: '100000', currency: 'USDC' },
        verbs: ['subscription.create'],
      },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, buyerKp.privateKey);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'policy.value_above_kyc_floor');
});

test('purchase.return Intent → signed ReturnAuthorization', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.return',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantRet',
    settler: 'settler:stateset.usdc.base-sepolia',
    original_settlement_id: 'icp_set_01HXYZORIGINAL000000000001',
    items: [{ sku: 'WIDGET-001', quantity: 2, reason: 'defective' }],
    desired_outcome: 'refund',
    max_refund: { amount: '60.00', currency: 'USDC' },
    narrative: 'Both widgets arrived broken',
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '500', currency: 'USDC' }, verbs: ['purchase.return'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, buyerKp.privateKey);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  const rBody = await r.json();
  assert.equal(r.status, 200, JSON.stringify(rBody));
  const { authorization, signature: merchantSig } = rBody;

  assert.equal(authorization.type, 'return.authorization');
  assert.equal(authorization.v, 'icp-1.0');
  assert.equal(authorization.intent_id, intent.intent_id);
  assert.equal(authorization.original_settlement_id, intent.original_settlement_id);
  assert.equal(authorization.outcome, 'refund');
  // 2 items × $10/item = $20 demo refund, well under $60 cap
  assert.equal(authorization.refund.amount.amount, '20.00');
  assert.equal(authorization.refund.amount.currency, 'USDC');
  assert.ok(authorization.return_id.startsWith('icp_ret_'));
  assert.ok(authorization.merchant_terms.rma_code.startsWith('RMA-'));
  assert.equal(merchantSig.alg, 'ed25519');
  assert.equal(Buffer.from(merchantSig.sig, 'hex').length, 64);

  // Capabilities endpoint advertises 3 verbs
  const caps = await (await fetch(`${baseUrl}/icp/v1/.well-known/icp`)).json();
  assert.ok(caps.capabilities.verbs.includes('purchase.return'));
});

test('purchase.return: large no-fault return rejected per demo policy', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.return',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantRet',
    settler: 'settler:stateset.usdc.base-sepolia',
    original_settlement_id: 'icp_set_01HXYZORIGINAL000000000099',
    // 100 items × $10 = $1000 refund, capped to max_refund $1000, and reason "no-longer-needed"
    items: Array.from({ length: 100 }, (_, i) => ({
      sku: `BULK-${i}`,
      quantity: 1,
      reason: 'no-longer-needed',
    })),
    desired_outcome: 'refund',
    max_refund: { amount: '1000.00', currency: 'USDC' },
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '5000', currency: 'USDC' }, verbs: ['purchase.return'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, buyerKp.privateKey);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'policy.return.not_eligible');
});

test('inventory.query → signed InventorySnapshot with 5 SKUs', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'inventory.query',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantInv',
    settler: 'settler:stateset.usdc.base-sepolia',
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['inventory.query'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: signEd25519(canonicalJson(intent), buyerKp.privateKey) },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  const rBody = await r.json();
  assert.equal(r.status, 200, JSON.stringify(rBody));
  const { snapshot, signature: merchantSig } = rBody;

  assert.equal(snapshot.type, 'inventory.snapshot');
  assert.equal(snapshot.intent_id, intent.intent_id);
  assert.equal(snapshot.items.length, 5);
  assert.ok(snapshot.snapshot_id.startsWith('icp_inv_'));
  assert.ok(snapshot.valid_until);
  assert.equal(merchantSig.alg, 'ed25519');
  assert.equal(Buffer.from(merchantSig.sig, 'hex').length, 64);

  // .well-known/icp advertises 5 verbs (after subscription.cancel)
  const caps = await (await fetch(`${baseUrl}/icp/v1/.well-known/icp`)).json();
  assert.ok(caps.capabilities.verbs.includes('inventory.query'));
  assert.ok(caps.capabilities.verbs.length >= 4);
});

test('inventory.query with in_stock_only filter excludes out-of-stock SKUs', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'inventory.query',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantInv',
    settler: 'settler:stateset.usdc.base-sepolia',
    filters: { in_stock_only: true },
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['inventory.query'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: signEd25519(canonicalJson(intent), buyerKp.privateKey) },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  const j = await r.json();
  assert.equal(r.status, 200);
  // WIDGET-002 has available_quantity: 0; should be filtered out
  assert.equal(j.snapshot.items.length, 4);
  assert.ok(!j.snapshot.items.some((it) => it.sku === 'WIDGET-002'));
});

test('subscription.cancel (immediate) → CancellationAuthorization with pro-rated refund', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'subscription.cancel',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantSaaS',
    settler: 'settler:stateset.usdc.base-sepolia',
    subscription_id: 'icp_sub_01HXYZTESTSUBSCRIPTION0001',
    effective: 'immediate',
    reason: 'no-longer-needed',
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['subscription.cancel'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, buyerKp.privateKey);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  const rBody = await r.json();
  assert.equal(r.status, 200, JSON.stringify(rBody));
  const { authorization } = rBody;
  assert.equal(authorization.type, 'subscription.cancellation');
  assert.equal(authorization.subscription_id, intent.subscription_id);
  assert.equal(authorization.final_occurrences, 0);
  assert.ok(authorization.pro_rated_refund);
  assert.equal(authorization.pro_rated_refund.amount.amount, '7.50');

  // .well-known/icp now advertises 5 verbs
  const caps = await (await fetch(`${baseUrl}/icp/v1/.well-known/icp`)).json();
  assert.ok(caps.capabilities.verbs.includes('subscription.cancel'));
  assert.equal(caps.capabilities.verbs.length, 5);
});

test('subscription.cancel on ANNUAL subscription downgrades to end-of-period', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'subscription.cancel',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantSaaS',
    settler: 'settler:stateset.usdc.base-sepolia',
    subscription_id: 'icp_sub_01HXYZTESTSUB000000000ANNUAL',
    effective: 'immediate',
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['subscription.cancel'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: exp.toISOString(),
  };
  const canonical = canonicalJson(intent);
  const sig = signEd25519(canonical, buyerKp.privateKey);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
    }),
  });
  const j = await r.json();
  assert.equal(r.status, 200);
  // Demo: ANNUAL subscriptions downgrade to end-of-period with no refund
  assert.equal(j.authorization.final_occurrences, 1);
  assert.equal(j.authorization.pro_rated_refund, null);
});

test('Intent over max_total is rejected with policy error', async () => {
  const body = buildSignedIntent();
  body.intent.max_total = { amount: '50.00', currency: 'USDC' }; // < 62.98
  body.signature.sig = signEd25519(canonicalJson(body.intent), buyerKp.privateKey);
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'policy.quote.exceeds_max_total');
});
