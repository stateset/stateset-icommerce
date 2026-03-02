/**
 * Unit tests for a2a/subscriptions.js — A2A Subscription Service
 *
 * Covers: createSubscription, pauseSubscription, resumeSubscription,
 * cancelSubscription, getSubscription, listSubscriptions, processBilling,
 * computeNextBillingDate, formatSubscription, VALID_INTERVALS
 */

import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  createA2ASubscriptionService,
  computeNextBillingDate,
  formatSubscription,
  VALID_INTERVALS,
} from '../../src/a2a/subscriptions.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a mock store where every method is a mock.fn */
function createMockStore(overrides = {}) {
  return {
    createSubscription: mock.fn(async (record) => record),
    getSubscription: mock.fn(async () => null),
    updateSubscription: mock.fn(async (id, updates) => ({ id, ...updates })),
    listSubscriptions: mock.fn(async () => []),
    getDueSubscriptions: mock.fn(async () => []),
    getExpiredTrials: mock.fn(async () => []),
    ...overrides,
  };
}

/** Minimal valid params for createSubscription */
function validCreateParams(overrides = {}) {
  return {
    subscriberAddress: '0xSubscriber',
    providerAddress: '0xProvider',
    planName: 'Pro Plan',
    amount: 49.99,
    ...overrides,
  };
}

/** Build a raw snake_case subscription row as returned by the store */
function makeStoreRow(overrides = {}) {
  const now = new Date().toISOString();
  return {
    id: 'sub-123',
    subscriber_address: '0xSubscriber',
    provider_address: '0xProvider',
    service_id: null,
    plan_name: 'Pro Plan',
    status: 'active',
    amount: 49990000,
    amount_decimal: 49.99,
    asset: 'USDC',
    network: 'set_chain',
    billing_interval: 'monthly',
    trial_end_date: null,
    current_period_start: now,
    current_period_end: now,
    next_billing_date: now,
    cancel_at_period_end: false,
    cancelled_at: null,
    past_due_since: null,
    max_past_due_cycles: 3,
    total_billed: 0,
    total_billed_decimal: 0,
    billing_count: 0,
    last_payment_id: null,
    metadata: null,
    created_at: now,
    updated_at: now,
    ...overrides,
  };
}

// ===========================================================================
// VALID_INTERVALS
// ===========================================================================

describe('VALID_INTERVALS', () => {
  it('contains exactly the five expected intervals', () => {
    assert.deepStrictEqual(VALID_INTERVALS, [
      'weekly',
      'biweekly',
      'monthly',
      'quarterly',
      'annual',
    ]);
  });
});

// ===========================================================================
// computeNextBillingDate
// ===========================================================================

describe('computeNextBillingDate', () => {
  const base = '2025-06-15T12:00:00.000Z';

  it('weekly — adds 7 days', () => {
    const result = computeNextBillingDate(base, 'weekly');
    const d = new Date(result);
    assert.strictEqual(d.getUTCDate(), 22);
    assert.strictEqual(d.getUTCMonth(), 5); // June = 5
  });

  it('biweekly — adds 14 days', () => {
    const result = computeNextBillingDate(base, 'biweekly');
    const d = new Date(result);
    assert.strictEqual(d.getUTCDate(), 29);
    assert.strictEqual(d.getUTCMonth(), 5);
  });

  it('monthly — adds 1 month', () => {
    const result = computeNextBillingDate(base, 'monthly');
    const d = new Date(result);
    assert.strictEqual(d.getUTCMonth(), 6); // July
    assert.strictEqual(d.getUTCDate(), 15);
  });

  it('quarterly — adds 3 months', () => {
    const result = computeNextBillingDate(base, 'quarterly');
    const d = new Date(result);
    assert.strictEqual(d.getUTCMonth(), 8); // September
    assert.strictEqual(d.getUTCDate(), 15);
  });

  it('annual — adds 1 year', () => {
    const result = computeNextBillingDate(base, 'annual');
    const d = new Date(result);
    assert.strictEqual(d.getUTCFullYear(), 2026);
    assert.strictEqual(d.getUTCMonth(), 5);
    assert.strictEqual(d.getUTCDate(), 15);
  });

  it('throws on invalid interval', () => {
    assert.throws(() => computeNextBillingDate(base, 'daily'), {
      message: /Invalid billing interval: daily/,
    });
  });

  it('accepts a Date object as fromDate', () => {
    const date = new Date('2025-01-01T00:00:00.000Z');
    const result = computeNextBillingDate(date, 'monthly');
    const d = new Date(result);
    assert.strictEqual(d.getUTCMonth(), 1); // February
  });

  it('returns a valid ISO string', () => {
    const result = computeNextBillingDate(base, 'monthly');
    assert.strictEqual(result, new Date(result).toISOString());
  });
});

