/**
 * A2A Cost Analytics Engine — Agent Economic Intelligence
 *
 * In-memory ledger that tracks spend, earnings, margins, and cost anomalies
 * across agent-to-agent commerce. Every payment, settlement, escrow fund,
 * subscription billing, and split payment is recorded with metadata so agents
 * can make informed economic decisions.
 *
 * @example
 * ```javascript
 * const analytics = createCostAnalytics();
 *
 * analytics.record({
 *   agentAddress: '0xBuyer',
 *   counterparty: '0xSeller',
 *   direction: 'spend',
 *   amount: 100,
 *   operation: 'quote_payment',
 *   sagaId: 'saga-001',
 *   timestamp: new Date().toISOString(),
 * });
 *
 * const summary = analytics.getAgentSpendSummary('0xBuyer');
 * // { totalSpent: 100, totalEarned: 0, netMargin: -100, avgTransactionSize: 100, transactionCount: 1 }
 *
 * const forecast = analytics.getBudgetForecast('0xBuyer', 1000);
 * // { dailyAvgSpend: ..., daysRemaining: ..., exhaustionDate: '...' }
 * ```
 */

import { randomUUID } from 'node:crypto';

/** Valid operation types for ledger entries */
const VALID_OPERATIONS = new Set([
  'quote_payment',
  'escrow_fund',
  'escrow_release',
  'escrow_refund',
  'subscription_billing',
  'split_payment',
  'settlement',
  'platform_fee',
  'refund',
  'other',
]);

/** Valid directions */
const VALID_DIRECTIONS = new Set(['spend', 'earn']);

/**
 * Create a cost analytics engine with an in-memory ledger.
 *
 * @returns {Object} Cost analytics API
 */
