// POST /icp/v1/settlements/cosign — the merchant counter-signature on a
// receipt an external Settler produced.
//
// Verifying the Settler's signature and the amount is not enough: the receipt
// also names a `settlement_id` and an `escrow_id`, and neither was checked.
// A Settler holding one valid key could therefore co-sign the same intent
// twice under two settlement IDs, or bind an escrow belonging to a different
// intent to this one — a merchant-signed double settlement.
//
// Run: PORT=0 node --test test/cosign.test.mjs

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { generateKeyPairSync } from 'node:crypto';

import {
  canonicalJson,
  deriveAidFromPubkeys,
  newId,
  newNonceHex,
  publicKeyToRaw,
  signEd25519,
  signQuoteAcceptance,
} from '../src/codec.mjs';

const settlerKp = generateKeyPairSync('ed25519');
const SETTLER = 'settler:stateset.usdc.base-sepolia';
process.env.PORT ??= '0';
process.env.ICP_SETTLER_KEYS_JSON = JSON.stringify({
  [SETTLER]: publicKeyToRaw(settlerKp.publicKey).toString('hex'),
});
const { server } = await import('../src/server.mjs');

const buyerKp = generateKeyPairSync('ed25519');
const buyerXkp = generateKeyPairSync('x25519');
const buyerEdPubRaw = publicKeyToRaw(buyerKp.publicKey);
const buyerXPubRaw = publicKeyToRaw(buyerXkp.publicKey);
const buyerAid = deriveAidFromPubkeys(buyerEdPubRaw, buyerXPubRaw);

let baseUrl;
before(async () => {
  if (!server.listening) await new Promise((resolve) => server.once('listening', resolve));
  baseUrl = `http://127.0.0.1:${server.address().port}`;
});
after(() => server.close());

