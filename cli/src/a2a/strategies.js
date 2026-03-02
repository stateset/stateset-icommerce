/**
 * Pluggable Negotiation Strategies for Autonomous AI Agents
 *
 * Each strategy is a factory function that returns an object implementing
 * the strategy interface. Strategies control how agents make autonomous
 * decisions about pricing, negotiation, and purchasing.
 *
 * @example
 * ```javascript
 * import { createNegotiatorStrategy } from './strategies.js';
 *
 * const strategy = createNegotiatorStrategy({
 *   targetDiscount: 0.15,
 *   maxRounds: 3,
 *   walkAwayAbove: 500,
 * });
 *
 * const decision = strategy.evaluateReceivedQuote(quote, { budget });
 * // => { action: 'counter', total: 85, message: 'Can you do $85?' }
 * ```
 *
 * Strategy Interface:
 *   evaluateReceivedQuote(quote, ctx)    → { action: 'accept'|'counter'|'decline', ...params }
 *   evaluateIncomingQuote(quote, ctx)    → { total, fees, tax, terms, message }
 *   evaluateCounterOffer(quote, ctx)     → { action: 'accept'|'revise'|'decline', ...params }
 *   evaluatePaymentRequest(request, ctx) → { action: 'pay'|'decline', reason? }
 */

// ---------------------------------------------------------------------------
// 1. AlwaysAccept — accepts every quote/request unconditionally (testing)
// ---------------------------------------------------------------------------

/**
 * Create a strategy that accepts everything.
 * Useful for testing and for agents that should never decline.
 *
 * @param {Object} [options]
 * @param {number} [options.defaultPrice=10] - Default price when no pricing info available
 * @returns {Object} Strategy
 */
export function createAlwaysAcceptStrategy(options = {}) {
  const { defaultPrice = 10 } = options;

  return {
    name: 'always-accept',

    evaluateReceivedQuote() {
      return { action: 'accept' };
    },

    evaluateIncomingQuote(quote) {
      // Sum item costs, or use default
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;

      const total = itemTotal > 0 ? itemTotal : defaultPrice;
      return {
        total,
        fees: 0,
        tax: 0,
        terms: 'Standard terms. No refunds.',
        message: 'Quote provided — ready to proceed.',
      };
    },

    evaluateCounterOffer() {
      return { action: 'accept' };
    },

    evaluatePaymentRequest() {
      return { action: 'pay' };
    },
  };
}

// ---------------------------------------------------------------------------
// 2. BudgetGated — accept if within budget, price with markup
// ---------------------------------------------------------------------------

/**
 * Create a strategy that gates decisions on budget constraints and applies
 * a configurable markup when pricing services.
 *
 * @param {Object} [options]
 * @param {number} [options.markup=1.3] - Pricing markup multiplier (e.g., 1.3 = 30% margin)
 * @param {number} [options.minMargin=0.1] - Minimum acceptable margin (fraction, e.g., 0.1 = 10%)
 * @param {number} [options.basePrice=50] - Fallback base price when items have no pricing
 * @returns {Object} Strategy
 */
