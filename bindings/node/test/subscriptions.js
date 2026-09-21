/**
 * Subscriptions API tests for @stateset/embedded Node.js bindings.
 *
 * Plan and cycle money is asserted through the `*Exact` twins. A fresh
 * `:memory:` store ships with demo plans, so plan listings are filtered by
 * the ids this file creates. Every test opens its own store.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');

const DAY_MS = 86_400_000;

async function customer(commerce, email = 'subscriber@example.com') {
  return commerce.customers.create({ email, firstName: 'Sub', lastName: 'Scriber' });
}

async function activePlan(commerce, input) {
  const plan = await commerce.subscriptions.createPlan({ billingInterval: 'monthly', ...input });
  return commerce.subscriptions.activatePlan(plan.id);
}

/** A plan at 29.99 with 10% off: cycle subtotal 29.99, discount 3.00, total 26.99. */
const goldPlan = () => ({ name: 'Gold', code: 'GOLD', price: 29.99, setupFee: 4.5, discountPercent: 0.1 });

test('createPlan returns a draft plan with exact money; getPlan, getPlanByCode, listPlans and updatePlan agree', async () => {
  const commerce = new Commerce(':memory:');
  const plan = await commerce.subscriptions.createPlan({ billingInterval: 'monthly', description: 'The good one', trialDays: 7, ...goldPlan() });
  assert.ok(plan.id);
  assert.equal(plan.code, 'GOLD');
  assert.equal(plan.name, 'Gold');
  assert.equal(plan.status, 'draft');
  assert.equal(plan.billingInterval, 'monthly');
  assert.equal(plan.priceExact, '29.99');
  assert.equal(plan.setupFeeExact, '4.5');
  assert.equal(plan.discountPercent, 0.1);
  assert.equal(plan.trialDays, 7);
  assert.equal(plan.currency, 'USD');

  assert.equal((await commerce.subscriptions.getPlan(plan.id)).id, plan.id);
  assert.equal((await commerce.subscriptions.getPlanByCode('GOLD')).id, plan.id);
  assert.ok((await commerce.subscriptions.listPlans({ status: 'draft' })).some((p) => p.id === plan.id));
  assert.equal(await commerce.subscriptions.getPlan('00000000-0000-0000-0000-000000000001'), null);

  const updated = await commerce.subscriptions.updatePlan(plan.id, { price: 39.99, trialDays: 0 });
  assert.equal(updated.priceExact, '39.99');
  assert.equal(updated.trialDays, 0);
  assert.equal((await commerce.subscriptions.activatePlan(plan.id)).status, 'active');
});

test('subscribe is refused on a draft plan and succeeds once the plan is active', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await commerce.subscriptions.createPlan({ billingInterval: 'monthly', ...goldPlan() });
  await assert.rejects(
    commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id }),
    (err) => err.code === 'VALIDATION' && /not active/i.test(err.message),
  );
  await commerce.subscriptions.activatePlan(plan.id);

  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id });
  assert.ok(sub.id);
  assert.match(sub.subscriptionNumber, /^SUB-/);
  assert.equal(sub.customerId, shopper.id);
  assert.equal(sub.planId, plan.id);
  assert.equal(sub.planName, 'Gold');
  assert.equal(sub.status, 'active');
  assert.equal(sub.billingInterval, 'monthly');
  assert.equal(sub.priceExact, '29.99');
  assert.equal(sub.discountPercent, 0.1);
  assert.equal(sub.currency, 'USD');
  assert.equal(sub.billingCycleCount, 0);
  assert.equal(sub.nextBillingDate, sub.currentPeriodEnd);
  assert.equal(sub.trialEndsAt, undefined);

  assert.equal((await commerce.subscriptions.get(sub.id)).id, sub.id);
  assert.equal((await commerce.subscriptions.getByNumber(sub.subscriptionNumber)).id, sub.id);
  assert.deepEqual((await commerce.subscriptions.list({ customerId: shopper.id })).map((s) => s.id), [sub.id]);
  assert.deepEqual((await commerce.subscriptions.list({ planId: plan.id, status: 'active' })).map((s) => s.id), [sub.id]);
  assert.equal(await commerce.subscriptions.get('00000000-0000-0000-0000-000000000001'), null);
});