// ===========================================================================
// formatSubscription
// ===========================================================================

describe('formatSubscription', () => {
  it('returns null for null input', () => {
    assert.strictEqual(formatSubscription(null), null);
  });

  it('returns null for undefined input', () => {
    assert.strictEqual(formatSubscription(undefined), null);
  });

  it('converts snake_case store row to camelCase', () => {
    const row = makeStoreRow({
      id: 'sub-abc',
      subscriber_address: '0xAlice',
      provider_address: '0xBob',
      service_id: 'svc-1',
      plan_name: 'Basic',
      billing_interval: 'weekly',
      cancel_at_period_end: true,
      cancelled_at: '2025-06-01T00:00:00Z',
      past_due_since: '2025-05-01T00:00:00Z',
      last_payment_id: 'pay-1',
      metadata: '{"key":"val"}',
    });

    const result = formatSubscription(row);

    assert.strictEqual(result.id, 'sub-abc');
    assert.strictEqual(result.subscriberAddress, '0xAlice');
    assert.strictEqual(result.providerAddress, '0xBob');
    assert.strictEqual(result.serviceId, 'svc-1');
    assert.strictEqual(result.planName, 'Basic');
    assert.strictEqual(result.billingInterval, 'weekly');
    assert.strictEqual(result.cancelAtPeriodEnd, true);
    assert.strictEqual(result.cancelledAt, '2025-06-01T00:00:00Z');
    assert.strictEqual(result.pastDueSince, '2025-05-01T00:00:00Z');
    assert.strictEqual(result.lastPaymentId, 'pay-1');
    assert.strictEqual(result.metadata, '{"key":"val"}');
  });

  it('coerces cancel_at_period_end falsy values to false', () => {
    const row = makeStoreRow({ cancel_at_period_end: 0 });
    assert.strictEqual(formatSubscription(row).cancelAtPeriodEnd, false);
  });

  it('coerces cancel_at_period_end truthy values to true', () => {
    const row = makeStoreRow({ cancel_at_period_end: 1 });
    assert.strictEqual(formatSubscription(row).cancelAtPeriodEnd, true);
  });

  it('sets nullable fields to null when absent', () => {
    const row = makeStoreRow({
      service_id: undefined,
      trial_end_date: undefined,
      cancelled_at: undefined,
      past_due_since: undefined,
      last_payment_id: undefined,
      metadata: undefined,
    });
    const result = formatSubscription(row);
    assert.strictEqual(result.serviceId, null);
    assert.strictEqual(result.trialEndDate, null);
    assert.strictEqual(result.cancelledAt, null);
    assert.strictEqual(result.pastDueSince, null);
    assert.strictEqual(result.lastPaymentId, null);
    assert.strictEqual(result.metadata, null);
  });
});

// ===========================================================================
// createSubscription
// ===========================================================================

