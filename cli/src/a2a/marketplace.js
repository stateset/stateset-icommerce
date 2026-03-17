/**
 * Marketplace Service — Multi-party RFQ broadcast, scoring, and awarding
 *
 * Enables buyers to broadcast Requests for Quotation (RFQs) to multiple
 * sellers, collect responses, score them by configurable criteria, and
 * award the best quote automatically.
 *
 * @example
 * ```javascript
 * import { createMarketplaceService } from './marketplace.js';
 *
 * const marketplace = createMarketplaceService(store, a2aService);
 * const rfq = await marketplace.broadcastRFQ({
 *   items: [{ description: 'Data analysis', quantity: 1 }],
 *   scoringCriteria: 'best_value',
 *   deadlineMinutes: 30,
 * });
 * ```
 */

import { randomUUID } from 'node:crypto';

/**
 * Scoring functions for comparing RFQ responses.
 * Each function takes (quote, reputation) and returns a numeric score (higher = better).
 */
const SCORING_FUNCTIONS = {
  cheapest: (quote) => {
    const total = quote.total_decimal ?? quote.total ?? Infinity;
    return total === 0 ? 0 : 1 / total;
  },
  best_value: (quote, reputation) => {
    const total = quote.total_decimal ?? quote.total ?? Infinity;
    const priceFactor = total > 0 ? 1 / total : 0;
    const rep = reputation?.average_score ?? 3;
    return rep * 0.4 + priceFactor * 100 * 0.6;
  },
  fastest: (quote) => {
    const total = quote.total_decimal ?? quote.total ?? Infinity;
    const responseTime =
      quote.quoted_at && quote.created_at
        ? new Date(quote.quoted_at).getTime() - new Date(quote.created_at).getTime()
        : Infinity;
    const timeFactor = responseTime > 0 ? 1 / responseTime : 0;
    const priceFactor = total > 0 ? 1 / total : 0;
    return timeFactor * 1000 * 0.5 + priceFactor * 100 * 0.5;
  },
};

/**
 * Create a marketplace service for multi-party RFQ management.
 *
 * @param {import('./store.js').A2AStore} store - A2A store instance
 * @param {Object} a2aService - A2A service instance (for quote operations)
 * @returns {Object} Marketplace service
 */
