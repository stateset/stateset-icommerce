/**
 * Tests for cli/src/a2a/introspection.js
 *
 * Covers: createIntrospectionService — recordDecision, getDecisionHistory,
 * recordTick, getTickMetrics, getAgentDashboard, getPerformanceReport,
 * recordStateTransition, getLifecycleHistory, clear, agent isolation.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createIntrospectionService } from '../../src/a2a/introspection.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const AGENT_A = '0xAgentA';
const AGENT_B = '0xAgentB';

function makeDecision(overrides = {}) {
  return {
    agentAddress: AGENT_A,
    type: 'quote_eval',
    action: 'accept',
    reason: 'Price within budget',
    context: { quoteId: 'q-1', amount: 50 },
    ...overrides,
  };
}

function makeTick(overrides = {}) {
  return {
    agentAddress: AGENT_A,
    durationMs: 100,
    quotesEvaluated: 5,
    quotesAccepted: 2,
    quotesRejected: 3,
    paymentsExecuted: 1,
    errors: 0,
    ...overrides,
  };
}

// ===========================================================================
// Test suites
// ===========================================================================

describe('createIntrospectionService', () => {
  let svc;

  beforeEach(() => {
    svc = createIntrospectionService();
  });

  // -------------------------------------------------------------------------
  // 1. recordDecision + getDecisionHistory
  // -------------------------------------------------------------------------

  describe('recordDecision / getDecisionHistory', () => {
    it('stores a decision and retrieves it', () => {
      const entry = svc.recordDecision(makeDecision());

      assert.equal(entry.agentAddress, AGENT_A);
      assert.equal(entry.type, 'quote_eval');
      assert.equal(entry.action, 'accept');
      assert.equal(entry.reason, 'Price within budget');
      assert.ok(entry.timestamp);
      assert.deepEqual(entry.context, { quoteId: 'q-1', amount: 50 });

      const history = svc.getDecisionHistory(AGENT_A);
      assert.equal(history.length, 1);
      assert.equal(history[0].type, 'quote_eval');
    });

    it('returns decisions in reverse chronological order', () => {
      svc.recordDecision(makeDecision({ reason: 'first' }));
      svc.recordDecision(makeDecision({ reason: 'second' }));
      svc.recordDecision(makeDecision({ reason: 'third' }));

      const history = svc.getDecisionHistory(AGENT_A);
      assert.equal(history.length, 3);
      assert.equal(history[0].reason, 'third');
      assert.equal(history[1].reason, 'second');
      assert.equal(history[2].reason, 'first');
    });

    it('respects the limit parameter', () => {
      for (let i = 0; i < 10; i++) {
        svc.recordDecision(makeDecision({ reason: `decision-${i}` }));
      }

      const history = svc.getDecisionHistory(AGENT_A, 3);
      assert.equal(history.length, 3);
      // Most recent 3
      assert.equal(history[0].reason, 'decision-9');
      assert.equal(history[1].reason, 'decision-8');
      assert.equal(history[2].reason, 'decision-7');
    });

    it('returns empty array for unknown agent', () => {
      const history = svc.getDecisionHistory('0xUnknown');
      assert.deepEqual(history, []);
    });

    it('throws if agentAddress is missing', () => {
      assert.throws(() => svc.recordDecision({}), /agentAddress is required/);
    });

    it('throws if type is missing', () => {
      assert.throws(
        () => svc.recordDecision({ agentAddress: AGENT_A }),
        /type is required/,
      );
    });

    it('throws if action is missing', () => {
      assert.throws(
        () => svc.recordDecision({ agentAddress: AGENT_A, type: 'quote_eval' }),
        /action is required/,
      );
    });

    it('defaults reason and context to null if not provided', () => {
      const entry = svc.recordDecision({
        agentAddress: AGENT_A,
        type: 'payment',
        action: 'accept',
      });
      assert.equal(entry.reason, null);
      assert.equal(entry.context, null);
    });

    it('records multiple decision types', () => {
      svc.recordDecision(makeDecision({ type: 'quote_eval', action: 'accept' }));
      svc.recordDecision(makeDecision({ type: 'payment', action: 'reject' }));
      svc.recordDecision(makeDecision({ type: 'strategy_change', action: 'skip' }));
      svc.recordDecision(makeDecision({ type: 'budget_check', action: 'accept' }));

      const history = svc.getDecisionHistory(AGENT_A);
      assert.equal(history.length, 4);
      const types = history.map((d) => d.type);
      assert.ok(types.includes('quote_eval'));
      assert.ok(types.includes('payment'));
      assert.ok(types.includes('strategy_change'));
      assert.ok(types.includes('budget_check'));
    });
  });

  // -------------------------------------------------------------------------
  // 2. recordTick + getTickMetrics
  // -------------------------------------------------------------------------

  describe('recordTick / getTickMetrics', () => {
    it('accumulates tick data and computes averages', () => {
      svc.recordTick(makeTick({ durationMs: 100, quotesEvaluated: 5, paymentsExecuted: 1, errors: 0 }));
      svc.recordTick(makeTick({ durationMs: 200, quotesEvaluated: 3, paymentsExecuted: 2, errors: 1 }));

      const metrics = svc.getTickMetrics(AGENT_A);
      assert.equal(metrics.totalTicks, 2);
      assert.equal(metrics.avgTickDurationMs, 150);
      assert.equal(metrics.quotesEvaluated, 8);
      assert.equal(metrics.paymentsExecuted, 3);
      assert.equal(metrics.errorsCount, 1);
    });

    it('returns zero metrics for unknown agent', () => {
      const metrics = svc.getTickMetrics('0xUnknown');
      assert.equal(metrics.totalTicks, 0);
      assert.equal(metrics.avgTickDurationMs, 0);
      assert.equal(metrics.ticksPerMinute, 0);
      assert.equal(metrics.quotesEvaluated, 0);
      assert.equal(metrics.paymentsExecuted, 0);
      assert.equal(metrics.errorsCount, 0);
    });

    it('throws if agentAddress is missing', () => {
      assert.throws(() => svc.recordTick({}), /agentAddress is required/);
    });

    it('throws if durationMs is missing', () => {
      assert.throws(
        () => svc.recordTick({ agentAddress: AGENT_A }),
        /durationMs is required/,
      );
    });

    it('defaults optional tick fields to zero', () => {
      const entry = svc.recordTick({ agentAddress: AGENT_A, durationMs: 50 });
      assert.equal(entry.quotesEvaluated, 0);
      assert.equal(entry.quotesAccepted, 0);
      assert.equal(entry.quotesRejected, 0);
      assert.equal(entry.paymentsExecuted, 0);
      assert.equal(entry.errors, 0);
    });

    it('computes ticksPerMinute from time span', () => {
      // Record ticks with known timestamps (we can't easily control Date.now,
      // but we can verify the formula by recording many ticks quickly)
      for (let i = 0; i < 10; i++) {
        svc.recordTick(makeTick({ durationMs: 10 }));
      }

      const metrics = svc.getTickMetrics(AGENT_A);
      assert.equal(metrics.totalTicks, 10);
      // ticksPerMinute should be a number >= 0
      assert.equal(typeof metrics.ticksPerMinute, 'number');
      assert.ok(metrics.ticksPerMinute >= 0);
    });
  });

  // -------------------------------------------------------------------------
  // 3. getAgentDashboard
  // -------------------------------------------------------------------------

  describe('getAgentDashboard', () => {
    it('returns a full dashboard for an agent with data', () => {
      svc.recordDecision(makeDecision({ type: 'quote_eval', action: 'accept' }));
      svc.recordDecision(makeDecision({ type: 'quote_eval', action: 'reject' }));
      svc.recordDecision(makeDecision({ type: 'payment', action: 'accept' }));
      svc.recordTick(makeTick({ durationMs: 100 }));
      svc.recordTick(makeTick({ durationMs: 200 }));
      svc.recordStateTransition(AGENT_A, 'idle', 'running', 'startup');

      const dashboard = svc.getAgentDashboard(AGENT_A);

      assert.equal(dashboard.agentAddress, AGENT_A);
      assert.equal(dashboard.runtimeStatus, 'running');
      assert.equal(dashboard.currentState, 'running');
      assert.equal(dashboard.totalDecisions, 3);
      assert.equal(dashboard.lifecycleTransitions, 1);
      assert.ok(dashboard.tickMetrics);
      assert.equal(dashboard.tickMetrics.totalTicks, 2);
      assert.ok(dashboard.decisionSummary);
      assert.equal(dashboard.decisionSummary['quote_eval:accept'], 1);
      assert.equal(dashboard.decisionSummary['quote_eval:reject'], 1);
      assert.equal(dashboard.decisionSummary['payment:accept'], 1);
      assert.ok(dashboard.lastTickAt);
    });

    it('returns a dashboard with defaults for unknown agent', () => {
      const dashboard = svc.getAgentDashboard('0xNew');

      assert.equal(dashboard.agentAddress, '0xNew');
      assert.equal(dashboard.runtimeStatus, 'unknown');
      assert.equal(dashboard.totalDecisions, 0);
      assert.equal(dashboard.lifecycleTransitions, 0);
      assert.equal(dashboard.lastTickAt, null);
      assert.deepEqual(dashboard.decisionSummary, {});
    });

    it('throws if agentAddress is missing', () => {
      assert.throws(() => svc.getAgentDashboard(), /agentAddress is required/);
    });
  });

  // -------------------------------------------------------------------------
  // 4. getPerformanceReport
  // -------------------------------------------------------------------------

  describe('getPerformanceReport', () => {
    it('computes rates from decision and tick data', () => {
      // 3 quote_eval: 2 accept, 1 reject => accept rate = 0.6667
      svc.recordDecision(makeDecision({ type: 'quote_eval', action: 'accept' }));
      svc.recordDecision(makeDecision({ type: 'quote_eval', action: 'accept' }));
      svc.recordDecision(makeDecision({ type: 'quote_eval', action: 'reject' }));

      // 2 payment: 2 accept, 0 reject => settlement rate = 1.0
      svc.recordDecision(makeDecision({ type: 'payment', action: 'accept' }));
      svc.recordDecision(makeDecision({ type: 'payment', action: 'accept' }));

      // Tick metrics
      svc.recordTick(makeTick({ durationMs: 100, errors: 0 }));
      svc.recordTick(makeTick({ durationMs: 200, errors: 0 }));

      const report = svc.getPerformanceReport(AGENT_A);

      assert.equal(report.agentAddress, AGENT_A);
      assert.equal(report.quoteAcceptRate, 0.6667);
      assert.equal(report.avgResponseTimeMs, 150);
      assert.equal(report.settlementSuccessRate, 1);
      assert.equal(report.disputeRate, 0);
      assert.equal(report.uptimePercent, 100);
    });

    it('computes uptimePercent accounting for errors', () => {
      svc.recordTick(makeTick({ durationMs: 50, errors: 0 }));
      svc.recordTick(makeTick({ durationMs: 50, errors: 0 }));
      svc.recordTick(makeTick({ durationMs: 50, errors: 1 }));
      svc.recordTick(makeTick({ durationMs: 50, errors: 0 }));

      const report = svc.getPerformanceReport(AGENT_A);
      // 3 out of 4 ticks error-free => 75%
      assert.equal(report.uptimePercent, 75);
    });

    it('returns defaults for agent with no data', () => {
      const report = svc.getPerformanceReport('0xEmpty');

      assert.equal(report.quoteAcceptRate, 0);
      assert.equal(report.avgResponseTimeMs, 0);
      assert.equal(report.settlementSuccessRate, 1);
      assert.equal(report.disputeRate, 0);
      assert.equal(report.uptimePercent, 100);
    });

    it('computes disputeRate from budget_check rejects', () => {
      svc.recordDecision(makeDecision({ type: 'quote_eval', action: 'accept' }));
      svc.recordDecision(makeDecision({ type: 'payment', action: 'accept' }));
      svc.recordDecision(makeDecision({ type: 'budget_check', action: 'reject' }));

      const report = svc.getPerformanceReport(AGENT_A);
      // 1 budget_check reject / 2 total (quote + payment) = 0.5
      assert.equal(report.disputeRate, 0.5);
    });

    it('throws if agentAddress is missing', () => {
      assert.throws(() => svc.getPerformanceReport(), /agentAddress is required/);
    });
  });

  // -------------------------------------------------------------------------
  // 5. recordStateTransition + getLifecycleHistory
  // -------------------------------------------------------------------------

  describe('recordStateTransition / getLifecycleHistory', () => {
    it('records and retrieves state transitions', () => {
      svc.recordStateTransition(AGENT_A, 'idle', 'starting', 'boot');
      svc.recordStateTransition(AGENT_A, 'starting', 'running', 'initialized');
      svc.recordStateTransition(AGENT_A, 'running', 'stopped', 'shutdown');

      const history = svc.getLifecycleHistory(AGENT_A);

      assert.equal(history.length, 3);
      assert.equal(history[0].fromState, 'idle');
      assert.equal(history[0].toState, 'starting');
      assert.equal(history[0].reason, 'boot');
      assert.equal(history[1].fromState, 'starting');
      assert.equal(history[1].toState, 'running');
      assert.equal(history[2].fromState, 'running');
      assert.equal(history[2].toState, 'stopped');
    });

    it('returns empty array for unknown agent', () => {
      assert.deepEqual(svc.getLifecycleHistory('0xUnknown'), []);
    });

    it('throws if agentAddress is missing', () => {
      assert.throws(
        () => svc.recordStateTransition(null, 'a', 'b'),
        /agentAddress is required/,
      );
    });

    it('throws if fromState is missing', () => {
      assert.throws(
        () => svc.recordStateTransition(AGENT_A, null, 'b'),
        /fromState is required/,
      );
    });

    it('throws if toState is missing', () => {
      assert.throws(
        () => svc.recordStateTransition(AGENT_A, 'a', null),
        /toState is required/,
      );
    });

    it('defaults reason to null', () => {
      const entry = svc.recordStateTransition(AGENT_A, 'idle', 'running');
      assert.equal(entry.reason, null);
    });
  });

  // -------------------------------------------------------------------------
  // 6. Agent data isolation
  // -------------------------------------------------------------------------

  describe('agent data isolation', () => {
    it('does not leak decisions between agents', () => {
      svc.recordDecision(makeDecision({ agentAddress: AGENT_A, reason: 'A decision' }));
      svc.recordDecision(makeDecision({ agentAddress: AGENT_B, reason: 'B decision' }));

      const historyA = svc.getDecisionHistory(AGENT_A);
      const historyB = svc.getDecisionHistory(AGENT_B);

      assert.equal(historyA.length, 1);
      assert.equal(historyA[0].reason, 'A decision');
      assert.equal(historyB.length, 1);
      assert.equal(historyB[0].reason, 'B decision');
    });

    it('does not leak ticks between agents', () => {
      svc.recordTick(makeTick({ agentAddress: AGENT_A, durationMs: 100 }));
      svc.recordTick(makeTick({ agentAddress: AGENT_B, durationMs: 200 }));

      const metricsA = svc.getTickMetrics(AGENT_A);
      const metricsB = svc.getTickMetrics(AGENT_B);

      assert.equal(metricsA.totalTicks, 1);
      assert.equal(metricsA.avgTickDurationMs, 100);
      assert.equal(metricsB.totalTicks, 1);
      assert.equal(metricsB.avgTickDurationMs, 200);
    });

    it('does not leak lifecycle between agents', () => {
      svc.recordStateTransition(AGENT_A, 'idle', 'running');
      svc.recordStateTransition(AGENT_B, 'idle', 'stopped');

      const historyA = svc.getLifecycleHistory(AGENT_A);
      const historyB = svc.getLifecycleHistory(AGENT_B);

      assert.equal(historyA.length, 1);
      assert.equal(historyA[0].toState, 'running');
      assert.equal(historyB.length, 1);
      assert.equal(historyB[0].toState, 'stopped');
    });

    it('dashboards are isolated per agent', () => {
      svc.recordDecision(makeDecision({ agentAddress: AGENT_A }));
      svc.recordTick(makeTick({ agentAddress: AGENT_A }));

      const dashA = svc.getAgentDashboard(AGENT_A);
      const dashB = svc.getAgentDashboard(AGENT_B);

      assert.equal(dashA.totalDecisions, 1);
      assert.equal(dashA.tickMetrics.totalTicks, 1);
      assert.equal(dashB.totalDecisions, 0);
      assert.equal(dashB.tickMetrics.totalTicks, 0);
    });
  });

  // -------------------------------------------------------------------------
  // 7. clear
  // -------------------------------------------------------------------------

  describe('clear', () => {
    it('removes all data for a specific agent', () => {
      svc.recordDecision(makeDecision({ agentAddress: AGENT_A }));
      svc.recordTick(makeTick({ agentAddress: AGENT_A }));
      svc.recordStateTransition(AGENT_A, 'idle', 'running');

      svc.clear(AGENT_A);

      assert.deepEqual(svc.getDecisionHistory(AGENT_A), []);
      assert.equal(svc.getTickMetrics(AGENT_A).totalTicks, 0);
      assert.deepEqual(svc.getLifecycleHistory(AGENT_A), []);
    });

    it('does not affect other agents', () => {
      svc.recordDecision(makeDecision({ agentAddress: AGENT_A }));
      svc.recordDecision(makeDecision({ agentAddress: AGENT_B }));

      svc.clear(AGENT_A);

      assert.deepEqual(svc.getDecisionHistory(AGENT_A), []);
      assert.equal(svc.getDecisionHistory(AGENT_B).length, 1);
    });

    it('is safe to call on unknown agent', () => {
      // Should not throw
      svc.clear('0xNonexistent');
    });

    it('throws if agentAddress is missing', () => {
      assert.throws(() => svc.clear(), /agentAddress is required/);
    });

    it('cleared agent shows empty dashboard', () => {
      svc.recordDecision(makeDecision({ agentAddress: AGENT_A }));
      svc.recordTick(makeTick({ agentAddress: AGENT_A }));
      svc.recordStateTransition(AGENT_A, 'idle', 'running');

      svc.clear(AGENT_A);

      const dashboard = svc.getAgentDashboard(AGENT_A);
      assert.equal(dashboard.totalDecisions, 0);
      assert.equal(dashboard.tickMetrics.totalTicks, 0);
      assert.equal(dashboard.lifecycleTransitions, 0);
      assert.equal(dashboard.runtimeStatus, 'unknown');
    });
  });
});
