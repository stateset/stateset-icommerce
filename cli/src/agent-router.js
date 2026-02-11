// ============================================================================
// Agent Router
// ============================================================================

/**
 * Confidence thresholds for routing decisions
 */
const ROUTING_THRESHOLDS = {
  HIGH_CONFIDENCE: 0.7, // Route with high confidence
  MEDIUM_CONFIDENCE: 0.4, // Route but note alternatives
  LOW_CONFIDENCE: 0.2, // Ambiguous - may need clarification
  MIN_SCORE: 2, // Minimum weighted score to consider a match
};

/**
 * Weighted keywords for agent routing
 * Each keyword has a weight (1-3):
 *   3 = Strong indicator (unique to this agent)
 *   2 = Moderate indicator
 *   1 = Weak indicator (may overlap with others)
 *
 * Format: { keyword: weight }
 */
const AGENT_KEYWORDS_WEIGHTED = {
  checkout: {
    // Strong indicators
    checkout: 3,
    'shopping cart': 3,
    'add to cart': 3,
    'complete checkout': 3,
    'abandoned cart': 3,
    'cart recovery': 3,
    // Moderate indicators
    cart: 2,
    shopping: 2,
    'shipping rate': 2,
    'shipping options': 2,
    'apply discount': 2,
    'coupon code': 2,
    // Weak indicators
    buy: 1,
    purchase: 1,
  },

  orders: {
    // Strong indicators
    'order status': 3,
    'order #': 3,
    'order number': 3,
    'ship order': 3,
    'cancel order': 3,
    'order history': 3,
    'update order': 3,
    'pending orders': 3,
    'order tracking': 3,
    'fulfill order': 3,
    // Moderate indicators
    order: 2,
    ship: 2,
    tracking: 2,
    fulfillment: 2,
    deliver: 2,
    shipping: 2,
    'tracking number': 2,
    shipped: 2,
  },

  inventory: {
    // Strong indicators
    'stock level': 3,
    'inventory count': 3,
    'adjust inventory': 3,
    'reserve stock': 3,
    'inventory item': 3,
    'on-hand': 3,
    allocated: 3,
    'release reservation': 3,
    'confirm reservation': 3,
    // Moderate indicators
    stock: 2,
    inventory: 2,
    restock: 2,
    warehouse: 2,
    sku: 2,
    'available quantity': 2,
    'stock check': 2,
    // Weak indicators
    reserve: 1,
    available: 1,
  },

  returns: {
    // Strong indicators
    'return request': 3,
    rma: 3,
    'return merchandise': 3,
    'approve return': 3,
    'reject return': 3,
    'pending returns': 3,
    'return status': 3,
    // Moderate indicators
    return: 2,
    refund: 2,
    exchange: 2,
    defective: 2,
    damaged: 2,
    'return policy': 2,
    'return label': 2,
    // Weak indicators
    broken: 1,
    'wrong item': 1,
  },

  analytics: {
    // Strong indicators
    analytics: 3,
    'sales report': 3,
    'revenue report': 3,
    forecast: 3,
    'predict demand': 3,
    'top products': 3,
    'best sellers': 3,
    'customer metrics': 3,
    'top customers': 3,
    'inventory health': 3,
    'low stock report': 3,
    'revenue forecast': 3,
    'demand forecast': 3,
    // Moderate indicators
    sales: 2,
    revenue: 2,
    metrics: 2,
    performance: 2,
    trend: 2,
    insight: 2,
    dashboard: 2,
    report: 2,
    aov: 2,
    'average order': 2,
    'lifetime value': 2,
    'vip customers': 2,
    // Weak indicators
    'how is business': 1,
    'how are sales': 1,
  },

  promotions: {
    // Strong indicators
    promotion: 3,
    'create promotion': 3,
    'activate promotion': 3,
    'promo code': 3,
    coupon: 3,
    'create coupon': 3,
    'validate coupon': 3,
    'percent off': 3,
    'percentage off': 3,
    bogo: 3,
    'buy one get one': 3,
    'tiered discount': 3,
    'flash sale': 3,
    // Moderate indicators
    discount: 2,
    sale: 2,
    deal: 2,
    offer: 2,
    campaign: 2,
    'free shipping promotion': 2,
    // Weak indicators
    save: 1,
  },

  subscriptions: {
    // Strong indicators
    subscription: 3,
    'subscription plan': 3,
    'recurring billing': 3,
    'billing cycle': 3,
    'pause subscription': 3,
    'cancel subscription': 3,
    'resume subscription': 3,
    'skip billing': 3,
    subscriber: 3,
    'create subscription': 3,
    'subscription events': 3,
    // Moderate indicators
    subscribe: 2,
    recurring: 2,
    'trial period': 2,
    'monthly plan': 2,
    'annual plan': 2,
    renewal: 2,
    membership: 2,
    // Weak indicators
    trial: 1,
    plan: 1,
    billing: 1,
  },

  storefront: {
    // Strong indicators
    'create store': 3,
    'new store': 3,
    storefront: 3,
    'build store': 3,
    'create website': 3,
    scaffold: 3,
    'ecommerce site': 3,
    'e-commerce site': 3,
    'online store': 3,
    'shop website': 3,
    'nextjs store': 3,
    'react store': 3,
    // Moderate indicators
    website: 2,
    'generate project': 2,
    // Weak indicators
    nextjs: 1,
    react: 1,
  },

  sync: {
    // Strong indicators
    'sync status': 3,
    'sync events': 3,
    'push events': 3,
    'pull events': 3,
    outbox: 3,
    sequencer: 3,
    'event sync': 3,
    'sync lag': 3,
    ves: 3,
    'verifiable event': 3,
    'pending events': 3,
    // Moderate indicators
    sync: 2,
    synchronize: 2,
  },

  manufacturing: {
    // Strong indicators
    bom: 3,
    'bill of materials': 3,
    'work order': 3,
    'create work order': 3,
    'start work order': 3,
    'complete work order': 3,
    manufacturing: 3,
    // Moderate indicators
    production: 2,
    manufacture: 2,
    assembly: 2,
    component: 2,
    yield: 2,
    // Weak indicators
    'build product': 1,
  },

  payments: {
    // Strong indicators
    payment: 3,
    'create payment': 3,
    'complete payment': 3,
    'process payment': 3,
    'payment status': 3,
    'payment method': 3,
    // Moderate indicators
    pay: 2,
    charge: 2,
    capture: 2,
    'credit card': 2,
    ach: 2,
    'digital wallet': 2,
    transaction: 2,
    // Weak indicators (overlap with returns for refund)
    refund: 1,
  },

  shipments: {
    // Strong indicators
    shipment: 3,
    'create shipment': 3,
    'deliver shipment': 3,
    'shipment status': 3,
    carrier: 3,
    'in transit': 3,
    // Moderate indicators
    fedex: 2,
    ups: 2,
    usps: 2,
    dhl: 2,
    parcel: 2,
    package: 2,
    // Weak indicators (overlap with orders)
    delivery: 1,
  },

  suppliers: {
    // Strong indicators
    supplier: 3,
    'create supplier': 3,
    'purchase order': 3,
    'create po': 3,
    'approve purchase order': 3,
    'send purchase order': 3,
    vendor: 3,
    // Moderate indicators
    procurement: 2,
    reorder: 2,
    replenish: 2,
    po: 2,
    // Weak indicators
    supply: 1,
  },

  invoices: {
    // Strong indicators
    invoice: 3,
    'create invoice': 3,
    'send invoice': 3,
    'overdue invoice': 3,
    'record payment': 3,
    'accounts receivable': 3,
    'net 30': 3,
    'net 60': 3,
    // Moderate indicators
    ar: 2,
    'payment terms': 2,
    b2b: 2,
    overdue: 2,
    // Weak indicators
    billing: 1,
  },

  warranties: {
    // Strong indicators
    warranty: 3,
    'create warranty': 3,
    'warranty claim': 3,
    'approve warranty': 3,
    'warranty status': 3,
    guarantee: 3,
    // Moderate indicators
    claim: 2,
    repair: 2,
    replacement: 2,
  },

  currency: {
    // Strong indicators
    'exchange rate': 3,
    'currency conversion': 3,
    'set exchange rate': 3,
    'convert currency': 3,
    'multi-currency': 3,
    'base currency': 3,
    'enable currencies': 3,
    'format currency': 3,
    // Moderate indicators
    currency: 2,
    forex: 2,
    conversion: 2,
    // Weak indicators (too generic alone)
    usd: 1,
    eur: 1,
    gbp: 1,
    jpy: 1,
  },

  tax: {
    // Strong indicators
    'sales tax': 3,
    'calculate tax': 3,
    'tax rate': 3,
    'tax exempt': 3,
    'tax exemption': 3,
    vat: 3,
    gst: 3,
    hst: 3,
    pst: 3,
    'tax jurisdiction': 3,
    nexus: 3,
    'cart tax': 3,
    // Moderate indicators
    tax: 2,
    exemption: 2,
  },
};

