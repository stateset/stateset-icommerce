/**
 * A2A Agent Memory — Counterparty Learning & Recommendation Engine
 *
 * Enables agents to remember and learn from past interactions with
 * counterparties. Profiles are computed on-demand from interaction history,
 * enabling pattern detection (late fulfillment, habitual negotiation,
 * declining reliability) and risk-aware decision making.
 *
 * Storage is in-memory (Map of Maps), scoped per agent address so each
 * agent has its own independent memory space.
 *
 * @example
 * ```javascript
 * const memory = createAgentMemory();
 *
 * // Record interactions over time
 * await memory.recordInteraction({
 *   agentAddress: '0xBuyer',
 *   counterpartyAddress: '0xSeller',
 *   interactionType: 'payment_sent',
 *   outcome: 'success',
 *   amount: 100,
 *   responseTimeMs: 1500,
 * });
 *
 * // Get learned counterparty profile
 * const profile = memory.getCounterpartyProfile('0xBuyer', '0xSeller');
 * // => { totalInteractions: 1, successRate: 1.0, reliabilityScore: 0.95, ... }
 *
 * // Get recommendation before transacting
 * const rec = memory.getRecommendation('0xBuyer', '0xSeller', 'payment_sent');
 * // => { recommended: true, confidence: 0.85, reason: "..." }
 * ```
 */

import { randomUUID } from 'node:crypto';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Valid interaction types */
const INTERACTION_TYPES = new Set([
  'quote_received',
  'quote_sent',
  'payment_sent',
  'payment_received',
  'negotiation',
  'dispute',
  'fulfillment',
  'rating',
]);

/** Valid interaction outcomes */
const OUTCOMES = new Set(['success', 'failure', 'timeout', 'rejected', 'accepted']);

/**
 * Threshold (ms) considered "timely". Responses faster than this contribute
 * positively to the reliability score.
 */
const TIMELY_RESPONSE_MS = 10_000;

/**
 * Window size for "recent" interactions used in risk-alert detection.
 * When the recent success rate drops >20% vs the overall rate, a risk alert
 * is raised.
 */
const RECENT_WINDOW = 10;

/**
 * Dispute/failure rate above which a counterparty is considered high risk.
 */
const HIGH_RISK_THRESHOLD = 0.2;

/**
 * Dispute/failure rate above which a counterparty is considered medium risk.
 */
const MEDIUM_RISK_THRESHOLD = 0.1;

/**
 * Minimum interactions before we assign "high" risk — below this we say
 * "medium" at worst to avoid premature judgment.
 */
const MIN_INTERACTIONS_FOR_HIGH_RISK = 3;

/**
 * Decline percentage that triggers a risk alert.
 */
const RISK_ALERT_DECLINE_THRESHOLD = 0.2;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Validate required string fields.
 * @param {Object} params
 * @param {string[]} fields
 */
function requireStrings(params, fields) {
  for (const f of fields) {
    if (typeof params[f] !== 'string' || params[f].length === 0) {
      throw new Error(`Missing or invalid required field: ${f}`);
    }
  }
}

/**
 * Compute the mean of an array of numbers.
 * @param {number[]} arr
 * @returns {number}
 */
function mean(arr) {
  if (arr.length === 0) return 0;
  return arr.reduce((a, b) => a + b, 0) / arr.length;
}

/**
 * Compute the reliability score from success rate and timeliness ratio.
 * Both inputs are in [0, 1]. Reliability is a weighted combination:
 *   0.7 * successRate + 0.3 * timelinessRate
 *
 * @param {number} successRate
 * @param {number} timelinessRate - fraction of interactions with responseTime < threshold
 * @returns {number} value in [0, 1]
 */
function computeReliabilityScore(successRate, timelinessRate) {
  return Math.round((0.7 * successRate + 0.3 * timelinessRate) * 1000) / 1000;
}

/**
 * Determine risk level from dispute/failure rate and total interaction count.
 * @param {number} negativeRate - fraction of dispute + failure outcomes
 * @param {number} totalInteractions
 * @returns {'low' | 'medium' | 'high'}
 */
