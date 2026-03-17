/**
 * Tests for cli/src/a2a/scheduler.js — A2A Scheduled Actions
 *
 * Covers: scheduleAction, processDueActions, recurring actions, maxExecutions,
 * cancelAction, failed executor, getAction, listActions, start/stop,
 * getMetrics, event emissions, idempotency, multi-agent independence.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { createSchedulerService } from '../../src/a2a/scheduler.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** ISO date in the past (default: 1 hour ago) */
function pastDate(ms = 3600000) {
  return new Date(Date.now() - ms).toISOString();
}

/** ISO date in the future (default: 1 hour from now) */
function futureDate(ms = 3600000) {
  return new Date(Date.now() + ms).toISOString();
}

/** Collect emitted events on a scheduler into a log array. */
function collectEvents(scheduler, eventNames) {
  const log = [];
  for (const name of eventNames) {
    scheduler.on(name, (data) => log.push({ event: name, data }));
  }
  return log;
}

// ===========================================================================
// 1. scheduleAction creates action with correct fields
// ===========================================================================

describe('scheduler — scheduleAction', () => {
  it('creates an action with all expected fields', () => {
    const scheduler = createSchedulerService();
    const executeAt = futureDate();

    const { actionId, action } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: { to: '0xBob', amount: 50 },
      executeAt,
      description: 'Pay invoice',
    });

    assert.ok(actionId, 'actionId should be defined');
    assert.equal(action.id, actionId);
    assert.equal(action.agentAddress, '0xAlice');
    assert.equal(action.actionType, 'payment');
    assert.deepEqual(action.payload, { to: '0xBob', amount: 50 });
    assert.equal(action.description, 'Pay invoice');
    assert.equal(action.status, 'pending');
    assert.equal(action.executeAt, new Date(executeAt).toISOString());
    assert.equal(action.repeatInterval, null);
    assert.equal(action.maxExecutions, null);
    assert.equal(action.executionCount, 0);
    assert.equal(action.lastExecutionId, null);
    assert.equal(action.lastExecutedAt, null);
    assert.equal(action.lastResult, null);
    assert.equal(action.lastError, null);
    assert.ok(action.createdAt);
    assert.ok(action.updatedAt);
  });

  it('creates recurring action with repeatInterval and maxExecutions', () => {
    const scheduler = createSchedulerService();

    const { action } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'escrow_check',
      payload: { escrowId: 'esc-001' },
      executeAt: futureDate(),
      repeatInterval: 60000,
      maxExecutions: 10,
      description: 'Hourly check',
    });

    assert.equal(action.repeatInterval, 60000);
    assert.equal(action.maxExecutions, 10);
  });

  it('rejects missing agentAddress', () => {
    const scheduler = createSchedulerService();
    assert.throws(
      () =>
        scheduler.scheduleAction({
          actionType: 'payment',
          payload: {},
          executeAt: futureDate(),
        }),
      /agentAddress is required/,
    );
  });

  it('rejects invalid actionType', () => {
    const scheduler = createSchedulerService();
    assert.throws(
      () =>
        scheduler.scheduleAction({
          agentAddress: '0xAlice',
          actionType: 'invalid_type',
          payload: {},
          executeAt: futureDate(),
        }),
      /Invalid actionType/,
    );
  });

  it('rejects missing executeAt', () => {
    const scheduler = createSchedulerService();
    assert.throws(
      () =>
        scheduler.scheduleAction({
          agentAddress: '0xAlice',
          actionType: 'payment',
          payload: {},
        }),
      /executeAt is required/,
    );
  });

  it('rejects invalid executeAt', () => {
    const scheduler = createSchedulerService();
    assert.throws(
      () =>
        scheduler.scheduleAction({
          agentAddress: '0xAlice',
          actionType: 'payment',
          payload: {},
          executeAt: 'not-a-date',
        }),
      /valid ISO 8601/,
    );
  });

  it('rejects non-positive repeatInterval', () => {
    const scheduler = createSchedulerService();
    assert.throws(
      () =>
        scheduler.scheduleAction({
          agentAddress: '0xAlice',
          actionType: 'payment',
          payload: {},
          executeAt: futureDate(),
          repeatInterval: -1,
        }),
      /repeatInterval must be a positive/,
    );
  });

  it('accepts all valid action types', () => {
    const scheduler = createSchedulerService();
    const types = [
      'payment',
      'quote_request',
      'escrow_check',
      'status_check',
      'custom',
      'reminder',
      'billing',
      'sla_check',
    ];

    for (const actionType of types) {
      const { action } = scheduler.scheduleAction({
        agentAddress: '0xAgent',
        actionType,
        payload: {},
        executeAt: futureDate(),
      });
      assert.equal(action.actionType, actionType);
    }
  });
});