export function createMarketplaceService(store, a2aService) {
  if (!store) throw new Error('store is required');

  /**
   * Broadcast an RFQ to matching sellers.
   *
   * @param {Object} params
   * @param {Array} params.items - Items to quote on
   * @param {string} [params.sellerFilter] - Category filter for sellers
   * @param {number} [params.maxResponses=10] - Max seller responses
   * @param {number} [params.deadlineMinutes=60] - RFQ deadline in minutes
   * @param {'cheapest'|'best_value'|'fastest'} [params.scoringCriteria='cheapest']
   * @param {string} params.buyerAddress - Buyer wallet address
   * @param {string} [params.buyerAgentId] - Buyer agent ID
   * @returns {Object} Created RFQ with response list
   */
  async function broadcastRFQ(params) {
    const {
      items,
      sellerFilter,
      maxResponses = 10,
      deadlineMinutes = 60,
      scoringCriteria = 'cheapest',
      buyerAddress,
      buyerAgentId,
    } = params;

    if (!buyerAddress) throw new Error('buyerAddress is required');
    if (!items || !Array.isArray(items) || items.length === 0) {
      throw new Error('items array is required and must not be empty');
    }

    const deadline = new Date(Date.now() + deadlineMinutes * 60 * 1000).toISOString();

    // Find matching sellers
    const filter = { active: 1 };
    if (sellerFilter) filter.category = sellerFilter;
    const services = store.listServices(filter);

    // Exclude buyer's own services
    const eligibleServices = services
      .filter((s) => s.agent_address !== buyerAddress)
      .sort(
        (left, right) =>
          String(left.created_at || '').localeCompare(String(right.created_at || '')) ||
          String(left.agent_address || '').localeCompare(String(right.agent_address || '')),
      )
      .slice(0, maxResponses);

    // Create RFQ record
    const rfq = store.createRFQ({
      buyer_address: buyerAddress,
      buyer_agent_id: buyerAgentId || null,
      items: JSON.stringify(items),
      seller_filter: sellerFilter || null,
      max_responses: maxResponses,
      deadline,
      scoring_criteria: scoringCriteria,
    });

    // Request quotes from each seller
    const responses = [];
    for (const svc of eligibleServices) {
      try {
        const quoteResult = a2aService
          ? await a2aService.requestQuote({
              seller: svc.agent_address,
              items,
              message: `RFQ ${rfq.id}: competitive bid request`,
              maxRounds: 1,
            })
          : null;

        const quoteId = quoteResult?.quote?.id || randomUUID();
        const resp = store.createRFQResponse({
          rfq_id: rfq.id,
          seller_address: svc.agent_address,
          quote_id: quoteId,
          status: 'pending',
        });
        responses.push(resp);
      } catch (err) {
        // Log but continue — some sellers may be unavailable
        console.debug(
          `[marketplace] Failed to request quote from ${svc.agent_address}: ${err.message}`,
        );
      }
    }

    return {
      rfq: store.getRFQ(rfq.id),
      responses,
      sellersContacted: responses.length,
      deadline,
    };
  }

  /**
   * Collect and score all RFQ responses.
   *
   * @param {string} rfqId - RFQ ID
   * @returns {Object} Scored responses with rankings
   */
  function collectRFQResponses(rfqId) {
    const rfq = store.getRFQ(rfqId);
    if (!rfq) throw new Error(`RFQ ${rfqId} not found`);

    const responses = store.listRFQResponses({ rfq_id: rfqId });
    const scoringFn = SCORING_FUNCTIONS[rfq.scoring_criteria] || SCORING_FUNCTIONS.cheapest;

    const scored = [];
    for (const resp of responses) {
      // Fetch the actual quote to score it
      const quote = store.getQuote(resp.quote_id);
      if (!quote || quote.status === 'requested') {
        // Quote not yet provided — skip
        scored.push({ ...resp, score: null, quote: null });
        continue;
      }

      // Get seller reputation for best_value scoring
      let reputation = null;
      try {
        reputation = store.getReputationScore(resp.seller_address);
      } catch (repErr) {
        console.debug('reputation lookup skipped:', repErr.message);
      }

      const score = scoringFn(quote, reputation);

      // Update response score
      store.updateRFQResponse(resp.id, { score, status: 'scored' });

      scored.push({
        ...resp,
        score,
        quote,
        reputation,
      });
    }

    // Rank by score (descending, nulls last)
    const ranked = scored
      .filter((r) => r.score !== null)
      .sort((a, b) => b.score - a.score)
      .map((r, i) => {
        store.updateRFQResponse(r.id, { rank: i + 1 });
        return { ...r, rank: i + 1 };
      });

    const unscored = scored.filter((r) => r.score === null);

    return {
      rfqId,
      scoringCriteria: rfq.scoring_criteria,
      ranked,
      unscored,
      totalResponses: responses.length,
      scoredCount: ranked.length,
    };
  }

  /**
   * Award an RFQ to the highest-scored (or specified) winner.
   *
   * @param {string} rfqId - RFQ ID
   * @param {string} [winnerId] - Force a specific response as winner
   * @returns {Object} Award result
   */
  async function awardRFQ(rfqId, winnerId) {
    const rfq = store.getRFQ(rfqId);
    if (!rfq) throw new Error(`RFQ ${rfqId} not found`);
    if (rfq.status !== 'open') throw new Error(`RFQ ${rfqId} is ${rfq.status}, not open`);

    const responses = store.listRFQResponses({ rfq_id: rfqId });
    const scored = responses
      .filter((r) => r.score !== null)
      .sort((a, b) => (b.score || 0) - (a.score || 0));

    if (scored.length === 0) throw new Error('No scored responses to award');

    let winner;
    if (winnerId) {
      winner = scored.find((r) => r.id === winnerId || r.quote_id === winnerId);
      if (!winner) throw new Error(`Winner ${winnerId} not found in scored responses`);
    } else {
      winner = scored[0];
    }

    // Accept the winning quote
    if (a2aService) {
      try {
        await a2aService.acceptQuote(winner.quote_id);
      } catch (err) {
        console.debug(`[marketplace] Accept quote failed: ${err.message}`);
      }
    }

    // Decline all losers
    const losers = scored.filter((r) => r.id !== winner.id);
    for (const loser of losers) {
      if (a2aService) {
        try {
          await a2aService.declineQuote(loser.quote_id, 'Lost in competitive bid');
        } catch (err) {
          console.debug(`[marketplace] Decline quote failed: ${err.message}`);
        }
      }
      store.updateRFQResponse(loser.id, { status: 'declined' });
    }

    // Update winner and RFQ
    store.updateRFQResponse(winner.id, { status: 'awarded' });
    store.updateRFQ(rfqId, {
      status: 'awarded',
      winning_quote_id: winner.quote_id,
      awarded_at: new Date().toISOString(),
    });

    return {
      rfqId,
      winnerId: winner.id,
      winningQuoteId: winner.quote_id,
      winnerAddress: winner.seller_address,
      winnerScore: winner.score,
      losersDeclined: losers.length,
    };
  }

  /**
   * Expire RFQs past their deadline.
   *
   * @returns {Object} Expiry results
   */
  function expireRFQs() {
    const now = new Date().toISOString();
    const openRFQs = store.listRFQs({ status: 'open' });
    let expired = 0;

    for (const rfq of openRFQs) {
      if (rfq.deadline && rfq.deadline < now) {
        store.updateRFQ(rfq.id, {
          status: 'expired',
          closed_at: now,
        });
        expired++;
      }
    }

    return { expired, checked: openRFQs.length };
  }

  /**
   * Get marketplace metrics for a service.
   *
   * @param {string} serviceId - Service ID
   * @returns {Object} Service metrics
   */
  function getServiceMetrics(serviceId) {
    const service = store.getService(serviceId);
    if (!service) throw new Error(`Service ${serviceId} not found`);

    // Compute metrics from quote history
    const allQuotes = store.listQuotes({ seller_address: service.agent_address });
    const total = allQuotes.length;
    const fulfilled = allQuotes.filter((q) => q.status === 'fulfilled').length;
    const accepted = allQuotes.filter((q) => q.status === 'accepted').length;
    const declined = allQuotes.filter((q) => q.status === 'declined').length;

    // Average response time (quoted_at - created_at)
    let totalResponseTime = 0;
    let responseCount = 0;
    for (const q of allQuotes) {
      if (q.quoted_at && q.created_at) {
        const diff = new Date(q.quoted_at).getTime() - new Date(q.created_at).getTime();
        if (diff > 0) {
          totalResponseTime += diff;
          responseCount++;
        }
      }
    }

    // Dispute rate
    let disputeRate = 0;
    try {
      const disputes = store.listDisputes({ filed_against: service.agent_address });
      disputeRate = total > 0 ? disputes.length / total : 0;
    } catch (disputeErr) {
      console.debug('dispute lookup failed:', disputeErr.message);
    }

    return {
      serviceId,
      serviceName: service.name,
      agentAddress: service.agent_address,
      totalTransactions: total,
      fulfilledCount: fulfilled,
      acceptedCount: accepted,
      declinedCount: declined,
      successRate: total > 0 ? fulfilled / total : 0,
      avgResponseTimeMs: responseCount > 0 ? Math.round(totalResponseTime / responseCount) : null,
      disputeRate,
    };
  }

  /**
   * Get aggregated agent status for marketplace visibility.
   *
   * @param {string} agentAddress - Agent wallet address
   * @returns {Object} Agent marketplace status
   */
  function getAgentStatus(agentAddress) {
    const services = store.listServices({ agent_address: agentAddress, active: 1 });
    let reputation = null;
    try {
      reputation = store.getReputationScore(agentAddress);
    } catch (repErr) {
      console.debug('reputation lookup skipped:', repErr.message);
    }

    const activeRFQs = store.listRFQResponses({ seller_address: agentAddress, status: 'pending' });

    return {
      agentAddress,
      activeServices: services.length,
      services: services.map((s) => ({ id: s.id, name: s.name, category: s.category })),
      reputation: reputation || { average_score: 0, trust_tier: 'sandbox', total_transactions: 0 },
      pendingRFQs: activeRFQs.length,
    };
  }

  /**
   * Auto-award open RFQs that have passed their deadline.
   *
   * For each expired RFQ with scored responses, awards to the highest-scored
   * response. RFQs with no scored responses are expired.
   *
   * @returns {Promise<Object>} Auto-award summary
   */
  async function autoAwardExpiredRFQs() {
    const now = new Date().toISOString();
    const openRFQs = store.listRFQs({ status: 'open' });
    let awarded = 0;
    let expired = 0;
    let skipped = 0;
    const awards = [];

    for (const rfq of openRFQs) {
      if (!rfq.deadline || rfq.deadline > now) {
        skipped++;
        continue; // Not past deadline yet
      }

      // Collect and score responses
      const scored = collectRFQResponses(rfq.id);

      if (scored.scoredCount > 0) {
        // Award to highest-scored response
        try {
          const result = await awardRFQ(rfq.id);
          awarded++;
          awards.push({
            rfqId: rfq.id,
            winnerId: result.winnerId,
            winnerAddress: result.winnerAddress,
            winnerScore: result.winnerScore,
          });
        } catch (err) {
          console.warn(`[marketplace] Auto-award failed for RFQ ${rfq.id}:`, err.message);
          // Expire if award fails
          store.updateRFQ(rfq.id, { status: 'expired', closed_at: now });
          expired++;
        }
      } else {
        // No responses — expire
        store.updateRFQ(rfq.id, { status: 'expired', closed_at: now });
        expired++;
      }
    }

    return { awarded, expired, skipped, awards };
  }

  /**
   * Run a full marketplace maintenance tick.
   * Combines auto-award, expiry, and cleanup.
   *
   * @returns {Promise<Object>} Maintenance summary
   */
  async function maintenanceTick() {
    const autoAwardResult = await autoAwardExpiredRFQs();
    return {
      timestamp: new Date().toISOString(),
      ...autoAwardResult,
    };
  }

  return {
    broadcastRFQ,
    collectRFQResponses,
    awardRFQ,
    expireRFQs,
    autoAwardExpiredRFQs,
    maintenanceTick,
    getServiceMetrics,
    getAgentStatus,
  };
}