function computeRiskLevel(negativeRate, totalInteractions) {
  if (negativeRate > HIGH_RISK_THRESHOLD && totalInteractions >= MIN_INTERACTIONS_FOR_HIGH_RISK) {
    return 'high';
  }
  if (negativeRate > MEDIUM_RISK_THRESHOLD) {
    return 'medium';
  }
  return 'low';
}

/**
 * Build a negotiation pattern summary from quote interactions.
 *
 * Looks at quote_received / quote_sent / negotiation interactions that have
 * metadata with `originalAmount` and `finalAmount` (or `amount` field) to
 * infer average discount percentage and counter-offer tendencies.
 *
 * @param {Object[]} interactions
 * @returns {{ avgDiscountPct: number, counterOfferRate: number, sampleSize: number }}
 */
function computeNegotiationPattern(interactions) {
  const negotiationTypes = new Set(['quote_received', 'quote_sent', 'negotiation']);
  const relevant = interactions.filter((i) => negotiationTypes.has(i.interactionType));

  const discounts = [];
  let counterOffers = 0;

  for (const ix of relevant) {
    const meta = ix.metadata || {};
    if (
      meta.originalAmount !== null &&
      meta.originalAmount !== undefined &&
      meta.finalAmount !== null &&
      meta.finalAmount !== undefined
    ) {
      const original = Number(meta.originalAmount);
      const final = Number(meta.finalAmount);
      if (original > 0) {
        discounts.push(((original - final) / original) * 100);
      }
    }
    if (meta.counterOffer === true || ix.outcome === 'rejected') {
      counterOffers += 1;
    }
  }

  return {
    avgDiscountPct: discounts.length > 0 ? Math.round(mean(discounts) * 100) / 100 : 0,
    counterOfferRate:
      relevant.length > 0 ? Math.round((counterOffers / relevant.length) * 1000) / 1000 : 0,
    sampleSize: relevant.length,
  };
}

/**
 * Tally frequency of values in an array and return them sorted by count desc.
 * @param {(string|undefined)[]} values
 * @returns {string[]} unique values sorted by frequency descending
 */
