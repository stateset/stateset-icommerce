/**
 * A2A Scheduled Actions — Autonomous Future Action Scheduling
 *
 * Enables agents to schedule future actions: deferred payments, periodic escrow
 * checks, recurring billing, SLA enforcement, and arbitrary custom actions.
 *
 * In-memory storage (Map), EventEmitter lifecycle events, idempotent execution,
 * and configurable executor function.
 *
 * @example
 * ```javascript
 * const scheduler = createSchedulerService({
 *   executor: async (action) => {
 *     if (action.actionType === 'payment') return a2a.pay(action.payload);
 *     if (action.actionType === 'escrow_check') return a2a.checkPaymentConditions(action.payload.escrowId);
 *   },
 * });
 *
 * // Schedule a payment in 3 days
 * const { actionId } = scheduler.scheduleAction({
 *   agentAddress: '0xAlice',
 *   actionType: 'payment',
 *   payload: { to: '0xBob', amount: 50, asset: 'USDC' },
 *   executeAt: new Date(Date.now() + 3 * 86400000).toISOString(),
 *   description: 'Invoice payment NET-30',
 * });
 *
 * // Recurring escrow check every hour
 * scheduler.scheduleAction({
 *   agentAddress: '0xAlice',
 *   actionType: 'escrow_check',
 *   payload: { escrowId: 'esc-001' },
 *   executeAt: new Date().toISOString(),
 *   repeatInterval: 3600000,
 *   maxExecutions: 24,
 *   description: 'Hourly escrow condition check',
 * });
 *
 * scheduler.start();
 * // ...later
 * scheduler.stop();
 * ```
 */

import { randomUUID } from 'node:crypto';
import { EventEmitter } from 'node:events';

/**
 * Valid action types for scheduled actions.
 * @type {ReadonlySet<string>}
 */
const VALID_ACTION_TYPES = new Set([
  'payment',
  'quote_request',
  'escrow_check',
  'status_check',
  'custom',
  'reminder',
  'billing',
  'sla_check',
]);

/**
 * Valid action states.
 * @type {ReadonlySet<string>}
 */
const VALID_STATUSES = new Set(['pending', 'executing', 'completed', 'failed', 'cancelled']);

/**
 * Default no-op executor — returns the action payload.
 *
 * @param {Object} action - The scheduled action
 * @returns {Object} The action payload (pass-through)
 */
function defaultExecutor(action) {
  return action.payload;
}

/**
 * Create a scheduler service instance.
 *
 * @param {Object} [options]
 * @param {Function} [options.executor] - Async function called for each due action.
 *   Receives the full action object, returns a result. Default: no-op returning payload.
 * @returns {Object} Scheduler service API
 */