/**
 * Negative keywords - reduce score when these appear with the agent's keywords
 * Helps disambiguate overlapping terms
 */
const NEGATIVE_KEYWORDS = {
  checkout: ['return', 'refund', 'analytics', 'report'],
  orders: ['cart', 'checkout', 'warehouse', 'supplier'],
  inventory: ['order status', 'checkout', 'return'],
  returns: ['checkout', 'cart', 'create order'],
  payments: ['subscription', 'billing cycle', 'return'],
  shipments: ['order status', 'inventory'],
  subscriptions: ['one-time', 'single purchase'],
};

/**
 * Determine which agent is best suited for a request
 * @param {string} request - User's request
 * @returns {string} - Agent name
 */
export function routeToAgent(request) {
  const result = routeToAgentWithConfidence(request);
  return result.primary.agent;
}

/**
 * Determine which agent is best suited with confidence scoring
 * @param {string} request - User's request
 * @returns {object} - { primary: { agent, score, confidence, level }, alternatives: [...], ambiguous: boolean }
 */
export function routeToAgentWithConfidence(request) {
  const lower = request.toLowerCase();

  // Score each agent based on weighted keyword matches
  const scores = {};
  let maxPossibleScore = 0;

  for (const [agent, keywords] of Object.entries(AGENT_KEYWORDS_WEIGHTED)) {
    let weightedScore = 0;
    const matchedKeywords = [];
    let agentMaxScore = 0;

    // Calculate weighted score for matches
    for (const [keyword, weight] of Object.entries(keywords)) {
      agentMaxScore += weight;
      if (lower.includes(keyword)) {
        weightedScore += weight;
        matchedKeywords.push({ keyword, weight });
      }
    }

    // Apply negative keyword penalties
    const negatives = NEGATIVE_KEYWORDS[agent] || [];
    for (const negKeyword of negatives) {
      if (lower.includes(negKeyword)) {
        weightedScore -= 1;
      }
    }

    // Ensure score doesn't go negative
    weightedScore = Math.max(0, weightedScore);

    // Calculate confidence as percentage of max possible score
    const confidence = agentMaxScore > 0 ? weightedScore / agentMaxScore : 0;

    // Determine confidence level
    let level = 'none';
    if (confidence >= ROUTING_THRESHOLDS.HIGH_CONFIDENCE) {
      level = 'high';
    } else if (confidence >= ROUTING_THRESHOLDS.MEDIUM_CONFIDENCE) {
      level = 'medium';
    } else if (confidence >= ROUTING_THRESHOLDS.LOW_CONFIDENCE) {
      level = 'low';
    }

    scores[agent] = {
      agent,
      score: weightedScore,
      confidence,
      level,
      matchedKeywords,
      maxPossibleScore: agentMaxScore,
    };

    maxPossibleScore = Math.max(maxPossibleScore, agentMaxScore);
  }

  // Rank agents by weighted score, then by confidence
  const ranked = Object.values(scores)
    .filter(
      (s) =>
        s.score >= ROUTING_THRESHOLDS.MIN_SCORE ||
        s.confidence >= ROUTING_THRESHOLDS.LOW_CONFIDENCE,
    )
    .sort((a, b) => {
      // Primary sort by score
      if (b.score !== a.score) return b.score - a.score;
      // Secondary sort by confidence
      return b.confidence - a.confidence;
    });

  // Determine if routing is ambiguous
  const topScore = ranked[0]?.score || 0;
  const secondScore = ranked[1]?.score || 0;
  const topConfidence = ranked[0]?.confidence || 0;

  // Ambiguous if top two have similar scores and neither is high confidence
  const ambiguous =
    ranked.length >= 2 &&
    Math.abs(topScore - secondScore) <= 2 &&
    topConfidence < ROUTING_THRESHOLDS.HIGH_CONFIDENCE;

  // Default to customer-service if no good matches
  const primary =
    ranked.length > 0 && ranked[0].score >= ROUTING_THRESHOLDS.MIN_SCORE
      ? ranked[0]
      : {
          agent: 'customer-service',
          score: 0,
          confidence: 0,
          level: 'default',
          matchedKeywords: [],
          reason: 'No specific agent matched, using general customer service',
        };

  return {
    primary,
    alternatives: ranked.slice(1, 4),
    ambiguous,
    allScores: scores,
    thresholds: ROUTING_THRESHOLDS,
  };
}
