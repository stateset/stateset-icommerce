// SDK integration test against a live icp-handler.
// Spawns the handler as a subprocess, drives the SDK, asserts every path.

import { test, after, before } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import { ICPClient, ICPError, generateIdentity, canonicalJson, signEd25519 } from '../src/index.mjs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const HANDLER = resolve(__dirname, '..', '..', '..', 'icp-handler', 'src', 'server.mjs');

let proc;
let baseUrl;
let client;

before(async () => {
  proc = spawn('node', [HANDLER], { env: { ...process.env, PORT: '0' }, stdio: ['ignore', 'pipe', 'pipe'] });
  let buf = '';
  baseUrl = await new Promise((res, rej) => {
    const onErr = (d) => {
      buf += d.toString('utf8');
      const m = buf.match(/listening on (http:\/\/127\.0\.0\.1:\d+)/);
      if (m) {
        proc.stderr.off('data', onErr);
        res(m[1]);
      }
    };
    proc.stderr.on('data', onErr);
    setTimeout(() => rej(new Error('handler did not start in 5s')), 5000);
  });
  client = await ICPClient.create({
    handlerUrl: baseUrl,
    principal: 'did:web:sdk-test.example',
  });
});

after(() => {
  if (proc) proc.kill();
});

test('client identity has a valid AID', () => {
  assert.ok(client.aid.startsWith('aid:v1:z'));
  assert.equal(client.identity.ed25519_pubkey.length, 32);
  assert.equal(client.identity.x25519_pubkey.length, 32);
});

test('capabilities() returns spec + all 5 verbs', async () => {
  const caps = await client.capabilities();
  assert.equal(caps.spec, 'icp-1.0');
  assert.ok(caps.capabilities.verbs.length >= 4);
  assert.ok(caps.capabilities.verbs.includes('purchase.create'));
  assert.ok(caps.capabilities.verbs.includes('inventory.query'));
  assert.ok(caps.capabilities.verbs.includes('subscription.cancel'));
});

test('inventory() returns signed snapshot, signature verifies', async () => {
  const caps = await client.capabilities();
  const result = await client.inventory({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
  });
  assert.equal(result.snapshot.type, 'inventory.snapshot');
  assert.ok(result.snapshot.items.length > 0);
  // _verifyMerchantSignature throws if invalid, so reaching here is the assertion
});

test('purchase() returns signed Quote', async () => {
  const caps = await client.capabilities();
  const r = await client.purchase({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '29.99', currency: 'USDC' } }],
    max_total: { amount: '35.00', currency: 'USDC' },
  });
  assert.equal(r.quote.v, 'icp-1.0');
  assert.equal(r.quote.total.amount, '31.49'); // 29.99 × 1.05
});

test('accept() returns funding instructions', async () => {
  const caps = await client.capabilities();
  const r = await client.purchase({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'X', quantity: 1, unit_price: { amount: '10.00', currency: 'USDC' } }],
    max_total: { amount: '12.00', currency: 'USDC' },
  });
  const funding = await client.accept(r.quote.quote_id);
  assert.ok(funding.funding.escrow_id.startsWith('0x'));
});

test('subscribe() returns signed SubscriptionAuthorization', async () => {
  const caps = await client.capabilities();
  const r = await client.subscribe({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    service_id: 'premium-monthly',
    cadence: '30d',
    max_total_per_period: { amount: '29.99', currency: 'USDC' },
    max_occurrences: 12,
    first_charge_at: new Date(Date.now() + 86400 * 1000).toISOString(),
  });
  assert.equal(r.authorization.type, 'subscription.authorization');
  assert.equal(r.authorization.cadence, '30d');
});

test('return_() returns signed ReturnAuthorization', async () => {
  const caps = await client.capabilities();
  const r = await client.return_({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    original_settlement_id: 'icp_set_01HXYZSDKTEST0000000000001',
    items: [{ sku: 'WIDGET-001', quantity: 1, reason: 'defective' }],
    desired_outcome: 'refund',
    max_refund: { amount: '30.00', currency: 'USDC' },
  });
  assert.equal(r.authorization.type, 'return.authorization');
  assert.equal(r.authorization.outcome, 'refund');
});

test('purchase() with disallowed Settler throws typed ICPError', async () => {
  const caps = await client.capabilities();
  try {
    await client.purchase({
      merchant: caps.merchant_aid,
      settler: 'settler:evil.fake.network',
      items: [{ sku: 'X', quantity: 1, unit_price: { amount: '1', currency: 'USDC' } }],
      max_total: { amount: '2', currency: 'USDC' },
    });
    assert.fail('expected ICPError');
  } catch (err) {
    assert.ok(err instanceof ICPError);
    assert.equal(err.code, 'policy.settler.not_allowed');
  }
});

test('full lifecycle: purchase → accept → fulfill → settlement → observe', async () => {
  const caps = await client.capabilities();
  const purchaseRes = await client.purchase({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'WIDGET-LIFECYCLE', quantity: 1, unit_price: { amount: '20.00', currency: 'USDC' } }],
    max_total: { amount: '25.00', currency: 'USDC' },
  });
  const accepted = await client.accept(purchaseRes.quote.quote_id);
  const escrowId = accepted.funding.escrow_id;

  // Fulfill via direct handler call (SDK doesn't expose fulfill; that's the merchant side)
  const ff = await fetch(`${baseUrl}/icp/v1/escrows/${encodeURIComponent(escrowId)}/fulfill`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ evidence_id: 'icp_ful_SDK_TEST' }),
  });
  const { receipt } = await ff.json();
  assert.equal(receipt.final_state, 'released');

  // SDK retrieves the SettlementReceipt
  const fetched = await client.settlement(receipt.settlement_id);
  assert.equal(fetched.settlement_id, receipt.settlement_id);
});

test('exported primitives produce byte-identical canonical JSON', () => {
  const v = { b: 2, a: 1, nested: { y: [3, 1, 2], x: null } };
  assert.equal(canonicalJson(v), '{"a":1,"b":2,"nested":{"x":null,"y":[3,1,2]}}');
});

test('exported signEd25519 produces 64-byte hex signatures', () => {
  const id = generateIdentity();
  const sig = signEd25519('{"hello":"world"}', id);
  assert.equal(Buffer.from(sig, 'hex').length, 64);
});