export function createSchedulerService(options = {}) {
  const executor = options.executor || defaultExecutor;
  const emitter = new EventEmitter();

  /** @type {Map<string, Object>} actionId -> action */
  const _actions = new Map();

  /** @type {Set<string>} executionIds currently in flight — prevents double execution */
  const _inflightExecutions = new Set();

  let _timer = null;
  let _running = false;

  // Metrics counters
  const _metrics = {
    totalScheduled: 0,
    totalExecuted: 0,
    totalFailed: 0,
  };

  // -------------------------------------------------------------------------
  // scheduleAction
  // -------------------------------------------------------------------------

  /**
   * Schedule a future action.
   *
   * @param {Object} params
   * @param {string} params.agentAddress - Agent wallet address or identifier
   * @param {string} params.actionType - One of: payment, quote_request, escrow_check,
   *   status_check, custom, reminder, billing, sla_check
   * @param {Object} params.payload - Arbitrary data passed to the executor
   * @param {string} params.executeAt - ISO 8601 date string for first execution
   * @param {number} [params.repeatInterval] - Repeat interval in ms (recurring action)
   * @param {number} [params.maxExecutions] - Max number of executions for recurring actions
   * @param {string} [params.description] - Human-readable description
   * @returns {{ actionId: string, action: Object }}
   */
  function scheduleAction(params) {
    const {
      agentAddress,
      actionType,
      payload,
      executeAt,
      repeatInterval,
      maxExecutions,
      description,
    } = params;

    // Validation
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }
    if (!actionType || !VALID_ACTION_TYPES.has(actionType)) {
      throw new Error(
        `Invalid actionType: ${actionType}. Must be one of: ${[...VALID_ACTION_TYPES].join(', ')}`,
      );
    }
    if (!executeAt) {
      throw new Error('executeAt is required');
    }
    const executeAtDate = new Date(executeAt);
    if (isNaN(executeAtDate.getTime())) {
      throw new Error('executeAt must be a valid ISO 8601 date string');
    }
    if (repeatInterval !== undefined && repeatInterval !== null) {
      if (typeof repeatInterval !== 'number' || repeatInterval <= 0) {
        throw new Error('repeatInterval must be a positive number (ms)');
      }
    }
    if (maxExecutions !== undefined && maxExecutions !== null) {
      if (typeof maxExecutions !== 'number' || maxExecutions < 1) {
        throw new Error('maxExecutions must be a positive integer');
      }
    }

    const now = new Date().toISOString();
    const actionId = randomUUID();

    const action = {
      id: actionId,
      agentAddress,
      actionType,
      payload: payload !== undefined ? payload : null,
      description: description || null,
      status: 'pending',
      executeAt: executeAtDate.toISOString(),
      repeatInterval: repeatInterval || null,
      maxExecutions: maxExecutions || null,
      executionCount: 0,
      lastExecutionId: null,
      lastExecutedAt: null,
      lastResult: null,
      lastError: null,
      createdAt: now,
      updatedAt: now,
    };

    _actions.set(actionId, action);
    _metrics.totalScheduled++;

    emitter.emit('action_scheduled', { ...action });

    return { actionId, action: { ...action } };
  }

  // -------------------------------------------------------------------------
  // cancelAction
  // -------------------------------------------------------------------------

  /**
   * Cancel a scheduled action. Only pending actions can be cancelled.
   *
   * @param {string} actionId
   * @returns {{ success: boolean, action: Object }}
   */
  function cancelAction(actionId) {
    const action = _actions.get(actionId);
    if (!action) {
      throw new Error(`Action not found: ${actionId}`);
    }
    if (action.status === 'cancelled') {
      throw new Error(`Action already cancelled: ${actionId}`);
    }
    if (action.status === 'executing') {
      throw new Error(`Cannot cancel action while executing: ${actionId}`);
    }
    if (action.status === 'completed') {
      throw new Error(`Cannot cancel completed action: ${actionId}`);
    }

    action.status = 'cancelled';
    action.updatedAt = new Date().toISOString();

    emitter.emit('action_cancelled', { ...action });

    return { success: true, action: { ...action } };
  }

  // -------------------------------------------------------------------------
  // getAction
  // -------------------------------------------------------------------------

  /**
   * Get action details.
   *
   * @param {string} actionId
   * @returns {Object|null} Action details or null if not found
   */
  function getAction(actionId) {
    const action = _actions.get(actionId);
    return action ? { ...action } : null;
  }

  // -------------------------------------------------------------------------
  // listActions
  // -------------------------------------------------------------------------

  /**
   * List scheduled actions with optional filters.
   *
   * @param {Object} [filter]
   * @param {string} [filter.agentAddress] - Filter by agent address
   * @param {string} [filter.status] - Filter by status
   * @param {string} [filter.actionType] - Filter by action type
   * @returns {Array<Object>} Matching actions
   */
  function listActions(filter = {}) {
    let results = [..._actions.values()];

    if (filter.agentAddress) {
      results = results.filter((a) => a.agentAddress === filter.agentAddress);
    }
    if (filter.status) {
      results = results.filter((a) => a.status === filter.status);
    }
    if (filter.actionType) {
      results = results.filter((a) => a.actionType === filter.actionType);
    }

    return results.map((a) => ({ ...a }));
  }

  // -------------------------------------------------------------------------
  // processDueActions
  // -------------------------------------------------------------------------

  /**
   * Execute all actions whose executeAt <= now.
   * For recurring actions, auto-reschedules the next execution.
   *
   * @returns {Promise<{ executed: number, failed: number, skipped: number }>}
   */
  async function processDueActions() {
    const now = new Date();
    let executed = 0;
    let failed = 0;
    let skipped = 0;

    // Collect due actions (snapshot to avoid mutation during iteration)
    const dueActions = [];
    for (const action of _actions.values()) {
      if (action.status !== 'pending') continue;
      if (new Date(action.executeAt) > now) continue;
      dueActions.push(action);
    }

    for (const action of dueActions) {
      // Generate unique execution ID for idempotency
      const executionId = randomUUID();

      // Guard: check status again (may have been cancelled concurrently)
      if (action.status !== 'pending') {
        skipped++;
        continue;
      }

      // Idempotency: skip if this action already has an in-flight execution
      if (_inflightExecutions.has(action.id)) {
        skipped++;
        continue;
      }

      _inflightExecutions.add(action.id);

      // Transition to executing
      action.status = 'executing';
      action.updatedAt = new Date().toISOString();
      emitter.emit('action_executing', { ...action, executionId });

      try {
        const result = await executor(action);

        action.executionCount++;
        action.lastExecutionId = executionId;
        action.lastExecutedAt = new Date().toISOString();
        action.lastResult = result !== undefined ? result : null;
        action.lastError = null;
        action.updatedAt = new Date().toISOString();

        // Determine if this is a recurring action that should reschedule
        const shouldReschedule =
          action.repeatInterval &&
          (action.maxExecutions === null || action.executionCount < action.maxExecutions);

        if (shouldReschedule) {
          // Reschedule: move executeAt forward and return to pending
          const nextExecuteAt = new Date(
            new Date(action.executeAt).getTime() + action.repeatInterval,
          );
          action.executeAt = nextExecuteAt.toISOString();
          action.status = 'pending';
        } else {
          action.status = 'completed';
        }

        _metrics.totalExecuted++;
        executed++;

        emitter.emit('action_completed', {
          ...action,
          executionId,
          result: result !== undefined ? result : null,
        });
      } catch (err) {
        action.executionCount++;
        action.lastExecutionId = executionId;
        action.lastExecutedAt = new Date().toISOString();
        action.lastError = err.message || String(err);
        action.lastResult = null;
        action.status = 'failed';
        action.updatedAt = new Date().toISOString();

        _metrics.totalFailed++;
        failed++;

        emitter.emit('action_failed', {
          ...action,
          executionId,
          error: err.message || String(err),
        });
      } finally {
        _inflightExecutions.delete(action.id);
      }
    }

    return { executed, failed, skipped };
  }

  // -------------------------------------------------------------------------
  // start / stop
  // -------------------------------------------------------------------------

  /**
   * Start the scheduler polling loop.
   *
   * @param {number} [intervalMs=10000] - Polling interval in ms (default 10s)
   */
  function start(intervalMs = 10_000) {
    if (_running) return;
    _running = true;

    _timer = setInterval(() => {
      processDueActions().catch((err) => {
        emitter.emit('error', err);
      });
    }, intervalMs);

    // Allow process to exit even if scheduler is running
    if (_timer.unref) _timer.unref();
  }

  /**
   * Stop the scheduler polling loop.
   */
  function stop() {
    if (!_running) return;
    _running = false;

    if (_timer) {
      clearInterval(_timer);
      _timer = null;
    }
  }

  // -------------------------------------------------------------------------
  // getMetrics
  // -------------------------------------------------------------------------

  /**
   * Get scheduler metrics.
   *
   * @returns {{
   *   totalScheduled: number,
   *   totalExecuted: number,
   *   totalFailed: number,
   *   pendingCount: number,
   *   recurringCount: number,
   *   running: boolean,
   * }}
   */
  function getMetrics() {
    let pendingCount = 0;
    let recurringCount = 0;

    for (const action of _actions.values()) {
      if (action.status === 'pending') pendingCount++;
      if (action.repeatInterval && (action.status === 'pending' || action.status === 'executing')) {
        recurringCount++;
      }
    }

    return {
      totalScheduled: _metrics.totalScheduled,
      totalExecuted: _metrics.totalExecuted,
      totalFailed: _metrics.totalFailed,
      pendingCount,
      recurringCount,
      running: _running,
    };
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  return {
    scheduleAction,
    cancelAction,
    getAction,
    listActions,
    processDueActions,
    start,
    stop,
    getMetrics,

    // EventEmitter delegate
    on: emitter.on.bind(emitter),
    off: emitter.off.bind(emitter),
    once: emitter.once.bind(emitter),
  };
}

export default { createSchedulerService };
