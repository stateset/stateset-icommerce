/**
 * Unit tests for a2a/billing-executor.js — Autonomous Subscription Billing Engine
 *
 * Covers: tick(), start(), stop(), getMetrics(), event emissions,
 * payment retry logic, dunning notifications, trial activation,
 * cancel_at_period_end, auto-cancel after maxPastDueCycles, skip when
 * tick already in progress.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createBillingExecutor } from '../../src/a2a/billing-executor.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a mock store where every method is a mock.fn */
function createMockStore(overrides = {}) {
  return {
    getExpiredTrials: mock.fn(async () => []),
    getDueSubscriptions: mock.fn(async () => []),
    updateSubscription: mock.fn(async (id, updates) => ({ id, ...updates })),
    ...overrides,
  };
}

/** Build a mock a2aService */
function createMockA2AService(overrides = {}) {
  return {
    pay: mock.fn(async () => ({ success: true, payment: { id: 'pay-001' } })),
    ...overrides,
  };
}

/** Build a mock notification service */
function createMockNotificationService(overrides = {}) {
  return {
    sendNotification: mock.fn(async () => ({ sent: true })),
    ...overrides,
  };
}

/** Build a raw subscription row as returned by the store */
function makeSubscriptionRow(overrides = {}) {
  const past = new Date(Date.now() - 86400000).toISOString(); // 1 day ago
  return {
    id: 'sub-100',
    subscriber_address: '0xAlice',
    provider_address: '0xProvider',
    plan_name: 'Pro Plan',
    status: 'active',
    amount: 49990000,
    amount_decimal: 49.99,
    asset: 'USDC',
    network: 'set_chain',
    billing_interval: 'monthly',
    current_period_start: past,
    current_period_end: past,
    next_billing_date: past,
    cancel_at_period_end: false,
    cancelled_at: null,
    past_due_since: null,
    max_past_due_cycles: 3,
    total_billed: 0,
    total_billed_decimal: 0,
    billing_count: 0,
    last_payment_id: null,
    ...overrides,
  };
}