export function createCostAnalytics() {
  /**
   * In-memory ledger.
   * Each entry: { id, agentAddress, counterparty, direction, amount, operation, sagaId, timestamp, metadata }
   * @type {Array<Object>}
   */
  const _ledger = [];

  /**
   * Escrow tracking for hold-time and release/refund metrics.
   * Key: sagaId, Value: { fundedAt, releasedAt, refundedAt, amount }
   * @type {Map<string, Object>}
   */
  const _escrowTracker = new Map();

  // -------------------------------------------------------------------------
  // record()
  // -------------------------------------------------------------------------

  /**
   * Record an economic event in the ledger.
   *
   * @param {Object} entry
   * @param {string} entry.agentAddress - The agent recording this event
   * @param {string} entry.counterparty - The other party
   * @param {'spend'|'earn'} entry.direction - Whether this is money out or in
   * @param {number} entry.amount - Positive numeric amount
   * @param {string} entry.operation - Operation type (quote_payment, escrow_fund, etc.)
   * @param {string} [entry.sagaId] - Optional saga/correlation ID
   * @param {string} [entry.timestamp] - ISO 8601 timestamp (defaults to now)
   * @param {Object} [entry.metadata] - Arbitrary metadata
   * @returns {Object} The stored entry with generated id
   */
  function record(entry) {
    if (!entry || typeof entry !== 'object') {
      throw new Error('record() requires an entry object');
    }
    if (!entry.agentAddress || typeof entry.agentAddress !== 'string') {
      throw new Error('record() requires a non-empty agentAddress string');
    }
    if (!entry.counterparty || typeof entry.counterparty !== 'string') {
      throw new Error('record() requires a non-empty counterparty string');
    }
    if (!VALID_DIRECTIONS.has(entry.direction)) {
      throw new Error(`record() direction must be one of: ${[...VALID_DIRECTIONS].join(', ')}`);
    }
    if (typeof entry.amount !== 'number' || entry.amount < 0 || !Number.isFinite(entry.amount)) {
      throw new Error('record() amount must be a non-negative finite number');
    }
    const operation = entry.operation || 'other';
    if (!VALID_OPERATIONS.has(operation)) {
      throw new Error(`record() operation must be one of: ${[...VALID_OPERATIONS].join(', ')}`);
    }

    const stored = {
      id: randomUUID(),
      agentAddress: entry.agentAddress,
      counterparty: entry.counterparty,
      direction: entry.direction,
      amount: entry.amount,
      operation,
      sagaId: entry.sagaId || null,
      timestamp: entry.timestamp || new Date().toISOString(),
      metadata: entry.metadata || {},
    };

    _ledger.push(stored);

    // Track escrow lifecycle
    if (operation === 'escrow_fund' && stored.sagaId) {
      _escrowTracker.set(stored.sagaId, {
        fundedAt: new Date(stored.timestamp).getTime(),
        releasedAt: null,
        refundedAt: null,
        amount: stored.amount,
      });
    } else if (operation === 'escrow_release' && stored.sagaId) {
      const tracker = _escrowTracker.get(stored.sagaId);
      if (tracker) {
        tracker.releasedAt = new Date(stored.timestamp).getTime();
      }
    } else if (operation === 'escrow_refund' && stored.sagaId) {
      const tracker = _escrowTracker.get(stored.sagaId);
      if (tracker) {
        tracker.refundedAt = new Date(stored.timestamp).getTime();
      }
    }

    return stored;
  }

  // -------------------------------------------------------------------------
  // Helper: get entries for a specific agent
  // -------------------------------------------------------------------------

  /**
   * Get all ledger entries for an agent.
   * @param {string} agentAddress
   * @returns {Array<Object>}
   */
  function _getAgentEntries(agentAddress) {
    return _ledger.filter((e) => e.agentAddress === agentAddress);
  }

  // -------------------------------------------------------------------------
  // getAgentSpendSummary()
  // -------------------------------------------------------------------------

  /**
   * Get spend summary for an agent.
   *
   * @param {string} agentAddress
   * @returns {{ totalSpent: number, totalEarned: number, netMargin: number, avgTransactionSize: number, transactionCount: number }}
   */
  function getAgentSpendSummary(agentAddress) {
    const entries = _getAgentEntries(agentAddress);

    if (entries.length === 0) {
      return {
        totalSpent: 0,
        totalEarned: 0,
        netMargin: 0,
        avgTransactionSize: 0,
        transactionCount: 0,
      };
    }

    let totalSpent = 0;
    let totalEarned = 0;

    for (const e of entries) {
      if (e.direction === 'spend') {
        totalSpent += e.amount;
      } else {
        totalEarned += e.amount;
      }
    }

    const netMargin = totalEarned - totalSpent;
    const totalVolume = totalSpent + totalEarned;
    const avgTransactionSize = totalVolume / entries.length;

    return {
      totalSpent,
      totalEarned,
      netMargin,
      avgTransactionSize,
      transactionCount: entries.length,
    };
  }

  // -------------------------------------------------------------------------
  // getCounterpartyBreakdown()
  // -------------------------------------------------------------------------

  /**
   * Get per-counterparty breakdown for an agent, ranked by total volume.
   *
   * @param {string} agentAddress
   * @returns {Array<{ counterparty: string, spent: number, earned: number, transactionCount: number, volume: number }>}
   */
  function getCounterpartyBreakdown(agentAddress) {
    const entries = _getAgentEntries(agentAddress);
    /** @type {Map<string, { spent: number, earned: number, transactionCount: number }>} */
    const byCounterparty = new Map();

    for (const e of entries) {
      let rec = byCounterparty.get(e.counterparty);
      if (!rec) {
        rec = { spent: 0, earned: 0, transactionCount: 0 };
        byCounterparty.set(e.counterparty, rec);
      }
      rec.transactionCount += 1;
      if (e.direction === 'spend') {
        rec.spent += e.amount;
      } else {
        rec.earned += e.amount;
      }
    }

    const result = [];
    for (const [counterparty, rec] of byCounterparty) {
      result.push({
        counterparty,
        spent: rec.spent,
        earned: rec.earned,
        transactionCount: rec.transactionCount,
        volume: rec.spent + rec.earned,
      });
    }

    // Rank by volume descending
    result.sort((a, b) => b.volume - a.volume);

    return result;
  }

  // -------------------------------------------------------------------------
  // getOperationBreakdown()
  // -------------------------------------------------------------------------

  /**
   * Get per-operation-type spend breakdown for an agent.
   *
   * @param {string} agentAddress
   * @returns {Array<{ operation: string, totalAmount: number, transactionCount: number, percentOfTotal: number }>}
   */
  function getOperationBreakdown(agentAddress) {
    const entries = _getAgentEntries(agentAddress);
    /** @type {Map<string, { totalAmount: number, transactionCount: number }>} */
    const byOp = new Map();
    let grandTotal = 0;

    for (const e of entries) {
      let rec = byOp.get(e.operation);
      if (!rec) {
        rec = { totalAmount: 0, transactionCount: 0 };
        byOp.set(e.operation, rec);
      }
      rec.totalAmount += e.amount;
      rec.transactionCount += 1;
      grandTotal += e.amount;
    }

    const result = [];
    for (const [operation, rec] of byOp) {
      result.push({
        operation,
        totalAmount: rec.totalAmount,
        transactionCount: rec.transactionCount,
        percentOfTotal: grandTotal > 0 ? (rec.totalAmount / grandTotal) * 100 : 0,
      });
    }

    // Rank by totalAmount descending
    result.sort((a, b) => b.totalAmount - a.totalAmount);

    return result;
  }

  // -------------------------------------------------------------------------
  // getDailySpendTrend()
  // -------------------------------------------------------------------------

  /**
   * Get daily spend totals for the last N days.
   *
   * @param {string} agentAddress
   * @param {number} [days=30] - Number of days to look back
   * @returns {Array<{ date: string, spent: number, earned: number, net: number }>}
   */
  function getDailySpendTrend(agentAddress, days = 30) {
    const entries = _getAgentEntries(agentAddress);
    const now = new Date();
    const cutoff = new Date(now);
    cutoff.setDate(cutoff.getDate() - days);
    cutoff.setHours(0, 0, 0, 0);

    // Build a map of date -> { spent, earned }
    /** @type {Map<string, { spent: number, earned: number }>} */
    const dailyMap = new Map();

    // Pre-fill all days so the output is dense (no gaps)
    for (let d = 0; d < days; d++) {
      const date = new Date(now);
      date.setDate(date.getDate() - d);
      const key = _dateKey(date);
      dailyMap.set(key, { spent: 0, earned: 0 });
    }

    for (const e of entries) {
      const entryDate = new Date(e.timestamp);
      if (entryDate < cutoff) continue;
      const key = _dateKey(entryDate);
      let rec = dailyMap.get(key);
      if (!rec) {
        rec = { spent: 0, earned: 0 };
        dailyMap.set(key, rec);
      }
      if (e.direction === 'spend') {
        rec.spent += e.amount;
      } else {
        rec.earned += e.amount;
      }
    }

    // Sort by date ascending
    const sorted = [...dailyMap.entries()].sort((a, b) => a[0].localeCompare(b[0]));

    return sorted.map(([date, rec]) => ({
      date,
      spent: rec.spent,
      earned: rec.earned,
      net: rec.earned - rec.spent,
    }));
  }

  /**
   * Convert a Date to a YYYY-MM-DD string.
   * @param {Date} date
   * @returns {string}
   */
  function _dateKey(date) {
    const y = date.getFullYear();
    const m = String(date.getMonth() + 1).padStart(2, '0');
    const d = String(date.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
  }

  // -------------------------------------------------------------------------
  // detectAnomalies()
  // -------------------------------------------------------------------------

  /**
   * Detect cost anomalies for an agent.
   *
   * Flags:
   *   - Individual transactions > 3x the agent's average transaction amount
   *   - Days where total spend > 2x the agent's daily average spend
   *
   * @param {string} agentAddress
   * @returns {{ transactionAnomalies: Array<Object>, dailyAnomalies: Array<Object> }}
   */
  function detectAnomalies(agentAddress) {
    const entries = _getAgentEntries(agentAddress);

    if (entries.length === 0) {
      return { transactionAnomalies: [], dailyAnomalies: [] };
    }

    // --- Transaction-level anomalies ---
    const amounts = entries.map((e) => e.amount);
    const avgAmount = amounts.reduce((sum, a) => sum + a, 0) / amounts.length;
    const txThreshold = avgAmount * 3;

    const transactionAnomalies = entries
      .filter((e) => e.amount > txThreshold)
      .map((e) => ({
        id: e.id,
        amount: e.amount,
        operation: e.operation,
        counterparty: e.counterparty,
        timestamp: e.timestamp,
        ratio: avgAmount > 0 ? e.amount / avgAmount : 0,
        threshold: txThreshold,
      }));

    // --- Daily spend anomalies ---
    /** @type {Map<string, number>} */
    const dailySpend = new Map();
    for (const e of entries) {
      if (e.direction !== 'spend') continue;
      const key = _dateKey(new Date(e.timestamp));
      dailySpend.set(key, (dailySpend.get(key) || 0) + e.amount);
    }

    const dailyValues = [...dailySpend.values()];
    const dailyAvg =
      dailyValues.length > 0 ? dailyValues.reduce((s, v) => s + v, 0) / dailyValues.length : 0;
    const dailyThreshold = dailyAvg * 2;

    const dailyAnomalies = [];
    for (const [date, total] of dailySpend) {
      if (total > dailyThreshold) {
        dailyAnomalies.push({
          date,
          totalSpend: total,
          dailyAverage: dailyAvg,
          ratio: dailyAvg > 0 ? total / dailyAvg : 0,
          threshold: dailyThreshold,
        });
      }
    }

    return { transactionAnomalies, dailyAnomalies };
  }

  // -------------------------------------------------------------------------
  // getEscrowMetrics()
  // -------------------------------------------------------------------------

  /**
   * Get escrow metrics across all agents.
   *
   * @returns {{ totalLocked: number, totalReleased: number, totalRefunded: number, avgHoldTimeMs: number, releaseRate: number, refundRate: number, escrowCount: number }}
   */
  function getEscrowMetrics() {
    if (_escrowTracker.size === 0) {
      return {
        totalLocked: 0,
        totalReleased: 0,
        totalRefunded: 0,
        avgHoldTimeMs: 0,
        releaseRate: 0,
        refundRate: 0,
        escrowCount: 0,
      };
    }

    let totalLocked = 0;
    let totalReleased = 0;
    let totalRefunded = 0;
    let holdTimeSum = 0;
    let holdTimeCount = 0;
    let releasedCount = 0;
    let refundedCount = 0;

    for (const tracker of _escrowTracker.values()) {
      totalLocked += tracker.amount;

      if (tracker.releasedAt) {
        totalReleased += tracker.amount;
        releasedCount += 1;
        holdTimeSum += tracker.releasedAt - tracker.fundedAt;
        holdTimeCount += 1;
      }

      if (tracker.refundedAt) {
        totalRefunded += tracker.amount;
        refundedCount += 1;
        // If not already counted via release, count hold time to refund
        if (!tracker.releasedAt) {
          holdTimeSum += tracker.refundedAt - tracker.fundedAt;
          holdTimeCount += 1;
        }
      }
    }

    const resolved = releasedCount + refundedCount;
    const avgHoldTimeMs = holdTimeCount > 0 ? holdTimeSum / holdTimeCount : 0;
    const releaseRate = resolved > 0 ? releasedCount / resolved : 0;
    const refundRate = resolved > 0 ? refundedCount / resolved : 0;

    return {
      totalLocked,
      totalReleased,
      totalRefunded,
      avgHoldTimeMs,
      releaseRate,
      refundRate,
      escrowCount: _escrowTracker.size,
    };
  }

  // -------------------------------------------------------------------------
  // getMarginAnalysis()
  // -------------------------------------------------------------------------

  /**
   * Get margin analysis for an agent.
   *
   * @param {string} agentAddress
   * @returns {{ grossMargin: number, perCounterparty: Array<{ counterparty: string, margin: number, spent: number, earned: number }>, bestCounterparty: Object|null, worstCounterparty: Object|null }}
   */
  function getMarginAnalysis(agentAddress) {
    const entries = _getAgentEntries(agentAddress);

    if (entries.length === 0) {
      return {
        grossMargin: 0,
        perCounterparty: [],
        bestCounterparty: null,
        worstCounterparty: null,
      };
    }

    let totalSpent = 0;
    let totalEarned = 0;

    /** @type {Map<string, { spent: number, earned: number }>} */
    const cpMap = new Map();

    for (const e of entries) {
      if (e.direction === 'spend') {
        totalSpent += e.amount;
      } else {
        totalEarned += e.amount;
      }

      let rec = cpMap.get(e.counterparty);
      if (!rec) {
        rec = { spent: 0, earned: 0 };
        cpMap.set(e.counterparty, rec);
      }
      if (e.direction === 'spend') {
        rec.spent += e.amount;
      } else {
        rec.earned += e.amount;
      }
    }

    const grossMargin = totalEarned - totalSpent;

    const perCounterparty = [];
    for (const [counterparty, rec] of cpMap) {
      perCounterparty.push({
        counterparty,
        margin: rec.earned - rec.spent,
        spent: rec.spent,
        earned: rec.earned,
      });
    }

    // Sort by margin descending
    perCounterparty.sort((a, b) => b.margin - a.margin);

    const bestCounterparty = perCounterparty.length > 0 ? perCounterparty[0] : null;
    const worstCounterparty =
      perCounterparty.length > 0 ? perCounterparty[perCounterparty.length - 1] : null;

    return {
      grossMargin,
      perCounterparty,
      bestCounterparty,
      worstCounterparty,
    };
  }

  // -------------------------------------------------------------------------
  // getBudgetForecast()
  // -------------------------------------------------------------------------

  /**
   * Forecast when a monthly budget will be exhausted based on daily spend trend.
   *
   * @param {string} agentAddress
   * @param {number} monthlyBudget - The agent's monthly budget
   * @param {number} [lookbackDays=30] - Days to use for trend calculation
   * @returns {{ dailyAvgSpend: number, daysRemaining: number|null, exhaustionDate: string|null, spentThisMonth: number, remainingBudget: number }}
   */
  function getBudgetForecast(agentAddress, monthlyBudget, lookbackDays = 30) {
    if (typeof monthlyBudget !== 'number' || monthlyBudget <= 0) {
      throw new Error('getBudgetForecast() requires a positive monthlyBudget');
    }

    const entries = _getAgentEntries(agentAddress);
    const now = new Date();

    // Compute spend this calendar month
    const monthStart = new Date(now.getFullYear(), now.getMonth(), 1);
    let spentThisMonth = 0;
    for (const e of entries) {
      if (e.direction === 'spend' && new Date(e.timestamp) >= monthStart) {
        spentThisMonth += e.amount;
      }
    }

    // Compute daily average from lookback window
    const cutoff = new Date(now);
    cutoff.setDate(cutoff.getDate() - lookbackDays);

    /** @type {Map<string, number>} */
    const dailySpend = new Map();
    for (const e of entries) {
      if (e.direction !== 'spend') continue;
      const eDate = new Date(e.timestamp);
      if (eDate < cutoff) continue;
      const key = _dateKey(eDate);
      dailySpend.set(key, (dailySpend.get(key) || 0) + e.amount);
    }

    const dailyValues = [...dailySpend.values()];
    const dailyAvgSpend =
      dailyValues.length > 0 ? dailyValues.reduce((s, v) => s + v, 0) / dailyValues.length : 0;

    const remainingBudget = monthlyBudget - spentThisMonth;

    let daysRemaining = null;
    let exhaustionDate = null;

    if (dailyAvgSpend > 0 && remainingBudget > 0) {
      daysRemaining = Math.ceil(remainingBudget / dailyAvgSpend);
      const exhaust = new Date(now);
      exhaust.setDate(exhaust.getDate() + daysRemaining);
      exhaustionDate = exhaust.toISOString().split('T')[0];
    } else if (remainingBudget <= 0) {
      daysRemaining = 0;
      exhaustionDate = _dateKey(now);
    }

    return {
      dailyAvgSpend,
      daysRemaining,
      exhaustionDate,
      spentThisMonth,
      remainingBudget,
    };
  }

  // -------------------------------------------------------------------------
  // getTopSpenders()
  // -------------------------------------------------------------------------

  /**
   * Get top-spending agents across the entire ledger.
   *
   * @param {number} [limit=10] - Max results
   * @returns {Array<{ agentAddress: string, totalSpent: number, totalEarned: number, transactionCount: number }>}
   */
  function getTopSpenders(limit = 10) {
    /** @type {Map<string, { totalSpent: number, totalEarned: number, transactionCount: number }>} */
    const agentMap = new Map();

    for (const e of _ledger) {
      let rec = agentMap.get(e.agentAddress);
      if (!rec) {
        rec = { totalSpent: 0, totalEarned: 0, transactionCount: 0 };
        agentMap.set(e.agentAddress, rec);
      }
      rec.transactionCount += 1;
      if (e.direction === 'spend') {
        rec.totalSpent += e.amount;
      } else {
        rec.totalEarned += e.amount;
      }
    }

    const result = [];
    for (const [agentAddress, rec] of agentMap) {
      result.push({
        agentAddress,
        totalSpent: rec.totalSpent,
        totalEarned: rec.totalEarned,
        transactionCount: rec.transactionCount,
      });
    }

    result.sort((a, b) => b.totalSpent - a.totalSpent);

    return result.slice(0, limit);
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  return {
    record,
    getAgentSpendSummary,
    getCounterpartyBreakdown,
    getOperationBreakdown,
    getDailySpendTrend,
    detectAnomalies,
    getEscrowMetrics,
    getMarginAnalysis,
    getBudgetForecast,
    getTopSpenders,
  };
}
