import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { verifyStripeRecovery } from '../templates/storefront/lib/stripe-recovery-proof.js';

const cli = fileURLToPath(
  new URL('../templates/storefront/scripts/verify-stripe-recovery.js', import.meta.url),
);

test('recovery preview requires no packages, database, or provider credentials', () => {
  const run = spawnSync(process.execPath, [cli], { env: {}, encoding: 'utf8' });
  assert.equal(run.status, 0, run.stderr);
  assert.deepEqual(JSON.parse(run.stdout).mode, 'preview');
  assert.equal(JSON.parse(run.stdout).providerCalls, false);
  const missing = spawnSync(process.execPath, [cli, '--check', '--event', 'evt_example'], {
    env: {},
    encoding: 'utf8',
  });
  assert.equal(missing.status, 1);
  assert.match(missing.stderr, /existing sandbox storefront database/);
});

test('recovery requires the requested sandbox failed renewal before consulting local evidence', async () => {
  const failed = {
    id: 'in_demo',
    object: 'invoice',
    livemode: false,
    status: 'open',
    collection_method: 'charge_automatically',
    customer: 'cus_demo',
    currency: 'usd',
    amount_remaining: 500,
    amount_paid: 0,
    billing_reason: 'subscription_cycle',
    parent: { subscription_details: { subscription: 'sub_demo' } },
  };
  const event = {
    id: 'evt_demo',
    type: 'invoice.payment_failed',
    created: 1800000000,
    livemode: false,
    data: { object: failed },
  };
  const verify = (remote, invoice = failed) =>
    verifyStripeRecovery(
      {},
      {
        events: { retrieve: async () => remote },
        invoices: { retrieve: async () => invoice },
      },
      event.id,
    );
  for (const change of [
    { livemode: true },
    { id: 'evt_other' },
    { type: 'invoice.paid' },
    { data: { object: { ...failed, billing_reason: 'subscription_create' } } },
  ])
    await assert.rejects(() => verify({ ...event, ...change }), /sandbox payment failure/);
  for (const change of [
    { customer: 'cus_other' },
    { currency: 'eur' },
    { livemode: true },
    { parent: { subscription_details: { subscription: 'sub_other' } } },
  ])
    await assert.rejects(() => verify(event, { ...failed, ...change }), /Current invoice differs/);
});
