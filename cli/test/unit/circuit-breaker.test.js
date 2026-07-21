/**
 * Unit tests for Circuit Breaker — Agent safety system
 *
 * Tests cli/src/a2a/circuit-breaker.js:
 *   - createCircuitBreaker() construction and validation
 *   - checkTransaction() — kill switch, per-tx, daily, monthly, failure rate, circuit state
 *   - recordSuccess() — ledger, half_open→closed transition
 *   - recordFailure() — ledger, trip on threshold, half_open→open
 *   - trip/reset — manual trip, tripAll, reset, resetAll, audit events
 *   - getSpendingSummary() — daily/monthly spend, remaining limits
 *   - State machine transitions — closed→open→half_open→closed full cycle
 *   - Edge cases — zero amounts, concurrent agents, empty names, config updates
 *   - Kill switch — global blocking, interaction with individual states
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import { createCircuitBreaker } from '../../src/a2a/circuit-breaker.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeStore() {
  const db = new Database(':memory:');
  db.pragma('journal_mode = WAL');
  return { db };
}

function countEvents(db, agentName) {
  const row = db
    .prepare('SELECT COUNT(*) AS cnt FROM a2a_circuit_breaker_events WHERE agent_name = ?')
    .get(agentName);
  return row.cnt;
}

function countLedger(db, agentName) {
  const row = db
    .prepare('SELECT COUNT(*) AS cnt FROM a2a_spending_ledger WHERE agent_name = ?')
    .get(agentName);
  return row.cnt;
}

function getEvents(db, agentName) {
  return db
    .prepare('SELECT * FROM a2a_circuit_breaker_events WHERE agent_name = ? ORDER BY created_at')
    .all(agentName);
}

function getLedgerEntries(db, agentName) {
  return db
    .prepare('SELECT * FROM a2a_spending_ledger WHERE agent_name = ? ORDER BY created_at')
    .all(agentName);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Circuit Breaker', () => {
  let store, cb;

  beforeEach(() => {
    store = makeStore();
    cb = createCircuitBreaker(store, { cooldownMs: 100, failureWindowMs: 60000 });
  });

  // =========================================================================
  // 1. Basic construction and defaults
  // =========================================================================
  describe('construction', () => {
    it('should create a circuit breaker with default config', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s);
      const state = breaker.getState('test-agent');
      assert.equal(state.config.maxSpendPerTx, 1000);
      assert.equal(state.config.dailySpendLimit, 10000);
      assert.equal(state.config.monthlySpendLimit, 100000);
      assert.equal(state.config.halfOpenMaxTxns, 3);
    });

    it('should throw when store is null', () => {
      assert.throws(() => createCircuitBreaker(null), /store with .db property is required/);
    });

    it('should throw when store has no db', () => {
      assert.throws(() => createCircuitBreaker({}), /store with .db property is required/);
    });

    it('should accept config overrides', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { maxSpendPerTx: 500, dailySpendLimit: 2000 });
      const state = breaker.getState('x');
      assert.equal(state.config.maxSpendPerTx, 500);
      assert.equal(state.config.dailySpendLimit, 2000);
      // Non-overridden defaults remain
      assert.equal(state.config.monthlySpendLimit, 100000);
    });

    it('should create SQLite tables on construction', () => {
      const tables = store.db
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'a2a_%'")
        .all()
        .map((r) => r.name);
      assert.ok(tables.includes('a2a_circuit_breaker_events'));
      assert.ok(tables.includes('a2a_spending_ledger'));
    });
  });

  // =========================================================================
  // 2. Initial state
  // =========================================================================
  describe('initial state', () => {
    it('should default to closed state', () => {
      const state = cb.getState('agent-1');
      assert.equal(state.state, 'closed');
    });

    it('should have null trippedAt for new agent', () => {
      const state = cb.getState('agent-1');
      assert.equal(state.trippedAt, null);
    });

    it('should have null reason for new agent', () => {
      const state = cb.getState('agent-1');
      assert.equal(state.reason, null);
    });

    it('should have zero halfOpenCount for new agent', () => {
      const state = cb.getState('agent-1');
      assert.equal(state.halfOpenCount, 0);
    });

    it('should track multiple agents independently', () => {
      cb.trip('agent-1', 'test');
      const s1 = cb.getState('agent-1');
      const s2 = cb.getState('agent-2');
      assert.equal(s1.state, 'open');
      assert.equal(s2.state, 'closed');
    });
  });

  // =========================================================================
  // 3. checkTransaction
  // =========================================================================
  describe('checkTransaction', () => {
    it('should allow a transaction under all limits', () => {
      const result = cb.checkTransaction('agent-1', 100);
      assert.equal(result.allowed, true);
      assert.equal(result.state, 'closed');
    });

    it('should block transaction exceeding per-tx limit', () => {
      const result = cb.checkTransaction('agent-1', 1500);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /per-transaction limit/);
    });

    it('should block when per-tx limit exactly matched', () => {
      // Default maxSpendPerTx = 1000
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { maxSpendPerTx: 100 });
      const result = breaker.checkTransaction('a', 101);
      assert.equal(result.allowed, false);
    });

    it('should allow per-tx amount exactly equal to limit', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { maxSpendPerTx: 100 });
      const result = breaker.checkTransaction('a', 100);
      assert.equal(result.allowed, true);
    });

    it('should block when daily spend would be exceeded', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { maxSpendPerTx: 10000, dailySpendLimit: 500 });
      // Record some spend first
      breaker.recordSuccess('agent-1', 400);
      const result = breaker.checkTransaction('agent-1', 200);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /[Dd]aily spend/);
    });

    it('should block when monthly spend would be exceeded', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, {
        maxSpendPerTx: 100000,
        dailySpendLimit: 100000,
        monthlySpendLimit: 500,
      });
      breaker.recordSuccess('agent-1', 400);
      const result = breaker.checkTransaction('agent-1', 200);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /[Mm]onthly spend/);
    });

    it('should block when global kill switch is on', () => {
      cb.tripAll('emergency');
      const result = cb.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /kill switch/i);
    });

    it('should block when circuit is open', () => {
      cb.trip('agent-1', 'overload');
      // With very short cooldown but freshly tripped
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 999999 });
      breaker.trip('agent-1', 'overload');
      const result = breaker.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, false);
      assert.equal(result.state, 'open');
    });

    it('should return reason string when blocked', () => {
      cb.trip('agent-1', 'test reason');
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 999999 });
      breaker.trip('agent-1', 'specific reason');
      const result = breaker.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, false);
      assert.ok(result.reason.length > 0);
    });

    it('should block when failure rate is exceeded', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, {
        maxFailureRate: 0.3,
        failureWindowMs: 60000,
        maxSpendPerTx: 10000,
        dailySpendLimit: 100000,
        monthlySpendLimit: 1000000,
      });
      // Record 7 failures and 3 successes = 70% failure rate
      for (let i = 0; i < 7; i++) breaker.recordFailure('agent-1', 10, 'err');
      for (let i = 0; i < 3; i++) breaker.recordSuccess('agent-1', 10);
      // Agent may have been tripped by recordFailure, reset it to test checkTransaction
      breaker.reset('agent-1');
      const result = breaker.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /[Ff]ailure rate/);
    });

    it('should block transaction with empty agent name', () => {
      const result = cb.checkTransaction('', 100);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /[Aa]gent name/);
    });

    it('should block negative amounts', () => {
      const result = cb.checkTransaction('agent-1', -50);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /non-negative/);
    });

    it('should allow zero-amount transactions', () => {
      const result = cb.checkTransaction('agent-1', 0);
      assert.equal(result.allowed, true);
    });

    it('should transition open→half_open after cooldown on check', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 50 });
      breaker.trip('agent-1', 'test');
      assert.equal(breaker.getState('agent-1').state, 'open');
      // Wait for cooldown
      await new Promise((r) => setTimeout(r, 80));
      const result = breaker.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, true);
      assert.equal(result.state, 'half_open');
    });

    it('should allow transaction when circuit is in half_open', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 50 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 80));
      const result = breaker.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, true);
    });
  });

  // =========================================================================
  // 4. recordSuccess
  // =========================================================================
  describe('recordSuccess', () => {
    it('should record an entry in the ledger', () => {
      cb.recordSuccess('agent-1', 50);
      assert.equal(countLedger(store.db, 'agent-1'), 1);
    });

    it('should record as success=1', () => {
      cb.recordSuccess('agent-1', 50);
      const entries = getLedgerEntries(store.db, 'agent-1');
      assert.equal(entries[0].success, 1);
    });

    it('should record correct amount', () => {
      cb.recordSuccess('agent-1', 123.45);
      const entries = getLedgerEntries(store.db, 'agent-1');
      assert.equal(entries[0].amount, 123.45);
    });

    it('should create an audit event', () => {
      cb.recordSuccess('agent-1', 50);
      const events = getEvents(store.db, 'agent-1');
      const successEvents = events.filter((e) => e.event_type === 'transaction_success');
      assert.equal(successEvents.length, 1);
    });

    it('should transition half_open→closed after threshold successes', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10, halfOpenMaxTxns: 3 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 30));
      // Force half_open via getState
      breaker.getState('agent-1');
      assert.equal(breaker.getState('agent-1').state, 'half_open');

      breaker.recordSuccess('agent-1', 10);
      assert.equal(breaker.getState('agent-1').state, 'half_open');
      breaker.recordSuccess('agent-1', 10);
      assert.equal(breaker.getState('agent-1').state, 'half_open');
      breaker.recordSuccess('agent-1', 10);
      assert.equal(breaker.getState('agent-1').state, 'closed');
    });

    it('should not transition if below half_open threshold', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10, halfOpenMaxTxns: 5 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');

      breaker.recordSuccess('agent-1', 10);
      breaker.recordSuccess('agent-1', 10);
      assert.equal(breaker.getState('agent-1').state, 'half_open');
      assert.equal(breaker.getState('agent-1').halfOpenCount, 2);
    });

    it('should update spending totals visible in summary', () => {
      cb.recordSuccess('agent-1', 100);
      cb.recordSuccess('agent-1', 200);
      const summary = cb.getSpendingSummary('agent-1');
      assert.equal(summary.today, 300);
    });

    it('should not crash with null/empty agent name', () => {
      cb.recordSuccess('', 10);
      cb.recordSuccess(null, 10);
      // No exception
    });

    it('should clear trippedAt and reason after closing from half_open', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10, halfOpenMaxTxns: 1 });
      breaker.trip('agent-1', 'test reason');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');
      breaker.recordSuccess('agent-1', 10);
      const state = breaker.getState('agent-1');
      assert.equal(state.state, 'closed');
      assert.equal(state.trippedAt, null);
      assert.equal(state.reason, null);
    });
  });

  // =========================================================================
  // 5. recordFailure
  // =========================================================================
  describe('recordFailure', () => {
    it('should record an entry in the ledger', () => {
      cb.recordFailure('agent-1', 50, 'timeout');
      assert.equal(countLedger(store.db, 'agent-1'), 1);
    });

    it('should record as success=0', () => {
      cb.recordFailure('agent-1', 50, 'timeout');
      const entries = getLedgerEntries(store.db, 'agent-1');
      assert.equal(entries[0].success, 0);
    });

    it('should record error message in ledger', () => {
      cb.recordFailure('agent-1', 50, 'connection refused');
      const entries = getLedgerEntries(store.db, 'agent-1');
      assert.equal(entries[0].error, 'connection refused');
    });

    it('should create an audit event', () => {
      cb.recordFailure('agent-1', 50, 'err');
      const events = getEvents(store.db, 'agent-1');
      const failEvents = events.filter((e) => e.event_type === 'transaction_failure');
      assert.equal(failEvents.length, 1);
    });

    it('should trip circuit when failure rate exceeds threshold', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { maxFailureRate: 0.3, failureWindowMs: 60000 });
      // 4 failures = 100% failure rate
      breaker.recordFailure('agent-1', 10, 'err');
      breaker.recordFailure('agent-1', 10, 'err');
      breaker.recordFailure('agent-1', 10, 'err');
      breaker.recordFailure('agent-1', 10, 'err');
      const state = breaker.getState('agent-1');
      assert.equal(state.state, 'open');
    });

    it('should not trip if failure rate is below threshold', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { maxFailureRate: 0.5, failureWindowMs: 60000 });
      // 1 failure, 3 successes = 25% failure rate
      breaker.recordSuccess('agent-1', 10);
      breaker.recordSuccess('agent-1', 10);
      breaker.recordSuccess('agent-1', 10);
      breaker.recordFailure('agent-1', 10, 'err');
      const state = breaker.getState('agent-1');
      assert.equal(state.state, 'closed');
    });

    it('should trip half_open back to open on failure', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10, halfOpenMaxTxns: 3 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');
      assert.equal(breaker.getState('agent-1').state, 'half_open');

      breaker.recordFailure('agent-1', 10, 'still broken');
      assert.equal(breaker.getState('agent-1').state, 'open');
    });

    it('should not crash with null/empty agent name', () => {
      cb.recordFailure('', 10, 'err');
      cb.recordFailure(null, 10, 'err');
      // No exception
    });

    it('should set reason when tripping from half_open', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10 });
      breaker.trip('agent-1', 'initial');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');
      breaker.recordFailure('agent-1', 10, 'still failing');
      const state = breaker.getState('agent-1');
      assert.equal(state.reason, 'still failing');
    });

    it('should record amount in ledger on failure', () => {
      cb.recordFailure('agent-1', 77.5, 'err');
      const entries = getLedgerEntries(store.db, 'agent-1');
      assert.equal(entries[0].amount, 77.5);
    });
  });

  // =========================================================================
  // 6. trip / reset
  // =========================================================================
  describe('trip', () => {
    it('should set state to open', () => {
      cb.trip('agent-1', 'manual trip');
      assert.equal(cb.getState('agent-1').state, 'open');
    });

    it('should store the trip reason', () => {
      cb.trip('agent-1', 'suspicious activity');
      assert.equal(cb.getState('agent-1').reason, 'suspicious activity');
    });

    it('should set trippedAt timestamp', () => {
      const before = Date.now();
      cb.trip('agent-1', 'test');
      const state = cb.getState('agent-1');
      assert.ok(state.trippedAt >= before);
      assert.ok(state.trippedAt <= Date.now());
    });

    it('should create a trip audit event', () => {
      cb.trip('agent-1', 'manual');
      const events = getEvents(store.db, 'agent-1');
      const tripEvents = events.filter((e) => e.event_type === 'trip');
      assert.equal(tripEvents.length, 1);
      assert.equal(tripEvents[0].reason, 'manual');
    });

    it('should be a no-op if already open', () => {
      cb.trip('agent-1', 'first');
      const t1 = cb.getState('agent-1').trippedAt;
      cb.trip('agent-1', 'second');
      const t2 = cb.getState('agent-1').trippedAt;
      assert.equal(t1, t2);
      assert.equal(cb.getState('agent-1').reason, 'first');
    });

    it('should trip from half_open state', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10 });
      breaker.trip('agent-1', 'first');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');
      assert.equal(breaker.getState('agent-1').state, 'half_open');
      breaker.trip('agent-1', 'again');
      assert.equal(breaker.getState('agent-1').state, 'open');
    });

    it('should use default reason if none provided', () => {
      cb.trip('agent-1');
      // Should not throw and should have some reason
      const state = cb.getState('agent-1');
      assert.ok(state.reason.length > 0);
    });
  });

  describe('tripAll', () => {
    it('should set globalKillSwitch to true', () => {
      cb.getState('agent-1'); // Register agent
      cb.tripAll('emergency');
      const state = cb.getState('agent-1');
      assert.equal(state.config.globalKillSwitch, true);
    });

    it('should trip all known agents to open', () => {
      cb.getState('agent-1');
      cb.getState('agent-2');
      cb.getState('agent-3');
      cb.tripAll('emergency');
      const states = cb.getAllStates();
      for (const s of states) {
        assert.equal(s.state, 'open');
      }
    });

    it('should create events for each agent', () => {
      cb.getState('agent-1');
      cb.getState('agent-2');
      cb.tripAll('test');
      assert.ok(countEvents(store.db, 'agent-1') > 0);
      assert.ok(countEvents(store.db, 'agent-2') > 0);
    });

    it('should create a global kill switch event', () => {
      cb.tripAll('emergency stop');
      const events = getEvents(store.db, '__global__');
      const killEvents = events.filter((e) => e.event_type === 'kill_switch_activated');
      assert.equal(killEvents.length, 1);
    });

    it('should block even newly created agents', () => {
      cb.tripAll('emergency');
      // agent-new hasn't been seen before
      const result = cb.checkTransaction('agent-new', 10);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /kill switch/i);
    });
  });

  describe('reset', () => {
    it('should set state back to closed', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      assert.equal(cb.getState('agent-1').state, 'closed');
    });

    it('should clear trippedAt', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      assert.equal(cb.getState('agent-1').trippedAt, null);
    });

    it('should clear reason', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      assert.equal(cb.getState('agent-1').reason, null);
    });

    it('should clear halfOpenCount', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      assert.equal(cb.getState('agent-1').halfOpenCount, 0);
    });

    it('should create a reset audit event', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      const events = getEvents(store.db, 'agent-1');
      const resetEvents = events.filter((e) => e.event_type === 'reset');
      assert.equal(resetEvents.length, 1);
    });

    it('should not clear global kill switch', () => {
      cb.tripAll('emergency');
      cb.reset('agent-1');
      const result = cb.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /kill switch/i);
    });

    it('should allow resetting agent that was never tripped', () => {
      cb.reset('agent-1');
      assert.equal(cb.getState('agent-1').state, 'closed');
    });
  });

  describe('resetAll', () => {
    it('should reset all agents to closed', () => {
      cb.trip('agent-1', 'test');
      cb.trip('agent-2', 'test');
      cb.resetAll();
      assert.equal(cb.getState('agent-1').state, 'closed');
      assert.equal(cb.getState('agent-2').state, 'closed');
    });

    it('should deactivate global kill switch', () => {
      cb.tripAll('emergency');
      cb.resetAll();
      const state = cb.getState('agent-1');
      assert.equal(state.config.globalKillSwitch, false);
    });

    it('should create events for all agents', () => {
      cb.trip('agent-1', 'test');
      cb.trip('agent-2', 'test');
      cb.resetAll();
      const events1 = getEvents(store.db, 'agent-1');
      const events2 = getEvents(store.db, 'agent-2');
      assert.ok(events1.some((e) => e.event_type === 'reset_all'));
      assert.ok(events2.some((e) => e.event_type === 'reset_all'));
    });

    it('should create a global kill switch deactivated event', () => {
      cb.tripAll('emergency');
      cb.resetAll();
      const events = getEvents(store.db, '__global__');
      const deactivated = events.filter((e) => e.event_type === 'kill_switch_deactivated');
      assert.equal(deactivated.length, 1);
    });

    it('should allow transactions after resetAll', () => {
      cb.tripAll('emergency');
      cb.resetAll();
      const result = cb.checkTransaction('agent-1', 10);
      assert.equal(result.allowed, true);
    });
  });

  // =========================================================================
  // 7. getSpendingSummary
  // =========================================================================
  describe('getSpendingSummary', () => {
    it('should return zeros for new agent', () => {
      const summary = cb.getSpendingSummary('new-agent');
      assert.equal(summary.today, 0);
      assert.equal(summary.thisMonth, 0);
    });

    it('should calculate today spend from successful transactions', () => {
      cb.recordSuccess('agent-1', 100);
      cb.recordSuccess('agent-1', 250);
      const summary = cb.getSpendingSummary('agent-1');
      assert.equal(summary.today, 350);
    });

    it('should calculate monthly spend from successful transactions', () => {
      cb.recordSuccess('agent-1', 500);
      const summary = cb.getSpendingSummary('agent-1');
      assert.equal(summary.thisMonth, 500);
    });

    it('should not count failed transactions in spend', () => {
      cb.recordSuccess('agent-1', 100);
      cb.recordFailure('agent-1', 200, 'err');
      const summary = cb.getSpendingSummary('agent-1');
      assert.equal(summary.today, 100);
    });

    it('should calculate remaining daily correctly', () => {
      // dailySpendLimit default from our test config = 10000
      cb.recordSuccess('agent-1', 3000);
      const summary = cb.getSpendingSummary('agent-1');
      assert.equal(summary.remainingDaily, 7000);
    });

    it('should calculate remaining monthly correctly', () => {
      // monthlySpendLimit default = 100000
      cb.recordSuccess('agent-1', 25000);
      const summary = cb.getSpendingSummary('agent-1');
      assert.equal(summary.remainingMonthly, 75000);
    });

    it('should not go negative on remaining', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, {
        maxSpendPerTx: 10000,
        dailySpendLimit: 100,
        monthlySpendLimit: 100,
      });
      breaker.recordSuccess('agent-1', 150);
      const summary = breaker.getSpendingSummary('agent-1');
      assert.equal(summary.remainingDaily, 0);
      assert.equal(summary.remainingMonthly, 0);
    });

    it('should handle multiple agents separately', () => {
      cb.recordSuccess('agent-1', 100);
      cb.recordSuccess('agent-2', 500);
      assert.equal(cb.getSpendingSummary('agent-1').today, 100);
      assert.equal(cb.getSpendingSummary('agent-2').today, 500);
    });

    it('should include config-based limits in remaining', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { dailySpendLimit: 777, monthlySpendLimit: 9999 });
      const summary = breaker.getSpendingSummary('agent-1');
      assert.equal(summary.remainingDaily, 777);
      assert.equal(summary.remainingMonthly, 9999);
    });

    it('should update after config change', () => {
      cb.updateConfig({ dailySpendLimit: 200 });
      cb.recordSuccess('agent-1', 100);
      const summary = cb.getSpendingSummary('agent-1');
      assert.equal(summary.remainingDaily, 100);
    });
  });

  // =========================================================================
  // 8. State machine transitions
  // =========================================================================
  describe('state machine', () => {
    it('should transition closed→open on trip', () => {
      assert.equal(cb.getState('agent-1').state, 'closed');
      cb.trip('agent-1', 'test');
      assert.equal(cb.getState('agent-1').state, 'open');
    });

    it('should transition open→half_open after cooldown', async () => {
      cb.trip('agent-1', 'test');
      assert.equal(cb.getState('agent-1').state, 'open');
      await new Promise((r) => setTimeout(r, 150)); // cooldownMs=100
      assert.equal(cb.getState('agent-1').state, 'half_open');
    });

    it('should transition half_open→closed after success threshold', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10, halfOpenMaxTxns: 2 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');
      assert.equal(breaker.getState('agent-1').state, 'half_open');

      breaker.recordSuccess('agent-1', 10);
      breaker.recordSuccess('agent-1', 10);
      assert.equal(breaker.getState('agent-1').state, 'closed');
    });

    it('should transition half_open→open on failure', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');
      assert.equal(breaker.getState('agent-1').state, 'half_open');

      breaker.recordFailure('agent-1', 10, 'err');
      assert.equal(breaker.getState('agent-1').state, 'open');
    });

    it('should complete full cycle: closed→open→half_open→closed', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10, halfOpenMaxTxns: 1 });

      assert.equal(breaker.getState('agent-1').state, 'closed');
      breaker.trip('agent-1', 'test');
      assert.equal(breaker.getState('agent-1').state, 'open');

      await new Promise((r) => setTimeout(r, 30));
      assert.equal(breaker.getState('agent-1').state, 'half_open');

      breaker.recordSuccess('agent-1', 10);
      assert.equal(breaker.getState('agent-1').state, 'closed');
    });

    it('should remain open during cooldown period', () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 999999 });
      breaker.trip('agent-1', 'test');
      assert.equal(breaker.getState('agent-1').state, 'open');
    });

    it('should reset halfOpenCount when tripping from half_open', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10, halfOpenMaxTxns: 5 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1');

      breaker.recordSuccess('agent-1', 10);
      breaker.recordSuccess('agent-1', 10);
      assert.equal(breaker.getState('agent-1').halfOpenCount, 2);

      breaker.recordFailure('agent-1', 10, 'err');
      assert.equal(breaker.getState('agent-1').halfOpenCount, 0);
    });

    it('should log state_change events for transitions', async () => {
      const s = makeStore();
      const breaker = createCircuitBreaker(s, { cooldownMs: 10 });
      breaker.trip('agent-1', 'test');
      await new Promise((r) => setTimeout(r, 30));
      breaker.getState('agent-1'); // triggers open→half_open

      const events = getEvents(s.db, 'agent-1');
      const stateChanges = events.filter((e) => e.event_type === 'state_change');
      assert.ok(stateChanges.length >= 1);
      assert.ok(stateChanges.some((e) => e.state_after === 'half_open'));
    });

    it('should handle multiple trip→reset cycles', () => {
      for (let i = 0; i < 5; i++) {
        cb.trip('agent-1', `trip-${i}`);
        assert.equal(cb.getState('agent-1').state, 'open');
        cb.reset('agent-1');
        assert.equal(cb.getState('agent-1').state, 'closed');
      }
    });
  });

  // =========================================================================
  // 9. getAllStates
  // =========================================================================
  describe('getAllStates', () => {
    it('should return empty array initially', () => {
      const states = cb.getAllStates();
      assert.equal(states.length, 0);
    });

    it('should return all registered agents', () => {
      cb.getState('agent-1');
      cb.getState('agent-2');
      cb.getState('agent-3');
      const states = cb.getAllStates();
      assert.equal(states.length, 3);
    });

    it('should include agent names', () => {
      cb.getState('alpha');
      cb.getState('beta');
      const states = cb.getAllStates();
      const names = states.map((s) => s.agentName);
      assert.ok(names.includes('alpha'));
      assert.ok(names.includes('beta'));
    });

    it('should reflect current state of each agent', () => {
      cb.getState('agent-1');
      cb.trip('agent-2', 'test');
      const states = cb.getAllStates();
      const s1 = states.find((s) => s.agentName === 'agent-1');
      const s2 = states.find((s) => s.agentName === 'agent-2');
      assert.equal(s1.state, 'closed');
      assert.equal(s2.state, 'open');
    });
  });

  // =========================================================================
  // 10. updateConfig
  // =========================================================================
  describe('updateConfig', () => {
    it('should update maxSpendPerTx', () => {
      cb.updateConfig({ maxSpendPerTx: 500 });
      const state = cb.getState('x');
      assert.equal(state.config.maxSpendPerTx, 500);
    });

    it('should update dailySpendLimit', () => {
      cb.updateConfig({ dailySpendLimit: 5000 });
      const state = cb.getState('x');
      assert.equal(state.config.dailySpendLimit, 5000);
    });

    it('should update monthlySpendLimit', () => {
      cb.updateConfig({ monthlySpendLimit: 50000 });
      const state = cb.getState('x');
      assert.equal(state.config.monthlySpendLimit, 50000);
    });

    it('should update multiple values at once', () => {
      cb.updateConfig({ maxSpendPerTx: 200, dailySpendLimit: 1000 });
      const state = cb.getState('x');
      assert.equal(state.config.maxSpendPerTx, 200);
      assert.equal(state.config.dailySpendLimit, 1000);
    });

    it('should ignore unknown config keys', () => {
      cb.updateConfig({ unknownKey: 999 });
      const state = cb.getState('x');
      assert.equal(state.config.unknownKey, undefined);
    });

    it('should not throw with null overrides', () => {
      cb.updateConfig(null);
      // No exception
    });

    it('should take effect immediately on checks', () => {
      cb.updateConfig({ maxSpendPerTx: 50 });
      const result = cb.checkTransaction('agent-1', 75);
      assert.equal(result.allowed, false);
    });

    it('should update cooldownMs', () => {
      cb.updateConfig({ cooldownMs: 5000 });
      const state = cb.getState('x');
      assert.equal(state.config.cooldownMs, 5000);
    });

    it('should update halfOpenMaxTxns', () => {
      cb.updateConfig({ halfOpenMaxTxns: 10 });
      const state = cb.getState('x');
      assert.equal(state.config.halfOpenMaxTxns, 10);
    });

    it('should update maxFailureRate', () => {
      cb.updateConfig({ maxFailureRate: 0.5 });
      const state = cb.getState('x');
      assert.equal(state.config.maxFailureRate, 0.5);
    });
  });

  // =========================================================================
  // 11. Edge cases
  // =========================================================================
  describe('edge cases', () => {
    it('should handle zero-amount transaction', () => {
      const result = cb.checkTransaction('agent-1', 0);
      assert.equal(result.allowed, true);
      cb.recordSuccess('agent-1', 0);
      assert.equal(cb.getSpendingSummary('agent-1').today, 0);
    });

    it('should handle very large amounts', () => {
      const result = cb.checkTransaction('agent-1', 999999);
      assert.equal(result.allowed, false); // Exceeds per-tx limit (default 1000)
    });

    it('should handle many agents', () => {
      for (let i = 0; i < 100; i++) {
        cb.getState(`agent-${i}`);
      }
      assert.equal(cb.getAllStates().length, 100);
    });

    it('should handle rapid successive calls', () => {
      for (let i = 0; i < 50; i++) {
        cb.recordSuccess('agent-1', 1);
      }
      assert.equal(cb.getSpendingSummary('agent-1').today, 50);
    });

    it('should handle special characters in agent name', () => {
      const name = 'agent/with.special-chars_123';
      cb.recordSuccess(name, 10);
      const summary = cb.getSpendingSummary(name);
      assert.equal(summary.today, 10);
    });

    it('should isolate agents completely', () => {
      cb.trip('agent-1', 'test');
      cb.recordSuccess('agent-2', 500);
      assert.equal(cb.getState('agent-1').state, 'open');
      assert.equal(cb.getState('agent-2').state, 'closed');
      assert.equal(cb.getSpendingSummary('agent-2').today, 500);
      assert.equal(cb.getSpendingSummary('agent-1').today, 0);
    });

    it('should handle NaN amount gracefully', () => {
      const result = cb.checkTransaction('agent-1', NaN);
      assert.equal(result.allowed, false);
    });

    it('should handle Infinity amount', () => {
      const result = cb.checkTransaction('agent-1', Infinity);
      assert.equal(result.allowed, false);
    });

    it('should persist ledger across multiple circuit breaker instances on same db', () => {
      const s = makeStore();
      const cb1 = createCircuitBreaker(s);
      cb1.recordSuccess('agent-1', 100);

      const cb2 = createCircuitBreaker(s);
      const summary = cb2.getSpendingSummary('agent-1');
      assert.equal(summary.today, 100);
    });

    it('should handle concurrent trip and reset', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      cb.trip('agent-1', 'again');
      cb.reset('agent-1');
      assert.equal(cb.getState('agent-1').state, 'closed');
    });
  });

  // =========================================================================
  // 12. Kill switch
  // =========================================================================
  describe('kill switch', () => {
    it('should block all agents when active', () => {
      cb.getState('agent-1');
      cb.getState('agent-2');
      cb.tripAll('emergency');
      assert.equal(cb.checkTransaction('agent-1', 10).allowed, false);
      assert.equal(cb.checkTransaction('agent-2', 10).allowed, false);
    });

    it('should block previously unseen agents', () => {
      cb.tripAll('emergency');
      assert.equal(cb.checkTransaction('brand-new-agent', 10).allowed, false);
    });

    it('should be deactivated by resetAll', () => {
      cb.tripAll('emergency');
      cb.resetAll();
      assert.equal(cb.checkTransaction('agent-1', 10).allowed, true);
    });

    it('should not be deactivated by individual reset', () => {
      cb.getState('agent-1');
      cb.tripAll('emergency');
      cb.reset('agent-1');
      assert.equal(cb.checkTransaction('agent-1', 10).allowed, false);
    });

    it('should override closed state', () => {
      // Agent is closed, but kill switch is on
      cb.getState('agent-1');
      cb.tripAll('emergency');
      cb.reset('agent-1'); // reset individual, but kill switch still on
      assert.equal(cb.getState('agent-1').state, 'closed'); // state says closed
      assert.equal(cb.checkTransaction('agent-1', 10).allowed, false); // but kill switch blocks
    });

    it('should be reflectable via config', () => {
      cb.tripAll('test');
      const state = cb.getState('agent-1');
      assert.equal(state.config.globalKillSwitch, true);
    });

    it('should persist across multiple checks', () => {
      cb.tripAll('test');
      for (let i = 0; i < 10; i++) {
        assert.equal(cb.checkTransaction(`agent-${i}`, 1).allowed, false);
      }
    });

    it('should be settable via updateConfig', () => {
      cb.updateConfig({ globalKillSwitch: true });
      assert.equal(cb.checkTransaction('agent-1', 10).allowed, false);
      cb.updateConfig({ globalKillSwitch: false });
      assert.equal(cb.checkTransaction('agent-1', 10).allowed, true);
    });

    it('should take priority over all other checks', () => {
      cb.updateConfig({ globalKillSwitch: true });
      // Even zero-amount should be blocked
      const result = cb.checkTransaction('agent-1', 0);
      assert.equal(result.allowed, false);
      assert.match(result.reason, /kill switch/i);
    });

    it('should have correct reason in check response', () => {
      cb.tripAll('security incident');
      const result = cb.checkTransaction('agent-1', 10);
      assert.match(result.reason, /kill switch/i);
    });
  });

  // =========================================================================
  // 13. Audit trail
  // =========================================================================
  describe('audit trail', () => {
    it('should record trip events with state_before and state_after', () => {
      cb.trip('agent-1', 'test');
      const events = getEvents(store.db, 'agent-1');
      const tripEvent = events.find((e) => e.event_type === 'trip');
      assert.equal(tripEvent.state_before, 'closed');
      assert.equal(tripEvent.state_after, 'open');
    });

    it('should record reset events', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      const events = getEvents(store.db, 'agent-1');
      const resetEvent = events.find((e) => e.event_type === 'reset');
      assert.equal(resetEvent.state_before, 'open');
      assert.equal(resetEvent.state_after, 'closed');
    });

    it('should have unique event IDs', () => {
      cb.trip('agent-1', 'a');
      cb.reset('agent-1');
      cb.trip('agent-1', 'b');
      cb.reset('agent-1');
      const events = getEvents(store.db, 'agent-1');
      const ids = events.map((e) => e.id);
      const uniqueIds = new Set(ids);
      assert.equal(ids.length, uniqueIds.size);
    });

    it('should have timestamps on all events', () => {
      cb.trip('agent-1', 'test');
      cb.reset('agent-1');
      const events = getEvents(store.db, 'agent-1');
      for (const event of events) {
        assert.ok(event.created_at);
      }
    });

    it('should record transaction success events with amount', () => {
      cb.recordSuccess('agent-1', 42.5);
      const events = getEvents(store.db, 'agent-1');
      const successEvent = events.find((e) => e.event_type === 'transaction_success');
      assert.equal(successEvent.amount, 42.5);
    });

    it('should record transaction failure events with reason', () => {
      cb.recordFailure('agent-1', 10, 'timeout error');
      const events = getEvents(store.db, 'agent-1');
      const failEvent = events.find((e) => e.event_type === 'transaction_failure');
      assert.equal(failEvent.reason, 'timeout error');
    });
  });

  // =========================================================================
  // 14. Spending ledger integrity
  // =========================================================================
  describe('spending ledger', () => {
    it('should have unique IDs per entry', () => {
      cb.recordSuccess('agent-1', 10);
      cb.recordSuccess('agent-1', 20);
      cb.recordFailure('agent-1', 30, 'err');
      const entries = getLedgerEntries(store.db, 'agent-1');
      const ids = new Set(entries.map((e) => e.id));
      assert.equal(ids.size, 3);
    });

    it('should store agent_name correctly', () => {
      cb.recordSuccess('my-special-agent', 50);
      const entries = getLedgerEntries(store.db, 'my-special-agent');
      assert.equal(entries.length, 1);
      assert.equal(entries[0].agent_name, 'my-special-agent');
    });

    it('should store created_at timestamp', () => {
      cb.recordSuccess('agent-1', 10);
      const entries = getLedgerEntries(store.db, 'agent-1');
      assert.ok(entries[0].created_at);
    });

    it('should allow querying by agent', () => {
      cb.recordSuccess('agent-1', 10);
      cb.recordSuccess('agent-2', 20);
      assert.equal(countLedger(store.db, 'agent-1'), 1);
      assert.equal(countLedger(store.db, 'agent-2'), 1);
    });

    it('should store both success and failure entries', () => {
      cb.recordSuccess('agent-1', 10);
      cb.recordFailure('agent-1', 20, 'err');
      const entries = getLedgerEntries(store.db, 'agent-1');
      assert.equal(entries.length, 2);
      assert.equal(entries[0].success, 1);
      assert.equal(entries[1].success, 0);
    });
  });
});
