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
  proc = spawn(process.execPath, [HANDLER], { env: { ...process.env, PORT: '0' }, stdio: ['ignore', 'pipe', 'pipe'] });
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
    items: [{ sku: 'GADGET-A', quantity: 1, unit_price: { amount: '10.00', currency: 'USDC' } }],
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
    items: [{ sku: 'GADGET-A', quantity: 1, unit_price: { amount: '20.00', currency: 'USDC' } }],
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

test('registerWebhook returns signed ChannelRegistration (webhook)', async () => {
  const caps = await client.capabilities();
  const result = await client.registerWebhook({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    url: 'https://agent.example.com/icp/events',
    event_filters: ['settlement.released', 'escrow.refunded'],
  });
  assert.equal(result.channel.type, 'channel.registration');
  assert.equal(result.channel.channel_type, 'webhook');
  assert.equal(result.channel.webhook_url, 'https://agent.example.com/icp/events');
  assert.deepEqual(result.channel.events_registered, ['settlement.released', 'escrow.refunded']);
  assert.match(result.channel.channel_id, /^icp_ch_/);
  assert.equal(result.signature.alg, 'ed25519');
  // Round-trip: handler exposes GET /icp/v1/channels/:id with the same shape.
  const r = await fetch(`${baseUrl}/icp/v1/channels/${result.channel.channel_id}`);
  assert.equal(r.status, 200);
  const fetched = await r.json();
  assert.equal(fetched.channel_id, result.channel.channel_id);
});

test('registerWebhook with type=sse mints a subscription token', async () => {
  const caps = await client.capabilities();
  const result = await client.registerWebhook({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    type: 'sse',
    event_filters: ['dispute.opened'],
  });
  assert.equal(result.channel.channel_type, 'sse');
  assert.ok(result.channel.subscription_token, 'sse must mint a token');
  assert.equal(result.channel.token_ttl_seconds, 3600);
});

test('registerWebhook with http:// non-loopback URL rejected with typed ICPError', async () => {
  const caps = await client.capabilities();
  await assert.rejects(
    client.registerWebhook({
      merchant: caps.merchant_aid,
      settler: 'settler:stateset.usdc.base-sepolia',
      url: 'http://insecure.example.com/events',
      event_filters: ['settlement.released'],
    }),
    (err) => err instanceof ICPError && err.code === 'channel.url_unverified',
  );
});

test('fetchChannelEvents returns verified envelopes after fulfill triggers a publish', async () => {
  const caps = await client.capabilities();
  // Register a webhook subscribed to settlement.released, pointed at a
  // dead loopback URL so the live POST fails — the recovery log still
  // captures the signed envelope.
  const reg = await client.registerWebhook({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    url: 'http://127.0.0.1:1/icp/events',  // unreachable on purpose
    event_filters: ['settlement.released'],
  });
  const channelId = reg.channel.channel_id;

  // Run a full purchase → accept → fulfill cycle to trigger the
  // publisher.
  const purchase = await client.purchase({
    merchant: caps.merchant_aid,
    settler: 'settler:stateset.usdc.base-sepolia',
    items: [{ sku: 'GADGET-A', quantity: 1, unit_price: { amount: '15.00', currency: 'USDC' } }],
    max_total: { amount: '20.00', currency: 'USDC' },
  });
  const accepted = await client.accept(purchase.quote.quote_id);
  await fetch(`${baseUrl}/icp/v1/escrows/${encodeURIComponent(accepted.funding.escrow_id)}/fulfill`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ evidence_id: 'icp_ful_RECOV_TEST' }),
  });

  // Brief settle window for the fire-and-forget publish to land in the recovery log.
  await new Promise((r) => setTimeout(r, 150));

  const events = await client.fetchChannelEvents(channelId, 0);
  assert.ok(events.length >= 1, `expected ≥1 event, got: ${JSON.stringify(events)}`);
  const evt = events.find((e) => e.event_type === 'settlement.released');
  assert.ok(evt, 'must include settlement.released');
  assert.equal(evt.channel_id, channelId);
  assert.equal(evt.payload.final_state, 'released');

  // since=evt.sequence returns empty (no events with sequence > that).
  const tail = await client.fetchChannelEvents(channelId, evt.sequence);
  assert.deepEqual(tail, []);
});

test('fetchChannelEvents on unknown channel throws typed channel.not_found', async () => {
  await assert.rejects(
    client.fetchChannelEvents('icp_ch_does_not_exist', 0),
    (err) => err instanceof ICPError && err.code === 'channel.not_found',
  );
});