describe('createSubscription', () => {
  let store;
  let svc;

  beforeEach(() => {
    store = createMockStore();
    svc = createA2ASubscriptionService(store);
  });

  it('creates a subscription with valid params and returns success', async () => {
    const result = await svc.createSubscription(validCreateParams());
    assert.strictEqual(result.success, true);
    assert.ok(result.subscription);
    assert.strictEqual(result.subscription.subscriberAddress, '0xSubscriber');
    assert.strictEqual(result.subscription.providerAddress, '0xProvider');
    assert.strictEqual(result.subscription.planName, 'Pro Plan');
    assert.strictEqual(result.subscription.status, 'active');
  });

  it('defaults to USDC asset', async () => {
    const result = await svc.createSubscription(validCreateParams());
    assert.strictEqual(result.subscription.asset, 'USDC');
  });

  it('defaults to set_chain network', async () => {
    const result = await svc.createSubscription(validCreateParams());
    assert.strictEqual(result.subscription.network, 'set_chain');
  });

  it('defaults to monthly billing interval', async () => {
    const result = await svc.createSubscription(validCreateParams());
    assert.strictEqual(result.subscription.billingInterval, 'monthly');
  });

  it('uppercases the asset', async () => {
    const result = await svc.createSubscription(validCreateParams({ asset: 'usdc' }));
    assert.strictEqual(result.subscription.asset, 'USDC');
  });

  it('converts amount to USDC smallest unit (6 decimals)', async () => {
    const result = await svc.createSubscription(validCreateParams({ amount: 10 }));
    // amount in store is smallest unit: 10 * 1_000_000 = 10_000_000
    const storeCall = store.createSubscription.mock.calls[0].arguments[0];
    assert.strictEqual(storeCall.amount, 10_000_000);
    assert.strictEqual(storeCall.amount_decimal, 10);
  });

  it('sets status to active when trialDays is 0', async () => {
    const result = await svc.createSubscription(validCreateParams({ trialDays: 0 }));
    assert.strictEqual(result.subscription.status, 'active');
    assert.strictEqual(result.subscription.trialEndDate, null);
  });

  it('sets status to trial when trialDays > 0', async () => {
    const result = await svc.createSubscription(validCreateParams({ trialDays: 14 }));
    assert.strictEqual(result.subscription.status, 'trial');
    assert.ok(result.subscription.trialEndDate);
    // trial end date should be ~14 days in the future
    const trialEnd = new Date(result.subscription.trialEndDate);
    const now = new Date();
    const diffDays = (trialEnd - now) / (1000 * 60 * 60 * 24);
    assert.ok(diffDays >= 13.9 && diffDays <= 14.1, `Expected ~14 days, got ${diffDays}`);
  });

  it('sets nextBillingDate to trial end when trial is active', async () => {
    const result = await svc.createSubscription(validCreateParams({ trialDays: 7 }));
    assert.strictEqual(result.subscription.nextBillingDate, result.subscription.trialEndDate);
  });

  it('calculates nextBillingDate from billing interval when no trial', async () => {
    const result = await svc.createSubscription(
      validCreateParams({ billingInterval: 'weekly' }),
    );
    const next = new Date(result.subscription.nextBillingDate);
    const now = new Date();
    // weekly = +7 days from creation time; allow ~1 day tolerance for timezone edge cases
    const diffDays = (next - now) / (1000 * 60 * 60 * 24);
    assert.ok(diffDays >= 6.0 && diffDays <= 8.0,
      `Expected ~7 days from now, got ${diffDays.toFixed(2)} days (${next.toISOString()})`);
  });

  it('passes metadata as stringified JSON to store', async () => {
    await svc.createSubscription(validCreateParams({ metadata: { tier: 'gold' } }));
    const storeCall = store.createSubscription.mock.calls[0].arguments[0];
    assert.strictEqual(storeCall.metadata, JSON.stringify({ tier: 'gold' }));
  });

  it('passes null metadata when not provided', async () => {
    await svc.createSubscription(validCreateParams());
    const storeCall = store.createSubscription.mock.calls[0].arguments[0];
    assert.strictEqual(storeCall.metadata, null);
  });

  it('generates a UUID for the subscription id', async () => {
    const result = await svc.createSubscription(validCreateParams());
    // UUID v4 format: 8-4-4-4-12 hex chars
    assert.match(result.subscription.id, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  });

  it('calls store.createSubscription exactly once', async () => {
    await svc.createSubscription(validCreateParams());
    assert.strictEqual(store.createSubscription.mock.calls.length, 1);
  });

  it('initializes total_billed and billing_count to 0', async () => {
    await svc.createSubscription(validCreateParams());
    const storeCall = store.createSubscription.mock.calls[0].arguments[0];
    assert.strictEqual(storeCall.total_billed, 0);
    assert.strictEqual(storeCall.total_billed_decimal, 0);
    assert.strictEqual(storeCall.billing_count, 0);
  });

  it('accepts a custom serviceId', async () => {
    const result = await svc.createSubscription(
      validCreateParams({ serviceId: 'svc-42' }),
    );
    assert.strictEqual(result.subscription.serviceId, 'svc-42');
  });

  // --- Validation errors ---

  it('throws when subscriberAddress is missing', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ subscriberAddress: '' })),
      { message: /subscriberAddress is required/ },
    );
  });

  it('throws when providerAddress is missing', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ providerAddress: '' })),
      { message: /providerAddress is required/ },
    );
  });

  it('throws when planName is missing', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ planName: '' })),
      { message: /planName is required/ },
    );
  });

  it('throws when amount is missing', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ amount: undefined })),
      { message: /amount is required/ },
    );
  });

  it('throws when amount is null', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ amount: null })),
      { message: /amount is required/ },
    );
  });

  it('throws when amount is negative', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ amount: -5 })),
      { message: /amount must be a positive number/ },
    );
  });

  it('throws when amount is zero', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ amount: 0 })),
      { message: /amount must be a positive number/ },
    );
  });

  it('throws when amount is a string', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ amount: '49.99' })),
      { message: /amount must be a positive number/ },
    );
  });

  it('throws when billingInterval is invalid', async () => {
    await assert.rejects(
      svc.createSubscription(validCreateParams({ billingInterval: 'daily' })),
      { message: /Invalid billingInterval: daily/ },
    );
  });
});

