// The reference SDK's PrincipalBinding must satisfy this handler's
// `checkDelegation` in ENFORCE mode.
//
// The client used to ship `sig: 'deadbeef'` with `kid: 'self'`, so the only
// way to transact against the reference handler was to turn delegation
// checking off (ICP_TRUST_MODE=demo). That made the SDK a demo toy and left
// every walkthrough one env var away from accepting unverified delegations.
// This test is the contract between the two halves: the binding
// `signPrincipalBinding()` produces verifies against the principal key an
// operator registered through ICP_PRINCIPAL_KEYS_JSON, and a tampered one
// still fails closed with the code the delegation suite pins.
//
// Run: PORT=0 node --test test/client-binding.test.mjs

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';

import {
  ICPClient,
  ICPError,
  generateIdentity,
  generatePrincipalIdentity,
  signPrincipalBinding,
} from '../../packages/icp-client/src/index.mjs';

const PRINCIPAL = 'did:web:sdk-binding.example';
const principalIdentity = generatePrincipalIdentity();
// The agent identity must exist before the handler starts: an operator-keyed
// handler admits only signer AIDs it was configured with.
const identity = generateIdentity();

process.env.PORT ??= '0';
process.env.ICP_TRUST_MODE = 'enforce';
process.env.ICP_MERCHANT_AID = 'aid:v1:zClientBindingTestMerchant';
process.env.ICP_PRINCIPAL_KEYS_JSON = JSON.stringify({
  [PRINCIPAL]: principalIdentity.ed25519_pubkey.toString('hex'),
});
process.env.ICP_AGENT_KEYS_JSON = JSON.stringify({
  [identity.aid]: identity.ed25519_pubkey.toString('hex'),
});
const { server } = await import('../src/server.mjs');

let handlerUrl;
before(async () => {
  if (!server.listening) await new Promise((resolve) => server.once('listening', resolve));
  handlerUrl = `http://127.0.0.1:${server.address().port}`;
});
after(() => server.close());

const PURCHASE = {
  merchant: 'aid:v1:zClientBindingTestMerchant',
  settler: 'settler:stateset.usdc.base-sepolia',
  items: [{ sku: 'WIDGET-001', quantity: 1, unit_price: { amount: '10.00', currency: 'USDC' } }],
  max_total: { amount: '20.00', currency: 'USDC' },
};

const codeOf = async (promise) => {
  try {
    await promise;
    return null;
  } catch (error) {
    assert.ok(error instanceof ICPError, `expected ICPError, got ${error}`);
    return error.code;
  }
};

test('a binding signed by the SDK verifies against the registered principal key', async () => {
  const client = await ICPClient.create({
    handlerUrl,
    principal: PRINCIPAL,
    identity,
    principalIdentity,
    verbs: ['purchase.create'],
  });
  const { quote } = await client.purchase(PURCHASE);
  assert.ok(quote.quote_id, JSON.stringify(quote));
});

test('a tampered binding is rejected with delegation.signature_invalid', async () => {
  const principalBinding = signPrincipalBinding(
    { principal: PRINCIPAL, agent: identity.aid, verbs: ['purchase.create'] },
    principalIdentity,
  );
  // Widen the authority after signing — the classic escalation attempt.
  principalBinding.authority.max_per_intent = { amount: '1000000', currency: 'USDC' };
  const client = await ICPClient.create({
    handlerUrl,
    principal: PRINCIPAL,
    identity,
    principalBinding,
  });
  assert.equal(await codeOf(client.purchase(PURCHASE)), 'delegation.signature_invalid');
});

test('a binding signed by a key that is not the principal is rejected', async () => {
  const impostor = generatePrincipalIdentity();
  const client = await ICPClient.create({
    handlerUrl,
    principal: PRINCIPAL,
    identity,
    principalIdentity: impostor,
    verbs: ['purchase.create'],
  });
  assert.equal(await codeOf(client.purchase(PURCHASE)), 'delegation.signature_invalid');
});

test('a binding that does not cover the verb is rejected as scope_mismatch', async () => {
  const client = await ICPClient.create({
    handlerUrl,
    principal: PRINCIPAL,
    identity,
    principalIdentity,
    verbs: ['inventory.query'],
  });
  assert.equal(await codeOf(client.purchase(PURCHASE)), 'delegation.scope_mismatch');
});

test('every client method transacts under the DEFAULT binding, in enforce mode', async () => {
  // The default delegation must cover the SDK's whole verb surface: a client
  // whose own methods answer delegation.scope_mismatch is not usable. The
  // handler is the judge here — this is the only place the scope check
  // actually runs, since a permissive handler never reaches it.
  const agent = await ICPClient.create({ handlerUrl, principal: PRINCIPAL, identity, principalIdentity });
  const merchant = 'aid:v1:zClientBindingTestMerchant';
  const settler = 'settler:stateset.usdc.base-sepolia';

  await agent.inventory({ merchant, settler });
  const bought = await agent.purchase(PURCHASE);
  assert.ok(bought.quote.quote_id);
  await agent.subscribe({
    merchant,
    settler,
    service_id: 'premium-monthly',
    cadence: '30d',
    max_total_per_period: { amount: '29.99', currency: 'USDC' },
    first_charge_at: new Date(Date.now() + 86_400_000).toISOString(),
  });
  await agent.cancel({
    merchant,
    settler,
    subscription_id: 'icp_sub_DEFAULTBINDINGTEST00001',
    effective: 'immediate',
  });
  await agent.return_({
    merchant,
    settler,
    original_settlement_id: 'icp_set_DEFAULTBINDINGTEST00001',
    items: [{ sku: 'WIDGET-001', quantity: 1, reason: 'defective' }],
    desired_outcome: 'refund',
  });
  await agent.requestQuote({
    merchant,
    settler,
    items: [{ sku: 'WIDGET-001', quantity: 500 }],
    purchase_window: '30d',
  });
  const channel = await agent.registerWebhook({
    merchant,
    settler,
    url: 'https://agent.example.com/icp/events',
    event_filters: ['settlement.released'],
  });
  assert.match(channel.channel.channel_id, /^icp_ch_/);
});

test('a client with no principal identity sends no binding and the handler decides', async () => {
  const client = await ICPClient.create({ handlerUrl, principal: PRINCIPAL, identity });
  assert.equal(await codeOf(client.purchase(PURCHASE)), 'delegation.required');
});