/** Build a raw expired trial row */
function makeTrialRow(overrides = {}) {
  return {
    id: 'trial-200',
    subscriber_address: '0xBob',
    provider_address: '0xProvider',
    plan_name: 'Starter Plan',
    status: 'trial',
    billing_interval: 'monthly',
    trial_end_date: new Date(Date.now() - 3600000).toISOString(), // expired 1h ago
    ...overrides,
  };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('createBillingExecutor', () => {
  /** @type {ReturnType<typeof createBillingExecutor>} */
  let executor;

  afterEach(() => {
    // Always stop to prevent lingering setIntervals
    if (executor) {
      executor.stop();
      executor = null;
    }
  });

  // -----------------------------------------------------------------------
  // 1. Successfully bills a due subscription
  // -----------------------------------------------------------------------
  describe('successful billing', () => {
    it('calls a2aService.pay and advances billing window', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.billed, 1);
      assert.equal(result.failed, 0);

      // a2aService.pay was called once
      assert.equal(a2a.pay.mock.callCount(), 1);
      const payCall = a2a.pay.mock.calls[0].arguments[0];
      assert.equal(payCall.to, '0xProvider');
      assert.equal(payCall.amount, 49.99);
      assert.equal(payCall.asset, 'USDC');
      assert.equal(payCall.network, 'set_chain');
      assert.ok(payCall.memo.includes('Pro Plan'));
      assert.ok(payCall.idempotencyKey.startsWith('sub-sub-100-'));

      // updateSubscription was called to advance window
      assert.equal(store.updateSubscription.mock.callCount(), 1);
      const updateArgs = store.updateSubscription.mock.calls[0].arguments;
      assert.equal(updateArgs[0], 'sub-100');
      const updates = updateArgs[1];
      assert.equal(updates.billing_count, 1);
      assert.equal(updates.total_billed_decimal, 49.99);
      assert.equal(updates.last_payment_id, 'pay-001');
      assert.equal(updates.past_due_since, null); // cleared
      assert.ok(updates.next_billing_date); // advanced
      assert.ok(updates.current_period_start);
      assert.ok(updates.current_period_end);
    });

    it('accumulates total_billed and billing_count across multiple billings', async () => {
      const sub = makeSubscriptionRow({
        total_billed: 100000000,
        total_billed_decimal: 100,
        billing_count: 2,
        amount: 25000000,
        amount_decimal: 25.0,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();

      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      assert.equal(updates.billing_count, 3);
      assert.equal(updates.total_billed, 125000000);
      assert.equal(updates.total_billed_decimal, 125);
    });

    it('emits billing_succeeded event', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const events = [];
      executor.on('billing_succeeded', (evt) => events.push(evt));

      await executor.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].subscriptionId, 'sub-100');
      assert.equal(events[0].paymentId, 'pay-001');
      assert.equal(events[0].amount, 49.99);
    });

    it('uses randomUUID as paymentId when pay result has no payment.id', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => ({ success: true })), // no payment.id
      });
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();

      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      // Should be a UUID (36 chars with dashes)
      assert.ok(updates.last_payment_id);
      assert.match(updates.last_payment_id, /^[0-9a-f]{8}-[0-9a-f]{4}-/);
    });
  });

  // -----------------------------------------------------------------------
  // 2. Handles payment failure (marks past_due, sends dunning notification)
  // -----------------------------------------------------------------------
  describe('payment failure', () => {
    it('marks subscription past_due when payment fails', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Insufficient funds'); }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 1 });

      const result = await executor.tick();

      assert.equal(result.billed, 0);
      assert.equal(result.failed, 1);

      // updateSubscription sets past_due_since and advances next_billing_date
      assert.equal(store.updateSubscription.mock.callCount(), 1);
      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      assert.ok(updates.past_due_since);
      assert.ok(updates.next_billing_date);
    });

    it('does not overwrite existing past_due_since on repeated failure', async () => {
      const oldPastDue = new Date(Date.now() - 7 * 86400000).toISOString();
      const sub = makeSubscriptionRow({ past_due_since: oldPastDue });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Still failing'); }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 1 });

      await executor.tick();

      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      // past_due_since should NOT be set (preserving the original)
      assert.equal(updates.past_due_since, undefined);
      // Only next_billing_date should be advanced
      assert.ok(updates.next_billing_date);
    });

    it('emits billing_failed event', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Network error'); }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 1 });

      const events = [];
      executor.on('billing_failed', (evt) => events.push(evt));

      await executor.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].subscriptionId, 'sub-100');
      assert.equal(events[0].error, 'Network error');
    });

    it('handles pay returning { success: false } without throwing', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => ({ success: false, error: 'Declined' })),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 1 });

      const result = await executor.tick();

      assert.equal(result.failed, 1);
      assert.equal(result.billed, 0);
    });
  });

  // -----------------------------------------------------------------------
  // 3. Auto-cancels after maxPastDueCycles exceeded
  // -----------------------------------------------------------------------
  describe('auto-cancel after maxPastDueCycles', () => {
    it('cancels subscription when past_due_cycles exceeds max', async () => {
      // Set past_due_since far enough back to exceed 3 monthly cycles (>90 days)
      const longAgo = new Date(Date.now() - 100 * 86400000).toISOString();
      const sub = makeSubscriptionRow({
        past_due_since: longAgo,
        max_past_due_cycles: 3,
        billing_interval: 'monthly',
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.cancelled, 1);
      assert.equal(result.billed, 0);

      // a2aService.pay should NOT be called (cancelled before billing attempt)
      assert.equal(a2a.pay.mock.callCount(), 0);

      // updateSubscription was called with cancelled status
      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      assert.equal(updates.status, 'cancelled');
      assert.ok(updates.cancelled_at);
    });

    it('emits subscription_cancelled event with reason max_past_due_cycles_exceeded', async () => {
      const longAgo = new Date(Date.now() - 100 * 86400000).toISOString();
      const sub = makeSubscriptionRow({
        past_due_since: longAgo,
        max_past_due_cycles: 3,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const events = [];
      executor.on('subscription_cancelled', (evt) => events.push(evt));

      await executor.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].subscriptionId, 'sub-100');
      assert.equal(events[0].reason, 'max_past_due_cycles_exceeded');
      assert.ok(events[0].pastDueCycles >= 3);
    });

    it('sends cancellation notification when past_due auto-cancels', async () => {
      const longAgo = new Date(Date.now() - 100 * 86400000).toISOString();
      const sub = makeSubscriptionRow({
        past_due_since: longAgo,
        max_past_due_cycles: 3,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      const notifications = createMockNotificationService();
      executor = createBillingExecutor(store, a2a, notifications);

      await executor.tick();

      assert.equal(notifications.sendNotification.mock.callCount(), 1);
      const notifCall = notifications.sendNotification.mock.calls[0].arguments[0];
      assert.equal(notifCall.recipientAddress, '0xAlice');
      assert.equal(notifCall.eventType, 'subscription.cancelled');
      assert.equal(notifCall.payload.subscriptionId, 'sub-100');
      assert.ok(notifCall.payload.reason.includes('Payment failed'));
    });

    it('does not cancel if past_due_cycles has not reached max', async () => {
      // Only 15 days past due with monthly interval — 0 full cycles
      const recentPastDue = new Date(Date.now() - 15 * 86400000).toISOString();
      const sub = makeSubscriptionRow({
        past_due_since: recentPastDue,
        max_past_due_cycles: 3,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      // Should attempt billing, not cancel
      assert.equal(result.cancelled, 0);
      assert.equal(a2a.pay.mock.callCount(), 1);
    });

    it('uses custom max_past_due_cycles from subscription row', async () => {
      // 40 days past due, weekly interval = 5 cycles, max is 5 => should cancel
      const pastDue = new Date(Date.now() - 40 * 86400000).toISOString();
      const sub = makeSubscriptionRow({
        past_due_since: pastDue,
        max_past_due_cycles: 5,
        billing_interval: 'weekly',
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.cancelled, 1);
    });
  });

  // -----------------------------------------------------------------------
  // 4. Processes cancel_at_period_end subscriptions
  // -----------------------------------------------------------------------
  describe('cancel_at_period_end', () => {
    it('cancels subscription when period has ended and cancel_at_period_end is true', async () => {
      const past = new Date(Date.now() - 86400000).toISOString();
      const sub = makeSubscriptionRow({
        cancel_at_period_end: true,
        current_period_end: past,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.cancelled, 1);
      assert.equal(result.billed, 0);

      // No payment should be attempted
      assert.equal(a2a.pay.mock.callCount(), 0);

      // Status set to cancelled
      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      assert.equal(updates.status, 'cancelled');
      assert.ok(updates.cancelled_at);
      assert.equal(updates.cancel_at_period_end, false);
    });

    it('emits subscription_cancelled with reason cancel_at_period_end', async () => {
      const past = new Date(Date.now() - 86400000).toISOString();
      const sub = makeSubscriptionRow({
        cancel_at_period_end: true,
        current_period_end: past,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const events = [];
      executor.on('subscription_cancelled', (evt) => events.push(evt));

      await executor.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].reason, 'cancel_at_period_end');
    });

    it('does not cancel if current_period_end is in the future', async () => {
      const future = new Date(Date.now() + 7 * 86400000).toISOString();
      const sub = makeSubscriptionRow({
        cancel_at_period_end: true,
        current_period_end: future,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      // Should still attempt billing, not cancel yet
      assert.equal(result.cancelled, 0);
      assert.equal(a2a.pay.mock.callCount(), 1);
    });

    it('does not cancel if cancel_at_period_end is false', async () => {
      const past = new Date(Date.now() - 86400000).toISOString();
      const sub = makeSubscriptionRow({
        cancel_at_period_end: false,
        current_period_end: past,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      // Should bill normally
      assert.equal(result.cancelled, 0);
      assert.equal(result.billed, 1);
    });
  });

  // -----------------------------------------------------------------------
  // 5. Activates expired trials
  // -----------------------------------------------------------------------
  describe('trial activation', () => {
    it('transitions expired trials to active status', async () => {
      const trial = makeTrialRow();
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => [trial]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.trialsActivated, 1);

      // updateSubscription called for trial activation
      assert.ok(store.updateSubscription.mock.callCount() >= 1);
      const updateCall = store.updateSubscription.mock.calls[0];
      assert.equal(updateCall.arguments[0], 'trial-200');
      const updates = updateCall.arguments[1];
      assert.equal(updates.status, 'active');
      assert.ok(updates.current_period_start);
      assert.ok(updates.current_period_end);
      assert.ok(updates.next_billing_date);
    });

    it('emits trial_activated event', async () => {
      const trial = makeTrialRow();
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => [trial]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const events = [];
      executor.on('trial_activated', (evt) => events.push(evt));

      await executor.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].subscriptionId, 'trial-200');
    });

    it('processes multiple expired trials', async () => {
      const trials = [
        makeTrialRow({ id: 'trial-1' }),
        makeTrialRow({ id: 'trial-2' }),
        makeTrialRow({ id: 'trial-3' }),
      ];
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => trials),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.trialsActivated, 3);
    });

    it('continues processing remaining trials if one fails', async () => {
      let callCount = 0;
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => [
          makeTrialRow({ id: 'trial-ok-1' }),
          makeTrialRow({ id: 'trial-bad' }),
          makeTrialRow({ id: 'trial-ok-2' }),
        ]),
        updateSubscription: mock.fn(async (id) => {
          callCount++;
          if (id === 'trial-bad') throw new Error('DB write error');
          return { id };
        }),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      // 2 out of 3 should activate
      assert.equal(result.trialsActivated, 2);
    });

    it('computes correct next billing date for different intervals', async () => {
      const weeklyTrial = makeTrialRow({ id: 'trial-w', billing_interval: 'weekly' });
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => [weeklyTrial]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();

      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      const nextDate = new Date(updates.next_billing_date);
      const now = new Date();
      // For weekly: next billing should be ~7 days from now
      const diffDays = (nextDate - now) / (24 * 60 * 60 * 1000);
      assert.ok(diffDays >= 6.9 && diffDays <= 7.1, `Expected ~7 days, got ${diffDays}`);
    });
  });

  // -----------------------------------------------------------------------
  // 6. Retries payment up to maxRetries
  // -----------------------------------------------------------------------
  describe('payment retries', () => {
    it('retries up to maxRetries times before failing', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Transient error'); }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 3 });

      const result = await executor.tick();

      assert.equal(result.failed, 1);
      assert.equal(a2a.pay.mock.callCount(), 3);
    });

    it('succeeds on second attempt after first failure', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      let attempt = 0;
      const a2a = createMockA2AService({
        pay: mock.fn(async () => {
          attempt++;
          if (attempt === 1) throw new Error('Temporary failure');
          return { success: true, payment: { id: 'pay-retry' } };
        }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 3 });

      const result = await executor.tick();

      assert.equal(result.billed, 1);
      assert.equal(result.failed, 0);
      assert.equal(a2a.pay.mock.callCount(), 2);
    });

    it('succeeds on final attempt', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      let attempt = 0;
      const a2a = createMockA2AService({
        pay: mock.fn(async () => {
          attempt++;
          if (attempt < 3) throw new Error('Failing');
          return { success: true, payment: { id: 'pay-final' } };
        }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 3 });

      const result = await executor.tick();

      assert.equal(result.billed, 1);
      assert.equal(a2a.pay.mock.callCount(), 3);
    });

    it('respects maxRetries=1 (no retries)', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Fail'); }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 1 });

      const result = await executor.tick();

      assert.equal(result.failed, 1);
      assert.equal(a2a.pay.mock.callCount(), 1);
    });

    it('handles mixed success/failure from non-throwing pay response', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      let attempt = 0;
      const a2a = createMockA2AService({
        pay: mock.fn(async () => {
          attempt++;
          if (attempt === 1) return { success: false, error: 'Declined' };
          return { success: true, payment: { id: 'pay-ok' } };
        }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 3 });

      const result = await executor.tick();

      assert.equal(result.billed, 1);
      assert.equal(a2a.pay.mock.callCount(), 2);
    });
  });

  // -----------------------------------------------------------------------
  // 7. Skips if tick already in progress
  // -----------------------------------------------------------------------
  describe('concurrent tick guard', () => {
    it('returns skipped result if tick is already running', async () => {
      // Create a slow store that delays getDueSubscriptions
      let resolveSlowCall;
      const slowPromise = new Promise((resolve) => {
        resolveSlowCall = resolve;
      });
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => {
          await slowPromise;
          return [];
        }),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      // Start first tick (will block on getExpiredTrials)
      const tick1 = executor.tick();

      // Start second tick — should be skipped
      const tick2Result = await executor.tick();

      assert.equal(tick2Result.skipped, true);
      assert.equal(tick2Result.reason, 'previous tick still running');

      // Resolve the slow call so tick1 completes
      resolveSlowCall();
      await tick1;
    });

    it('allows a new tick after previous one completes', async () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result1 = await executor.tick();
      assert.equal(result1.skipped, undefined);

      const result2 = await executor.tick();
      assert.equal(result2.skipped, undefined);
    });
  });

  // -----------------------------------------------------------------------
  // 8. start() / stop() lifecycle
  // -----------------------------------------------------------------------
  describe('start/stop lifecycle', () => {
    it('start() sets running to true in metrics', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      assert.equal(executor.getMetrics().running, false);

      executor.start();
      assert.equal(executor.getMetrics().running, true);
    });

    it('stop() sets running to false in metrics', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      executor.start();
      assert.equal(executor.getMetrics().running, true);

      executor.stop();
      assert.equal(executor.getMetrics().running, false);
    });

    it('start() is idempotent — calling twice does not create duplicate timers', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      executor.start();
      executor.start(); // should be a no-op

      assert.equal(executor.getMetrics().running, true);

      // stop once should be sufficient
      executor.stop();
      assert.equal(executor.getMetrics().running, false);
    });

    it('stop() is idempotent — calling twice does not throw', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      executor.start();
      executor.stop();
      executor.stop(); // should not throw

      assert.equal(executor.getMetrics().running, false);
    });

    it('emits started event on start()', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      const events = [];
      executor.on('started', () => events.push('started'));

      executor.start();
      assert.equal(events.length, 1);
    });

    it('emits stopped event on stop()', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      const events = [];
      executor.on('stopped', () => events.push('stopped'));

      executor.start();
      executor.stop();
      assert.equal(events.length, 1);
    });

    it('does not emit started if already running', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      const events = [];
      executor.on('started', () => events.push('started'));

      executor.start();
      executor.start();
      assert.equal(events.length, 1); // only one emit
    });

    it('does not emit stopped if not running', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      const events = [];
      executor.on('stopped', () => events.push('stopped'));

      executor.stop(); // never started
      assert.equal(events.length, 0);
    });
  });

  // -----------------------------------------------------------------------
  // 9. getMetrics() after multiple ticks
  // -----------------------------------------------------------------------
  describe('getMetrics()', () => {
    it('returns initial metrics when no ticks have run', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 5000 });

      const metrics = executor.getMetrics();

      assert.equal(metrics.totalTicks, 0);
      assert.equal(metrics.totalBilled, 0);
      assert.equal(metrics.totalFailed, 0);
      assert.equal(metrics.totalCancelled, 0);
      assert.equal(metrics.totalTrialsActivated, 0);
      assert.equal(metrics.totalDunningsSent, 0);
      assert.equal(metrics.lastTickAt, null);
      assert.equal(metrics.lastTickDurationMs, 0);
      assert.equal(metrics.running, false);
      assert.equal(metrics.intervalMs, 5000);
    });

    it('accumulates metrics across multiple ticks', async () => {
      const sub1 = makeSubscriptionRow({ id: 'sub-1' });
      const sub2 = makeSubscriptionRow({ id: 'sub-2' });
      let tickCount = 0;
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => {
          tickCount++;
          if (tickCount === 1) return [sub1];
          if (tickCount === 2) return [sub2];
          return [];
        }),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();
      await executor.tick();
      await executor.tick(); // empty tick

      const metrics = executor.getMetrics();

      assert.equal(metrics.totalTicks, 3);
      assert.equal(metrics.totalBilled, 2);
      assert.ok(metrics.lastTickAt);
      assert.ok(metrics.lastTickDurationMs >= 0);
    });

    it('returns a copy (not a reference to internal state)', async () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const metrics1 = executor.getMetrics();
      metrics1.totalTicks = 999; // mutate the copy

      const metrics2 = executor.getMetrics();
      assert.equal(metrics2.totalTicks, 0); // unaffected
    });

    it('tracks cancelled subscriptions in metrics', async () => {
      const past = new Date(Date.now() - 86400000).toISOString();
      const sub = makeSubscriptionRow({
        cancel_at_period_end: true,
        current_period_end: past,
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();

      assert.equal(executor.getMetrics().totalCancelled, 1);
    });

    it('tracks trials activated in metrics', async () => {
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => [makeTrialRow()]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();

      assert.equal(executor.getMetrics().totalTrialsActivated, 1);
    });

    it('tracks dunnings sent in metrics', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Fail'); }),
      });
      const notifications = createMockNotificationService();
      executor = createBillingExecutor(store, a2a, notifications, { maxRetries: 1 });

      await executor.tick();

      assert.equal(executor.getMetrics().totalDunningsSent, 1);
    });

    it('emits tick_complete event with result', async () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const events = [];
      executor.on('tick_complete', (evt) => events.push(evt));

      await executor.tick();

      assert.equal(events.length, 1);
      assert.equal(events[0].billed, 0);
      assert.equal(events[0].failed, 0);
      assert.equal(events[0].cancelled, 0);
      assert.ok(events[0].durationMs >= 0);
    });
  });

  // -----------------------------------------------------------------------
  // 10. Sends dunning notification on failure
  // -----------------------------------------------------------------------
  describe('dunning notifications', () => {
    it('sends dunning notification when payment fails', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Card declined'); }),
      });
      const notifications = createMockNotificationService();
      executor = createBillingExecutor(store, a2a, notifications, { maxRetries: 1 });

      const result = await executor.tick();

      assert.equal(result.dunningsSent, 1);
      assert.equal(notifications.sendNotification.mock.callCount(), 1);

      const notifCall = notifications.sendNotification.mock.calls[0].arguments[0];
      assert.equal(notifCall.recipientAddress, '0xAlice');
      assert.equal(notifCall.eventType, 'subscription.payment_failed');
      assert.equal(notifCall.payload.subscriptionId, 'sub-100');
      assert.equal(notifCall.payload.planName, 'Pro Plan');
      assert.equal(notifCall.payload.amount, 49.99);
      assert.equal(notifCall.payload.error, 'Card declined');
    });

    it('does not send dunning if notificationService is null', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Fail'); }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 1 });

      const result = await executor.tick();

      // No crash, dunningsSent stays 0
      assert.equal(result.dunningsSent, 0);
      assert.equal(result.failed, 1);
    });

    it('does not crash if sendNotification throws', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Fail'); }),
      });
      const notifications = createMockNotificationService({
        sendNotification: mock.fn(async () => { throw new Error('Notification service down'); }),
      });
      executor = createBillingExecutor(store, a2a, notifications, { maxRetries: 1 });

      // Should not throw
      const result = await executor.tick();

      assert.equal(result.failed, 1);
      assert.equal(result.dunningsSent, 0); // notification failed
    });

    it('sends dunning for each failed subscription in a batch', async () => {
      const sub1 = makeSubscriptionRow({ id: 'sub-A', subscriber_address: '0xAlice' });
      const sub2 = makeSubscriptionRow({ id: 'sub-B', subscriber_address: '0xBob' });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub1, sub2]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Fail'); }),
      });
      const notifications = createMockNotificationService();
      executor = createBillingExecutor(store, a2a, notifications, { maxRetries: 1 });

      const result = await executor.tick();

      assert.equal(result.dunningsSent, 2);
      assert.equal(notifications.sendNotification.mock.callCount(), 2);

      const recipients = notifications.sendNotification.mock.calls.map(
        (c) => c.arguments[0].recipientAddress,
      );
      assert.ok(recipients.includes('0xAlice'));
      assert.ok(recipients.includes('0xBob'));
    });

    it('includes pastDueSince in dunning payload', async () => {
      const sub = makeSubscriptionRow();
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Fail'); }),
      });
      const notifications = createMockNotificationService();
      executor = createBillingExecutor(store, a2a, notifications, { maxRetries: 1 });

      await executor.tick();

      const payload = notifications.sendNotification.mock.calls[0].arguments[0].payload;
      assert.ok(payload.pastDueSince);
    });

    it('includes asset in dunning payload', async () => {
      const sub = makeSubscriptionRow({ asset: 'ssUSD' });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService({
        pay: mock.fn(async () => { throw new Error('Fail'); }),
      });
      const notifications = createMockNotificationService();
      executor = createBillingExecutor(store, a2a, notifications, { maxRetries: 1 });

      await executor.tick();

      const payload = notifications.sendNotification.mock.calls[0].arguments[0].payload;
      assert.equal(payload.asset, 'ssUSD');
    });
  });

  // -----------------------------------------------------------------------
  // Additional edge cases
  // -----------------------------------------------------------------------
  describe('edge cases', () => {
    it('handles empty getDueSubscriptions and getExpiredTrials', async () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.billed, 0);
      assert.equal(result.failed, 0);
      assert.equal(result.cancelled, 0);
      assert.equal(result.trialsActivated, 0);
      assert.equal(result.dunningsSent, 0);
    });

    it('processes multiple due subscriptions in one tick', async () => {
      const subs = [
        makeSubscriptionRow({ id: 'sub-1' }),
        makeSubscriptionRow({ id: 'sub-2' }),
        makeSubscriptionRow({ id: 'sub-3' }),
      ];
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => subs),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      const result = await executor.tick();

      assert.equal(result.billed, 3);
      assert.equal(a2a.pay.mock.callCount(), 3);
    });

    it('handles mixed success and failure in same tick', async () => {
      const goodSub = makeSubscriptionRow({ id: 'sub-good' });
      const badSub = makeSubscriptionRow({ id: 'sub-bad' });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [goodSub, badSub]),
      });
      let callIdx = 0;
      const a2a = createMockA2AService({
        pay: mock.fn(async () => {
          callIdx++;
          if (callIdx === 1) return { success: true, payment: { id: 'pay-ok' } };
          throw new Error('Second payment fails');
        }),
      });
      executor = createBillingExecutor(store, a2a, null, { maxRetries: 1 });

      const result = await executor.tick();

      assert.equal(result.billed, 1);
      assert.equal(result.failed, 1);
    });

    it('clears past_due_since when payment succeeds after prior failure', async () => {
      const sub = makeSubscriptionRow({
        past_due_since: new Date(Date.now() - 5 * 86400000).toISOString(),
      });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();

      const updates = store.updateSubscription.mock.calls[0].arguments[1];
      assert.equal(updates.past_due_since, null); // cleared
    });

    it('uses default asset/network when subscription lacks them', async () => {
      const sub = makeSubscriptionRow({ asset: undefined, network: undefined });
      const store = createMockStore({
        getDueSubscriptions: mock.fn(async () => [sub]),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await executor.tick();

      const payArgs = a2a.pay.mock.calls[0].arguments[0];
      assert.equal(payArgs.asset, 'USDC');
      assert.equal(payArgs.network, 'set_chain');
    });

    it('resets _tickInProgress after error so next tick is not skipped', async () => {
      let callCount = 0;
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => {
          callCount++;
          if (callCount === 1) throw new Error('Transient DB error');
          return [];
        }),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      // First tick throws
      await assert.rejects(() => executor.tick(), { message: 'Transient DB error' });

      // Second tick should NOT be skipped (proves _tickInProgress was reset)
      const result = await executor.tick();
      assert.equal(result.skipped, undefined);
      assert.equal(result.billed, 0);
    });

    it('tick throws when store.getExpiredTrials fails', async () => {
      const store = createMockStore({
        getExpiredTrials: mock.fn(async () => { throw new Error('DB crash'); }),
        getDueSubscriptions: mock.fn(async () => []),
      });
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      await assert.rejects(() => executor.tick(), { message: 'DB crash' });
    });

    it('uses options.intervalMs default of 60000', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null);

      assert.equal(executor.getMetrics().intervalMs, 60000);
    });

    it('uses custom intervalMs', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 30000 });

      assert.equal(executor.getMetrics().intervalMs, 30000);
    });

    it('supports on/off event listener management', () => {
      const store = createMockStore();
      const a2a = createMockA2AService();
      executor = createBillingExecutor(store, a2a, null, { intervalMs: 100_000 });

      const events = [];
      const handler = () => events.push('started');

      executor.on('started', handler);
      executor.start();
      assert.equal(events.length, 1);

      executor.stop();
      executor.off('started', handler);
      executor.start();
      assert.equal(events.length, 1); // handler was removed
    });
  });
});