// ===========================================================================
// 2. processDueActions executes actions whose time has passed
// ===========================================================================

describe('scheduler — processDueActions (due actions)', () => {
  it('executes actions whose executeAt is in the past', async () => {
    const executedActions = [];
    const scheduler = createSchedulerService({
      executor: async (action) => {
        executedActions.push(action.id);
        return { ok: true };
      },
    });

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: { amount: 100 },
      executeAt: pastDate(),
    });

    const result = await scheduler.processDueActions();

    assert.equal(result.executed, 1);
    assert.equal(result.failed, 0);
    assert.equal(result.skipped, 0);
    assert.equal(executedActions.length, 1);
    assert.equal(executedActions[0], actionId);

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'completed');
    assert.equal(action.executionCount, 1);
    assert.ok(action.lastExecutionId);
    assert.ok(action.lastExecutedAt);
    assert.deepEqual(action.lastResult, { ok: true });
  });

  it('executes multiple due actions in one pass', async () => {
    const scheduler = createSchedulerService();

    scheduler.scheduleAction({
      agentAddress: '0xA',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xB',
      actionType: 'billing',
      payload: {},
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xC',
      actionType: 'reminder',
      payload: {},
      executeAt: pastDate(),
    });

    const result = await scheduler.processDueActions();
    assert.equal(result.executed, 3);
  });
});

// ===========================================================================
// 3. processDueActions skips actions in the future
// ===========================================================================

describe('scheduler — processDueActions (future actions)', () => {
  it('skips actions whose executeAt is in the future', async () => {
    const executedActions = [];
    const scheduler = createSchedulerService({
      executor: async (action) => {
        executedActions.push(action.id);
      },
    });

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });

    const result = await scheduler.processDueActions();

    assert.equal(result.executed, 0);
    assert.equal(result.failed, 0);
    assert.equal(result.skipped, 0);
    assert.equal(executedActions.length, 0);
  });

  it('executes past but not future actions in mixed set', async () => {
    const executed = [];
    const scheduler = createSchedulerService({
      executor: async (action) => {
        executed.push(action.id);
      },
    });

    const { actionId: pastId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'reminder',
      payload: {},
      executeAt: futureDate(),
    });

    const result = await scheduler.processDueActions();
    assert.equal(result.executed, 1);
    assert.equal(executed[0], pastId);
  });
});

// ===========================================================================
// 4. Recurring actions auto-reschedule after execution
// ===========================================================================

describe('scheduler — recurring actions', () => {
  it('auto-reschedules with new executeAt after successful execution', async () => {
    const scheduler = createSchedulerService();
    const baseTime = pastDate();

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'escrow_check',
      payload: { escrowId: 'esc-1' },
      executeAt: baseTime,
      repeatInterval: 60000,
    });

    await scheduler.processDueActions();

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'pending', 'recurring action returns to pending');
    assert.equal(action.executionCount, 1);

    // Verify executeAt has been pushed forward by repeatInterval
    const originalTime = new Date(baseTime).getTime();
    const newTime = new Date(action.executeAt).getTime();
    assert.equal(newTime, originalTime + 60000);
  });

  it('executes multiple times across multiple processDueActions calls', async () => {
    let callCount = 0;
    const scheduler = createSchedulerService({
      executor: async () => {
        callCount++;
      },
    });

    // Schedule with a very short interval in the past
    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'status_check',
      payload: {},
      executeAt: pastDate(200000), // 200s ago
      repeatInterval: 1, // 1ms — so re-scheduled time is still in the past
    });

    await scheduler.processDueActions();
    assert.equal(callCount, 1);

    // The action has been rescheduled to 200s ago + 1ms, still in the past
    await scheduler.processDueActions();
    assert.equal(callCount, 2);

    await scheduler.processDueActions();
    assert.equal(callCount, 3);
  });
});