export function createBudgetGatedStrategy(options = {}) {
  const { markup = 1.3, minMargin = 0.1, basePrice = 50 } = options;

  return {
    name: 'budget-gated',

    evaluateReceivedQuote(quote, ctx) {
      const total = quote.total ?? quote.total_decimal ?? 0;
      if (ctx.budget && !ctx.runtime.canAfford(total)) {
        return {
          action: 'decline',
          reason: `Exceeds budget (${total} > available)`,
        };
      }
      return { action: 'accept' };
    },

    evaluateIncomingQuote(quote) {
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;

      const cost = itemTotal > 0 ? itemTotal : basePrice;
      const total = Math.round(cost * markup * 100) / 100;
      const fees = Math.round((total - cost) * 0.2 * 100) / 100; // 20% of margin as fees

      return {
        total,
        fees,
        tax: 0,
        terms: `Standard terms. ${Math.round((markup - 1) * 100)}% service margin included.`,
        message: `Priced at $${total} (${Math.round((markup - 1) * 100)}% margin).`,
      };
    },

    evaluateCounterOffer(quote) {
      const counterTotal = quote.total_decimal ?? quote.total ?? 0;
      // Calculate our cost basis from the original quote items
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;
      const cost = itemTotal > 0 ? itemTotal : basePrice;
      const floor = Math.round(cost * (1 + minMargin) * 100) / 100;

      if (counterTotal >= floor) {
        // Counter is above our minimum margin — accept it
        return { action: 'accept' };
      }

      // Revise to midpoint between counter and our last price
      const lastPrice = quote._lastPrice || cost * markup;
      const midpoint = Math.round(((counterTotal + lastPrice) / 2) * 100) / 100;
      const revised = Math.max(midpoint, floor);

      return {
        action: 'revise',
        total: revised,
        message: `Best I can do is $${revised} (minimum margin: ${Math.round(minMargin * 100)}%).`,
      };
    },

    evaluatePaymentRequest(request, ctx) {
      const amount = request.amount_decimal ?? request.amount ?? 0;
      if (ctx.budget && !ctx.runtime.canAfford(amount)) {
        return {
          action: 'decline',
          reason: `Exceeds budget (${amount} > available)`,
        };
      }
      return { action: 'pay' };
    },
  };
}

// ---------------------------------------------------------------------------
// 3. Negotiator — counter-offer toward a target discount
// ---------------------------------------------------------------------------

/**
 * Create a strategy that negotiates toward a target discount before accepting.
 *
 * @param {Object} [options]
 * @param {number} [options.targetDiscount=0.15] - Target discount fraction (0.15 = 15% off)
 * @param {number} [options.maxRounds=3] - Max negotiation rounds before accepting
 * @param {number} [options.walkAwayAbove=Infinity] - Decline if quote exceeds this
 * @param {number} [options.acceptBelow=0] - Auto-accept if total is under this
 * @param {number} [options.sellerMarkup=1.4] - Markup when acting as seller
 * @param {number} [options.sellerFloor=0.15] - Min margin as seller before declining
 * @returns {Object} Strategy
 */
export function createNegotiatorStrategy(options = {}) {
  const {
    targetDiscount = 0.15,
    maxRounds = 3,
    walkAwayAbove = Infinity,
    acceptBelow = 0,
    sellerMarkup = 1.4,
    sellerFloor = 0.15,
  } = options;

  return {
    name: 'negotiator',

    evaluateReceivedQuote(quote, ctx) {
      const total = quote.total ?? quote.total_decimal ?? 0;

      // Auto-accept cheap quotes
      if (acceptBelow > 0 && total <= acceptBelow) {
        return { action: 'accept' };
      }

      // Walk away from expensive quotes
      if (total > walkAwayAbove) {
        return {
          action: 'decline',
          reason: `Price $${total} exceeds walk-away threshold ($${walkAwayAbove})`,
        };
      }

      // Budget check
      if (ctx.budget && !ctx.runtime.canAfford(total)) {
        return {
          action: 'decline',
          reason: `Cannot afford $${total}`,
        };
      }

      // Check negotiation round
      const round = quote.counter_count || 0;
      if (round >= maxRounds) {
        // Exhausted rounds — accept current price
        return { action: 'accept' };
      }

      // Counter-offer at target discount
      const target = Math.round(total * (1 - targetDiscount) * 100) / 100;
      return {
        action: 'counter',
        total: target,
        message: `Can you do $${target}? That's ${Math.round(targetDiscount * 100)}% off your ask.`,
      };
    },

    evaluateIncomingQuote(quote) {
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;

      const cost = itemTotal > 0 ? itemTotal : 50;
      const total = Math.round(cost * sellerMarkup * 100) / 100;

      return {
        total,
        fees: 0,
        tax: 0,
        terms: 'Standard terms.',
        message: `Quoted at $${total}.`,
      };
    },

    evaluateCounterOffer(quote) {
      const counterTotal = quote.total_decimal ?? quote.total ?? 0;
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;
      const cost = itemTotal > 0 ? itemTotal : 50;
      const floor = Math.round(cost * (1 + sellerFloor) * 100) / 100;

      if (counterTotal >= floor) {
        return { action: 'accept' };
      }

      // Split the difference
      const lastAsk = quote._lastPrice || cost * sellerMarkup;
      const revised = Math.round(Math.max((counterTotal + lastAsk) / 2, floor) * 100) / 100;

      return {
        action: 'revise',
        total: revised,
        message: `How about $${revised}? Meeting you halfway.`,
      };
    },

    evaluatePaymentRequest(request, ctx) {
      const amount = request.amount_decimal ?? request.amount ?? 0;
      if (amount > walkAwayAbove) {
        return { action: 'decline', reason: `Amount $${amount} exceeds threshold` };
      }
      if (ctx.budget && !ctx.runtime.canAfford(amount)) {
        return { action: 'decline', reason: `Cannot afford $${amount}` };
      }
      return { action: 'pay' };
    },
  };
}