// ===========================================================================
// pauseSubscription
// ===========================================================================

describe('pauseSubscription', () => {
  it('pauses an active subscription', async () => {
    const row = makeStoreRow({ status: 'active' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
      updateSubscription: mock.fn(async (id, updates) => ({ ...row, ...updates })),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.pauseSubscription('sub-123');
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.subscription.status, 'paused');
    assert.strictEqual(store.updateSubscription.mock.calls[0].arguments[1].status, 'paused');
  });

  it('throws when subscription is not found', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => null),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.pauseSubscription('nonexistent'), {
      message: /Subscription not found/,
    });
  });

  it('throws when subscription is not active (paused)', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => makeStoreRow({ status: 'paused' })),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.pauseSubscription('sub-123'), {
      message: /Cannot pause subscription in status: paused/,
    });
  });

  it('throws when subscription is cancelled', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => makeStoreRow({ status: 'cancelled' })),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.pauseSubscription('sub-123'), {
      message: /Cannot pause subscription in status: cancelled/,
    });
  });

  it('throws when subscription is in trial', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => makeStoreRow({ status: 'trial' })),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.pauseSubscription('sub-123'), {
      message: /Cannot pause subscription in status: trial/,
    });
  });
});

// ===========================================================================
// resumeSubscription
// ===========================================================================

describe('resumeSubscription', () => {
  it('resumes a paused subscription and sets status to active', async () => {
    const row = makeStoreRow({ status: 'paused', billing_interval: 'monthly' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
      updateSubscription: mock.fn(async (id, updates) => ({ ...row, ...updates })),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.resumeSubscription('sub-123');
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.subscription.status, 'active');
  });

  it('recalculates billing dates from now', async () => {
    const row = makeStoreRow({ status: 'paused', billing_interval: 'weekly' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
      updateSubscription: mock.fn(async (id, updates) => ({ ...row, ...updates })),
    });
    const svc = createA2ASubscriptionService(store);

    const before = new Date();
    await svc.resumeSubscription('sub-123');
    const after = new Date();

    const updateArgs = store.updateSubscription.mock.calls[0].arguments[1];
    assert.ok(updateArgs.current_period_start);
    assert.ok(updateArgs.current_period_end);
    assert.ok(updateArgs.next_billing_date);
    // next_billing_date should be ~7 days ahead for weekly
    const next = new Date(updateArgs.next_billing_date);
    const start = new Date(updateArgs.current_period_start);
    assert.ok(start >= before && start <= after);
    const diffDays = (next - start) / (1000 * 60 * 60 * 24);
    assert.ok(diffDays >= 6.9 && diffDays <= 7.1);
  });

  it('throws when subscription is not found', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => null),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.resumeSubscription('nonexistent'), {
      message: /Subscription not found/,
    });
  });

  it('throws when subscription is not paused (active)', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => makeStoreRow({ status: 'active' })),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.resumeSubscription('sub-123'), {
      message: /Cannot resume subscription in status: active/,
    });
  });

  it('throws when subscription is cancelled', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => makeStoreRow({ status: 'cancelled' })),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.resumeSubscription('sub-123'), {
      message: /Cannot resume subscription in status: cancelled/,
    });
  });
});