// ===========================================================================
// 5. maxExecutions limits recurring actions
// ===========================================================================

describe('scheduler — maxExecutions', () => {
  it('stops recurring after reaching maxExecutions', async () => {
    let execCount = 0;
    const scheduler = createSchedulerService({
      executor: async () => {
        execCount++;
      },
    });

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'sla_check',
      payload: {},
      executeAt: pastDate(300000),
      repeatInterval: 1,
      maxExecutions: 3,
    });

    // Run enough times to hit the limit
    await scheduler.processDueActions(); // 1
    await scheduler.processDueActions(); // 2
    await scheduler.processDueActions(); // 3
    await scheduler.processDueActions(); // should not execute

    assert.equal(execCount, 3);

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'completed');
    assert.equal(action.executionCount, 3);
  });

  it('completes on last allowed execution', async () => {
    const scheduler = createSchedulerService();

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'billing',
      payload: {},
      executeAt: pastDate(),
      repeatInterval: 1,
      maxExecutions: 1,
    });

    await scheduler.processDueActions();

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'completed');
    assert.equal(action.executionCount, 1);
  });
});

// ===========================================================================
// 6. cancelAction prevents execution
// ===========================================================================

describe('scheduler — cancelAction', () => {
  it('prevents a pending action from executing', async () => {
    let executed = false;
    const scheduler = createSchedulerService({
      executor: async () => {
        executed = true;
      },
    });

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    scheduler.cancelAction(actionId);

    const result = await scheduler.processDueActions();
    assert.equal(result.executed, 0);
    assert.equal(executed, false);

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'cancelled');
  });

  it('throws when cancelling a non-existent action', () => {
    const scheduler = createSchedulerService();
    assert.throws(() => scheduler.cancelAction('non-existent'), /Action not found/);
  });

  it('throws when cancelling an already cancelled action', () => {
    const scheduler = createSchedulerService();

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });

    scheduler.cancelAction(actionId);
    assert.throws(() => scheduler.cancelAction(actionId), /already cancelled/);
  });

  it('throws when cancelling a completed action', async () => {
    const scheduler = createSchedulerService();

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();
    assert.throws(() => scheduler.cancelAction(actionId), /Cannot cancel completed/);
  });
});

// ===========================================================================
// 7. Failed executor marks action as failed
// ===========================================================================

describe('scheduler — failed executor', () => {
  it('marks action as failed when executor throws', async () => {
    const scheduler = createSchedulerService({
      executor: async () => {
        throw new Error('Insufficient funds');
      },
    });

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: { amount: 99999 },
      executeAt: pastDate(),
    });

    const result = await scheduler.processDueActions();
    assert.equal(result.executed, 0);
    assert.equal(result.failed, 1);

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'failed');
    assert.equal(action.lastError, 'Insufficient funds');
    assert.equal(action.executionCount, 1);
    assert.ok(action.lastExecutionId);
  });

  it('does not reschedule a recurring action that fails', async () => {
    const scheduler = createSchedulerService({
      executor: async () => {
        throw new Error('Network error');
      },
    });

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'escrow_check',
      payload: {},
      executeAt: pastDate(),
      repeatInterval: 60000,
      maxExecutions: 5,
    });

    await scheduler.processDueActions();

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'failed');
    // Failed — does not return to pending
  });
});

