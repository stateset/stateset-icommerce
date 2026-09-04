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
    _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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
  const accepted = await r2.json();
  const { funding, order, inventory_reservation: reservation } = accepted;
  assert.ok(funding.escrow_id.startsWith('0x'));
  assert.equal(funding.chain, 'base-sepolia');
  assert.ok(order.order_id.startsWith('ord_'));
  assert.equal(order.status, 'authorized');
  assert.equal(reservation.status, 'reserved');
  assert.equal(reservation.items[0].available_after, 45);

  const retry = await fetch(`${baseUrl}/icp/v1/quotes/${quote.quote_id}/accept`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({}),
  });
  assert.equal(retry.status, 200);
  const retried = await retry.json();
  assert.equal(retried.order.order_id, order.order_id);
  assert.deepEqual(retried.inventory_reservation, reservation);

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
    _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
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

  // .well-known/icp advertises subscription.cancel + other verbs
  const caps = await (await fetch(`${baseUrl}/icp/v1/.well-known/icp`)).json();
  assert.ok(caps.capabilities.verbs.includes('subscription.cancel'));
  assert.ok(caps.capabilities.verbs.length >= 5);
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
    }),
  });
  const j = await r.json();
  assert.equal(r.status, 200);
  // Demo: ANNUAL subscriptions downgrade to end-of-period with no refund
  assert.equal(j.authorization.final_occurrences, 1);
  assert.equal(j.authorization.pro_rated_refund, null);
});

test('quote.request → signed PriceProposal with volume tier discount', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'quote.request',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantRfq',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 500 }],
    purchase_window: '30d',
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['quote.request'] },
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
    }),
  });
  const rBody = await r.json();
  assert.equal(r.status, 200, JSON.stringify(rBody));
  const { proposal } = rBody;
  assert.equal(proposal.type, 'price.proposal');
  // 500 × $29.99 with 20% volume discount = 500 × $23.992 = $11996.00
  assert.equal(proposal.items[0].volume_tier, '500+');
  assert.equal(proposal.items[0].unit_price.amount, '23.99');
  assert.equal(proposal.total.amount, '11996.00');
  assert.ok(proposal.valid_until);
  assert.ok(proposal.proposal_id.startsWith('icp_pp_'));
});

test('purchase.create with from_proposal_id honors proposal prices', async () => {
  const now = new Date();
  const quoteIntent = {
    v: 'icp-1.0',
    verb: 'quote.request',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantRfq',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 100 }],
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['quote.request', 'purchase.create'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300 * 1000).toISOString(),
  };
  const qr = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent: quoteIntent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: signEd25519(canonicalJson(quoteIntent), buyerKp.privateKey) },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
    }),
  });
  const { proposal } = await qr.json();
  // 100 × $29.99 with 10% volume tier = $2699.10
  assert.equal(proposal.total.amount, '2699.10');

  const now2 = new Date();
  const purchaseIntent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantRfq',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 100, unit_price: { amount: '26.99', currency: 'USDC' } }],
    max_total: { amount: '2699.10', currency: 'USDC' },
    from_proposal_id: proposal.proposal_id,
    expiry: new Date(now2.getTime() + 300 * 1000).toISOString(),
    principal_binding: quoteIntent.principal_binding,
    nonce: newNonceHex(),
    iat: now2.toISOString(),
    exp: new Date(now2.getTime() + 300 * 1000).toISOString(),
  };
  const pr = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent: purchaseIntent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: signEd25519(canonicalJson(purchaseIntent), buyerKp.privateKey) },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
    }),
  });
  const prBody = await pr.json();
  assert.equal(pr.status, 200, JSON.stringify(prBody));
  // Quote should match proposal exactly — NO 5% handling fee applied
  assert.equal(prBody.quote.total.amount, '2699.10');
  assert.equal(prBody.quote.from_proposal_id, proposal.proposal_id);
});

test('purchase.create with unknown from_proposal_id is rejected', async () => {
  const now = new Date();
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zMerchantRfq',
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'X', quantity: 1, unit_price: { amount: '1', currency: 'USDC' } }],
    max_total: { amount: '2', currency: 'USDC' },
    from_proposal_id: 'icp_pp_DOESNOTEXIST00000000000001',
    expiry: new Date(now.getTime() + 300 * 1000).toISOString(),
    principal_binding: {
      principal: 'did:web:test.example',
      agent: buyerAid,
      authority: { max_per_intent: { amount: '100', currency: 'USDC' }, verbs: ['purchase.create'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300 * 1000).toISOString(),
  };
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: buyerAid, sig: signEd25519(canonicalJson(intent), buyerKp.privateKey) },
      _pubkey_hex: buyerEdPubRaw.toString('hex'),
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
    }),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'quote.proposal_not_found');
});