// ===========================================================================
// cancelSubscription
// ===========================================================================

describe('cancelSubscription', () => {
  it('cancels immediately by default', async () => {
    const row = makeStoreRow({ status: 'active' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
      updateSubscription: mock.fn(async (id, updates) => ({ ...row, ...updates })),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.cancelSubscription('sub-123');
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.subscription.status, 'cancelled');
    const updateArgs = store.updateSubscription.mock.calls[0].arguments[1];
    assert.strictEqual(updateArgs.status, 'cancelled');
    assert.ok(updateArgs.cancelled_at);
    assert.strictEqual(updateArgs.cancel_at_period_end, false);
  });

  it('cancels at period end when immediate=false', async () => {
    const row = makeStoreRow({ status: 'active' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
      updateSubscription: mock.fn(async (id, updates) => ({ ...row, ...updates })),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.cancelSubscription('sub-123', { immediate: false });
    assert.strictEqual(result.success, true);
    const updateArgs = store.updateSubscription.mock.calls[0].arguments[1];
    assert.strictEqual(updateArgs.cancel_at_period_end, true);
    assert.strictEqual(updateArgs.status, undefined); // status not changed yet
  });

  it('throws when subscription is already cancelled', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => makeStoreRow({ status: 'cancelled' })),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.cancelSubscription('sub-123'), {
      message: /Subscription is already cancelled/,
    });
  });

  it('throws when subscription is not found', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => null),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.cancelSubscription('nonexistent'), {
      message: /Subscription not found/,
    });
  });

  it('can cancel a paused subscription immediately', async () => {
    const row = makeStoreRow({ status: 'paused' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
      updateSubscription: mock.fn(async (id, updates) => ({ ...row, ...updates })),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.cancelSubscription('sub-123');
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.subscription.status, 'cancelled');
  });

  it('can cancel a trial subscription immediately', async () => {
    const row = makeStoreRow({ status: 'trial' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
      updateSubscription: mock.fn(async (id, updates) => ({ ...row, ...updates })),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.cancelSubscription('sub-123');
    assert.strictEqual(result.success, true);
    assert.strictEqual(result.subscription.status, 'cancelled');
  });
});

// ===========================================================================
// getSubscription
// ===========================================================================

describe('getSubscription', () => {
  it('returns a formatted subscription when found', async () => {
    const row = makeStoreRow({ id: 'sub-abc', plan_name: 'Gold' });
    const store = createMockStore({
      getSubscription: mock.fn(async () => row),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.getSubscription('sub-abc');
    assert.strictEqual(result.id, 'sub-abc');
    assert.strictEqual(result.planName, 'Gold');
    assert.strictEqual(result.subscriberAddress, '0xSubscriber');
  });

  it('throws when not found', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => null),
    });
    const svc = createA2ASubscriptionService(store);

    await assert.rejects(svc.getSubscription('missing'), {
      message: /Subscription not found/,
    });
  });

  it('passes the subscription ID to the store', async () => {
    const store = createMockStore({
      getSubscription: mock.fn(async () => makeStoreRow()),
    });
    const svc = createA2ASubscriptionService(store);

    await svc.getSubscription('sub-xyz');
    assert.strictEqual(store.getSubscription.mock.calls[0].arguments[0], 'sub-xyz');
  });
});

// ===========================================================================
// listSubscriptions
// ===========================================================================