// ===========================================================================
// 8. getAction returns correct action details
// ===========================================================================

describe('scheduler — getAction', () => {
  it('returns action details for a valid actionId', () => {
    const scheduler = createSchedulerService();

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'reminder',
      payload: { msg: 'Check your escrow' },
      executeAt: futureDate(),
      description: 'Escrow reminder',
    });

    const action = scheduler.getAction(actionId);
    assert.equal(action.id, actionId);
    assert.equal(action.agentAddress, '0xAlice');
    assert.equal(action.actionType, 'reminder');
    assert.deepEqual(action.payload, { msg: 'Check your escrow' });
    assert.equal(action.description, 'Escrow reminder');
  });

  it('returns null for unknown actionId', () => {
    const scheduler = createSchedulerService();
    assert.equal(scheduler.getAction('non-existent'), null);
  });

  it('returns a copy, not a reference', () => {
    const scheduler = createSchedulerService();

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'custom',
      payload: { x: 1 },
      executeAt: futureDate(),
    });

    const a1 = scheduler.getAction(actionId);
    const a2 = scheduler.getAction(actionId);
    assert.notEqual(a1, a2);
    assert.deepEqual(a1, a2);
  });
});

// ===========================================================================
// 9. listActions filters by agentAddress, status, and actionType
// ===========================================================================

describe('scheduler — listActions', () => {
  let scheduler;

  beforeEach(() => {
    scheduler = createSchedulerService();

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'escrow_check',
      payload: {},
      executeAt: futureDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xBob',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });
  });

  it('lists all actions without filter', () => {
    const actions = scheduler.listActions();
    assert.equal(actions.length, 3);
  });

  it('filters by agentAddress', () => {
    const aliceActions = scheduler.listActions({ agentAddress: '0xAlice' });
    assert.equal(aliceActions.length, 2);
    assert.ok(aliceActions.every((a) => a.agentAddress === '0xAlice'));
  });

  it('filters by actionType', () => {
    const payments = scheduler.listActions({ actionType: 'payment' });
    assert.equal(payments.length, 2);
    assert.ok(payments.every((a) => a.actionType === 'payment'));
  });

  it('filters by status', async () => {
    // Cancel one to create a different status
    const all = scheduler.listActions();
    scheduler.cancelAction(all[0].id);

    const pending = scheduler.listActions({ status: 'pending' });
    assert.equal(pending.length, 2);

    const cancelled = scheduler.listActions({ status: 'cancelled' });
    assert.equal(cancelled.length, 1);
  });

  it('combines multiple filters', () => {
    const result = scheduler.listActions({
      agentAddress: '0xAlice',
      actionType: 'payment',
    });
    assert.equal(result.length, 1);
    assert.equal(result[0].agentAddress, '0xAlice');
    assert.equal(result[0].actionType, 'payment');
  });

  it('returns empty array for no matches', () => {
    const result = scheduler.listActions({ agentAddress: '0xCharlie' });
    assert.equal(result.length, 0);
  });
});

// ===========================================================================
// 10. start/stop lifecycle
// ===========================================================================

describe('scheduler — start/stop', () => {
  let scheduler;

  afterEach(() => {
    if (scheduler) scheduler.stop();
  });

  it('start sets running to true', () => {
    scheduler = createSchedulerService();
    assert.equal(scheduler.getMetrics().running, false);

    scheduler.start(60000); // long interval so nothing fires
    assert.equal(scheduler.getMetrics().running, true);
  });

  it('stop sets running to false', () => {
    scheduler = createSchedulerService();
    scheduler.start(60000);
    assert.equal(scheduler.getMetrics().running, true);

    scheduler.stop();
    assert.equal(scheduler.getMetrics().running, false);
  });

  it('calling start twice is idempotent', () => {
    scheduler = createSchedulerService();
    scheduler.start(60000);
    scheduler.start(60000); // should not throw or create duplicate timers
    assert.equal(scheduler.getMetrics().running, true);
  });

  it('calling stop when not running is safe', () => {
    scheduler = createSchedulerService();
    scheduler.stop(); // should not throw
    assert.equal(scheduler.getMetrics().running, false);
  });

  it('processes due actions on interval', async () => {
    let execCount = 0;
    scheduler = createSchedulerService({
      executor: async () => {
        execCount++;
      },
    });

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    // Start with a very short interval
    scheduler.start(10);

    // Wait long enough for at least one tick
    await new Promise((resolve) => setTimeout(resolve, 50));

    scheduler.stop();

    assert.ok(execCount >= 1, `Expected at least 1 execution, got ${execCount}`);
  });
});

