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

function parseMetadataObject(value) {
  if (!value) return null;
  if (typeof value === 'object') return value;
  if (typeof value !== 'string') return null;
  try {
    const parsed = JSON.parse(value);
    return parsed && typeof parsed === 'object' ? parsed : null;
  } catch {
    return null;
  }
}

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

  function _getEntryMetadata(entry) {
    return parseMetadataObject(entry?.metadata);
  }

  function _getEntryAsset(entry) {
    return entry?.asset || _getEntryMetadata(entry)?.asset || 'UNKNOWN';
  }

  function _getEntryNetwork(entry) {
    const metadata = _getEntryMetadata(entry);
    return (
      entry?.network || metadata?.network || metadata?.chain_id || metadata?.chainId || 'unknown'
    );
  }

  function _matchesEntryFilter(entry, filter = {}) {
    if (filter.asset && _getEntryAsset(entry) !== filter.asset) {
      return false;
    }
    if (filter.network && _getEntryNetwork(entry) !== filter.network) {
      return false;
    }
    return true;
  }

  function _createLeafSummaryBucket() {
    return {
      totalSpent: 0,
      totalEarned: 0,
      netMargin: 0,
      totalVolume: 0,
      transactionCount: 0,
      avgTransactionSize: 0,
    };
  }

  function _createBreakdownBucket() {
    return {
      ..._createLeafSummaryBucket(),
      networks: {},
    };
  }

  function _applyEntryToSummaryBucket(bucket, entry) {
    if (entry.direction === 'spend') {
      bucket.totalSpent += entry.amount;
    } else {
      bucket.totalEarned += entry.amount;
    }
    bucket.totalVolume += entry.amount;
    bucket.transactionCount += 1;
  }

  function _finalizeLeafSummaryBucket(bucket) {
    bucket.netMargin = bucket.totalEarned - bucket.totalSpent;
    bucket.avgTransactionSize =
      bucket.transactionCount > 0 ? bucket.totalVolume / bucket.transactionCount : 0;
    return bucket;
  }

  function _finalizeBreakdownByAsset(breakdownByAsset) {
    const assets = Object.keys(breakdownByAsset).sort();
    for (const asset of assets) {
      const assetBucket = breakdownByAsset[asset];
      _finalizeLeafSummaryBucket(assetBucket);

      const orderedNetworks = Object.keys(assetBucket.networks).sort();
      const nextNetworks = {};
      for (const network of orderedNetworks) {
        nextNetworks[network] = _finalizeLeafSummaryBucket(assetBucket.networks[network]);
      }
      assetBucket.networks = nextNetworks;
    }

    return assets;
  }

  function _buildEntryBreakdown(entries) {
    const breakdownByAsset = {};
    const overall = _createLeafSummaryBucket();

    for (const entry of entries) {
      const asset = _getEntryAsset(entry);
      const network = _getEntryNetwork(entry);
      const assetBucket = breakdownByAsset[asset] || _createBreakdownBucket();
      const networkBucket = assetBucket.networks[network] || _createLeafSummaryBucket();

      _applyEntryToSummaryBucket(overall, entry);
      _applyEntryToSummaryBucket(assetBucket, entry);
      _applyEntryToSummaryBucket(networkBucket, entry);

      assetBucket.networks[network] = networkBucket;
      breakdownByAsset[asset] = assetBucket;
    }

    const assets = _finalizeBreakdownByAsset(breakdownByAsset);
    _finalizeLeafSummaryBucket(overall);

    return {
      breakdownByAsset,
      assets,
      ...overall,
    };
  }

  function _getAggregateDescriptor(assets, filter = {}) {
    const aggregateAsset = filter.asset || (assets.length === 1 ? assets[0] : null);
    const aggregateTotalsMeaningful = Boolean(aggregateAsset) || assets.length <= 1;
    return {
      aggregateAsset,
      aggregateTotalsMeaningful,
    };
  }

  function _createAnomalyBucket() {
    return {
      transactionCount: 0,
      spendTransactionCount: 0,
      avgTransactionAmount: 0,
      transactionThreshold: 0,
      transactionAnomalies: [],
      dailyAverageSpend: 0,
      dailySpendThreshold: 0,
      dailyAnomalies: [],
    };
  }

  function _computeAnomalySummary(entries) {
    if (entries.length === 0) {
      return _createAnomalyBucket();
    }

    const amounts = entries.map((entry) => entry.amount);
    const avgTransactionAmount = amounts.reduce((sum, amount) => sum + amount, 0) / amounts.length;
    const transactionThreshold = avgTransactionAmount * 3;

    const transactionAnomalies = entries
      .filter((entry) => entry.amount > transactionThreshold)
      .map((entry) => ({
        id: entry.id,
        amount: entry.amount,
        operation: entry.operation,
        counterparty: entry.counterparty,
        timestamp: entry.timestamp,
        ratio: avgTransactionAmount > 0 ? entry.amount / avgTransactionAmount : 0,
        threshold: transactionThreshold,
      }));

    const dailySpend = new Map();
    for (const entry of entries) {
      if (entry.direction !== 'spend') continue;
      const key = _dateKey(new Date(entry.timestamp));
      dailySpend.set(key, (dailySpend.get(key) || 0) + entry.amount);
    }

    const dailyValues = [...dailySpend.values()];
    const dailyAverageSpend =
      dailyValues.length > 0
        ? dailyValues.reduce((sum, value) => sum + value, 0) / dailyValues.length
        : 0;
    const dailySpendThreshold = dailyAverageSpend * 2;

    const dailyAnomalies = [];
    for (const [date, totalSpend] of dailySpend) {
      if (totalSpend > dailySpendThreshold) {
        dailyAnomalies.push({
          date,
          totalSpend,
          dailyAverage: dailyAverageSpend,
          ratio: dailyAverageSpend > 0 ? totalSpend / dailyAverageSpend : 0,
          threshold: dailySpendThreshold,
        });
      }
    }

    return {
      transactionCount: entries.length,
      spendTransactionCount: entries.filter((entry) => entry.direction === 'spend').length,
      avgTransactionAmount,
      transactionThreshold,
      transactionAnomalies,
      dailyAverageSpend,
      dailySpendThreshold,
      dailyAnomalies,
    };
  }

  function _buildAnomalyBreakdown(entries) {
    const grouped = new Map();

    for (const entry of entries) {
      const asset = _getEntryAsset(entry);
      const network = _getEntryNetwork(entry);
      const assetGroup = grouped.get(asset) || { entries: [], networks: new Map() };
      const networkEntries = assetGroup.networks.get(network) || [];
      assetGroup.entries.push(entry);
      networkEntries.push(entry);
      assetGroup.networks.set(network, networkEntries);
      grouped.set(asset, assetGroup);
    }

    const breakdownByAsset = {};
    const transactionAnomalies = [];
    const dailyAnomalies = [];

    for (const asset of [...grouped.keys()].sort()) {
      const assetGroup = grouped.get(asset);
      const assetSummary = _computeAnomalySummary(assetGroup.entries);
      const assetBucket = {
        ...assetSummary,
        transactionAnomalies: assetSummary.transactionAnomalies.map((entry) => ({
          ...entry,
          asset,
        })),
        dailyAnomalies: assetSummary.dailyAnomalies.map((entry) => ({ ...entry, asset })),
        networks: {},
      };

      for (const network of [...assetGroup.networks.keys()].sort()) {
        const networkSummary = _computeAnomalySummary(assetGroup.networks.get(network));
        assetBucket.networks[network] = {
          ...networkSummary,
          transactionAnomalies: networkSummary.transactionAnomalies.map((entry) => ({
            ...entry,
            asset,
            network,
          })),
          dailyAnomalies: networkSummary.dailyAnomalies.map((entry) => ({
            ...entry,
            asset,
            network,
          })),
        };
        transactionAnomalies.push(...assetBucket.networks[network].transactionAnomalies);
        dailyAnomalies.push(...assetBucket.networks[network].dailyAnomalies);
      }

      breakdownByAsset[asset] = assetBucket;
    }

    return {
      breakdownByAsset,
      assets: Object.keys(breakdownByAsset),
      transactionAnomalies,
      dailyAnomalies,
    };
  }

  function _createBudgetLeafBucket() {
    return {
      spentThisMonth: 0,
      dailyAvgSpend: 0,
      _dailySpend: new Map(),
    };
  }

  function _appendBudgetSpend(bucket, dateKey, amount) {
    bucket._dailySpend.set(dateKey, (bucket._dailySpend.get(dateKey) || 0) + amount);
  }

  function _finalizeBudgetLeafBucket(bucket) {
    const dailyValues = [...bucket._dailySpend.values()];
    bucket.dailyAvgSpend =
      dailyValues.length > 0
        ? dailyValues.reduce((sum, value) => sum + value, 0) / dailyValues.length
        : 0;
    delete bucket._dailySpend;
    return bucket;
  }

  function _buildBudgetBreakdown(entries, lookbackDays = 30, now = new Date()) {
    const monthStart = new Date(now.getFullYear(), now.getMonth(), 1);
    const cutoff = new Date(now);
    cutoff.setDate(cutoff.getDate() - lookbackDays);

    const overall = _createBudgetLeafBucket();
    const breakdownByAsset = {};

    for (const entry of entries) {
      if (entry.direction !== 'spend') continue;

      const asset = _getEntryAsset(entry);
      const network = _getEntryNetwork(entry);
      const entryDate = new Date(entry.timestamp);
      const assetBucket = breakdownByAsset[asset] || {
        ..._createBudgetLeafBucket(),
        networks: {},
      };
      const networkBucket = assetBucket.networks[network] || _createBudgetLeafBucket();

      if (entryDate >= monthStart) {
        overall.spentThisMonth += entry.amount;
        assetBucket.spentThisMonth += entry.amount;
        networkBucket.spentThisMonth += entry.amount;
      }

      if (entryDate >= cutoff) {
        const key = _dateKey(entryDate);
        _appendBudgetSpend(overall, key, entry.amount);
        _appendBudgetSpend(assetBucket, key, entry.amount);
        _appendBudgetSpend(networkBucket, key, entry.amount);
      }

      assetBucket.networks[network] = networkBucket;
      breakdownByAsset[asset] = assetBucket;
    }

    const assets = Object.keys(breakdownByAsset).sort();
    for (const asset of assets) {
      const assetBucket = breakdownByAsset[asset];
      const orderedNetworks = Object.keys(assetBucket.networks).sort();
      const nextNetworks = {};

      for (const network of orderedNetworks) {
        nextNetworks[network] = _finalizeBudgetLeafBucket(assetBucket.networks[network]);
      }

      assetBucket.networks = nextNetworks;
      _finalizeBudgetLeafBucket(assetBucket);
    }

    _finalizeBudgetLeafBucket(overall);

    return {
      breakdownByAsset,
      assets,
      spentThisMonth: overall.spentThisMonth,
      dailyAvgSpend: overall.dailyAvgSpend,
    };
  }

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

    const metadataObject = parseMetadataObject(entry.metadata);

    const stored = {
      id: randomUUID(),
      agentAddress: entry.agentAddress,
      counterparty: entry.counterparty,
      direction: entry.direction,
      amount: entry.amount,
      asset: entry.asset || metadataObject?.asset || null,
      network:
        entry.network ||
        metadataObject?.network ||
        metadataObject?.chain_id ||
        metadataObject?.chainId ||
        null,
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
  function _getAgentEntries(agentAddress, filter = {}) {
    return _ledger.filter((e) => e.agentAddress === agentAddress && _matchesEntryFilter(e, filter));
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
  function getAgentSpendSummary(agentAddress, filter = {}) {
    const entries = _getAgentEntries(agentAddress, filter);
    const summary = _buildEntryBreakdown(entries);
    const aggregate = _getAggregateDescriptor(summary.assets, filter);

    return {
      totalSpent: summary.totalSpent,
      totalEarned: summary.totalEarned,
      netMargin: summary.netMargin,
      avgTransactionSize: summary.avgTransactionSize,
      transactionCount: summary.transactionCount,
      asset: filter.asset || null,
      network: filter.network || null,
      assets: summary.assets,
      aggregateAsset: aggregate.aggregateAsset,
      aggregateTotalsMeaningful: aggregate.aggregateTotalsMeaningful,
      netMarginMeaningful: aggregate.aggregateTotalsMeaningful,
      avgTransactionSizeMeaningful: aggregate.aggregateTotalsMeaningful,
      breakdownByAsset: summary.breakdownByAsset,
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
  function getCounterpartyBreakdown(agentAddress, filter = {}) {
    const entries = _getAgentEntries(agentAddress, filter);
    /** @type {Map<string, Array<Object>>} */
    const byCounterparty = new Map();

    for (const entry of entries) {
      const current = byCounterparty.get(entry.counterparty) || [];
      current.push(entry);
      byCounterparty.set(entry.counterparty, current);
    }

    const result = [];
    for (const [counterparty, counterpartyEntries] of byCounterparty) {
      const summary = _buildEntryBreakdown(counterpartyEntries);
      const aggregate = _getAggregateDescriptor(summary.assets, filter);
      result.push({
        counterparty,
        spent: summary.totalSpent,
        earned: summary.totalEarned,
        netMargin: summary.netMargin,
        transactionCount: summary.transactionCount,
        volume: summary.totalVolume,
        asset: filter.asset || null,
        network: filter.network || null,
        assets: summary.assets,
        aggregateAsset: aggregate.aggregateAsset,
        aggregateVolumeMeaningful: aggregate.aggregateTotalsMeaningful,
        marginMeaningful: aggregate.aggregateTotalsMeaningful,
        breakdownByAsset: summary.breakdownByAsset,
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
  function getOperationBreakdown(agentAddress, filter = {}) {
    const entries = _getAgentEntries(agentAddress, filter);
    const overallSummary = _buildEntryBreakdown(entries);
    const overallAggregate = _getAggregateDescriptor(overallSummary.assets, filter);
    /** @type {Map<string, Array<Object>>} */
    const byOp = new Map();

    for (const e of entries) {
      const current = byOp.get(e.operation) || [];
      current.push(e);
      byOp.set(e.operation, current);
    }

    const result = [];
    for (const [operation, operationEntries] of byOp) {
      const summary = _buildEntryBreakdown(operationEntries);
      const aggregate = _getAggregateDescriptor(summary.assets, filter);
      result.push({
        operation,
        totalAmount: summary.totalVolume,
        transactionCount: summary.transactionCount,
        percentOfTotal:
          overallSummary.totalVolume > 0
            ? (summary.totalVolume / overallSummary.totalVolume) * 100
            : 0,
        asset: filter.asset || null,
        network: filter.network || null,
        assets: summary.assets,
        aggregateAsset: aggregate.aggregateAsset,
        aggregateTotalsMeaningful: aggregate.aggregateTotalsMeaningful,
        totalAmountMeaningful: aggregate.aggregateTotalsMeaningful,
        percentOfTotalMeaningful: overallAggregate.aggregateTotalsMeaningful,
        breakdownByAsset: summary.breakdownByAsset,
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
  function getDailySpendTrend(agentAddress, days = 30, filter = {}) {
    if (days && typeof days === 'object' && !Array.isArray(days)) {
      filter = days;
      days = 30;
    }
    const entries = _getAgentEntries(agentAddress, filter);
    const now = new Date();
    const cutoff = new Date(now);
    cutoff.setDate(cutoff.getDate() - days);
    cutoff.setHours(0, 0, 0, 0);

    // Build a map of date -> entries for that day
    /** @type {Map<string, Array<Object>>} */
    const dailyMap = new Map();

    // Pre-fill all days so the output is dense (no gaps)
    for (let d = 0; d < days; d++) {
      const date = new Date(now);
      date.setDate(date.getDate() - d);
      const key = _dateKey(date);
      dailyMap.set(key, []);
    }

    for (const e of entries) {
      const entryDate = new Date(e.timestamp);
      if (entryDate < cutoff) continue;
      const key = _dateKey(entryDate);
      const rec = dailyMap.get(key) || [];
      rec.push(e);
      dailyMap.set(key, rec);
    }

    // Sort by date ascending
    const sorted = [...dailyMap.entries()].sort((a, b) => a[0].localeCompare(b[0]));

    return sorted.map(([date, dayEntries]) => {
      const summary = _buildEntryBreakdown(dayEntries);
      const aggregate = _getAggregateDescriptor(summary.assets, filter);
      return {
        date,
        spent: summary.totalSpent,
        earned: summary.totalEarned,
        net: summary.netMargin,
        asset: filter.asset || null,
        network: filter.network || null,
        assets: summary.assets,
        aggregateAsset: aggregate.aggregateAsset,
        aggregateTotalsMeaningful: aggregate.aggregateTotalsMeaningful,
        netMeaningful: aggregate.aggregateTotalsMeaningful,
        breakdownByAsset: summary.breakdownByAsset,
      };
    });
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
  function detectAnomalies(agentAddress, filter = {}) {
    const entries = _getAgentEntries(agentAddress, filter);
    const breakdown = _buildAnomalyBreakdown(entries);
    const aggregate = _getAggregateDescriptor(breakdown.assets, filter);

    return {
      transactionAnomalies: breakdown.transactionAnomalies,
      dailyAnomalies: breakdown.dailyAnomalies,
      asset: filter.asset || null,
      network: filter.network || null,
      assets: breakdown.assets,
      aggregateAsset: aggregate.aggregateAsset,
      aggregateAnomaliesMeaningful: aggregate.aggregateTotalsMeaningful,
      breakdownByAsset: breakdown.breakdownByAsset,
    };
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
  function getMarginAnalysis(agentAddress, filter = {}) {
    const entries = _getAgentEntries(agentAddress, filter);
    const summary = _buildEntryBreakdown(entries);
    const aggregate = _getAggregateDescriptor(summary.assets, filter);

    const perCounterparty = getCounterpartyBreakdown(agentAddress, filter).map((entry) => ({
      counterparty: entry.counterparty,
      margin: entry.earned - entry.spent,
      spent: entry.spent,
      earned: entry.earned,
      asset: entry.asset,
      network: entry.network,
      assets: entry.assets,
      aggregateAsset: entry.aggregateAsset,
      aggregateMarginMeaningful: entry.marginMeaningful,
      breakdownByAsset: entry.breakdownByAsset,
    }));

    // Sort by margin descending
    perCounterparty.sort((a, b) => b.margin - a.margin);

    const bestCounterparty = perCounterparty.length > 0 ? perCounterparty[0] : null;
    const worstCounterparty =
      perCounterparty.length > 0 ? perCounterparty[perCounterparty.length - 1] : null;

    return {
      grossMargin: summary.netMargin,
      asset: filter.asset || null,
      network: filter.network || null,
      assets: summary.assets,
      aggregateAsset: aggregate.aggregateAsset,
      grossMarginMeaningful: aggregate.aggregateTotalsMeaningful,
      breakdownByAsset: summary.breakdownByAsset,
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
   * @param {Object} [filter] - Optional asset/network filter
   * @param {Date|string} [now] - Reference clock (defaults to the current time)
   * @returns {{ dailyAvgSpend: number, daysRemaining: number|null, exhaustionDate: string|null, spentThisMonth: number, remainingBudget: number }}
   */
  function getBudgetForecast(agentAddress, monthlyBudget, lookbackDays = 30, filter = {}, now) {
    if (lookbackDays && typeof lookbackDays === 'object' && !Array.isArray(lookbackDays)) {
      filter = lookbackDays;
      lookbackDays = 30;
    }
    if (typeof monthlyBudget !== 'number' || monthlyBudget <= 0) {
      throw new Error('getBudgetForecast() requires a positive monthlyBudget');
    }

    const entries = _getAgentEntries(agentAddress, filter);
    // `now` is injectable so month-bucketed forecasts can be tested
    // deterministically (spent-this-month depends on the calendar).
    now = now ? new Date(now) : new Date();
    const budgetSummary = _buildBudgetBreakdown(entries, lookbackDays, now);
    const aggregate = _getAggregateDescriptor(budgetSummary.assets, filter);
    const budgetForecastMeaningful = aggregate.aggregateTotalsMeaningful;

    let dailyAvgSpend = null;
    let daysRemaining = null;
    let exhaustionDate = null;
    let spentThisMonth = null;
    let remainingBudget = null;

    if (budgetForecastMeaningful) {
      dailyAvgSpend = budgetSummary.dailyAvgSpend;
      spentThisMonth = budgetSummary.spentThisMonth;
      remainingBudget = monthlyBudget - spentThisMonth;

      if (dailyAvgSpend > 0 && remainingBudget > 0) {
        daysRemaining = Math.ceil(remainingBudget / dailyAvgSpend);
        const exhaust = new Date(now);
        exhaust.setDate(exhaust.getDate() + daysRemaining);
        exhaustionDate = exhaust.toISOString().split('T')[0];
      } else if (remainingBudget <= 0) {
        daysRemaining = 0;
        exhaustionDate = _dateKey(now);
      }
    }

    return {
      dailyAvgSpend,
      daysRemaining,
      exhaustionDate,
      spentThisMonth,
      remainingBudget,
      monthlyBudget,
      lookbackDays,
      asset: filter.asset || null,
      network: filter.network || null,
      assets: budgetSummary.assets,
      aggregateAsset: aggregate.aggregateAsset,
      budgetForecastMeaningful,
      breakdownByAsset: budgetSummary.breakdownByAsset,
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
  function getTopSpenders(limit = 10, filter = {}) {
    /** @type {Map<string, Array<Object>>} */
    const agentMap = new Map();

    for (const entry of _ledger) {
      if (!_matchesEntryFilter(entry, filter)) {
        continue;
      }
      const current = agentMap.get(entry.agentAddress) || [];
      current.push(entry);
      agentMap.set(entry.agentAddress, current);
    }

    const result = [];
    for (const [agentAddress, entries] of agentMap) {
      const summary = _buildEntryBreakdown(entries);
      const aggregate = _getAggregateDescriptor(summary.assets, filter);
      result.push({
        agentAddress,
        totalSpent: summary.totalSpent,
        totalEarned: summary.totalEarned,
        transactionCount: summary.transactionCount,
        asset: filter.asset || null,
        network: filter.network || null,
        assets: summary.assets,
        aggregateAsset: aggregate.aggregateAsset,
        aggregateTotalsMeaningful: aggregate.aggregateTotalsMeaningful,
        breakdownByAsset: summary.breakdownByAsset,
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