async function post(path, body) {
  const response = await fetch(`${baseUrl}${path}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  });
  return { status: response.status, json: await response.json() };
}

/** Drive one purchase to an accepted quote and return its identities. */
async function acceptedPurchase(sku = 'WIDGET-001') {
  const now = new Date();
  const intent = {
    v: 'icp-1.0',
    verb: 'purchase.create',
    intent_id: newId('icp_int'),
    buyer: buyerAid,
    merchant: 'aid:v1:zCosignTestMerchant',
    settler: SETTLER,
    items: [{ sku, quantity: 1, unit_price: { amount: '10.00', currency: 'USDC' } }],
    max_total: { amount: '40.00', currency: 'USDC' },
    expiry: new Date(now.getTime() + 300_000).toISOString(),
    nonce: newNonceHex(),
    iat: now.toISOString(),
    exp: new Date(now.getTime() + 300_000).toISOString(),
  };
  const submitted = await post('/icp/v1/intents', {
    intent,
    signature: {
      alg: 'ed25519',
      kid: buyerAid,
      sig: signEd25519(canonicalJson(intent), buyerKp.privateKey),
    },
    _pubkey_hex: buyerEdPubRaw.toString('hex'),
    _x_pubkey_hex: buyerXPubRaw.toString('hex'),
  });
  assert.equal(submitted.status, 200, JSON.stringify(submitted.json));
  const quote = submitted.json.quote;
  const accepted = await post(
    `/icp/v1/quotes/${quote.quote_id}/accept`,
    signQuoteAcceptance(quote.quote_id, buyerAid, buyerKp.privateKey),
  );
  assert.equal(accepted.status, 200, JSON.stringify(accepted.json));
  return {
    intentId: intent.intent_id,
    escrowId: accepted.json.funding.escrow_id,
    total: quote.total,
  };
}

/** A Settler-signed receipt, exactly as the co-sign endpoint canonicalizes it. */
function settlerReceipt(purchase, overrides = {}) {
  const receipt = {
    type: 'icp.settlement.receipt',
    v: 'icp-1.0',
    settlement_id: newId('icp_set'),
    settler: SETTLER,
    escrow_id: purchase.escrowId,
    intent_id: purchase.intentId,
    final_state: 'released',
    amount: purchase.total,
    rail: 'external-settler',
    rail_txid: `0x${'ab'.repeat(32)}`,
    settled_at: new Date().toISOString(),
    released_to: '0x1111111111111111111111111111111111111111',
    ...overrides,
  };
  return {
    ...receipt,
    settler_signature: {
      alg: 'ed25519',
      kid: SETTLER,
      sig: signEd25519(canonicalJson(receipt), settlerKp.privateKey),
    },
  };
}

test('a well-formed external receipt is co-signed', async () => {
  const purchase = await acceptedPurchase();
  const receipt = settlerReceipt(purchase);
  const { status, json } = await post('/icp/v1/settlements/cosign', { receipt });
  assert.equal(status, 200, JSON.stringify(json));
  assert.equal(json.receipt.settlement_id, receipt.settlement_id);
  assert.ok(json.receipt.merchant_signature?.sig);

  // Re-submitting the identical receipt is idempotent, not a second settlement.
  const again = await post('/icp/v1/settlements/cosign', { receipt });
  assert.equal(again.status, 200, JSON.stringify(again.json));
  assert.deepEqual(again.json.receipt, json.receipt);
});

test('a receipt without a usable settlement_id is rejected', async () => {
  const purchase = await acceptedPurchase();
  for (const settlement_id of [undefined, '', '   ', 42, { id: 'x' }, 'a'.repeat(300)]) {
    const { status, json } = await post('/icp/v1/settlements/cosign', {
      receipt: settlerReceipt(purchase, { settlement_id }),
    });
    assert.equal(status, 400, `settlement_id ${JSON.stringify(settlement_id)}`);
    assert.equal(json.code, 'format.missing_field');
  }
});

test('a receipt cannot bind an escrow that belongs to another intent', async () => {
  const mine = await acceptedPurchase();
  const other = await acceptedPurchase('WIDGET-003');
  assert.notEqual(mine.escrowId, other.escrowId);

  const stolen = settlerReceipt(mine, { escrow_id: other.escrowId });
  const { status, json } = await post('/icp/v1/settlements/cosign', { receipt: stolen });
  assert.equal(status, 409);
  assert.equal(json.code, 'settlement.escrow_mismatch');

  const unknown = settlerReceipt(mine, { escrow_id: '0xdeadbeef' });
  const missing = await post('/icp/v1/settlements/cosign', { receipt: unknown });
  assert.equal(missing.status, 409);
  assert.equal(missing.json.code, 'settlement.escrow_mismatch');
});

test('a second co-sign for an already-settled intent is a conflict', async () => {
  const purchase = await acceptedPurchase('GADGET-A');
  const first = await post('/icp/v1/settlements/cosign', { receipt: settlerReceipt(purchase) });
  assert.equal(first.status, 200, JSON.stringify(first.json));

  // A fresh settlement_id over the same escrow/intent is a double settlement.
  const second = await post('/icp/v1/settlements/cosign', { receipt: settlerReceipt(purchase) });
  assert.equal(second.status, 409);
  assert.equal(second.json.code, 'settlement.already_settled');

  // …including a refund receipt, which needs its own governed flow.
  const refund = await post('/icp/v1/settlements/cosign', {
    receipt: settlerReceipt(purchase, { final_state: 'refunded' }),
  });
  assert.equal(refund.status, 409);
  assert.equal(refund.json.code, 'settlement.already_settled');
});

test('an escrow settled by fulfillment cannot then be co-signed externally', async () => {
  const purchase = await acceptedPurchase('GADGET-B');
  const fulfilled = await post(`/icp/v1/escrows/${purchase.escrowId}/fulfill`, {});
  assert.equal(fulfilled.status, 200, JSON.stringify(fulfilled.json));

  const { status, json } = await post('/icp/v1/settlements/cosign', {
    receipt: settlerReceipt(purchase),
  });
  assert.equal(status, 409);
  assert.equal(json.code, 'settlement.already_settled');
});
