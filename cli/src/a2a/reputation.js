/**
 * A2A Reputation and Trust Scoring Engine
 *
 * Manages reputation scores, feedback, and trust tier promotions
 * for AI agents participating in A2A commerce.
 *
 * @example
 * ```javascript
 * const reputation = createReputationService(store);
 *
 * // Rate an agent after a transaction
 * await reputation.rateAgent({
 *   agentAddress: '0xSeller',
 *   reviewerAddress: '0xBuyer',
 *   transactionType: 'escrow',
 *   transactionId: 'escrow-123',
 *   score: 5,
 *   dimensions: { reliability: 5, quality: 5, speed: 4, communication: 5 },
 *   comment: 'Excellent service, fast delivery',
 * });
 *
 * // Check reputation
 * const rep = await reputation.getReputation('0xSeller');
 * // => { trustTier: 'standard', averageScore: 4.8, ... }
 *
 * // Get feedback summary
 * const summary = await reputation.getFeedbackSummary('0xSeller');
 * ```
 */

import { randomUUID, createHash } from 'node:crypto';

// Transaction types for feedback
const TRANSACTION_TYPES = ['quote', 'payment', 'escrow', 'service'];

// Score dimensions (each rated 1-5)
const SCORE_DIMENSIONS = ['reliability', 'quality', 'speed', 'communication'];

// Trust tiers in ascending order
const TRUST_TIERS = ['sandbox', 'standard', 'verified', 'enterprise'];

/**
 * Trust tier promotion thresholds
 *
 * sandbox   -> standard:   5+ completed transactions, avg score >= 3.5
 * standard  -> verified:   25+ transactions, avg score >= 4.0, 0 unresolved disputes
 * verified  -> enterprise: 100+ transactions, avg score >= 4.5, dispute rate < 2%
 */
const TIER_THRESHOLDS = {
  standard: {
    minTransactions: 5,
    minAverageScore: 3.5,
  },
  verified: {
    minTransactions: 25,
    minAverageScore: 4.0,
    maxUnresolvedDisputes: 0,
  },
  enterprise: {
    minTransactions: 100,
    minAverageScore: 4.5,
    maxDisputeRate: 0.02,
  },
};

/**
 * Default reputation record for agents with no history
 */
function defaultReputation(agentAddress) {
  return {
    agent_address: agentAddress,
    total_transactions: 0,
    successful_transactions: 0,
    disputed_transactions: 0,
    average_score: 0,
    dimension_scores: JSON.stringify({
      reliability: 0,
      quality: 0,
      speed: 0,
      communication: 0,
    }),
    trust_tier: 'sandbox',
    last_updated: new Date().toISOString(),
  };
}

/**
 * Create a reputation and trust scoring service
 *
 * @param {Object} store - Store with feedback/reputation CRUD methods
 * @param {Function} store.createFeedback - Create a feedback record
 * @param {Function} store.getFeedback - Get feedback by ID
 * @param {Function} store.updateFeedback - Update feedback fields
 * @param {Function} store.listFeedback - List feedback with filters
 * @param {Function} store.getReputationScore - Get reputation score for an agent
 * @param {Function} store.upsertReputationScore - Create or update reputation score
 * @returns {Object} Reputation service methods
 */