test('subscribing seeds billing cycle 1 with exact plan-discounted amounts', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, goldPlan());
  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id });

  const cycles = await commerce.subscriptions.listBillingCycles({ subscriptionId: sub.id });
  assert.equal(cycles.length, 1);
  const [cycle] = cycles;
  assert.equal(cycle.subscriptionId, sub.id);
  assert.equal(cycle.cycleNumber, 1);
  assert.equal(cycle.status, 'scheduled');
  assert.equal(cycle.periodStart, sub.currentPeriodStart);
  assert.equal(cycle.periodEnd, sub.currentPeriodEnd);
  assert.equal(cycle.subtotalExact, '29.99');
  // 10% of 29.99 is 2.999, which rounds to 3.00.
  assert.equal(cycle.discountExact, '3.00');
  assert.equal(cycle.taxExact, '0');
  assert.equal(cycle.totalExact, '26.99');
  assert.equal(cycle.currency, 'USD');
  assert.equal(cycle.retryCount, 0);

  assert.equal((await commerce.subscriptions.getBillingCycle(cycle.id)).id, cycle.id);
  assert.equal((await commerce.subscriptions.listBillingCycles({ subscriptionId: sub.id, status: 'scheduled' })).length, 1);
  assert.equal((await commerce.subscriptions.listBillingCycles({ subscriptionId: sub.id, status: 'paid' })).length, 0);
});

test('a price override is billed exactly and the plan discount applies to it', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, goldPlan());
  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id, price: 24.99 });
  assert.equal(sub.priceExact, '24.99');
  const [cycle] = await commerce.subscriptions.listBillingCycles({ subscriptionId: sub.id });
  assert.equal(cycle.subtotalExact, '24.99');
  assert.equal(cycle.discountExact, '2.50');
  assert.equal(cycle.totalExact, '22.49');
});

test('trial days put the subscription in trial until the trial end; skipTrial bypasses it', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, { name: 'Trial', price: 10, trialDays: 14 });
  const start = '2026-09-01T00:00:00Z';

  const trial = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id, startDate: start });
  assert.equal(trial.status, 'trial');
  assert.equal(Date.parse(trial.trialEndsAt), Date.parse(start) + 14 * DAY_MS);
  assert.equal(trial.currentPeriodEnd, trial.trialEndsAt);
  assert.equal(trial.nextBillingDate, trial.trialEndsAt);

  const direct = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id, startDate: start, skipTrial: true });
  assert.equal(direct.status, 'active');
  assert.equal(direct.trialEndsAt, undefined);
  assert.equal(Date.parse(direct.currentPeriodEnd), Date.parse(start) + 30 * DAY_MS);
});

test('the billing interval sets the period length: weekly is 7 days, custom is customIntervalDays', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const start = '2026-09-01T00:00:00Z';

  const weekly = await activePlan(commerce, { name: 'Weekly', billingInterval: 'weekly', price: 5 });
  const weeklySub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: weekly.id, startDate: start });
  assert.equal(Date.parse(weeklySub.currentPeriodStart), Date.parse(start));
  assert.equal(Date.parse(weeklySub.currentPeriodEnd), Date.parse(start) + 7 * DAY_MS);
  assert.equal(weeklySub.nextBillingDate, weeklySub.currentPeriodEnd);

  const custom = await activePlan(commerce, { name: 'Every ten days', billingInterval: 'custom', customIntervalDays: 10, price: 5 });
  const customSub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: custom.id, startDate: start });
  assert.equal(customSub.billingInterval, 'custom');
  assert.equal(customSub.customIntervalDays, 10);
  assert.equal(Date.parse(customSub.currentPeriodEnd), Date.parse(start) + 10 * DAY_MS);

  assert.ok((await commerce.subscriptions.listPlans({ billingInterval: 'weekly' })).some((p) => p.id === weekly.id));
});

