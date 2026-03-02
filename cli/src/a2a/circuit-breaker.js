/**
 * Circuit Breaker — Agent safety system for spending limits and failure detection
 *
 * Implements a per-agent circuit breaker with three states:
 *   closed   → normal operation, transactions allowed
 *   open     → tripped, all transactions blocked
 *   half_open → testing recovery, limited transactions allowed
 *
 * Also enforces per-transaction, daily, and monthly spending limits using a
 * rolling-window ledger, plus a global kill switch for emergency shutdown.
 *
 * @example
 * ```javascript
 * import { createCircuitBreaker } from './circuit-breaker.js';
 *
 * const cb = createCircuitBreaker(store);
 * const check = cb.checkTransaction('agent-1', 500);
 * if (check.allowed) {
 *   cb.recordSuccess('agent-1', 500);
 * } else {
 *   console.warn(`Blocked: ${check.reason}`);
 * }
 * ```
 */

import { randomUUID } from 'node:crypto';

// ---------------------------------------------------------------------------
// Default configuration
// ---------------------------------------------------------------------------

const DEFAULT_CONFIG = {
  maxSpendPerTx: 1000,
  dailySpendLimit: 10000,
  monthlySpendLimit: 100000,
  maxFailureRate: 0.3,
  failureWindowMs: 300000, // 5 minutes
  cooldownMs: 60000, // 1 minute
  halfOpenMaxTxns: 3,
  globalKillSwitch: false,
};

// ---------------------------------------------------------------------------
// SQL schema for circuit breaker tables
// ---------------------------------------------------------------------------

const CB_SCHEMA = `
CREATE TABLE IF NOT EXISTS a2a_circuit_breaker_events (
  id TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  event_type TEXT NOT NULL,
  reason TEXT,
  amount REAL,
  state_before TEXT,
  state_after TEXT,
  metadata TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_cb_events_agent ON a2a_circuit_breaker_events(agent_name);
CREATE INDEX IF NOT EXISTS idx_cb_events_type ON a2a_circuit_breaker_events(event_type);

CREATE TABLE IF NOT EXISTS a2a_spending_ledger (
  id TEXT PRIMARY KEY,
  agent_name TEXT NOT NULL,
  amount REAL NOT NULL,
  success INTEGER NOT NULL DEFAULT 1,
  error TEXT,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_spending_agent ON a2a_spending_ledger(agent_name);
CREATE INDEX IF NOT EXISTS idx_spending_created ON a2a_spending_ledger(created_at);
`;

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create a circuit breaker service for agent safety.
 *
 * @param {Object} store - A2A store instance (must have `.db` property)
 * @param {Object} [configOverrides={}] - Configuration overrides
 * @returns {Object} Circuit breaker service
 */