// ===========================================================================
// 11. getMetrics tracks counts correctly
// ===========================================================================

describe('scheduler — getMetrics', () => {
  it('returns correct initial metrics', () => {
    const scheduler = createSchedulerService();
    const metrics = scheduler.getMetrics();

    assert.equal(metrics.totalScheduled, 0);
    assert.equal(metrics.totalExecuted, 0);
    assert.equal(metrics.totalFailed, 0);
    assert.equal(metrics.pendingCount, 0);
    assert.equal(metrics.recurringCount, 0);
    assert.equal(metrics.running, false);
  });

  it('increments totalScheduled on each scheduleAction call', () => {
    const scheduler = createSchedulerService();

    scheduler.scheduleAction({
      agentAddress: '0xA',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xB',
      actionType: 'billing',
      payload: {},
      executeAt: futureDate(),
    });

    assert.equal(scheduler.getMetrics().totalScheduled, 2);
    assert.equal(scheduler.getMetrics().pendingCount, 2);
  });

  it('tracks executed and failed correctly', async () => {
    let shouldFail = false;
    const scheduler = createSchedulerService({
      executor: async () => {
        if (shouldFail) throw new Error('fail');
        return { ok: true };
      },
    });

    scheduler.scheduleAction({
      agentAddress: '0xA',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();
    assert.equal(scheduler.getMetrics().totalExecuted, 1);
    assert.equal(scheduler.getMetrics().totalFailed, 0);

    shouldFail = true;
    scheduler.scheduleAction({
      agentAddress: '0xB',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();
    assert.equal(scheduler.getMetrics().totalExecuted, 1);
    assert.equal(scheduler.getMetrics().totalFailed, 1);
  });

  it('pendingCount decreases after execution', async () => {
    const scheduler = createSchedulerService();

    scheduler.scheduleAction({
      agentAddress: '0xA',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xB',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });

    assert.equal(scheduler.getMetrics().pendingCount, 2);

    await scheduler.processDueActions();
    assert.equal(scheduler.getMetrics().pendingCount, 1);
  });

  it('recurringCount tracks active recurring actions', () => {
    const scheduler = createSchedulerService();

    scheduler.scheduleAction({
      agentAddress: '0xA',
      actionType: 'escrow_check',
      payload: {},
      executeAt: futureDate(),
      repeatInterval: 60000,
    });
    scheduler.scheduleAction({
      agentAddress: '0xB',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });

    assert.equal(scheduler.getMetrics().recurringCount, 1);
  });
});

// ===========================================================================
// 12. Events emitted at each state transition
// ===========================================================================

describe('scheduler — events', () => {
  it('emits action_scheduled on scheduleAction', () => {
    const scheduler = createSchedulerService();
    const events = collectEvents(scheduler, ['action_scheduled']);

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: { amount: 10 },
      executeAt: futureDate(),
    });

    assert.equal(events.length, 1);
    assert.equal(events[0].event, 'action_scheduled');
    assert.equal(events[0].data.agentAddress, '0xAlice');
    assert.equal(events[0].data.status, 'pending');
  });

  it('emits action_executing before execution', async () => {
    const events = [];
    const scheduler = createSchedulerService({
      executor: async (action) => {
        // At this point, action_executing should already have been emitted
        events.push({ event: 'in_executor', actionId: action.id });
        return { ok: true };
      },
    });

    scheduler.on('action_executing', (data) => {
      events.push({ event: 'action_executing', actionId: data.id });
    });

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();

    assert.equal(events[0].event, 'action_executing');
    assert.equal(events[1].event, 'in_executor');
  });

  it('emits action_completed after successful execution', async () => {
    const scheduler = createSchedulerService({
      executor: async () => ({ paid: true }),
    });
    const events = collectEvents(scheduler, [
      'action_executing',
      'action_completed',
    ]);

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();

    const completed = events.filter((e) => e.event === 'action_completed');
    assert.equal(completed.length, 1);
    assert.deepEqual(completed[0].data.result, { paid: true });
    assert.ok(completed[0].data.executionId);
  });

  it('emits action_failed when executor throws', async () => {
    const scheduler = createSchedulerService({
      executor: async () => {
        throw new Error('boom');
      },
    });
    const events = collectEvents(scheduler, ['action_failed']);

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();

    assert.equal(events.length, 1);
    assert.equal(events[0].event, 'action_failed');
    assert.equal(events[0].data.error, 'boom');
  });

  it('emits action_cancelled on cancelAction', () => {
    const scheduler = createSchedulerService();
    const events = collectEvents(scheduler, ['action_cancelled']);

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: futureDate(),
    });

    scheduler.cancelAction(actionId);

    assert.equal(events.length, 1);
    assert.equal(events[0].event, 'action_cancelled');
    assert.equal(events[0].data.status, 'cancelled');
  });

  it('emits action_completed for recurring action with executionId', async () => {
    const scheduler = createSchedulerService();
    const events = collectEvents(scheduler, ['action_completed']);

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'escrow_check',
      payload: {},
      executeAt: pastDate(),
      repeatInterval: 60000,
    });

    await scheduler.processDueActions();

    assert.equal(events.length, 1);
    assert.ok(events[0].data.executionId, 'completed event should include executionId');
    // For recurring, status returns to pending after completion event
    assert.equal(events[0].data.status, 'pending');
  });
});