// ---------------------------------------------------------------------------
// 4. BestOfN — collect quotes from multiple sellers, pick the best
// ---------------------------------------------------------------------------

/**
 * Create a strategy that collects quotes from multiple sellers and picks
 * the best one based on configurable criteria.
 *
 * This strategy is stateful — it accumulates quotes in an internal map
 * keyed by a request tag. Use `collectQuote()` to feed quotes in, and
 * `selectBest()` to pick the winner once enough are collected.
 *
 * @param {Object} [options]
 * @param {number} [options.minQuotes=2] - Minimum quotes to collect before deciding
 * @param {'cheapest'|'best_value'} [options.selection='cheapest'] - Selection criteria
 * @returns {Object} Strategy with extra methods: collectQuote, selectBest, reset
 */
export function createBestOfNStrategy(options = {}) {
  const { minQuotes = 2, selection = 'cheapest' } = options;

  // Internal state: tag → [{ quote, score }]
  const collected = new Map();

  /**
   * Score a quote based on selection criteria
   */
  function scoreQuote(quote) {
    const total = quote.total ?? quote.total_decimal ?? Infinity;
    switch (selection) {
      case 'cheapest':
        // Lower is better → negate for max-selection
        return -total;
      case 'best_value': {
        // Factor in reputation if available
        const reputation = quote._sellerReputation ?? 3;
        const priceFactor = 1 / Math.max(total, 0.01);
        return reputation * 0.6 + priceFactor * 100 * 0.4;
      }
      default:
        return -total;
    }
  }

  return {
    name: 'best-of-n',

    /**
     * Feed a received quote into the collection for a given request tag.
     * @param {string} tag - Grouping key for quotes from the same request
     * @param {Object} quote - The received quote
     */
    collectQuote(tag, quote) {
      if (!collected.has(tag)) {
        collected.set(tag, []);
      }
      collected.get(tag).push({ quote, score: scoreQuote(quote) });
    },

    /**
     * Check if enough quotes have been collected for a tag.
     * @param {string} tag
     * @returns {boolean}
     */
    hasEnoughQuotes(tag) {
      const quotes = collected.get(tag);
      return quotes !== undefined && quotes !== null && quotes.length >= minQuotes;
    },

    /**
     * Select the best quote for a tag.
     * @param {string} tag
     * @returns {{ winner: Object, losers: Object[] } | null}
     */
    selectBest(tag) {
      const quotes = collected.get(tag);
      if (!quotes || quotes.length === 0) return null;

      // Sort by score descending (higher is better)
      const sorted = [...quotes].sort((a, b) => b.score - a.score);
      return {
        winner: sorted[0].quote,
        losers: sorted.slice(1).map((e) => e.quote),
      };
    },

    /**
     * Reset collected quotes for a tag (or all).
     * @param {string} [tag] - If omitted, clears everything
     */
    reset(tag) {
      if (tag) {
        collected.delete(tag);
      } else {
        collected.clear();
      }
    },

    /**
     * Get collected count for a tag.
     * @param {string} tag
     * @returns {number}
     */
    getCollectedCount(tag) {
      return collected.get(tag)?.length ?? 0;
    },

    // Standard strategy interface:

    evaluateReceivedQuote(quote, ctx) {
      // In BestOfN mode, the runtime should use collectQuote + selectBest
      // rather than evaluating one-at-a-time. But if called directly,
      // defer unless we can't afford it.
      const total = quote.total ?? quote.total_decimal ?? 0;
      if (ctx.budget && !ctx.runtime.canAfford(total)) {
        return { action: 'decline', reason: 'Cannot afford' };
      }
      return { action: 'defer' };
    },

    evaluateIncomingQuote(quote) {
      // BestOfN is a buyer strategy — not expected to price as seller
      // but provide a basic implementation for completeness
      const total = Array.isArray(quote.items)
        ? quote.items.reduce((sum, i) => sum + (i.unit_price || 0) * (i.quantity || 1), 0)
        : 50;
      return { total, fees: 0, tax: 0, terms: 'Standard', message: 'Quote provided.' };
    },

    evaluateCounterOffer() {
      // BestOfN buyer typically doesn't get counter-offers as seller
      return { action: 'accept' };
    },

    evaluatePaymentRequest(request, ctx) {
      const amount = request.amount_decimal ?? request.amount ?? 0;
      if (ctx.budget && !ctx.runtime.canAfford(amount)) {
        return { action: 'decline', reason: 'Cannot afford' };
      }
      return { action: 'pay' };
    },
  };
}