test('pause and resume: only an active subscription pauses, only a paused one resumes', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, goldPlan());
  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id });

  await assert.rejects(commerce.subscriptions.resume(sub.id), (err) => err.code === 'VALIDATION' && /active/i.test(err.message));

  const paused = await commerce.subscriptions.pause(sub.id, { reason: 'travelling' });
  assert.equal(paused.status, 'paused');
  assert.ok(paused.pausedAt);
  assert.equal(paused.nextBillingDate, undefined, 'a paused subscription is not due');
  await assert.rejects(commerce.subscriptions.pause(sub.id), (err) => err.code === 'VALIDATION' && /paused/i.test(err.message));
  assert.equal((await commerce.subscriptions.get(sub.id)).status, 'paused');

  const resumed = await commerce.subscriptions.resume(sub.id);
  assert.equal(resumed.status, 'active');
  assert.equal(resumed.pausedAt, undefined);
  assert.ok(resumed.nextBillingDate);

  const events = (await commerce.subscriptions.getEvents(sub.id)).map((e) => e.eventType);
  for (const expected of ['created', 'paused', 'resumed']) {
    assert.ok(events.includes(expected), `events ${events} should include ${expected}`);
  }
  assert.ok((await commerce.subscriptions.getEvents(sub.id)).some((e) => e.eventType === 'paused' && /travelling/.test(e.description)));
});

test('skipBilling pushes the next billing date out by one interval, and only for an active subscription', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, { name: 'Weekly', billingInterval: 'weekly', price: 5 });
  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id, startDate: '2026-09-01T00:00:00Z' });

  const skipped = await commerce.subscriptions.skipBilling(sub.id, { reason: 'on holiday' });
  assert.equal(skipped.status, 'active');
  assert.equal(Date.parse(skipped.nextBillingDate), Date.parse(sub.nextBillingDate) + 7 * DAY_MS);
  assert.equal(skipped.currentPeriodEnd, skipped.nextBillingDate);
  assert.ok((await commerce.subscriptions.getEvents(sub.id)).some((e) => e.eventType === 'skipped' && e.description === 'on holiday'));

  await commerce.subscriptions.pause(sub.id);
  await assert.rejects(commerce.subscriptions.skipBilling(sub.id), (err) => err.code === 'VALIDATION' && /active/i.test(err.message));
});

test('cancel at period end keeps the subscription until endsAt; a cancelled subscription is terminal', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, goldPlan());
  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id });

  const cancelled = await commerce.subscriptions.cancel(sub.id, { reason: 'too expensive', feedback: 'price' });
  assert.equal(cancelled.status, 'cancelled');
  assert.ok(cancelled.cancelledAt);
  assert.equal(cancelled.endsAt, sub.currentPeriodEnd, 'access runs to the end of the paid period');
  assert.equal(cancelled.nextBillingDate, undefined);

  await assert.rejects(commerce.subscriptions.cancel(sub.id), (err) => err.code === 'VALIDATION' && /cancelled/i.test(err.message));
  await assert.rejects(commerce.subscriptions.pause(sub.id), (err) => err.code === 'VALIDATION' && /cancelled/i.test(err.message));
  await assert.rejects(commerce.subscriptions.skipBilling(sub.id), (err) => err.code === 'VALIDATION');
  assert.equal((await commerce.subscriptions.get(sub.id)).status, 'cancelled');
  assert.ok((await commerce.subscriptions.getEvents(sub.id)).some((e) => e.eventType === 'cancelled' && e.description === 'too expensive'));
  assert.deepEqual((await commerce.subscriptions.list({ customerId: shopper.id, status: 'cancelled' })).map((s) => s.id), [sub.id]);
});

test('an immediate cancel ends the subscription now: it lands in expired with endsAt = cancelledAt', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, goldPlan());
  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id });

  const ended = await commerce.subscriptions.cancel(sub.id, { immediate: true });
  // The engine's immediate cancel is an expiry, not a scheduled cancellation.
  assert.equal(ended.status, 'expired');
  assert.equal(ended.endsAt, ended.cancelledAt);
  assert.equal(ended.nextBillingDate, undefined);
  await assert.rejects(commerce.subscriptions.resume(sub.id), (err) => err.code === 'VALIDATION');
});