// ===========================================================================
// 13. Idempotency — same action not executed twice
// ===========================================================================

describe('scheduler — idempotency', () => {
  it('completed action is not executed again', async () => {
    let execCount = 0;
    const scheduler = createSchedulerService({
      executor: async () => {
        execCount++;
      },
    });

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();
    assert.equal(execCount, 1);

    // Running again should not re-execute (action is now completed)
    await scheduler.processDueActions();
    assert.equal(execCount, 1);
  });

  it('failed action is not retried automatically', async () => {
    let execCount = 0;
    const scheduler = createSchedulerService({
      executor: async () => {
        execCount++;
        throw new Error('fail');
      },
    });

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();
    assert.equal(execCount, 1);

    await scheduler.processDueActions();
    assert.equal(execCount, 1); // still 1, not retried
  });

  it('cancelled action is never executed', async () => {
    let execCount = 0;
    const scheduler = createSchedulerService({
      executor: async () => {
        execCount++;
      },
    });

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    scheduler.cancelAction(actionId);

    await scheduler.processDueActions();
    await scheduler.processDueActions();
    assert.equal(execCount, 0);
  });

  it('each execution gets a unique executionId', async () => {
    const executionIds = [];
    const scheduler = createSchedulerService({
      executor: async () => {
        // no-op
      },
    });

    scheduler.on('action_completed', (data) => {
      executionIds.push(data.executionId);
    });

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'sla_check',
      payload: {},
      executeAt: pastDate(200000),
      repeatInterval: 1,
      maxExecutions: 3,
    });

    await scheduler.processDueActions();
    await scheduler.processDueActions();
    await scheduler.processDueActions();

    assert.equal(executionIds.length, 3);
    // All unique
    const unique = new Set(executionIds);
    assert.equal(unique.size, 3, 'Each execution should have a unique executionId');
  });
});

// ===========================================================================
// 14. Multiple agents have independent schedules
// ===========================================================================