// ---------------------------------------------------------------------------
// 5. ReputationAware — trust-tier gated, reputation-discounted pricing
// ---------------------------------------------------------------------------

/**
 * Trust tier hierarchy for comparison.
 */
const TRUST_TIER_RANK = { sandbox: 0, standard: 1, verified: 2, enterprise: 3 };

/**
 * Create a strategy that factors reputation and trust tiers into decisions.
 *
 * As a buyer: declines quotes from agents below a minimum trust tier,
 * expects reputation-based discounts from verified/enterprise sellers.
 *
 * As a seller: offers lower markup to high-reputation buyers,
 * auto-rates counterparties after fulfillment.
 *
 * @param {Object} [options]
 * @param {string} [options.minTrustTier='standard'] - Decline agents below this tier
 * @param {number} [options.minAvgScore=3.5] - Decline agents with score below this
 * @param {number} [options.reputationDiscount=0.05] - 5% discount for 'verified' sellers
 * @param {number} [options.enterpriseDiscount=0.10] - 10% discount for 'enterprise' sellers
 * @param {number} [options.baseMarkup=1.4] - Base seller markup multiplier
 * @param {number} [options.maxRounds=2] - Max negotiation rounds
 * @param {number} [options.highTrustMarkdown=0.10] - Markup reduction for verified+ buyers
 * @returns {Object} Strategy
 */