export function createReputationService(store) {
  /**
   * Rate an agent after a transaction
   *
   * @param {Object} params - Rating parameters
   * @param {string} params.agentAddress - Address of the agent being rated
   * @param {string} params.reviewerAddress - Address of the reviewer
   * @param {string} params.transactionType - Type of transaction
   * @param {string} params.transactionId - ID of the transaction
   * @param {number} params.score - Overall score (1-5)
   * @param {Object} [params.dimensions] - Dimension scores { reliability, quality, speed, communication }
   * @param {string} [params.comment] - Optional comment
   * @returns {Promise<Object>} Created feedback and updated reputation
   */
  async function rateAgent(params) {
    const {
      agentAddress,
      reviewerAddress,
      transactionType,
      transactionId,
      score,
      dimensions,
      comment,
    } = params;

    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }
    if (!reviewerAddress) {
      throw new Error('reviewerAddress is required');
    }
    if (!transactionType || !TRANSACTION_TYPES.includes(transactionType)) {
      throw new Error(`transactionType must be one of: ${TRANSACTION_TYPES.join(', ')}`);
    }
    if (!transactionId) {
      throw new Error('transactionId is required');
    }
    if (score === undefined || score === null) {
      throw new Error('score is required');
    }
    if (score < 1 || score > 5 || !Number.isInteger(score)) {
      throw new Error('score must be an integer between 1 and 5');
    }

    // Validate dimensions if provided
    if (dimensions) {
      for (const dim of SCORE_DIMENSIONS) {
        if (dimensions[dim] !== undefined) {
          const val = dimensions[dim];
          if (val < 1 || val > 5 || !Number.isInteger(val)) {
            throw new Error(`${dim} dimension score must be an integer between 1 and 5`);
          }
        }
      }
    }

    const now = new Date().toISOString();
    const feedbackId = randomUUID();

    const feedback = {
      id: feedbackId,
      agent_address: agentAddress,
      reviewer_address: reviewerAddress,
      transaction_type: transactionType,
      transaction_id: transactionId,
      score,
      dimensions: dimensions ? JSON.stringify(dimensions) : null,
      comment: comment || null,
      response: null,
      response_at: null,
      revoked: false,
      created_at: now,
      updated_at: now,
    };

    await store.createFeedback(feedback);

    // Recalculate reputation scores
    await recalculateReputation(agentAddress);

    // Check for trust tier promotion
    await checkTrustTierPromotion(agentAddress);

    const created = await store.getFeedback(feedbackId);

    return {
      success: true,
      feedback: formatFeedback(created),
      reputationUpdated: true,
    };
  }

  /**
   * Get reputation for an agent
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Promise<Object>} Reputation data
   */
  async function getReputation(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    let reputation = await store.getReputationScore(agentAddress);

    if (!reputation) {
      reputation = defaultReputation(agentAddress);
    }

    return {
      success: true,
      reputation: formatReputation(reputation),
    };
  }

  /**
   * Respond to feedback (by the rated agent)
   *
   * @param {string} feedbackId - Feedback ID
   * @param {Object} params - Response parameters
   * @param {string} params.response - Response text
   * @param {string} params.responderAddress - Address of the responder
   * @returns {Promise<Object>} Updated feedback
   */
  async function respondToFeedback(feedbackId, params) {
    const { response, responderAddress } = params;

    if (!feedbackId) {
      throw new Error('feedbackId is required');
    }
    if (!response) {
      throw new Error('response is required');
    }
    if (!responderAddress) {
      throw new Error('responderAddress is required');
    }

    const feedback = await store.getFeedback(feedbackId);
    if (!feedback) {
      throw new Error(`Feedback not found: ${feedbackId}`);
    }

    // Only the rated agent can respond
    if (feedback.agent_address !== responderAddress) {
      throw new Error('Only the rated agent can respond to feedback');
    }

    const now = new Date().toISOString();

    await store.updateFeedback(feedbackId, {
      response,
      response_at: now,
      updated_at: now,
    });

    const updated = await store.getFeedback(feedbackId);

    return {
      success: true,
      feedback: formatFeedback(updated),
    };
  }

  /**
   * Recalculate reputation scores for an agent
   *
   * Queries all non-revoked feedback and computes aggregated metrics.
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Promise<Object>} Updated reputation scores
   */
  async function recalculateReputation(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    // Get all non-revoked feedback for this agent
    const allFeedback = await store.listFeedback({ agent_address: agentAddress });
    const activeFeedback = allFeedback.filter((f) => !f.revoked);

    if (activeFeedback.length === 0) {
      // No feedback yet, ensure default record exists
      const existing = await store.getReputationScore(agentAddress);
      if (!existing) {
        const defaults = defaultReputation(agentAddress);
        await store.upsertReputationScore(defaults);
        return defaults;
      }
      return existing;
    }

    // Calculate average score
    const totalScore = activeFeedback.reduce((sum, f) => sum + f.score, 0);
    const averageScore = totalScore / activeFeedback.length;

    // Calculate dimension averages
    const dimensionTotals = { reliability: 0, quality: 0, speed: 0, communication: 0 };
    const dimensionCounts = { reliability: 0, quality: 0, speed: 0, communication: 0 };

    for (const f of activeFeedback) {
      const dims = typeof f.dimensions === 'string' ? JSON.parse(f.dimensions) : f.dimensions;

      if (dims) {
        for (const dim of SCORE_DIMENSIONS) {
          if (dims[dim] !== undefined && dims[dim] !== null) {
            dimensionTotals[dim] += dims[dim];
            dimensionCounts[dim] += 1;
          }
        }
      }
    }

    const dimensionScores = {};
    for (const dim of SCORE_DIMENSIONS) {
      dimensionScores[dim] =
        dimensionCounts[dim] > 0
          ? Math.round((dimensionTotals[dim] / dimensionCounts[dim]) * 100) / 100
          : 0;
    }

    // Count transaction outcomes
    const totalTransactions = activeFeedback.length;
    // A "successful" transaction is one with score >= 3
    const successfulTransactions = activeFeedback.filter((f) => f.score >= 3).length;
    const disputedTransactions = activeFeedback.filter(
      (f) => f.transaction_type === 'escrow' && f.score <= 2,
    ).length;

    // Get current reputation to preserve trust tier
    const existing = await store.getReputationScore(agentAddress);
    const currentTier = existing ? existing.trust_tier : 'sandbox';

    const now = new Date().toISOString();

    const reputationRecord = {
      agent_address: agentAddress,
      total_transactions: totalTransactions,
      successful_transactions: successfulTransactions,
      disputed_transactions: disputedTransactions,
      average_score: Math.round(averageScore * 100) / 100,
      dimension_scores: JSON.stringify(dimensionScores),
      trust_tier: currentTier,
      last_updated: now,
    };

    await store.upsertReputationScore(reputationRecord);
    return reputationRecord;
  }

  /**
   * Check if an agent qualifies for trust tier promotion
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Promise<Object>} Promotion result
   */
  async function checkTrustTierPromotion(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    let reputation = await store.getReputationScore(agentAddress);
    if (!reputation) {
      return { promoted: false, previousTier: 'sandbox', currentTier: 'sandbox' };
    }

    const previousTier = reputation.trust_tier || 'sandbox';
    const currentTierIndex = TRUST_TIERS.indexOf(previousTier);

    // Already at the highest tier
    if (currentTierIndex >= TRUST_TIERS.length - 1) {
      return { promoted: false, previousTier, currentTier: previousTier };
    }

    const nextTier = TRUST_TIERS[currentTierIndex + 1];
    const threshold = TIER_THRESHOLDS[nextTier];

    if (!threshold) {
      return { promoted: false, previousTier, currentTier: previousTier };
    }

    // Evaluate promotion criteria
    let eligible = true;

    if (reputation.total_transactions < threshold.minTransactions) {
      eligible = false;
    }

    if (reputation.average_score < threshold.minAverageScore) {
      eligible = false;
    }

    if (
      threshold.maxUnresolvedDisputes !== undefined &&
      reputation.disputed_transactions > threshold.maxUnresolvedDisputes
    ) {
      eligible = false;
    }

    if (threshold.maxDisputeRate !== undefined && reputation.total_transactions > 0) {
      const disputeRate = reputation.disputed_transactions / reputation.total_transactions;
      if (disputeRate >= threshold.maxDisputeRate) {
        eligible = false;
      }
    }

    if (eligible) {
      const now = new Date().toISOString();
      await store.upsertReputationScore({
        ...reputation,
        trust_tier: nextTier,
        last_updated: now,
      });

      return { promoted: true, previousTier, currentTier: nextTier };
    }

    return { promoted: false, previousTier, currentTier: previousTier };
  }

  /**
   * List feedback with optional filters
   *
   * @param {Object} [filter] - Filter options
   * @param {string} [filter.agent_address] - Filter by rated agent
   * @param {string} [filter.reviewer_address] - Filter by reviewer
   * @param {string} [filter.transaction_type] - Filter by transaction type
   * @returns {Promise<Array>} Formatted feedback records
   */
  async function listFeedback(filter = {}) {
    const feedback = await store.listFeedback(filter);
    return feedback.map(formatFeedback);
  }

  /**
   * Get aggregated feedback summary for an agent
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Promise<Object>} Aggregated feedback stats
   */
  async function getFeedbackSummary(agentAddress) {
    if (!agentAddress) {
      throw new Error('agentAddress is required');
    }

    const allFeedback = await store.listFeedback({ agent_address: agentAddress });
    const activeFeedback = allFeedback.filter((f) => !f.revoked);

    if (activeFeedback.length === 0) {
      return {
        success: true,
        summary: {
          agentAddress,
          totalReviews: 0,
          averageScore: 0,
          scoreDistribution: { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 },
          dimensionAverages: { reliability: 0, quality: 0, speed: 0, communication: 0 },
          byTransactionType: {},
          recentReviews: [],
        },
      };
    }

    // Score distribution
    const scoreDistribution = { 1: 0, 2: 0, 3: 0, 4: 0, 5: 0 };
    for (const f of activeFeedback) {
      scoreDistribution[f.score] = (scoreDistribution[f.score] || 0) + 1;
    }

    // Average score
    const totalScore = activeFeedback.reduce((sum, f) => sum + f.score, 0);
    const averageScore = Math.round((totalScore / activeFeedback.length) * 100) / 100;

    // Dimension averages
    const dimensionTotals = { reliability: 0, quality: 0, speed: 0, communication: 0 };
    const dimensionCounts = { reliability: 0, quality: 0, speed: 0, communication: 0 };

    for (const f of activeFeedback) {
      const dims = typeof f.dimensions === 'string' ? JSON.parse(f.dimensions) : f.dimensions;

      if (dims) {
        for (const dim of SCORE_DIMENSIONS) {
          if (dims[dim] !== undefined && dims[dim] !== null) {
            dimensionTotals[dim] += dims[dim];
            dimensionCounts[dim] += 1;
          }
        }
      }
    }

    const dimensionAverages = {};
    for (const dim of SCORE_DIMENSIONS) {
      dimensionAverages[dim] =
        dimensionCounts[dim] > 0
          ? Math.round((dimensionTotals[dim] / dimensionCounts[dim]) * 100) / 100
          : 0;
    }

    // Breakdown by transaction type
    const byTransactionType = {};
    for (const f of activeFeedback) {
      const type = f.transaction_type;
      if (!byTransactionType[type]) {
        byTransactionType[type] = { count: 0, totalScore: 0, averageScore: 0 };
      }
      byTransactionType[type].count += 1;
      byTransactionType[type].totalScore += f.score;
    }
    for (const type of Object.keys(byTransactionType)) {
      const entry = byTransactionType[type];
      entry.averageScore = Math.round((entry.totalScore / entry.count) * 100) / 100;
      delete entry.totalScore;
    }

    // Recent reviews (last 5)
    const sorted = [...activeFeedback].sort(
      (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime(),
    );
    const recentReviews = sorted.slice(0, 5).map(formatFeedback);

    return {
      success: true,
      summary: {
        agentAddress,
        totalReviews: activeFeedback.length,
        averageScore,
        scoreDistribution,
        dimensionAverages,
        byTransactionType,
        recentReviews,
      },
    };
  }

  /**
   * Format a feedback record for API output
   *
   * @param {Object} f - Raw feedback record
   * @returns {Object} Formatted feedback
   */
  function formatFeedback(f) {
    const dims =
      typeof f.dimensions === 'string' ? JSON.parse(f.dimensions || 'null') : f.dimensions;

    return {
      id: f.id,
      agentAddress: f.agent_address,
      reviewerAddress: f.reviewer_address,
      transactionType: f.transaction_type,
      transactionId: f.transaction_id,
      score: f.score,
      dimensions: dims,
      comment: f.comment,
      response: f.response,
      responseAt: f.response_at,
      revoked: Boolean(f.revoked),
      createdAt: f.created_at,
      updatedAt: f.updated_at,
    };
  }

  /**
   * Format a reputation record for API output
   *
   * @param {Object} r - Raw reputation record
   * @returns {Object} Formatted reputation
   */
  function formatReputation(r) {
    const dims =
      typeof r.dimension_scores === 'string'
        ? JSON.parse(r.dimension_scores || '{}')
        : r.dimension_scores;

    return {
      agentAddress: r.agent_address,
      totalTransactions: r.total_transactions,
      successfulTransactions: r.successful_transactions,
      disputedTransactions: r.disputed_transactions,
      averageScore: r.average_score,
      dimensionScores: dims,
      trustTier: r.trust_tier,
      lastUpdated: r.last_updated,
    };
  }

  return {
    // Core operations
    rateAgent,
    getReputation,
    respondToFeedback,

    // Reputation management
    recalculateReputation,
    checkTrustTierPromotion,

    // Query operations
    listFeedback,
    getFeedbackSummary,

    // Format helpers (exposed for testing/reuse)
    formatFeedback,
    formatReputation,
  };
}

export default { createReputationService };