describe('listSubscriptions', () => {
  it('returns an empty array when no subscriptions', async () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    const result = await svc.listSubscriptions();
    assert.deepStrictEqual(result, []);
  });

  it('returns formatted subscriptions', async () => {
    const rows = [
      makeStoreRow({ id: 'sub-1', plan_name: 'Basic' }),
      makeStoreRow({ id: 'sub-2', plan_name: 'Pro' }),
    ];
    const store = createMockStore({
      listSubscriptions: mock.fn(async () => rows),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.listSubscriptions();
    assert.strictEqual(result.length, 2);
    assert.strictEqual(result[0].id, 'sub-1');
    assert.strictEqual(result[0].planName, 'Basic');
    assert.strictEqual(result[1].id, 'sub-2');
    assert.strictEqual(result[1].planName, 'Pro');
  });

  it('converts camelCase filter keys to snake_case', async () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    await svc.listSubscriptions({
      subscriberAddress: '0xAlice',
      providerAddress: '0xBob',
      status: 'active',
      serviceId: 'svc-1',
    });

    const passedFilter = store.listSubscriptions.mock.calls[0].arguments[0];
    assert.strictEqual(passedFilter.subscriber_address, '0xAlice');
    assert.strictEqual(passedFilter.provider_address, '0xBob');
    assert.strictEqual(passedFilter.status, 'active');
    assert.strictEqual(passedFilter.service_id, 'svc-1');
  });

  it('passes through limit and offset', async () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    await svc.listSubscriptions({ limit: 10, offset: 20 });

    const passedFilter = store.listSubscriptions.mock.calls[0].arguments[0];
    assert.strictEqual(passedFilter.limit, 10);
    assert.strictEqual(passedFilter.offset, 20);
  });

  it('passes through snake_case filter keys directly', async () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    await svc.listSubscriptions({
      subscriber_address: '0xDirect',
      provider_address: '0xDirectProvider',
      service_id: 'svc-direct',
    });

    const passedFilter = store.listSubscriptions.mock.calls[0].arguments[0];
    assert.strictEqual(passedFilter.subscriber_address, '0xDirect');
    assert.strictEqual(passedFilter.provider_address, '0xDirectProvider');
    assert.strictEqual(passedFilter.service_id, 'svc-direct');
  });

  it('defaults to empty filter when no argument', async () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    await svc.listSubscriptions();

    const passedFilter = store.listSubscriptions.mock.calls[0].arguments[0];
    assert.deepStrictEqual(passedFilter, {});
  });
});

// ===========================================================================
// processBilling
// ===========================================================================