export function createReputationAwareStrategy(options = {}) {
  const {
    minTrustTier = 'standard',
    minAvgScore = 3.5,
    reputationDiscount = 0.05,
    enterpriseDiscount = 0.1,
    baseMarkup = 1.4,
    maxRounds = 2,
    highTrustMarkdown = 0.1,
  } = options;

  const minTierRank = TRUST_TIER_RANK[minTrustTier] ?? 1;

  return {
    name: 'reputation-aware',

    evaluateReceivedQuote(quote, ctx) {
      const total = quote.total ?? quote.total_decimal ?? 0;

      // Budget check
      if (ctx.budget && !ctx.runtime.canAfford(total)) {
        return { action: 'decline', reason: `Cannot afford $${total}` };
      }

      // Reputation gate: check seller's trust tier
      const sellerTier = quote._sellerTrustTier || 'sandbox';
      const sellerTierRank = TRUST_TIER_RANK[sellerTier] ?? 0;
      if (sellerTierRank < minTierRank) {
        return {
          action: 'decline',
          reason: `Seller trust tier "${sellerTier}" below minimum "${minTrustTier}"`,
        };
      }

      // Reputation gate: check seller's average score
      const sellerScore = quote._sellerAvgScore ?? 0;
      if (sellerScore > 0 && sellerScore < minAvgScore) {
        return {
          action: 'decline',
          reason: `Seller score ${sellerScore} below minimum ${minAvgScore}`,
        };
      }

      // Negotiation rounds check
      const round = quote.counter_count || 0;
      if (round >= maxRounds) {
        return { action: 'accept' };
      }

      // Apply reputation-based discount expectation
      let discountRate = 0;
      if (sellerTier === 'enterprise') {
        discountRate = enterpriseDiscount;
      } else if (sellerTier === 'verified') {
        discountRate = reputationDiscount;
      }

      if (discountRate > 0 && round === 0) {
        const target = Math.round(total * (1 - discountRate) * 100) / 100;
        return {
          action: 'counter',
          total: target,
          message: `Requesting ${Math.round(discountRate * 100)}% reputation discount (${sellerTier} tier).`,
        };
      }

      return { action: 'accept' };
    },

    evaluateIncomingQuote(quote) {
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;

      const cost = itemTotal > 0 ? itemTotal : 50;

      // Adjust markup based on buyer reputation
      const buyerTier = quote._buyerTrustTier || 'sandbox';
      const buyerTierRank = TRUST_TIER_RANK[buyerTier] ?? 0;
      let effectiveMarkup = baseMarkup;
      if (buyerTierRank >= TRUST_TIER_RANK.verified) {
        effectiveMarkup = baseMarkup - highTrustMarkdown;
      }

      const total = Math.round(cost * effectiveMarkup * 100) / 100;
      const discount =
        buyerTierRank >= TRUST_TIER_RANK.verified
          ? ` (${Math.round(highTrustMarkdown * 100)}% trust discount applied)`
          : '';

      return {
        total,
        fees: 0,
        tax: 0,
        terms: `Standard terms.${discount}`,
        message: `Quoted at $${total}.${discount}`,
      };
    },

    evaluateCounterOffer(quote) {
      const counterTotal = quote.total_decimal ?? quote.total ?? 0;
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;
      const cost = itemTotal > 0 ? itemTotal : 50;

      // Accept more readily from high-reputation buyers
      const buyerTier = quote._buyerTrustTier || 'sandbox';
      const buyerTierRank = TRUST_TIER_RANK[buyerTier] ?? 0;
      const minMargin = buyerTierRank >= TRUST_TIER_RANK.verified ? 0.05 : 0.15;
      const floor = Math.round(cost * (1 + minMargin) * 100) / 100;

      if (counterTotal >= floor) {
        return { action: 'accept' };
      }

      const lastAsk = quote._lastPrice || cost * baseMarkup;
      const revised = Math.round(Math.max((counterTotal + lastAsk) / 2, floor) * 100) / 100;
      return {
        action: 'revise',
        total: revised,
        message: `Best I can do is $${revised}.`,
      };
    },

    evaluatePaymentRequest(request, ctx) {
      const amount = request.amount_decimal ?? request.amount ?? 0;
      if (ctx.budget && !ctx.runtime.canAfford(amount)) {
        return { action: 'decline', reason: `Cannot afford $${amount}` };
      }
      return { action: 'pay' };
    },

    /**
     * Suggest a rating after fulfillment.
     * @param {Object} _quote - The fulfilled quote
     * @returns {{ score: number, comment: string }}
     */
    postFulfillmentRating(_quote) {
      // Default to positive rating — future: check delivery time, response quality
      return { score: 4, comment: 'Transaction completed successfully.' };
    },
  };
}

// ---------------------------------------------------------------------------
// 6. DynamicPricing — volume, reputation, demand, and loyalty adjustments
// ---------------------------------------------------------------------------