test('archivePlan is terminal: no new subscriptions and no reactivation', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, goldPlan());
  const archived = await commerce.subscriptions.archivePlan(plan.id);
  assert.equal(archived.status, 'archived');
  assert.ok((await commerce.subscriptions.listPlans({ status: 'archived' })).some((p) => p.id === plan.id));
  await assert.rejects(
    commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id }),
    (err) => err.code === 'VALIDATION' && /not active/i.test(err.message),
  );
  await assert.rejects(commerce.subscriptions.activatePlan(plan.id), (err) => err.code === 'VALIDATION' && /archived/i.test(err.message));
});

test('a duplicate plan code is a CONFLICT', async () => {
  const commerce = new Commerce(':memory:');
  await commerce.subscriptions.createPlan({ name: 'One', code: 'DUP', billingInterval: 'monthly', price: 5 });
  await assert.rejects(
    commerce.subscriptions.createPlan({ name: 'Two', code: 'DUP', billingInterval: 'monthly', price: 5 }),
    (err) => err.code === 'CONFLICT',
  );
});

test('malformed inputs are refused with VALIDATION, never coerced', async () => {
  const commerce = new Commerce(':memory:');
  const shopper = await customer(commerce);
  const plan = await activePlan(commerce, goldPlan());
  const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id });
  const isValidation = (pattern) => (err) => err.code === 'VALIDATION' && pattern.test(err.message);
  const base = { name: 'x', billingInterval: 'monthly', price: 1 };

  await assert.rejects(commerce.subscriptions.createPlan({ ...base, billingInterval: 'fortnightly' }), isValidation(/billing interval/i));
  await assert.rejects(commerce.subscriptions.createPlan({ ...base, currency: 'DOLLARS' }), isValidation(/currency/i));
  await assert.rejects(commerce.subscriptions.createPlan({ ...base, price: -1 }), isValidation(/price/i));
  await assert.rejects(commerce.subscriptions.createPlan({ ...base, discountPercent: 10 }), isValidation(/discount_percent/i));
  await assert.rejects(commerce.subscriptions.subscribe({ customerId: shopper.id, planId: 'not-a-uuid' }), isValidation(/plan/i));
  await assert.rejects(commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id, startDate: 'tomorrow' }), isValidation(/start date/i));
  await assert.rejects(commerce.subscriptions.update(sub.id, { discountPercent: 10 }), isValidation(/discount_percent/i));
  await assert.rejects(commerce.subscriptions.update(sub.id, { status: 'weird' }), isValidation(/status/i));
  await assert.rejects(commerce.subscriptions.list({ status: 'weird' }), isValidation(/status/i));
  await assert.rejects(commerce.subscriptions.listBillingCycles({ subscriptionId: sub.id, status: 'weird' }), isValidation(/status/i));
  await assert.rejects(commerce.subscriptions.get('not-a-uuid'), isValidation(/uuid/i));

  const touched = await commerce.subscriptions.update(sub.id, { discountPercent: 0.5 });
  assert.equal(touched.discountPercent, 0.5);
  assert.equal(touched.priceExact, '29.99');
  assert.deepEqual((await commerce.subscriptions.list({ customerId: shopper.id })).map((s) => s.id), [sub.id], 'nothing else was written');
});

test(
  'skipBilling marks the skipped cycle as skipped',
  { todo: 'engine: skip_billing_cycle advances next_billing_date but leaves billing cycle 1 scheduled over the skipped period' },
  async () => {
    const commerce = new Commerce(':memory:');
    const shopper = await customer(commerce);
    const plan = await activePlan(commerce, { name: 'Weekly', billingInterval: 'weekly', price: 5 });
    const sub = await commerce.subscriptions.subscribe({ customerId: shopper.id, planId: plan.id, startDate: '2026-09-01T00:00:00Z' });
    await commerce.subscriptions.skipBilling(sub.id);
    const cycles = await commerce.subscriptions.listBillingCycles({ subscriptionId: sub.id });
    assert.deepEqual(
      cycles.filter((c) => c.status === 'skipped').map((c) => c.cycleNumber),
      [1],
      `expected cycle 1 skipped, got ${JSON.stringify(cycles.map((c) => [c.cycleNumber, c.status]))}`,
    );
  },
);