function tallyAndSort(values) {
  const counts = new Map();
  for (const v of values) {
    if (v === null || v === undefined) continue;
    counts.set(v, (counts.get(v) || 0) + 1);
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([v]) => v);
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an agent memory instance.
 *
 * @returns {Object} Agent memory API
 */
export function createAgentMemory() {
  /**
   * Top-level map: agentAddress -> Map(counterpartyAddress -> interaction[])
   * @type {Map<string, Map<string, Object[]>>}
   */
  const store = new Map();

  /**
   * Get (or create) the interaction list for an agent/counterparty pair.
   * @param {string} agentAddress
   * @param {string} counterpartyAddress
   * @param {boolean} [create=true]
   * @returns {Object[]|null}
   */
  function getInteractionList(agentAddress, counterpartyAddress, create = true) {
    let agentMap = store.get(agentAddress);
    if (!agentMap) {
      if (!create) return null;
      agentMap = new Map();
      store.set(agentAddress, agentMap);
    }
    let list = agentMap.get(counterpartyAddress);
    if (!list) {
      if (!create) return null;
      list = [];
      agentMap.set(counterpartyAddress, list);
    }
    return list;
  }

  // -------------------------------------------------------------------------
  // recordInteraction
  // -------------------------------------------------------------------------

  /**
   * Log an interaction with a counterparty.
   *
   * @param {Object} params
   * @param {string} params.agentAddress
   * @param {string} params.counterpartyAddress
   * @param {string} params.interactionType
   * @param {string} params.outcome
   * @param {number} [params.amount]
   * @param {number} [params.responseTimeMs]
   * @param {Object} [params.metadata]
   * @returns {{ id: string, timestamp: string }}
   */
  function recordInteraction(params) {
    requireStrings(params, ['agentAddress', 'counterpartyAddress', 'interactionType', 'outcome']);

    if (!INTERACTION_TYPES.has(params.interactionType)) {
      throw new Error(
        `Invalid interactionType: ${params.interactionType}. ` +
          `Must be one of: ${[...INTERACTION_TYPES].join(', ')}`,
      );
    }

    if (!OUTCOMES.has(params.outcome)) {
      throw new Error(
        `Invalid outcome: ${params.outcome}. ` + `Must be one of: ${[...OUTCOMES].join(', ')}`,
      );
    }

    const record = {
      id: randomUUID(),
      agentAddress: params.agentAddress,
      counterpartyAddress: params.counterpartyAddress,
      interactionType: params.interactionType,
      outcome: params.outcome,
      amount: params.amount ?? null,
      responseTimeMs: params.responseTimeMs ?? null,
      metadata: params.metadata ?? {},
      timestamp: new Date().toISOString(),
    };

    const list = getInteractionList(params.agentAddress, params.counterpartyAddress);
    list.push(record);

    return { id: record.id, timestamp: record.timestamp };
  }

  // -------------------------------------------------------------------------
  // getInteractionHistory
  // -------------------------------------------------------------------------

  /**
   * Retrieve recent interactions between an agent and counterparty.
   *
   * @param {string} agentAddress
   * @param {string} counterpartyAddress
   * @param {number} [limit=50]
   * @returns {Object[]}
   */
  function getInteractionHistory(agentAddress, counterpartyAddress, limit = 50) {
    const list = getInteractionList(agentAddress, counterpartyAddress, false);
    if (!list || list.length === 0) return [];
    // Return newest first, limited
    return list.slice(-limit).reverse();
  }

  // -------------------------------------------------------------------------
  // getCounterpartyProfile
  // -------------------------------------------------------------------------

  /**
   * Compute a learned counterparty profile from interaction history.
   *
   * @param {string} agentAddress
   * @param {string} counterpartyAddress
   * @returns {Object} Counterparty profile
   */
  function getCounterpartyProfile(agentAddress, counterpartyAddress) {
    const list = getInteractionList(agentAddress, counterpartyAddress, false);

    // Safe defaults for empty history
    if (!list || list.length === 0) {
      return {
        counterpartyAddress,
        totalInteractions: 0,
        successRate: 0,
        avgResponseTimeMs: 0,
        avgTransactionAmount: 0,
        reliabilityScore: 0,
        negotiationPattern: { avgDiscountPct: 0, counterOfferRate: 0, sampleSize: 0 },
        preferredAssets: [],
        preferredNetworks: [],
        riskLevel: 'low',
        lastInteractionAt: null,
        firstInteractionAt: null,
        relationship_duration_days: 0,
      };
    }

    const total = list.length;

    // Success rate
    const successes = list.filter(
      (i) => i.outcome === 'success' || i.outcome === 'accepted',
    ).length;
    const successRate = Math.round((successes / total) * 1000) / 1000;

    // Average response time
    const responseTimes = list
      .map((i) => i.responseTimeMs)
      .filter((t) => t !== null && t !== undefined);
    const avgResponseTimeMs = Math.round(mean(responseTimes));

    // Average transaction amount
    const amounts = list.map((i) => i.amount).filter((a) => a !== null && a !== undefined);
    const avgTransactionAmount = Math.round(mean(amounts) * 100) / 100;

    // Timeliness for reliability
    const timelyCount = responseTimes.filter((t) => t <= TIMELY_RESPONSE_MS).length;
    const timelinessRate = responseTimes.length > 0 ? timelyCount / responseTimes.length : 0.5; // neutral when no timing data
    const reliabilityScore = computeReliabilityScore(successRate, timelinessRate);

    // Negotiation pattern
    const negotiationPattern = computeNegotiationPattern(list);

    // Preferred assets & networks
    const preferredAssets = tallyAndSort(list.map((i) => (i.metadata || {}).asset));
    const preferredNetworks = tallyAndSort(list.map((i) => (i.metadata || {}).network));

    // Risk level — based on dispute + failure rate
    const negativeOutcomes = list.filter(
      (i) => i.interactionType === 'dispute' || i.outcome === 'failure',
    ).length;
    const negativeRate = negativeOutcomes / total;
    const riskLevel = computeRiskLevel(negativeRate, total);

    // Timestamps
    const firstInteractionAt = list[0].timestamp;
    const lastInteractionAt = list[list.length - 1].timestamp;
    const durationMs = new Date(lastInteractionAt) - new Date(firstInteractionAt);
    const relationship_duration_days = Math.floor(durationMs / (24 * 60 * 60 * 1000));

    return {
      counterpartyAddress,
      totalInteractions: total,
      successRate,
      avgResponseTimeMs,
      avgTransactionAmount,
      reliabilityScore,
      negotiationPattern,
      preferredAssets,
      preferredNetworks,
      riskLevel,
      lastInteractionAt,
      firstInteractionAt,
      relationship_duration_days,
    };
  }

  // -------------------------------------------------------------------------
  // getTopCounterparties
  // -------------------------------------------------------------------------

  /**
   * Get top N counterparties for an agent, ranked by a given metric.
   *
   * @param {string} agentAddress
   * @param {Object} [opts]
   * @param {number} [opts.limit=10]
   * @param {'volume' | 'success_rate' | 'reliability'} [opts.sortBy='volume']
   * @returns {Object[]} Array of counterparty profiles
   */
  function getTopCounterparties(agentAddress, opts = {}) {
    const { limit = 10, sortBy = 'volume' } = opts;
    const agentMap = store.get(agentAddress);
    if (!agentMap) return [];

    const profiles = [];
    for (const cpAddr of agentMap.keys()) {
      profiles.push(getCounterpartyProfile(agentAddress, cpAddr));
    }

    // Sort by the requested metric
    switch (sortBy) {
      case 'success_rate':
        profiles.sort((a, b) => b.successRate - a.successRate);
        break;
      case 'reliability':
        profiles.sort((a, b) => b.reliabilityScore - a.reliabilityScore);
        break;
      case 'volume':
      default:
        profiles.sort((a, b) => b.totalInteractions - a.totalInteractions);
        break;
    }

    return profiles.slice(0, limit);
  }

  // -------------------------------------------------------------------------
  // getRecommendation
  // -------------------------------------------------------------------------

  /**
   * Get an AI-style recommendation for transacting with a counterparty.
   *
   * @param {string} agentAddress
   * @param {string} counterpartyAddress
   * @param {string} actionType - The action being considered
   * @returns {{ recommended: boolean, confidence: number, reason: string, suggestedTerms?: Object }}
   */
  function getRecommendation(agentAddress, counterpartyAddress, _actionType) {
    const profile = getCounterpartyProfile(agentAddress, counterpartyAddress);

    // No history — neutral recommendation
    if (profile.totalInteractions === 0) {
      return {
        recommended: true,
        confidence: 0.3,
        reason: 'No prior interaction history. Proceed with caution.',
        suggestedTerms: { escrow: true },
      };
    }

    const reasons = [];
    let score = 0; // accumulate a weighted score

    // Success rate component (weight: 40%)
    if (profile.successRate >= 0.8) {
      score += 0.4 * profile.successRate;
      reasons.push(
        `${Math.round(profile.successRate * 100)}% success rate over ${profile.totalInteractions} transactions`,
      );
    } else if (profile.successRate >= 0.5) {
      score += 0.4 * profile.successRate;
      reasons.push(`${Math.round(profile.successRate * 100)}% success rate — moderate reliability`);
    } else {
      score += 0.4 * profile.successRate;
      reasons.push(
        `Only ${Math.round(profile.successRate * 100)}% success rate — poor track record`,
      );
    }

    // Reliability component (weight: 30%)
    score += 0.3 * profile.reliabilityScore;
    if (profile.avgResponseTimeMs > 0) {
      reasons.push(`avg response ${Math.round(profile.avgResponseTimeMs)}ms`);
    }

    // Risk component (weight: 30%)
    if (profile.riskLevel === 'high') {
      score -= 0.3;
      reasons.push(`HIGH risk — elevated dispute/failure rate`);
    } else if (profile.riskLevel === 'medium') {
      score -= 0.1;
      reasons.push(`medium risk — some disputes/failures`);
    } else {
      score += 0.3;
    }

    // Check recent dispute density
    const list = getInteractionList(agentAddress, counterpartyAddress, false) || [];
    const recent = list.slice(-5);
    const recentDisputes = recent.filter(
      (i) => i.interactionType === 'dispute' || i.outcome === 'failure',
    ).length;
    if (recentDisputes >= 3) {
      score -= 0.2;
      reasons.push(`${recentDisputes} disputes in last ${recent.length} deals`);
    }

    // Clamp score to [0, 1]
    const confidence = Math.round(Math.max(0, Math.min(1, Math.abs(score) + 0.3)) * 100) / 100;
    const recommended = score > 0.3;

    const result = {
      recommended,
      confidence: Math.min(confidence, 1),
      reason: reasons.join(', '),
    };

    // Suggest terms for medium-risk counterparties
    if (profile.riskLevel === 'medium' || profile.riskLevel === 'high') {
      result.suggestedTerms = { escrow: true };
      if (profile.negotiationPattern.avgDiscountPct > 0) {
        result.suggestedTerms.expectedDiscount = profile.negotiationPattern.avgDiscountPct;
      }
    }

    return result;
  }

  // -------------------------------------------------------------------------
  // getAgentInsights
  // -------------------------------------------------------------------------

  /**
   * Get aggregate insights for an agent across all counterparties.
   *
   * @param {string} agentAddress
   * @returns {Object} Aggregate insights
   */
  function getAgentInsights(agentAddress) {
    const agentMap = store.get(agentAddress);

    if (!agentMap || agentMap.size === 0) {
      return {
        totalCounterparties: 0,
        avgSuccessRate: 0,
        topPerformers: [],
        riskAlerts: [],
        networkPreferences: [],
        assetPreferences: [],
      };
    }

    const profiles = [];
    const allNetworks = [];
    const allAssets = [];

    for (const cpAddr of agentMap.keys()) {
      const profile = getCounterpartyProfile(agentAddress, cpAddr);
      profiles.push(profile);
      allNetworks.push(...profile.preferredNetworks);
      allAssets.push(...profile.preferredAssets);
    }

    // Average success rate
    const avgSuccessRate = Math.round(mean(profiles.map((p) => p.successRate)) * 1000) / 1000;

    // Top performers by reliability
    const topPerformers = [...profiles]
      .sort((a, b) => b.reliabilityScore - a.reliabilityScore)
      .slice(0, 5)
      .map((p) => ({
        counterpartyAddress: p.counterpartyAddress,
        reliabilityScore: p.reliabilityScore,
        successRate: p.successRate,
        totalInteractions: p.totalInteractions,
      }));

    // Risk alerts: counterparties whose recent success rate dropped >20% vs
    // overall success rate
    const riskAlerts = [];
    for (const cpAddr of agentMap.keys()) {
      const list = agentMap.get(cpAddr);
      if (!list || list.length < RECENT_WINDOW) continue;

      const overallSuccesses = list.filter(
        (i) => i.outcome === 'success' || i.outcome === 'accepted',
      ).length;
      const overallRate = overallSuccesses / list.length;

      const recentSlice = list.slice(-RECENT_WINDOW);
      const recentSuccesses = recentSlice.filter(
        (i) => i.outcome === 'success' || i.outcome === 'accepted',
      ).length;
      const recentRate = recentSuccesses / recentSlice.length;

      const decline = overallRate - recentRate;
      if (decline > RISK_ALERT_DECLINE_THRESHOLD) {
        riskAlerts.push({
          counterpartyAddress: cpAddr,
          overallSuccessRate: Math.round(overallRate * 1000) / 1000,
          recentSuccessRate: Math.round(recentRate * 1000) / 1000,
          decline: Math.round(decline * 1000) / 1000,
        });
      }
    }

    return {
      totalCounterparties: agentMap.size,
      avgSuccessRate,
      topPerformers,
      riskAlerts,
      networkPreferences: tallyAndSort(allNetworks),
      assetPreferences: tallyAndSort(allAssets),
    };
  }

  // -------------------------------------------------------------------------
  // forget / clear
  // -------------------------------------------------------------------------

  /**
   * Remove all memory of a specific counterparty for an agent.
   *
   * @param {string} agentAddress
   * @param {string} counterpartyAddress
   * @returns {boolean} true if data was removed
   */
  function forget(agentAddress, counterpartyAddress) {
    const agentMap = store.get(agentAddress);
    if (!agentMap) return false;
    return agentMap.delete(counterpartyAddress);
  }

  /**
   * Clear all memories for an agent.
   *
   * @param {string} agentAddress
   * @returns {boolean} true if data was removed
   */
  function clear(agentAddress) {
    return store.delete(agentAddress);
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------

  return {
    recordInteraction,
    getInteractionHistory,
    getCounterpartyProfile,
    getTopCounterparties,
    getRecommendation,
    getAgentInsights,
    forget,
    clear,
  };
}