describe('scheduler — multi-agent independence', () => {
  it('actions for different agents are independent', async () => {
    const executedBy = [];
    const scheduler = createSchedulerService({
      executor: async (action) => {
        executedBy.push(action.agentAddress);
      },
    });

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: { amount: 10 },
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xBob',
      actionType: 'payment',
      payload: { amount: 20 },
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xCharlie',
      actionType: 'payment',
      payload: { amount: 30 },
      executeAt: futureDate(), // not yet due
    });

    await scheduler.processDueActions();

    assert.equal(executedBy.length, 2);
    assert.ok(executedBy.includes('0xAlice'));
    assert.ok(executedBy.includes('0xBob'));
    assert.ok(!executedBy.includes('0xCharlie'));
  });

  it('cancelling one agent action does not affect another', async () => {
    const executed = [];
    const scheduler = createSchedulerService({
      executor: async (action) => {
        executed.push(action.agentAddress);
      },
    });

    const { actionId: aliceId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xBob',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });

    scheduler.cancelAction(aliceId);

    await scheduler.processDueActions();

    assert.equal(executed.length, 1);
    assert.equal(executed[0], '0xBob');
  });

  it('listActions correctly filters per-agent', () => {
    const scheduler = createSchedulerService();

    for (let i = 0; i < 5; i++) {
      scheduler.scheduleAction({
        agentAddress: '0xAlice',
        actionType: 'payment',
        payload: { i },
        executeAt: futureDate(),
      });
    }
    for (let i = 0; i < 3; i++) {
      scheduler.scheduleAction({
        agentAddress: '0xBob',
        actionType: 'escrow_check',
        payload: { i },
        executeAt: futureDate(),
      });
    }

    assert.equal(scheduler.listActions({ agentAddress: '0xAlice' }).length, 5);
    assert.equal(scheduler.listActions({ agentAddress: '0xBob' }).length, 3);
    assert.equal(scheduler.listActions().length, 8);
  });

  it('metrics reflect all agents combined', async () => {
    const scheduler = createSchedulerService();

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'payment',
      payload: {},
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xBob',
      actionType: 'billing',
      payload: {},
      executeAt: pastDate(),
    });
    scheduler.scheduleAction({
      agentAddress: '0xCharlie',
      actionType: 'reminder',
      payload: {},
      executeAt: futureDate(),
    });

    assert.equal(scheduler.getMetrics().totalScheduled, 3);
    assert.equal(scheduler.getMetrics().pendingCount, 3);

    await scheduler.processDueActions();

    assert.equal(scheduler.getMetrics().totalExecuted, 2);
    assert.equal(scheduler.getMetrics().pendingCount, 1);
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('scheduler — edge cases', () => {
  it('default executor returns the action payload', async () => {
    const scheduler = createSchedulerService(); // no custom executor

    scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'custom',
      payload: { key: 'value' },
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();

    const actions = scheduler.listActions({ status: 'completed' });
    assert.equal(actions.length, 1);
    assert.deepEqual(actions[0].lastResult, { key: 'value' });
  });

  it('handles null payload gracefully', async () => {
    const scheduler = createSchedulerService();

    const { actionId } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'reminder',
      payload: null,
      executeAt: pastDate(),
    });

    await scheduler.processDueActions();

    const action = scheduler.getAction(actionId);
    assert.equal(action.status, 'completed');
  });

  it('handles undefined payload', () => {
    const scheduler = createSchedulerService();

    const { action } = scheduler.scheduleAction({
      agentAddress: '0xAlice',
      actionType: 'reminder',
      executeAt: futureDate(),
    });

    assert.equal(action.payload, null);
  });

  it('processDueActions returns zeros when no actions exist', async () => {
    const scheduler = createSchedulerService();
    const result = await scheduler.processDueActions();

    assert.equal(result.executed, 0);
    assert.equal(result.failed, 0);
    assert.equal(result.skipped, 0);
  });
});