export function createCircuitBreaker(store, configOverrides = {}) {
  if (!store || !store.db) {
    throw new Error('store with .db property is required');
  }

  const db = store.db;
  const config = { ...DEFAULT_CONFIG, ...configOverrides };

  // Create tables
  db.exec(CB_SCHEMA);

  // Per-agent state map
  const agentStates = new Map();

  // -------------------------------------------------------------------------
  // Internal helpers
  // -------------------------------------------------------------------------

  function _getOrCreateState(agentName) {
    if (!agentStates.has(agentName)) {
      agentStates.set(agentName, {
        state: 'closed',
        trippedAt: null,
        reason: null,
        halfOpenCount: 0,
        lastFailureCheck: 0,
      });
    }
    return agentStates.get(agentName);
  }

  function _logEvent(
    agentName,
    eventType,
    { reason, amount, stateBefore, stateAfter, metadata } = {},
  ) {
    const stmt = db.prepare(
      `INSERT INTO a2a_circuit_breaker_events (id, agent_name, event_type, reason, amount, state_before, state_after, metadata, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))`,
    );
    stmt.run(
      randomUUID(),
      agentName,
      eventType,
      reason ?? null,
      amount ?? null,
      stateBefore ?? null,
      stateAfter ?? null,
      metadata ? JSON.stringify(metadata) : null,
    );
  }

  function _getDailySpend(agentName) {
    const row = db
      .prepare(
        `SELECT COALESCE(SUM(amount), 0) AS total
       FROM a2a_spending_ledger
       WHERE agent_name = ? AND success = 1 AND created_at >= datetime('now', '-1 day')`,
      )
      .get(agentName);
    return row ? row.total : 0;
  }

  function _getMonthlySpend(agentName) {
    const row = db
      .prepare(
        `SELECT COALESCE(SUM(amount), 0) AS total
       FROM a2a_spending_ledger
       WHERE agent_name = ? AND success = 1 AND created_at >= datetime('now', '-30 day')`,
      )
      .get(agentName);
    return row ? row.total : 0;
  }

  function _getFailureRate(agentName) {
    const windowSeconds = Math.floor(config.failureWindowMs / 1000);
    const row = db
      .prepare(
        `SELECT
         COUNT(*) AS total,
         SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) AS failures
       FROM a2a_spending_ledger
       WHERE agent_name = ? AND created_at >= datetime('now', '-' || ? || ' seconds')`,
      )
      .get(agentName, windowSeconds);
    if (!row || row.total === 0) return 0;
    return row.failures / row.total;
  }

  function _recordLedger(agentName, amount, success, error) {
    const stmt = db.prepare(
      `INSERT INTO a2a_spending_ledger (id, agent_name, amount, success, error, created_at)
       VALUES (?, ?, ?, ?, ?, datetime('now'))`,
    );
    stmt.run(randomUUID(), agentName, amount, success ? 1 : 0, error ?? null);
  }

  function _shouldTransitionToHalfOpen(agentState) {
    if (agentState.state !== 'open' || !agentState.trippedAt) return false;
    const elapsed = Date.now() - agentState.trippedAt;
    return elapsed >= config.cooldownMs;
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  /**
   * Check whether a transaction should be allowed.
   *
   * @param {string} agentName - Agent name
   * @param {number} amount - Transaction amount
   * @returns {{ allowed: boolean, reason?: string, state: string }}
   */
  function checkTransaction(agentName, amount) {
    if (!agentName || typeof agentName !== 'string' || agentName.trim() === '') {
      return { allowed: false, reason: 'Agent name is required', state: 'unknown' };
    }
    if (typeof amount !== 'number' || !Number.isFinite(amount) || amount < 0) {
      return { allowed: false, reason: 'Amount must be a non-negative number', state: 'unknown' };
    }

    const agentState = _getOrCreateState(agentName);

    // Global kill switch
    if (config.globalKillSwitch) {
      return { allowed: false, reason: 'Global kill switch is active', state: agentState.state };
    }

    // Check circuit state
    if (agentState.state === 'open') {
      if (_shouldTransitionToHalfOpen(agentState)) {
        const before = agentState.state;
        agentState.state = 'half_open';
        agentState.halfOpenCount = 0;
        _logEvent(agentName, 'state_change', {
          reason: 'Cooldown elapsed, entering half-open',
          stateBefore: before,
          stateAfter: 'half_open',
        });
      } else {
        return {
          allowed: false,
          reason: `Circuit is open: ${agentState.reason || 'tripped'}`,
          state: 'open',
        };
      }
    }

    // Per-transaction limit
    if (amount > config.maxSpendPerTx) {
      return {
        allowed: false,
        reason: `Amount ${amount} exceeds per-transaction limit of ${config.maxSpendPerTx}`,
        state: agentState.state,
      };
    }

    // Daily spending limit
    const dailySpend = _getDailySpend(agentName);
    if (dailySpend + amount > config.dailySpendLimit) {
      return {
        allowed: false,
        reason: `Daily spend ${dailySpend + amount} would exceed limit of ${config.dailySpendLimit}`,
        state: agentState.state,
      };
    }

    // Monthly spending limit
    const monthlySpend = _getMonthlySpend(agentName);
    if (monthlySpend + amount > config.monthlySpendLimit) {
      return {
        allowed: false,
        reason: `Monthly spend ${monthlySpend + amount} would exceed limit of ${config.monthlySpendLimit}`,
        state: agentState.state,
      };
    }

    // Failure rate check
    const failureRate = _getFailureRate(agentName);
    if (failureRate > config.maxFailureRate) {
      return {
        allowed: false,
        reason: `Failure rate ${(failureRate * 100).toFixed(1)}% exceeds maximum of ${(config.maxFailureRate * 100).toFixed(1)}%`,
        state: agentState.state,
      };
    }

    return { allowed: true, state: agentState.state };
  }

  /**
   * Record a successful transaction.
   *
   * @param {string} agentName - Agent name
   * @param {number} amount - Transaction amount
   */
  function recordSuccess(agentName, amount) {
    if (!agentName || typeof agentName !== 'string') return;
    _recordLedger(agentName, amount, true, null);

    const agentState = _getOrCreateState(agentName);

    _logEvent(agentName, 'transaction_success', {
      amount,
      stateBefore: agentState.state,
      stateAfter: agentState.state,
    });

    // Half-open: count test transactions and close if threshold met
    if (agentState.state === 'half_open') {
      agentState.halfOpenCount += 1;
      if (agentState.halfOpenCount >= config.halfOpenMaxTxns) {
        const before = agentState.state;
        agentState.state = 'closed';
        agentState.trippedAt = null;
        agentState.reason = null;
        agentState.halfOpenCount = 0;
        _logEvent(agentName, 'state_change', {
          reason: `Half-open threshold met (${config.halfOpenMaxTxns} successes)`,
          stateBefore: before,
          stateAfter: 'closed',
        });
      }
    }
  }

  /**
   * Record a failed transaction. Trips the circuit if failure rate exceeds threshold.
   *
   * @param {string} agentName - Agent name
   * @param {number} amount - Transaction amount
   * @param {string} [error] - Error message
   */
  function recordFailure(agentName, amount, error) {
    if (!agentName || typeof agentName !== 'string') return;
    _recordLedger(agentName, amount, false, error);

    const agentState = _getOrCreateState(agentName);

    _logEvent(agentName, 'transaction_failure', {
      reason: error,
      amount,
      stateBefore: agentState.state,
      stateAfter: agentState.state,
    });

    // If half_open and a failure occurs, trip back to open
    if (agentState.state === 'half_open') {
      const before = agentState.state;
      agentState.state = 'open';
      agentState.trippedAt = Date.now();
      agentState.reason = error || 'Failure during half-open testing';
      agentState.halfOpenCount = 0;
      _logEvent(agentName, 'state_change', {
        reason: agentState.reason,
        stateBefore: before,
        stateAfter: 'open',
      });
      return;
    }

    // Check failure rate and trip if exceeded
    const failureRate = _getFailureRate(agentName);
    if (failureRate > config.maxFailureRate && agentState.state === 'closed') {
      trip(agentName, `Failure rate ${(failureRate * 100).toFixed(1)}% exceeded threshold`);
    }
  }

  /**
   * Manually trip the circuit breaker for an agent.
   *
   * @param {string} agentName - Agent name
   * @param {string} reason - Reason for tripping
   */
  function trip(agentName, reason) {
    if (!agentName || typeof agentName !== 'string') return;
    const agentState = _getOrCreateState(agentName);
    const before = agentState.state;

    // No-op if already open
    if (before === 'open') return;

    agentState.state = 'open';
    agentState.trippedAt = Date.now();
    agentState.reason = reason || 'Manually tripped';
    agentState.halfOpenCount = 0;

    _logEvent(agentName, 'trip', {
      reason: agentState.reason,
      stateBefore: before,
      stateAfter: 'open',
    });
  }

  /**
   * Trip ALL agents and activate the global kill switch.
   *
   * @param {string} reason - Reason for global trip
   */
  function tripAll(reason) {
    config.globalKillSwitch = true;

    for (const [name, agentState] of agentStates.entries()) {
      const before = agentState.state;
      agentState.state = 'open';
      agentState.trippedAt = Date.now();
      agentState.reason = reason || 'Global kill switch activated';
      agentState.halfOpenCount = 0;

      _logEvent(name, 'trip_all', {
        reason: agentState.reason,
        stateBefore: before,
        stateAfter: 'open',
      });
    }

    // Log a global event
    _logEvent('__global__', 'kill_switch_activated', {
      reason: reason || 'Global kill switch activated',
      stateBefore: null,
      stateAfter: null,
    });
  }

  /**
   * Reset the circuit breaker for a single agent.
   *
   * @param {string} agentName - Agent name
   */
  function reset(agentName) {
    if (!agentName || typeof agentName !== 'string') return;
    const agentState = _getOrCreateState(agentName);
    const before = agentState.state;

    agentState.state = 'closed';
    agentState.trippedAt = null;
    agentState.reason = null;
    agentState.halfOpenCount = 0;

    _logEvent(agentName, 'reset', {
      reason: 'Manual reset',
      stateBefore: before,
      stateAfter: 'closed',
    });
  }

  /**
   * Reset ALL agents and deactivate the global kill switch.
   */
  function resetAll() {
    config.globalKillSwitch = false;

    for (const [name, agentState] of agentStates.entries()) {
      const before = agentState.state;
      agentState.state = 'closed';
      agentState.trippedAt = null;
      agentState.reason = null;
      agentState.halfOpenCount = 0;

      _logEvent(name, 'reset_all', {
        reason: 'Global reset',
        stateBefore: before,
        stateAfter: 'closed',
      });
    }

    _logEvent('__global__', 'kill_switch_deactivated', {
      reason: 'Global reset',
      stateBefore: null,
      stateAfter: null,
    });
  }

  /**
   * Get the current state of a single agent's circuit breaker.
   *
   * @param {string} agentName - Agent name
   * @returns {{ state: string, trippedAt: number|null, reason: string|null, halfOpenCount: number, config: Object }}
   */
  function getState(agentName) {
    const agentState = _getOrCreateState(agentName);

    // Check if open state should auto-transition to half_open
    if (agentState.state === 'open' && _shouldTransitionToHalfOpen(agentState)) {
      const before = agentState.state;
      agentState.state = 'half_open';
      agentState.halfOpenCount = 0;
      _logEvent(agentName, 'state_change', {
        reason: 'Cooldown elapsed, entering half-open',
        stateBefore: before,
        stateAfter: 'half_open',
      });
    }

    return {
      state: agentState.state,
      trippedAt: agentState.trippedAt,
      reason: agentState.reason,
      halfOpenCount: agentState.halfOpenCount,
      config: { ...config },
    };
  }

  /**
   * Get all known agent states.
   *
   * @returns {Array<{ agentName: string, state: string, trippedAt: number|null, reason: string|null, halfOpenCount: number }>}
   */
  function getAllStates() {
    const results = [];
    for (const [name, agentState] of agentStates.entries()) {
      results.push({
        agentName: name,
        state: agentState.state,
        trippedAt: agentState.trippedAt,
        reason: agentState.reason,
        halfOpenCount: agentState.halfOpenCount,
      });
    }
    return results;
  }

  /**
   * Get spending summary for an agent.
   *
   * @param {string} agentName - Agent name
   * @returns {{ today: number, thisMonth: number, remainingDaily: number, remainingMonthly: number }}
   */
  function getSpendingSummary(agentName) {
    const today = _getDailySpend(agentName);
    const thisMonth = _getMonthlySpend(agentName);
    return {
      today,
      thisMonth,
      remainingDaily: Math.max(0, config.dailySpendLimit - today),
      remainingMonthly: Math.max(0, config.monthlySpendLimit - thisMonth),
    };
  }

  /**
   * Update circuit breaker configuration.
   *
   * @param {Object} overrides - Configuration keys to update
   */
  function updateConfig(overrides) {
    if (!overrides || typeof overrides !== 'object') return;
    for (const [key, value] of Object.entries(overrides)) {
      if (Object.prototype.hasOwnProperty.call(DEFAULT_CONFIG, key)) {
        config[key] = value;
      }
    }
  }

  return {
    checkTransaction,
    recordSuccess,
    recordFailure,
    trip,
    tripAll,
    reset,
    resetAll,
    getState,
    getAllStates,
    getSpendingSummary,
    updateConfig,
  };
}