/**
 * Create a dynamic pricing strategy that adjusts prices based on volume,
 * buyer reputation, demand surges, peak hours, and loyalty.
 *
 * @param {Object} [config]
 * @param {Array<{minQty: number, discount: number}>} [config.volumeBreaks] - Volume discount tiers
 * @param {Object<string, number>} [config.reputationTiers] - Markup adjustment by trust tier
 * @param {{ start: number, end: number, surgeMultiplier: number }} [config.peakHours] - Peak hours config
 * @param {number} [config.demandSurgeThreshold=10] - Requests/hour to trigger surge
 * @param {number} [config.demandSurgeMultiplier=1.3] - Surge pricing multiplier
 * @param {Object<number, number>} [config.loyaltyTiers] - Transaction count → discount mapping
 * @param {number} [config.baseMarkup=1.3] - Base pricing multiplier
 * @param {number} [config.minMargin=0.05] - Minimum margin floor (fraction)
 * @returns {Object} Strategy
 */
export function createDynamicPricingStrategy(config = {}) {
  const {
    volumeBreaks = [
      { minQty: 10, discount: 0.05 },
      { minQty: 50, discount: 0.1 },
      { minQty: 100, discount: 0.15 },
    ],
    reputationTiers = {
      enterprise: -0.15,
      verified: -0.1,
      standard: 0,
      sandbox: 0.2,
    },
    peakHours = { start: 9, end: 17, surgeMultiplier: 1.2 },
    demandSurgeThreshold = 10,
    demandSurgeMultiplier = 1.3,
    loyaltyTiers = { 5: -0.05, 10: -0.1, 20: -0.15 },
    baseMarkup = 1.3,
    minMargin = 0.05,
  } = config;

  // Internal demand tracking
  const requestTimestamps = [];

  /**
   * Track a new request for demand calculation.
   */
  function trackRequest() {
    const now = Date.now();
    requestTimestamps.push(now);
    // Prune older than 1 hour
    const oneHourAgo = now - 3600000;
    while (requestTimestamps.length > 0 && requestTimestamps[0] < oneHourAgo) {
      requestTimestamps.shift();
    }
  }

  /**
   * Get current requests per hour.
   */
  function getRequestsPerHour() {
    const now = Date.now();
    const oneHourAgo = now - 3600000;
    return requestTimestamps.filter((t) => t >= oneHourAgo).length;
  }

  /**
   * Compute the volume discount for a given total quantity.
   */
  function getVolumeDiscount(totalQty) {
    const sorted = [...volumeBreaks].sort((a, b) => b.minQty - a.minQty);
    for (const tier of sorted) {
      if (totalQty >= tier.minQty) return tier.discount;
    }
    return 0;
  }

  /**
   * Get the reputation-based adjustment.
   */
  function getReputationAdjustment(buyerTier) {
    return reputationTiers[buyerTier] ?? reputationTiers.standard ?? 0;
  }

  /**
   * Check if current time is during peak hours.
   */
  function isPeakHour() {
    const hour = new Date().getHours();
    return hour >= peakHours.start && hour < peakHours.end;
  }

  /**
   * Get the loyalty discount based on transaction count.
   */
  function getLoyaltyDiscount(transactionCount) {
    const thresholds = Object.keys(loyaltyTiers)
      .map(Number)
      .sort((a, b) => b - a);
    for (const threshold of thresholds) {
      if (transactionCount >= threshold) return loyaltyTiers[threshold];
    }
    return 0;
  }

  /**
   * Compute the final markup combining all adjustments.
   */
  function computeEffectiveMarkup(quote) {
    let markup = baseMarkup;

    // 1. Volume discount
    const totalQty = Array.isArray(quote.items)
      ? quote.items.reduce((sum, i) => sum + (i.quantity || 1), 0)
      : 1;
    const volumeDiscount = getVolumeDiscount(totalQty);
    markup -= volumeDiscount;

    // 2. Reputation adjustment
    const buyerTier = quote._buyerTrustTier || 'standard';
    markup += getReputationAdjustment(buyerTier);

    // 3. Peak hours surge
    if (isPeakHour()) {
      markup *= peakHours.surgeMultiplier;
    }

    // 4. Demand surge
    trackRequest();
    if (getRequestsPerHour() >= demandSurgeThreshold) {
      markup *= demandSurgeMultiplier;
    }

    // 5. Loyalty discount
    const txCount = quote._buyerTransactionCount || 0;
    markup += getLoyaltyDiscount(txCount);

    // Floor at 1 + minMargin
    return Math.max(markup, 1 + minMargin);
  }

  return {
    name: 'dynamic-pricing',

    evaluateReceivedQuote(quote, ctx) {
      const total = quote.total ?? quote.total_decimal ?? 0;
      if (ctx.budget && !ctx.runtime.canAfford(total)) {
        return { action: 'decline', reason: `Cannot afford $${total}` };
      }
      return { action: 'accept' };
    },

    evaluateIncomingQuote(quote) {
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;

      const cost = itemTotal > 0 ? itemTotal : 50;
      const effectiveMarkup = computeEffectiveMarkup(quote);
      const total = Math.round(cost * effectiveMarkup * 100) / 100;
      const fees = Math.round((total - cost) * 0.15 * 100) / 100;

      const adjustments = [];
      const totalQty = Array.isArray(quote.items)
        ? quote.items.reduce((sum, i) => sum + (i.quantity || 1), 0)
        : 1;
      const vd = getVolumeDiscount(totalQty);
      if (vd > 0) adjustments.push(`volume -${Math.round(vd * 100)}%`);
      const buyerTier = quote._buyerTrustTier || 'standard';
      const ra = getReputationAdjustment(buyerTier);
      if (ra !== 0) adjustments.push(`reputation ${ra > 0 ? '+' : ''}${Math.round(ra * 100)}%`);
      if (isPeakHour())
        adjustments.push(`peak +${Math.round((peakHours.surgeMultiplier - 1) * 100)}%`);
      if (getRequestsPerHour() >= demandSurgeThreshold) adjustments.push(`demand surge`);
      const ld = getLoyaltyDiscount(quote._buyerTransactionCount || 0);
      if (ld !== 0) adjustments.push(`loyalty ${Math.round(ld * 100)}%`);

      return {
        total,
        fees,
        tax: 0,
        terms: `Dynamic pricing. Effective markup: ${Math.round((effectiveMarkup - 1) * 100)}%.`,
        message: `$${total}${adjustments.length > 0 ? ` (${adjustments.join(', ')})` : ''}`,
      };
    },

    evaluateCounterOffer(quote) {
      const counterTotal = quote.total_decimal ?? quote.total ?? 0;
      const itemTotal = Array.isArray(quote.items)
        ? quote.items.reduce((sum, item) => {
            const price = item.unit_price || item.unitPrice || 0;
            const qty = item.quantity || 1;
            return sum + price * qty;
          }, 0)
        : 0;
      const cost = itemTotal > 0 ? itemTotal : 50;
      const floor = Math.round(cost * (1 + minMargin) * 100) / 100;

      if (counterTotal >= floor) {
        return { action: 'accept' };
      }

      const effectiveMarkup = computeEffectiveMarkup(quote);
      const lastAsk = quote._lastPrice || cost * effectiveMarkup;
      const revised = Math.round(Math.max((counterTotal + lastAsk) / 2, floor) * 100) / 100;

      return {
        action: 'revise',
        total: revised,
        message: `Dynamic pricing floor: $${floor}. Revised to $${revised}.`,
      };
    },

    evaluatePaymentRequest(request, ctx) {
      const amount = request.amount_decimal ?? request.amount ?? 0;
      if (ctx.budget && !ctx.runtime.canAfford(amount)) {
        return { action: 'decline', reason: `Cannot afford $${amount}` };
      }
      return { action: 'pay' };
    },

    // Expose for testing
    getRequestsPerHour,
    getVolumeDiscount,
    getReputationAdjustment,
    getLoyaltyDiscount,
    isPeakHour,
    computeEffectiveMarkup,
  };
}
