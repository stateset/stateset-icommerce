/**
 * Unit tests for a2a/cost-analytics.js — Agent Economic Intelligence Engine
 *
 * Covers: record(), getAgentSpendSummary, getCounterpartyBreakdown,
 * getOperationBreakdown, getDailySpendTrend, detectAnomalies,
 * getEscrowMetrics, getMarginAnalysis, getBudgetForecast, getTopSpenders,
 * validation, empty state, and agent isolation.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { createCostAnalytics } from '../../src/a2a/cost-analytics.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Today as YYYY-MM-DD */
function todayKey() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/** N days ago ISO string (midday to avoid timezone edge) */
function daysAgo(n) {
  const d = new Date();
  d.setDate(d.getDate() - n);
  d.setHours(12, 0, 0, 0);
  return d.toISOString();
}

/** N days ago as a YYYY-MM-DD key */
function daysAgoKey(n) {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}

/** Shorthand for a basic spend entry */
function spendEntry(overrides = {}) {
  return {
    agentAddress: '0xAlice',
    counterparty: '0xBob',
    direction: 'spend',
    amount: 100,
    operation: 'quote_payment',
    timestamp: new Date().toISOString(),
    ...overrides,
  };
}

/** Shorthand for a basic earn entry */
function earnEntry(overrides = {}) {
  return {
    agentAddress: '0xAlice',
    counterparty: '0xBob',
    direction: 'earn',
    amount: 100,
    operation: 'settlement',
    timestamp: new Date().toISOString(),
    ...overrides,
  };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('createCostAnalytics', () => {
  /** @type {ReturnType<typeof createCostAnalytics>} */
  let analytics;

  beforeEach(() => {
    analytics = createCostAnalytics();
  });

  // =========================================================================
  // 1. record() stores entries and getAgentSpendSummary reflects them
  // =========================================================================

  describe('record() + getAgentSpendSummary()', () => {
    it('stores an entry and reflects in summary', () => {
      const stored = analytics.record(spendEntry({ amount: 50 }));
      assert.ok(stored.id, 'should have generated id');
      assert.equal(stored.amount, 50);
      assert.equal(stored.direction, 'spend');
      assert.equal(stored.operation, 'quote_payment');

      const summary = analytics.getAgentSpendSummary('0xAlice');
      assert.equal(summary.totalSpent, 50);
      assert.equal(summary.totalEarned, 0);
      assert.equal(summary.netMargin, -50);
      assert.equal(summary.transactionCount, 1);
      assert.equal(summary.avgTransactionSize, 50);
    });

    it('accumulates multiple entries correctly', () => {
      analytics.record(spendEntry({ amount: 100 }));
      analytics.record(spendEntry({ amount: 200 }));
      analytics.record(earnEntry({ amount: 150 }));

      const summary = analytics.getAgentSpendSummary('0xAlice');
      assert.equal(summary.totalSpent, 300);
      assert.equal(summary.totalEarned, 150);
      assert.equal(summary.netMargin, -150);
      assert.equal(summary.transactionCount, 3);
      // avgTransactionSize = (300 + 150) / 3 = 150
      assert.equal(summary.avgTransactionSize, 150);
    });

    it('assigns default timestamp when none provided', () => {
      const entry = { ...spendEntry() };
      delete entry.timestamp;
      const stored = analytics.record(entry);
      assert.ok(stored.timestamp, 'should have a timestamp');
      // Should be close to now
      const diff = Math.abs(Date.now() - new Date(stored.timestamp).getTime());
      assert.ok(diff < 5000, 'timestamp should be within 5 seconds of now');
    });

    it('defaults operation to "other" when omitted', () => {
      const entry = { ...spendEntry() };
      delete entry.operation;
      const stored = analytics.record(entry);
      assert.equal(stored.operation, 'other');
    });

    it('preserves metadata', () => {
      const stored = analytics.record(spendEntry({ metadata: { orderId: 'ORD-1' } }));
      assert.deepEqual(stored.metadata, { orderId: 'ORD-1' });
    });

    it('preserves sagaId', () => {
      const stored = analytics.record(spendEntry({ sagaId: 'saga-xyz' }));
      assert.equal(stored.sagaId, 'saga-xyz');
    });
  });

  // =========================================================================
  // record() validation
  // =========================================================================

  describe('record() validation', () => {
    it('throws on null entry', () => {
      assert.throws(() => analytics.record(null), /requires an entry object/);
    });

    it('throws on non-object entry', () => {
      assert.throws(() => analytics.record('bad'), /requires an entry object/);
    });

    it('throws on missing agentAddress', () => {
      assert.throws(
        () => analytics.record({ ...spendEntry(), agentAddress: '' }),
        /requires a non-empty agentAddress/,
      );
    });

    it('throws on missing counterparty', () => {
      assert.throws(
        () => analytics.record({ ...spendEntry(), counterparty: '' }),
        /requires a non-empty counterparty/,
      );
    });

    it('throws on invalid direction', () => {
      assert.throws(
        () => analytics.record({ ...spendEntry(), direction: 'borrow' }),
        /direction must be one of/,
      );
    });

    it('throws on negative amount', () => {
      assert.throws(
        () => analytics.record({ ...spendEntry(), amount: -10 }),
        /amount must be a non-negative/,
      );
    });

    it('throws on NaN amount', () => {
      assert.throws(
        () => analytics.record({ ...spendEntry(), amount: NaN }),
        /amount must be a non-negative/,
      );
    });

    it('throws on Infinity amount', () => {
      assert.throws(
        () => analytics.record({ ...spendEntry(), amount: Infinity }),
        /amount must be a non-negative/,
      );
    });

    it('throws on invalid operation', () => {
      assert.throws(
        () => analytics.record({ ...spendEntry(), operation: 'hack_bank' }),
        /operation must be one of/,
      );
    });

    it('accepts zero amount', () => {
      const stored = analytics.record(spendEntry({ amount: 0 }));
      assert.equal(stored.amount, 0);
    });
  });

  // =========================================================================
  // 2. Counterparty breakdown ranks by volume
  // =========================================================================

  describe('getCounterpartyBreakdown()', () => {
    it('ranks counterparties by total volume descending', () => {
      // 0xBob: volume = 100 + 50 = 150
      analytics.record(spendEntry({ counterparty: '0xBob', amount: 100 }));
      analytics.record(earnEntry({ counterparty: '0xBob', amount: 50 }));
      // 0xCharlie: volume = 500
      analytics.record(spendEntry({ counterparty: '0xCharlie', amount: 500 }));
      // 0xDave: volume = 25
      analytics.record(spendEntry({ counterparty: '0xDave', amount: 25 }));

      const breakdown = analytics.getCounterpartyBreakdown('0xAlice');
      assert.equal(breakdown.length, 3);
      assert.equal(breakdown[0].counterparty, '0xCharlie');
      assert.equal(breakdown[0].volume, 500);
      assert.equal(breakdown[1].counterparty, '0xBob');
      assert.equal(breakdown[1].volume, 150);
      assert.equal(breakdown[1].spent, 100);
      assert.equal(breakdown[1].earned, 50);
      assert.equal(breakdown[1].transactionCount, 2);
      assert.equal(breakdown[2].counterparty, '0xDave');
      assert.equal(breakdown[2].volume, 25);
    });

    it('returns empty array for unknown agent', () => {
      const breakdown = analytics.getCounterpartyBreakdown('0xUnknown');
      assert.deepEqual(breakdown, []);
    });
  });

  // =========================================================================
  // 3. Operation breakdown categorizes correctly
  // =========================================================================

  describe('getOperationBreakdown()', () => {
    it('categorizes operations with correct percentages', () => {
      analytics.record(spendEntry({ operation: 'quote_payment', amount: 200 }));
      analytics.record(spendEntry({ operation: 'quote_payment', amount: 100 }));
      analytics.record(spendEntry({ operation: 'escrow_fund', amount: 300 }));
      analytics.record(earnEntry({ operation: 'settlement', amount: 400 }));

      const breakdown = analytics.getOperationBreakdown('0xAlice');
      assert.equal(breakdown.length, 3);

      // settlement: 400 = 40% of 1000
      assert.equal(breakdown[0].operation, 'settlement');
      assert.equal(breakdown[0].totalAmount, 400);
      assert.equal(breakdown[0].transactionCount, 1);
      assert.ok(Math.abs(breakdown[0].percentOfTotal - 40) < 0.01);

      // quote_payment: 300 = 30%
      assert.equal(breakdown[1].operation, 'quote_payment');
      assert.equal(breakdown[1].totalAmount, 300);
      assert.equal(breakdown[1].transactionCount, 2);
      assert.ok(Math.abs(breakdown[1].percentOfTotal - 30) < 0.01);

      // escrow_fund: 300 = 30%
      assert.equal(breakdown[2].operation, 'escrow_fund');
      assert.equal(breakdown[2].totalAmount, 300);
    });

    it('returns empty array for unknown agent', () => {
      assert.deepEqual(analytics.getOperationBreakdown('0xGhost'), []);
    });
  });

  // =========================================================================
  // 4. Daily spend trend aggregates by day
  // =========================================================================

  describe('getDailySpendTrend()', () => {
    it('aggregates spend and earn by day', () => {
      const today = new Date();
      today.setHours(12, 0, 0, 0);
      const todayISO = today.toISOString();

      analytics.record(spendEntry({ amount: 50, timestamp: todayISO }));
      analytics.record(spendEntry({ amount: 30, timestamp: todayISO }));
      analytics.record(earnEntry({ amount: 100, timestamp: todayISO }));

      const trend = analytics.getDailySpendTrend('0xAlice', 7);
      assert.ok(trend.length >= 1);

      const todayRec = trend.find((t) => t.date === todayKey());
      assert.ok(todayRec, 'should have an entry for today');
      assert.equal(todayRec.spent, 80);
      assert.equal(todayRec.earned, 100);
      assert.equal(todayRec.net, 20);
    });

    it('pre-fills missing days with zeroes', () => {
      analytics.record(spendEntry({ amount: 10 }));
      const trend = analytics.getDailySpendTrend('0xAlice', 7);
      assert.equal(trend.length, 7);
      // Most days should be zero
      const zeroDays = trend.filter((t) => t.spent === 0 && t.earned === 0);
      assert.ok(zeroDays.length >= 6);
    });

    it('excludes entries older than the lookback window', () => {
      analytics.record(spendEntry({ amount: 999, timestamp: daysAgo(60) }));
      analytics.record(spendEntry({ amount: 10, timestamp: daysAgo(1) }));

      const trend = analytics.getDailySpendTrend('0xAlice', 7);
      const total = trend.reduce((s, t) => s + t.spent, 0);
      assert.equal(total, 10); // old entry excluded
    });

    it('returns sorted by date ascending', () => {
      analytics.record(spendEntry({ amount: 10 }));
      const trend = analytics.getDailySpendTrend('0xAlice', 7);
      for (let i = 1; i < trend.length; i++) {
        assert.ok(trend[i].date >= trend[i - 1].date, 'should be ascending');
      }
    });
  });

  // =========================================================================
  // 5. Anomaly detection flags >3x average transactions
  // =========================================================================

  describe('detectAnomalies() — transaction level', () => {
    it('flags transactions >3x average', () => {
      // Record 10 normal transactions at 100 each
      for (let i = 0; i < 10; i++) {
        analytics.record(spendEntry({ amount: 100 }));
      }
      // Record 1 outlier at 500 (avg=~136, threshold=~409, 500>409)
      const outlier = analytics.record(spendEntry({ amount: 500 }));

      const { transactionAnomalies } = analytics.detectAnomalies('0xAlice');
      assert.ok(transactionAnomalies.length >= 1, 'should flag at least 1 anomaly');

      const flagged = transactionAnomalies.find((a) => a.id === outlier.id);
      assert.ok(flagged, 'outlier should be flagged');
      assert.equal(flagged.amount, 500);
      assert.ok(flagged.ratio > 3, 'ratio should exceed 3');
    });

    it('does not flag transactions at or below 3x average', () => {
      // All same amount => avg = 100, threshold = 300
      for (let i = 0; i < 5; i++) {
        analytics.record(spendEntry({ amount: 100 }));
      }
      // 300 = exactly 3x, should NOT be flagged (> not >=)
      analytics.record(spendEntry({ amount: 300 }));

      const { transactionAnomalies } = analytics.detectAnomalies('0xAlice');
      // avg = (5*100 + 300)/6 = 800/6 ≈ 133.3, threshold ≈ 400
      // 300 < 400 => not flagged
      assert.equal(transactionAnomalies.length, 0);
    });

    it('returns empty for unknown agent', () => {
      const result = analytics.detectAnomalies('0xGhost');
      assert.deepEqual(result.transactionAnomalies, []);
      assert.deepEqual(result.dailyAnomalies, []);
    });
  });

  // =========================================================================
  // 6. Anomaly detection flags >2x daily average
  // =========================================================================

  describe('detectAnomalies() — daily level', () => {
    it('flags days with spend >2x daily average', () => {
      // Day 1: $100 spend
      analytics.record(spendEntry({ amount: 50, timestamp: daysAgo(3) }));
      analytics.record(spendEntry({ amount: 50, timestamp: daysAgo(3) }));
      // Day 2: $100 spend
      analytics.record(spendEntry({ amount: 100, timestamp: daysAgo(2) }));
      // Day 3: $100 spend
      analytics.record(spendEntry({ amount: 100, timestamp: daysAgo(1) }));
      // Day 4: $500 spend (anomaly) — daily avg of 4 days = (100+100+100+500)/4 = 200
      // threshold = 400, 500 > 400 => flagged
      analytics.record(spendEntry({ amount: 500, timestamp: daysAgo(0) }));

      const { dailyAnomalies } = analytics.detectAnomalies('0xAlice');
      assert.ok(dailyAnomalies.length >= 1, 'should flag at least 1 day');

      const todayAnomaly = dailyAnomalies.find((a) => a.date === todayKey());
      assert.ok(todayAnomaly, 'today should be flagged');
      assert.equal(todayAnomaly.totalSpend, 500);
      assert.ok(todayAnomaly.ratio > 2, 'ratio should exceed 2');
    });

    it('does not flag days below the threshold', () => {
      // All days equal spend => no day exceeds 2x average
      for (let i = 0; i < 5; i++) {
        analytics.record(spendEntry({ amount: 100, timestamp: daysAgo(i) }));
      }

      const { dailyAnomalies } = analytics.detectAnomalies('0xAlice');
      assert.equal(dailyAnomalies.length, 0);
    });
  });

  // =========================================================================
  // 7. Escrow metrics compute hold time and release/refund rates
  // =========================================================================

  describe('getEscrowMetrics()', () => {
    it('tracks escrow fund, release, and refund lifecycle', () => {
      const fundTime = new Date('2026-03-01T10:00:00Z').toISOString();
      const releaseTime = new Date('2026-03-01T12:00:00Z').toISOString(); // 2 hours later

      // Escrow 1: funded and released
      analytics.record(spendEntry({
        operation: 'escrow_fund',
        amount: 200,
        sagaId: 'escrow-1',
        timestamp: fundTime,
      }));
      analytics.record(earnEntry({
        operation: 'escrow_release',
        amount: 200,
        sagaId: 'escrow-1',
        timestamp: releaseTime,
      }));

      // Escrow 2: funded and refunded (3 hours hold)
      const refundTime = new Date('2026-03-01T13:00:00Z').toISOString();
      analytics.record(spendEntry({
        operation: 'escrow_fund',
        amount: 300,
        sagaId: 'escrow-2',
        timestamp: fundTime,
      }));
      analytics.record(earnEntry({
        operation: 'escrow_refund',
        amount: 300,
        sagaId: 'escrow-2',
        timestamp: refundTime,
      }));

      // Escrow 3: funded but still locked
      analytics.record(spendEntry({
        operation: 'escrow_fund',
        amount: 100,
        sagaId: 'escrow-3',
        timestamp: fundTime,
      }));

      const metrics = analytics.getEscrowMetrics();
      assert.equal(metrics.escrowCount, 3);
      assert.equal(metrics.totalLocked, 600); // 200 + 300 + 100
      assert.equal(metrics.totalReleased, 200);
      assert.equal(metrics.totalRefunded, 300);

      // Release rate: 1 released / (1 released + 1 refunded) = 0.5
      assert.equal(metrics.releaseRate, 0.5);
      assert.equal(metrics.refundRate, 0.5);

      // Avg hold time: (2h + 3h) / 2 = 2.5 hours = 9_000_000 ms
      const twoHoursMs = 2 * 60 * 60 * 1000;
      const threeHoursMs = 3 * 60 * 60 * 1000;
      const expectedAvgHold = (twoHoursMs + threeHoursMs) / 2;
      assert.equal(metrics.avgHoldTimeMs, expectedAvgHold);
    });

    it('returns zeroes when no escrows exist', () => {
      const metrics = analytics.getEscrowMetrics();
      assert.equal(metrics.totalLocked, 0);
      assert.equal(metrics.totalReleased, 0);
      assert.equal(metrics.totalRefunded, 0);
      assert.equal(metrics.avgHoldTimeMs, 0);
      assert.equal(metrics.releaseRate, 0);
      assert.equal(metrics.refundRate, 0);
      assert.equal(metrics.escrowCount, 0);
    });

    it('tracks 100% release rate when all escrows are released', () => {
      analytics.record(spendEntry({
        operation: 'escrow_fund',
        amount: 100,
        sagaId: 'e1',
        timestamp: '2026-01-01T00:00:00Z',
      }));
      analytics.record(earnEntry({
        operation: 'escrow_release',
        amount: 100,
        sagaId: 'e1',
        timestamp: '2026-01-01T01:00:00Z',
      }));

      const metrics = analytics.getEscrowMetrics();
      assert.equal(metrics.releaseRate, 1);
      assert.equal(metrics.refundRate, 0);
    });

    it('tracks 100% refund rate when all escrows are refunded', () => {
      analytics.record(spendEntry({
        operation: 'escrow_fund',
        amount: 50,
        sagaId: 'e2',
        timestamp: '2026-01-01T00:00:00Z',
      }));
      analytics.record(earnEntry({
        operation: 'escrow_refund',
        amount: 50,
        sagaId: 'e2',
        timestamp: '2026-01-01T02:00:00Z',
      }));

      const metrics = analytics.getEscrowMetrics();
      assert.equal(metrics.releaseRate, 0);
      assert.equal(metrics.refundRate, 1);
    });
  });

  // =========================================================================
  // 8. Margin analysis computes gross margin per counterparty
  // =========================================================================

  describe('getMarginAnalysis()', () => {
    it('computes gross margin and per-counterparty margins', () => {
      // Spend 200 to Bob, earn 300 from Bob => margin +100
      analytics.record(spendEntry({ counterparty: '0xBob', amount: 200 }));
      analytics.record(earnEntry({ counterparty: '0xBob', amount: 300 }));

      // Spend 500 to Charlie, earn 100 from Charlie => margin -400
      analytics.record(spendEntry({ counterparty: '0xCharlie', amount: 500 }));
      analytics.record(earnEntry({ counterparty: '0xCharlie', amount: 100 }));

      const margin = analytics.getMarginAnalysis('0xAlice');

      // Gross margin: (300 + 100) - (200 + 500) = 400 - 700 = -300
      assert.equal(margin.grossMargin, -300);

      assert.equal(margin.perCounterparty.length, 2);

      // Best: Bob (+100), Worst: Charlie (-400)
      assert.equal(margin.bestCounterparty.counterparty, '0xBob');
      assert.equal(margin.bestCounterparty.margin, 100);
      assert.equal(margin.worstCounterparty.counterparty, '0xCharlie');
      assert.equal(margin.worstCounterparty.margin, -400);
    });

    it('returns zeroes for unknown agent', () => {
      const margin = analytics.getMarginAnalysis('0xNobody');
      assert.equal(margin.grossMargin, 0);
      assert.deepEqual(margin.perCounterparty, []);
      assert.equal(margin.bestCounterparty, null);
      assert.equal(margin.worstCounterparty, null);
    });

    it('handles agent with single counterparty (best === worst)', () => {
      analytics.record(spendEntry({ counterparty: '0xBob', amount: 100 }));
      analytics.record(earnEntry({ counterparty: '0xBob', amount: 50 }));

      const margin = analytics.getMarginAnalysis('0xAlice');
      assert.equal(margin.bestCounterparty.counterparty, '0xBob');
      assert.equal(margin.worstCounterparty.counterparty, '0xBob');
      assert.equal(margin.grossMargin, -50);
    });

    it('sorts perCounterparty by margin descending', () => {
      analytics.record(earnEntry({ counterparty: '0xA', amount: 1000 }));
      analytics.record(spendEntry({ counterparty: '0xB', amount: 500 }));
      analytics.record(spendEntry({ counterparty: '0xC', amount: 200 }));

      const margin = analytics.getMarginAnalysis('0xAlice');
      for (let i = 1; i < margin.perCounterparty.length; i++) {
        assert.ok(
          margin.perCounterparty[i].margin <= margin.perCounterparty[i - 1].margin,
          'should be descending by margin',
        );
      }
    });
  });

  // =========================================================================
  // 9. Budget forecast predicts exhaustion date
  // =========================================================================

  describe('getBudgetForecast()', () => {
    it('predicts exhaustion date based on daily spend', () => {
      // Spend $100/day for 5 days
      for (let i = 1; i <= 5; i++) {
        analytics.record(spendEntry({ amount: 100, timestamp: daysAgo(i) }));
      }

      // Budget = $1000, spent $500 in last 5 days, avg = $100/day
      // Spent this month depends on current month boundary, but we have recent spend
      const forecast = analytics.getBudgetForecast('0xAlice', 1000);
      assert.equal(forecast.dailyAvgSpend, 100);
      assert.ok(forecast.remainingBudget <= 1000);
      assert.ok(forecast.daysRemaining >= 1, 'should predict some remaining days');
      assert.ok(forecast.exhaustionDate !== null, 'should have exhaustion date');
    });

    it('returns daysRemaining=0 when budget already exhausted', () => {
      // Spend more than the budget this month
      analytics.record(spendEntry({ amount: 2000, timestamp: new Date().toISOString() }));

      const forecast = analytics.getBudgetForecast('0xAlice', 1000);
      assert.equal(forecast.daysRemaining, 0);
      assert.equal(forecast.remainingBudget, -1000);
      assert.equal(forecast.exhaustionDate, todayKey());
    });

    it('returns null exhaustionDate when no spend history', () => {
      const forecast = analytics.getBudgetForecast('0xAlice', 1000);
      assert.equal(forecast.dailyAvgSpend, 0);
      assert.equal(forecast.daysRemaining, null);
      assert.equal(forecast.exhaustionDate, null);
      assert.equal(forecast.spentThisMonth, 0);
      assert.equal(forecast.remainingBudget, 1000);
    });

    it('throws on non-positive budget', () => {
      assert.throws(
        () => analytics.getBudgetForecast('0xAlice', 0),
        /requires a positive monthlyBudget/,
      );
      assert.throws(
        () => analytics.getBudgetForecast('0xAlice', -500),
        /requires a positive monthlyBudget/,
      );
    });

    it('only counts spend direction in daily average (ignores earn)', () => {
      analytics.record(spendEntry({ amount: 100, timestamp: daysAgo(1) }));
      analytics.record(earnEntry({ amount: 5000, timestamp: daysAgo(1) }));

      const forecast = analytics.getBudgetForecast('0xAlice', 1000);
      assert.equal(forecast.dailyAvgSpend, 100);
    });

    it('uses custom lookbackDays parameter', () => {
      // Old spend outside 7-day window
      analytics.record(spendEntry({ amount: 999, timestamp: daysAgo(20) }));
      // Recent spend within 7-day window
      analytics.record(spendEntry({ amount: 50, timestamp: daysAgo(2) }));

      const forecast = analytics.getBudgetForecast('0xAlice', 1000, 7);
      // Only the recent $50 should be in daily average (old one excluded)
      assert.equal(forecast.dailyAvgSpend, 50);
    });
  });

  // =========================================================================
  // 10. Empty state returns zeros
  // =========================================================================

  describe('empty state', () => {
    it('getAgentSpendSummary returns zeros', () => {
      const summary = analytics.getAgentSpendSummary('0xAnyone');
      assert.equal(summary.totalSpent, 0);
      assert.equal(summary.totalEarned, 0);
      assert.equal(summary.netMargin, 0);
      assert.equal(summary.avgTransactionSize, 0);
      assert.equal(summary.transactionCount, 0);
    });

    it('getCounterpartyBreakdown returns empty array', () => {
      assert.deepEqual(analytics.getCounterpartyBreakdown('0xAnyone'), []);
    });

    it('getOperationBreakdown returns empty array', () => {
      assert.deepEqual(analytics.getOperationBreakdown('0xAnyone'), []);
    });

    it('getDailySpendTrend returns pre-filled zeroes', () => {
      const trend = analytics.getDailySpendTrend('0xAnyone', 7);
      assert.equal(trend.length, 7);
      for (const day of trend) {
        assert.equal(day.spent, 0);
        assert.equal(day.earned, 0);
        assert.equal(day.net, 0);
      }
    });

    it('detectAnomalies returns empty arrays', () => {
      const result = analytics.detectAnomalies('0xAnyone');
      assert.deepEqual(result.transactionAnomalies, []);
      assert.deepEqual(result.dailyAnomalies, []);
    });

    it('getEscrowMetrics returns all zeros', () => {
      const m = analytics.getEscrowMetrics();
      assert.equal(m.totalLocked, 0);
      assert.equal(m.escrowCount, 0);
      assert.equal(m.releaseRate, 0);
      assert.equal(m.refundRate, 0);
    });

    it('getMarginAnalysis returns zero margin and null counterparties', () => {
      const margin = analytics.getMarginAnalysis('0xAnyone');
      assert.equal(margin.grossMargin, 0);
      assert.equal(margin.bestCounterparty, null);
      assert.equal(margin.worstCounterparty, null);
    });

    it('getTopSpenders returns empty array', () => {
      assert.deepEqual(analytics.getTopSpenders(), []);
    });
  });

  // =========================================================================
  // 11. Multiple agents don't leak data between each other
  // =========================================================================

  describe('agent isolation', () => {
    it('entries for one agent are not visible to another', () => {
      analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 500 }));
      analytics.record(earnEntry({ agentAddress: '0xBob', counterparty: '0xAlice', amount: 300 }));

      const aliceSummary = analytics.getAgentSpendSummary('0xAlice');
      assert.equal(aliceSummary.totalSpent, 500);
      assert.equal(aliceSummary.totalEarned, 0);
      assert.equal(aliceSummary.transactionCount, 1);

      const bobSummary = analytics.getAgentSpendSummary('0xBob');
      assert.equal(bobSummary.totalSpent, 0);
      assert.equal(bobSummary.totalEarned, 300);
      assert.equal(bobSummary.transactionCount, 1);
    });

    it('counterparty breakdowns are scoped to agent', () => {
      analytics.record(spendEntry({ agentAddress: '0xAlice', counterparty: '0xCharlie', amount: 100 }));
      analytics.record(spendEntry({ agentAddress: '0xBob', counterparty: '0xCharlie', amount: 999 }));

      const aliceBreakdown = analytics.getCounterpartyBreakdown('0xAlice');
      assert.equal(aliceBreakdown.length, 1);
      assert.equal(aliceBreakdown[0].spent, 100);

      const bobBreakdown = analytics.getCounterpartyBreakdown('0xBob');
      assert.equal(bobBreakdown.length, 1);
      assert.equal(bobBreakdown[0].spent, 999);
    });

    it('operation breakdowns are scoped to agent', () => {
      analytics.record(spendEntry({ agentAddress: '0xAlice', operation: 'quote_payment', amount: 100 }));
      analytics.record(spendEntry({ agentAddress: '0xBob', operation: 'subscription_billing', amount: 200 }));

      const aliceOps = analytics.getOperationBreakdown('0xAlice');
      assert.equal(aliceOps.length, 1);
      assert.equal(aliceOps[0].operation, 'quote_payment');

      const bobOps = analytics.getOperationBreakdown('0xBob');
      assert.equal(bobOps.length, 1);
      assert.equal(bobOps[0].operation, 'subscription_billing');
    });

    it('margin analysis is scoped to agent', () => {
      analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 100 }));
      analytics.record(earnEntry({ agentAddress: '0xBob', counterparty: '0xAlice', amount: 5000 }));

      const aliceMargin = analytics.getMarginAnalysis('0xAlice');
      assert.equal(aliceMargin.grossMargin, -100);

      const bobMargin = analytics.getMarginAnalysis('0xBob');
      assert.equal(bobMargin.grossMargin, 5000);
    });

    it('anomaly detection is scoped to agent', () => {
      // Alice: many small, one large
      for (let i = 0; i < 10; i++) {
        analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 10 }));
      }
      analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 500 }));

      // Bob: only large
      analytics.record(spendEntry({ agentAddress: '0xBob', counterparty: '0xAlice', amount: 1000 }));

      const aliceAnomalies = analytics.detectAnomalies('0xAlice');
      assert.ok(aliceAnomalies.transactionAnomalies.length >= 1, 'Alice should have anomalies');

      const bobAnomalies = analytics.detectAnomalies('0xBob');
      assert.equal(bobAnomalies.transactionAnomalies.length, 0, 'Bob has only 1 tx so no anomaly');
    });
  });

  // =========================================================================
  // 12. getTopSpenders returns correct ranking
  // =========================================================================

  describe('getTopSpenders()', () => {
    it('ranks agents by total spend descending', () => {
      analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 100 }));
      analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 200 }));
      analytics.record(spendEntry({ agentAddress: '0xBob', counterparty: '0xAlice', amount: 500 }));
      analytics.record(spendEntry({ agentAddress: '0xCharlie', counterparty: '0xAlice', amount: 50 }));
      analytics.record(earnEntry({ agentAddress: '0xDave', counterparty: '0xAlice', amount: 9999 }));

      const top = analytics.getTopSpenders();
      assert.equal(top.length, 4);

      assert.equal(top[0].agentAddress, '0xBob');
      assert.equal(top[0].totalSpent, 500);

      assert.equal(top[1].agentAddress, '0xAlice');
      assert.equal(top[1].totalSpent, 300);

      assert.equal(top[2].agentAddress, '0xCharlie');
      assert.equal(top[2].totalSpent, 50);

      // Dave only earns, totalSpent = 0
      assert.equal(top[3].agentAddress, '0xDave');
      assert.equal(top[3].totalSpent, 0);
      assert.equal(top[3].totalEarned, 9999);
    });

    it('respects the limit parameter', () => {
      analytics.record(spendEntry({ agentAddress: '0xA', amount: 100 }));
      analytics.record(spendEntry({ agentAddress: '0xB', counterparty: '0xA', amount: 200 }));
      analytics.record(spendEntry({ agentAddress: '0xC', counterparty: '0xA', amount: 300 }));
      analytics.record(spendEntry({ agentAddress: '0xD', counterparty: '0xA', amount: 400 }));

      const top2 = analytics.getTopSpenders(2);
      assert.equal(top2.length, 2);
      assert.equal(top2[0].agentAddress, '0xD');
      assert.equal(top2[1].agentAddress, '0xC');
    });

    it('returns empty for empty ledger', () => {
      assert.deepEqual(analytics.getTopSpenders(), []);
    });

    it('includes transaction counts', () => {
      analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 10 }));
      analytics.record(spendEntry({ agentAddress: '0xAlice', amount: 20 }));
      analytics.record(earnEntry({ agentAddress: '0xAlice', amount: 5 }));

      const top = analytics.getTopSpenders();
      const alice = top.find((t) => t.agentAddress === '0xAlice');
      assert.equal(alice.transactionCount, 3);
      assert.equal(alice.totalSpent, 30);
      assert.equal(alice.totalEarned, 5);
    });
  });

  // =========================================================================
  // Additional edge cases
  // =========================================================================

  describe('edge cases', () => {
    it('handles all valid operation types', () => {
      const operations = [
        'quote_payment', 'escrow_fund', 'escrow_release', 'escrow_refund',
        'subscription_billing', 'split_payment', 'settlement',
        'platform_fee', 'refund', 'other',
      ];
      for (const op of operations) {
        const stored = analytics.record(spendEntry({ operation: op, amount: 10 }));
        assert.equal(stored.operation, op);
      }
      const breakdown = analytics.getOperationBreakdown('0xAlice');
      assert.equal(breakdown.length, operations.length);
    });

    it('each record() returns a unique id', () => {
      const ids = new Set();
      for (let i = 0; i < 50; i++) {
        const stored = analytics.record(spendEntry({ amount: 1 }));
        assert.ok(!ids.has(stored.id), 'should be unique');
        ids.add(stored.id);
      }
    });

    it('escrow_release without prior fund does not crash', () => {
      analytics.record(earnEntry({
        operation: 'escrow_release',
        sagaId: 'nonexistent',
        amount: 100,
      }));
      const metrics = analytics.getEscrowMetrics();
      assert.equal(metrics.escrowCount, 0); // no fund recorded
    });

    it('escrow_refund without prior fund does not crash', () => {
      analytics.record(earnEntry({
        operation: 'escrow_refund',
        sagaId: 'nonexistent',
        amount: 100,
      }));
      const metrics = analytics.getEscrowMetrics();
      assert.equal(metrics.escrowCount, 0);
    });

    it('escrow_fund without sagaId does not track escrow', () => {
      analytics.record(spendEntry({ operation: 'escrow_fund', amount: 100 }));
      const metrics = analytics.getEscrowMetrics();
      assert.equal(metrics.escrowCount, 0);
    });

    it('multiple analytics instances are independent', () => {
      const a1 = createCostAnalytics();
      const a2 = createCostAnalytics();

      a1.record(spendEntry({ amount: 999 }));
      assert.equal(a1.getAgentSpendSummary('0xAlice').totalSpent, 999);
      assert.equal(a2.getAgentSpendSummary('0xAlice').totalSpent, 0);
    });

    it('large volume does not degrade (smoke test)', () => {
      for (let i = 0; i < 1000; i++) {
        analytics.record(spendEntry({ amount: i, counterparty: `0xCP-${i % 10}` }));
      }
      const summary = analytics.getAgentSpendSummary('0xAlice');
      assert.equal(summary.transactionCount, 1000);
      // sum of 0..999 = 499500
      assert.equal(summary.totalSpent, 499500);
    });
  });
});