describe('processBilling', () => {
  it('returns zeroed summary when nothing is due', async () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.deepStrictEqual(result, { processed: 0, succeeded: 0, failed: 0, cancelled: 0 });
  });

  it('bills a due subscription and advances totals', async () => {
    const dueSub = makeStoreRow({
      id: 'sub-due',
      status: 'active',
      amount: 10_000_000,
      amount_decimal: 10,
      total_billed: 0,
      total_billed_decimal: 0,
      billing_count: 0,
      billing_interval: 'monthly',
      cancel_at_period_end: false,
      current_period_end: new Date(Date.now() + 86400000).toISOString(),
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => [dueSub]),
      getExpiredTrials: mock.fn(async () => []),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.strictEqual(result.processed, 1);
    assert.strictEqual(result.succeeded, 1);
    assert.strictEqual(result.failed, 0);
    assert.strictEqual(result.cancelled, 0);

    const updateArgs = store.updateSubscription.mock.calls[0].arguments[1];
    assert.strictEqual(updateArgs.total_billed, 10_000_000);
    assert.strictEqual(updateArgs.total_billed_decimal, 10);
    assert.strictEqual(updateArgs.billing_count, 1);
    assert.ok(updateArgs.last_payment_id);
    assert.ok(updateArgs.current_period_start);
    assert.ok(updateArgs.current_period_end);
    assert.ok(updateArgs.next_billing_date);
  });

  it('accumulates billing totals for previously billed subscriptions', async () => {
    const dueSub = makeStoreRow({
      id: 'sub-repeat',
      amount: 5_000_000,
      amount_decimal: 5,
      total_billed: 15_000_000,
      total_billed_decimal: 15,
      billing_count: 3,
      billing_interval: 'monthly',
      cancel_at_period_end: false,
      current_period_end: new Date(Date.now() + 86400000).toISOString(),
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => [dueSub]),
      getExpiredTrials: mock.fn(async () => []),
    });
    const svc = createA2ASubscriptionService(store);

    await svc.processBilling();

    const updateArgs = store.updateSubscription.mock.calls[0].arguments[1];
    assert.strictEqual(updateArgs.total_billed, 20_000_000);
    assert.strictEqual(updateArgs.total_billed_decimal, 20);
    assert.strictEqual(updateArgs.billing_count, 4);
  });

  it('cancels subscriptions marked cancel_at_period_end when period has ended', async () => {
    const pastEnd = new Date(Date.now() - 86400000).toISOString();
    const dueSub = makeStoreRow({
      id: 'sub-cancel-end',
      cancel_at_period_end: true,
      current_period_end: pastEnd,
      billing_interval: 'monthly',
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => [dueSub]),
      getExpiredTrials: mock.fn(async () => []),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.strictEqual(result.cancelled, 1);
    assert.strictEqual(result.succeeded, 0);

    const updateArgs = store.updateSubscription.mock.calls[0].arguments[1];
    assert.strictEqual(updateArgs.status, 'cancelled');
    assert.ok(updateArgs.cancelled_at);
    assert.strictEqual(updateArgs.cancel_at_period_end, false);
  });

  it('does NOT cancel when cancel_at_period_end but period has not ended', async () => {
    const futureEnd = new Date(Date.now() + 86400000 * 30).toISOString();
    const dueSub = makeStoreRow({
      id: 'sub-not-yet',
      cancel_at_period_end: true,
      current_period_end: futureEnd,
      billing_interval: 'monthly',
      amount: 1_000_000,
      amount_decimal: 1,
      total_billed: 0,
      total_billed_decimal: 0,
      billing_count: 0,
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => [dueSub]),
      getExpiredTrials: mock.fn(async () => []),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    // It should bill, not cancel, because the period hasn't ended
    assert.strictEqual(result.cancelled, 0);
    assert.strictEqual(result.succeeded, 1);
  });

  it('transitions expired trials to active', async () => {
    const trial = makeStoreRow({
      id: 'sub-trial',
      status: 'trial',
      billing_interval: 'monthly',
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => []),
      getExpiredTrials: mock.fn(async () => [trial]),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.strictEqual(result.processed, 1);
    assert.strictEqual(result.succeeded, 1);

    const updateArgs = store.updateSubscription.mock.calls[0].arguments[1];
    assert.strictEqual(updateArgs.status, 'active');
    assert.ok(updateArgs.current_period_start);
    assert.ok(updateArgs.current_period_end);
    assert.ok(updateArgs.next_billing_date);
  });

  it('handles mixed due subscriptions and expired trials', async () => {
    const dueSub = makeStoreRow({
      id: 'sub-due',
      amount: 1_000_000,
      amount_decimal: 1,
      total_billed: 0,
      total_billed_decimal: 0,
      billing_count: 0,
      billing_interval: 'weekly',
      cancel_at_period_end: false,
      current_period_end: new Date(Date.now() + 86400000).toISOString(),
    });
    const expiredTrial = makeStoreRow({
      id: 'sub-trial-expired',
      status: 'trial',
      billing_interval: 'monthly',
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => [dueSub]),
      getExpiredTrials: mock.fn(async () => [expiredTrial]),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.strictEqual(result.processed, 2);
    assert.strictEqual(result.succeeded, 2);
    assert.strictEqual(result.failed, 0);
    assert.strictEqual(result.cancelled, 0);
  });

  it('counts billing failures without throwing', async () => {
    const dueSub = makeStoreRow({
      id: 'sub-fail',
      billing_interval: 'monthly',
      cancel_at_period_end: false,
      current_period_end: new Date(Date.now() + 86400000).toISOString(),
      amount: 1_000_000,
      amount_decimal: 1,
      total_billed: 0,
      total_billed_decimal: 0,
      billing_count: 0,
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => [dueSub]),
      getExpiredTrials: mock.fn(async () => []),
      updateSubscription: mock.fn(async () => {
        throw new Error('DB write failed');
      }),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.strictEqual(result.processed, 1);
    assert.strictEqual(result.failed, 1);
    assert.strictEqual(result.succeeded, 0);
  });

  it('counts trial transition failures without throwing', async () => {
    const trial = makeStoreRow({
      id: 'sub-trial-fail',
      status: 'trial',
      billing_interval: 'monthly',
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => []),
      getExpiredTrials: mock.fn(async () => [trial]),
      updateSubscription: mock.fn(async () => {
        throw new Error('DB unavailable');
      }),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.strictEqual(result.processed, 1);
    assert.strictEqual(result.failed, 1);
    assert.strictEqual(result.succeeded, 0);
  });

  it('processes multiple due subscriptions', async () => {
    const subs = [
      makeStoreRow({
        id: 'sub-1',
        amount: 1_000_000,
        amount_decimal: 1,
        total_billed: 0,
        total_billed_decimal: 0,
        billing_count: 0,
        billing_interval: 'monthly',
        cancel_at_period_end: false,
        current_period_end: new Date(Date.now() + 86400000).toISOString(),
      }),
      makeStoreRow({
        id: 'sub-2',
        amount: 2_000_000,
        amount_decimal: 2,
        total_billed: 0,
        total_billed_decimal: 0,
        billing_count: 0,
        billing_interval: 'weekly',
        cancel_at_period_end: false,
        current_period_end: new Date(Date.now() + 86400000).toISOString(),
      }),
    ];
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => subs),
      getExpiredTrials: mock.fn(async () => []),
    });
    const svc = createA2ASubscriptionService(store);

    const result = await svc.processBilling();
    assert.strictEqual(result.processed, 2);
    assert.strictEqual(result.succeeded, 2);
    assert.strictEqual(store.updateSubscription.mock.calls.length, 2);
  });

  it('generates a unique payment ID per billing', async () => {
    const dueSub = makeStoreRow({
      id: 'sub-pay',
      amount: 1_000_000,
      amount_decimal: 1,
      total_billed: 0,
      total_billed_decimal: 0,
      billing_count: 0,
      billing_interval: 'monthly',
      cancel_at_period_end: false,
      current_period_end: new Date(Date.now() + 86400000).toISOString(),
    });
    const store = createMockStore({
      getDueSubscriptions: mock.fn(async () => [dueSub]),
      getExpiredTrials: mock.fn(async () => []),
    });
    const svc = createA2ASubscriptionService(store);

    await svc.processBilling();

    const paymentId = store.updateSubscription.mock.calls[0].arguments[1].last_payment_id;
    assert.match(paymentId, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  });

  it('passes nowIso to getDueSubscriptions and getExpiredTrials', async () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    const before = new Date();
    await svc.processBilling();
    const after = new Date();

    const dueArg = store.getDueSubscriptions.mock.calls[0].arguments[0];
    const trialArg = store.getExpiredTrials.mock.calls[0].arguments[0];
    // Both should be the same ISO string
    assert.strictEqual(dueArg, trialArg);
    // And within the time window
    const ts = new Date(dueArg);
    assert.ok(ts >= before && ts <= after);
  });
});

// ===========================================================================
// Service factory returns all expected methods
// ===========================================================================

describe('createA2ASubscriptionService', () => {
  it('returns an object with all expected methods', () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);

    assert.strictEqual(typeof svc.createSubscription, 'function');
    assert.strictEqual(typeof svc.pauseSubscription, 'function');
    assert.strictEqual(typeof svc.resumeSubscription, 'function');
    assert.strictEqual(typeof svc.cancelSubscription, 'function');
    assert.strictEqual(typeof svc.getSubscription, 'function');
    assert.strictEqual(typeof svc.listSubscriptions, 'function');
    assert.strictEqual(typeof svc.processBilling, 'function');
  });

  it('returns exactly 7 methods', () => {
    const store = createMockStore();
    const svc = createA2ASubscriptionService(store);
    assert.strictEqual(Object.keys(svc).length, 7);
  });
});