test('payout.request → signed PayoutAuthorization with itemized fees', async () => {
  const now = new Date();
  const exp = new Date(now.getTime() + 300 * 1000);
  const intent = {
    v: 'icp-1.0',
    verb: 'payout.request',
    intent_id: newId('icp_int'),
    seller: buyerAid,
    platform: 'aid:v1:zMarketplacePlatform',
    settler: 'settler:stateset.usdc.base-sepolia',
    amount: { amount: '1000.00', currency: 'USDC' },
    destination: { type: 'wallet', wallet_address: '0x1111111111111111111111111111111111111111' },
    expedited: false,
    principal_binding: {
      principal: 'did:web:seller-corp.example',
      agent: buyerAid,
      authority: {
        max_per_intent: { amount: '0', currency: 'USDC' },
        max_per_payout: { amount: '2000', currency: 'USDC' },
        verbs: ['payout.request'],
      },
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
      _x_pubkey_hex: buyerXPubRaw.toString('hex'),
    }),
  });
  const rBody = await r.json();
  assert.equal(r.status, 200, JSON.stringify(rBody));
  const { authorization } = rBody;
  assert.equal(authorization.type, 'payout.authorization');
  assert.equal(authorization.seller, buyerAid);
  // 3% commission on $1000 = $30; 1% reserve = $10; approved = $960
  assert.equal(authorization.fees.length, 2);
  assert.equal(authorization.fees[0].type, 'platform_commission');
  assert.equal(authorization.fees[0].amount.amount, '30.00');
  assert.equal(authorization.fees[1].type, 'chargeback_reserve');
  assert.equal(authorization.fees[1].amount.amount, '10.00');
  assert.ok(authorization.fees[1].release_at);
  assert.equal(authorization.approved_amount.amount, '960.00');
  assert.ok(authorization.payout_id.startsWith('icp_pay_'));

  // .well-known/icp advertises the 7 commerce verbs (100% commerce coverage),
  // plus any additional operational verbs like channel.register from ICPIP-0005.
  const caps = await (await fetch(`${baseUrl}/icp/v1/.well-known/icp`)).json();
  for (const verb of [
    'purchase.create',
    'subscription.create',
    'subscription.cancel',
    'purchase.return',
    'inventory.query',
    'quote.request',
    'payout.request',
  ]) {
    assert.ok(caps.capabilities.verbs.includes(verb), `missing verb: ${verb}`);
  }
});

test('payout.request: insufficient balance is rejected', async () => {
  const freshKp = generateKeyPairSync('ed25519');
  const freshXkp = generateKeyPairSync('x25519');
  const freshEdPubRaw = publicKeyToRaw(freshKp.publicKey);
  const freshXPubRaw = publicKeyToRaw(freshXkp.publicKey);
  const freshAid = `aid:v1:z${base58btcEncode(
    createHash('sha256').update(Buffer.concat([freshEdPubRaw, Buffer.from([0x00]), freshXPubRaw])).digest()
  )}`;

  const now = new Date();
  const intent = {
    v: 'icp-1.0',
    verb: 'payout.request',
    intent_id: newId('icp_int'),
    seller: freshAid,
    platform: 'aid:v1:zMarketplacePlatform',
    settler: 'settler:stateset.usdc.base-sepolia',
    amount: { amount: '10000.00', currency: 'USDC' },
    destination: { type: 'wallet', wallet_address: '0x2222222222222222222222222222222222222222' },
    principal_binding: {
      principal: 'did:web:test.example',
      agent: freshAid,
      authority: { max_per_intent: { amount: '0', currency: 'USDC' }, verbs: ['payout.request'] },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300 * 1000).toISOString(),
  };
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: freshAid, sig: signEd25519(canonicalJson(intent), freshKp.privateKey) },
      _pubkey_hex: freshEdPubRaw.toString('hex'),
      _x_pubkey_hex: freshXPubRaw.toString('hex'),
    }),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'policy.payout.insufficient_balance');
});

test('payout.request: exceeds max_per_payout is rejected', async () => {
  const freshKp = generateKeyPairSync('ed25519');
  const freshXkp = generateKeyPairSync('x25519');
  const freshEdPubRaw = publicKeyToRaw(freshKp.publicKey);
  const freshXPubRaw = publicKeyToRaw(freshXkp.publicKey);
  const freshAid = `aid:v1:z${base58btcEncode(
    createHash('sha256').update(Buffer.concat([freshEdPubRaw, Buffer.from([0x00]), freshXPubRaw])).digest()
  )}`;

  const now = new Date();
  const intent = {
    v: 'icp-1.0',
    verb: 'payout.request',
    intent_id: newId('icp_int'),
    seller: freshAid,
    platform: 'aid:v1:zMarketplacePlatform',
    settler: 'settler:stateset.usdc.base-sepolia',
    amount: { amount: '4000.00', currency: 'USDC' },
    destination: { type: 'wallet', wallet_address: '0x3333333333333333333333333333333333333333' },
    principal_binding: {
      principal: 'did:web:test.example',
      agent: freshAid,
      authority: {
        max_per_intent: { amount: '0', currency: 'USDC' },
        max_per_payout: { amount: '500', currency: 'USDC' },
        verbs: ['payout.request'],
      },
      expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
      revocation: 'https://test.example/revoke',
      signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' },
    },
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300 * 1000).toISOString(),
  };
  const r = await fetch(`${baseUrl}/icp/v1/intents`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      intent,
      signature: { alg: 'ed25519', kid: freshAid, sig: signEd25519(canonicalJson(intent), freshKp.privateKey) },
      _pubkey_hex: freshEdPubRaw.toString('hex'),
      _x_pubkey_hex: freshXPubRaw.toString('hex'),
    }),
  });
  assert.equal(r.status, 422);
  const j = await r.json();
  assert.equal(j.code, 'policy.payout.exceeds_max_per_payout');
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
