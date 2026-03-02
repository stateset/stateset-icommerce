/**
 * Demo Scenarios — Reusable agent-to-agent commerce demonstrations
 *
 * Provides pre-built demo flows that can be invoked from the
 * `stateset-agents demo` CLI or programmatically.
 *
 * @example
 * ```javascript
 * import { runBasicNegotiation } from './demo-scenarios.js';
 * import { A2AStore } from './store.js';
 * import { makeCommerceProxy } from './agent-runtime.js';
 *
 * const store = new A2AStore({ dbPath: ':memory:' });
 * store.init();
 * const commerce = makeCommerceProxy(store);
 * const result = await runBasicNegotiation(commerce);
 * ```
 */

import { randomUUID, randomBytes } from 'node:crypto';
import { createAgentRuntime } from './agent-runtime.js';
import {
  createBudgetGatedStrategy,
  createNegotiatorStrategy,
  createBestOfNStrategy,
  createDynamicPricingStrategy,
} from './strategies.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeWallet() {
  return '0x' + randomBytes(20).toString('hex');
}

function makeSigningKey() {
  return {
    privateKey: randomBytes(32).toString('hex'),
    publicKey: randomBytes(32).toString('hex'),
  };
}

/**
 * Available demo scenario names.
 * @type {string[]}
 */
export const DEMO_SCENARIOS = [
  'basic-negotiation',
  'marketplace',
  'escrow-deal',
  'rfq-competition',
  'workflow-pipeline',
];

// ---------------------------------------------------------------------------
// Scenario 1: Basic Negotiation
// ---------------------------------------------------------------------------

/**
 * Two agents negotiate a quote: seller provides, buyer counter-offers, seller
 * revises, buyer accepts & pays.
 *
 * @param {Object} commerce - Commerce proxy from makeCommerceProxy()
 * @param {Object} [options]
 * @param {Function} [options.log] - Logging function (default: console.log)
 * @param {Object} [options.settlement] - Settlement config { chainId, simulate, configDir }
 * @returns {Promise<Object>} Demo result summary
 */
export async function runBasicNegotiation(commerce, options = {}) {
  const log = options.log || console.log;
  const settlementConfig = options.settlement || null;

  log('[demo] Starting basic negotiation scenario...');

  // Create seller agent
  const seller = createAgentRuntime({
    name: 'DataForge AI',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createNegotiatorStrategy({
      basePrice: 50,
      markup: 1.5,
      minAcceptable: 40,
      concessionRate: 0.15,
    }),
    budget: { daily: 1000, perTransaction: 500 },
    logger: () => {},
  });

  // Create settlement service for buyer if configured
  let buyerSettlement = null;
  if (settlementConfig) {
    const { createSettlementService } = await import('./settlement.js');
    buyerSettlement = createSettlementService({
      chainId: settlementConfig.chainId || 'base',
      agentId: 'demo-buyer-' + randomUUID().slice(0, 8),
      simulate: settlementConfig.simulate !== false,
      configDir: settlementConfig.configDir || '.stateset',
      logger: (...args) => log('[settlement]', ...args),
    });
    log(
      `[demo] Settlement enabled: chain=${buyerSettlement.chainId}, simulate=${buyerSettlement.isSimulation}`,
    );
    try {
      const addr = await buyerSettlement.getAddress();
      log(`[demo] Buyer chain wallet: ${addr}`);
      if (!buyerSettlement.isSimulation) {
        const bal = await buyerSettlement.getBalance();
        log(`[demo] Buyer on-chain balance: ${bal.balance} ${bal.symbol}`);
      }
    } catch (err) {
      log(`[demo] Wallet info: ${err.message}`);
    }
  }

  // Create buyer agent
  const buyer = createAgentRuntime({
    name: 'InsightBot',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createBudgetGatedStrategy({ markup: 1.0 }),
    budget: { daily: 500, perTransaction: 200, startingBalance: 1000 },
    logger: () => {},
    settlement: buyerSettlement,
  });

  // 1. Seller registers a service
  const service = seller.registerService({
    name: 'Sentiment Analysis',
    category: 'analytics',
    description: 'Deep sentiment analysis on customer reviews',
    pricingModel: 'quote',
    pricingDetails: { basePrice: 50 },
  });
  log(`[demo] Seller registered service: ${service.name} (${service.id})`);

  // 2. Buyer discovers and requests a quote
  const quoteResult = await buyer.a2a.requestQuote({
    seller: seller.walletAddress,
    items: [{ description: 'Analyze 10k reviews', quantity: 1 }],
    message: 'Need sentiment analysis on 10k customer reviews',
    maxRounds: 3,
  });
  const quoteId = quoteResult.quote.id;
  log(`[demo] Buyer requested quote: ${quoteId}`);

  // 3. Seller tick — provides quote
  await seller.tick();
  const quotedQuote = commerce.a2a().getQuote(quoteId);
  log(`[demo] Seller provided quote: $${quotedQuote.total_decimal}`);

  // 4. Buyer tick — evaluates and counter-offers or accepts
  await buyer.tick();
  const afterBuyer = commerce.a2a().getQuote(quoteId);
  log(`[demo] Buyer response: status=${afterBuyer.status}`);

  // 5. If countered, seller responds
  if (afterBuyer.status === 'counter_offered') {
    await seller.tick();
    const afterSeller = commerce.a2a().getQuote(quoteId);
    log(`[demo] Seller revised: status=${afterSeller.status}`);

    // 6. Buyer evaluates again
    await buyer.tick();
  }

  const finalQuote = commerce.a2a().getQuote(quoteId);
  log(`[demo] Final quote status: ${finalQuote.status}`);

  // 7. If accepted, seller fulfills
  if (finalQuote.status === 'accepted') {
    await seller.tick();
    const fulfilled = commerce.a2a().getQuote(quoteId);
    log(`[demo] Fulfillment status: ${fulfilled.status}`);
  }

  // Collect settlement info
  let settlementInfo = null;
  if (buyerSettlement) {
    const payments = commerce.a2a().listPayments({ sender_address: buyer.walletAddress });
    const settled = payments.filter((p) => p.tx_hash);
    for (const p of settled) {
      const meta = p.metadata ? JSON.parse(p.metadata) : {};
      log(
        `[demo] Settled on-chain: tx=${p.tx_hash} block=${p.block_number} explorer=${meta.explorer_url || 'N/A'}`,
      );
    }
    settlementInfo = {
      chain: buyerSettlement.chainId,
      simulate: buyerSettlement.isSimulation,
      settledCount: settled.length,
      settlements: settled.map((p) => ({
        paymentId: p.id,
        txHash: p.tx_hash,
        blockNumber: p.block_number,
        status: p.status,
      })),
    };
  }

  // Cleanup
  seller.destroy();
  buyer.destroy();

  return {
    scenario: 'basic-negotiation',
    quoteId,
    finalStatus: commerce.a2a().getQuote(quoteId).status,
    sellerWallet: seller.walletAddress,
    buyerWallet: buyer.walletAddress,
    serviceId: service.id,
    settlement: settlementInfo,
  };
}

// ---------------------------------------------------------------------------
// Scenario 2: Marketplace (3 sellers, best-of-N selection)
// ---------------------------------------------------------------------------

/**
 * Buyer requests quotes from 3 sellers and uses best-of-N strategy to pick
 * the best deal.
 *
 * @param {Object} commerce - Commerce proxy from makeCommerceProxy()
 * @param {Object} [options]
 * @param {Function} [options.log] - Logging function
 * @param {Object} [options.settlement] - Settlement config { chainId, simulate, configDir }
 * @returns {Promise<Object>} Demo result summary
 */
export async function runMarketplace(commerce, options = {}) {
  const log = options.log || console.log;
  const settlementConfig = options.settlement || null;

  log('[demo] Starting marketplace scenario...');

  // Create 3 seller agents with different pricing
  const sellers = [
    { name: 'CheapBot', basePrice: 30, markup: 1.2 },
    { name: 'PremiumBot', basePrice: 80, markup: 1.8 },
    { name: 'MidRangeBot', basePrice: 50, markup: 1.4 },
  ].map(({ name, basePrice, markup }) =>
    createAgentRuntime({
      name,
      walletAddress: makeWallet(),
      signingKey: makeSigningKey(),
      commerce,
      strategy: createNegotiatorStrategy({ basePrice, markup, minAcceptable: basePrice * 0.8 }),
      budget: { daily: 5000 },
      logger: () => {},
    }),
  );

  // Create settlement service for buyer if configured
  let buyerSettlement = null;
  if (settlementConfig) {
    const { createSettlementService } = await import('./settlement.js');
    buyerSettlement = createSettlementService({
      chainId: settlementConfig.chainId || 'base',
      agentId: 'demo-mktbuyer-' + randomUUID().slice(0, 8),
      simulate: settlementConfig.simulate !== false,
      configDir: settlementConfig.configDir || '.stateset',
      logger: (...args) => log('[settlement]', ...args),
    });
    log(`[demo] Settlement enabled: chain=${buyerSettlement.chainId}`);
  }

  // Create buyer with best-of-N
  const buyer = createAgentRuntime({
    name: 'SmartBuyer',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createBestOfNStrategy({ maxQuotes: 3 }),
    budget: { daily: 500, perTransaction: 200, startingBalance: 1000 },
    logger: () => {},
    settlement: buyerSettlement,
  });

  // Each seller registers a service
  const _services = sellers.map((s, i) => {
    const svc = s.registerService({
      name: `Data Analytics ${i + 1}`,
      category: 'analytics',
      description: `Analytics service from ${s.name}`,
      pricingModel: 'quote',
    });
    log(`[demo] ${s.name} registered: ${svc.name}`);
    return svc;
  });

  // Buyer requests quotes from all sellers
  const quoteResults = await Promise.all(
    sellers.map((s) =>
      buyer.a2a.requestQuote({
        seller: s.walletAddress,
        items: [{ description: 'Process 5k records', quantity: 1 }],
        message: 'Need data processing',
      }),
    ),
  );
  const quoteIds = quoteResults.map((r) => r.quote.id);
  log(`[demo] Buyer requested ${quoteIds.length} quotes`);

  // All sellers provide quotes
  for (const s of sellers) {
    await s.tick();
  }

  // Log quotes received
  for (const qId of quoteIds) {
    const quoted = commerce.a2a().getQuote(qId);
    log(`[demo] Quote from seller: $${quoted.total_decimal} (status: ${quoted.status})`);
  }

  // Buyer evaluates all (best-of-N defers until N quotes are ready)
  await buyer.tick();

  // Check results
  const results = quoteIds.map((qId) => commerce.a2a().getQuote(qId));
  const accepted = results.find((q) => q.status === 'accepted');
  const declined = results.filter((q) => q.status === 'declined');

  log(
    `[demo] Buyer accepted: ${accepted ? `$${accepted.total_decimal}` : 'none'}, declined: ${declined.length}`,
  );

  // Collect settlement info
  let settlementInfo = null;
  if (buyerSettlement) {
    const payments = commerce.a2a().listPayments({ sender_address: buyer.walletAddress });
    const settled = payments.filter((p) => p.tx_hash);
    for (const p of settled) {
      log(`[demo] Settled on-chain: tx=${p.tx_hash}`);
    }
    settlementInfo = {
      chain: buyerSettlement.chainId,
      simulate: buyerSettlement.isSimulation,
      settledCount: settled.length,
    };
  }

  // Cleanup
  sellers.forEach((s) => s.destroy());
  buyer.destroy();

  return {
    scenario: 'marketplace',
    quoteIds,
    acceptedQuoteId: accepted?.id || null,
    declinedCount: declined.length,
    sellerCount: sellers.length,
    settlement: settlementInfo,
  };
}

// ---------------------------------------------------------------------------
// Scenario 3: Escrow Deal with Reputation + Split
// ---------------------------------------------------------------------------

/**
 * Buyer and seller negotiate, then use escrow + reputation rating +
 * split payment (platform fee).
 *
 * @param {Object} commerce - Commerce proxy from makeCommerceProxy()
 * @param {Object} [options]
 * @param {Function} [options.log] - Logging function
 * @param {Object} [options.settlement] - Settlement config { chainId, simulate, configDir }
 * @returns {Promise<Object>} Demo result summary
 */
export async function runEscrowDeal(commerce, options = {}) {
  const log = options.log || console.log;
  const settlementConfig = options.settlement || null;

  log('[demo] Starting escrow deal scenario...');

  const platformAddress = makeWallet();

  const seller = createAgentRuntime({
    name: 'EscrowSeller',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createNegotiatorStrategy({ basePrice: 100, markup: 1.3, minAcceptable: 80 }),
    budget: { daily: 5000 },
    logger: () => {},
  });

  // Create settlement service for buyer if configured
  let buyerSettlement = null;
  if (settlementConfig) {
    const { createSettlementService } = await import('./settlement.js');
    buyerSettlement = createSettlementService({
      chainId: settlementConfig.chainId || 'base',
      agentId: 'demo-escrow-buyer-' + randomUUID().slice(0, 8),
      simulate: settlementConfig.simulate !== false,
      configDir: settlementConfig.configDir || '.stateset',
      logger: (...args) => log('[settlement]', ...args),
    });
    log(`[demo] Settlement enabled: chain=${buyerSettlement.chainId}`);
  }

  const buyer = createAgentRuntime({
    name: 'EscrowBuyer',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createBudgetGatedStrategy({ markup: 1.0 }),
    budget: { daily: 1000, perTransaction: 500, startingBalance: 5000 },
    logger: () => {},
    settlement: buyerSettlement,
  });

  // Seller registers service
  seller.registerService({
    name: 'Premium Data Pipeline',
    category: 'data',
    description: 'Enterprise data pipeline setup',
  });
  log('[demo] Seller registered service');

  // Quick negotiation
  const quoteRes = await buyer.a2a.requestQuote({
    seller: seller.walletAddress,
    items: [{ description: 'Setup data pipeline', quantity: 1 }],
    message: 'Need enterprise pipeline',
  });
  const quoteId = quoteRes.quote.id;
  await seller.tick();
  await buyer.tick();

  const quoteState = commerce.a2a().getQuote(quoteId);
  log(`[demo] Quote status: ${quoteState.status}`);

  // Create escrow deal
  let escrowResult = null;
  try {
    escrowResult = await buyer.createEscrowDeal({
      sellerAddress: seller.walletAddress,
      amount: 100,
      conditions: [{ type: 'seller_fulfilled', quoteId }],
      expiresInHours: 24,
    });
    log(`[demo] Escrow created: ${escrowResult.escrow?.id || 'N/A'}`);
  } catch (err) {
    log(`[demo] Escrow creation: ${err.message}`);
  }

  // Create split payment (90% seller, 5% platform)
  let splitResult = null;
  try {
    splitResult = await buyer.createSplitDeal({
      totalAmount: 100,
      recipients: [
        { address: seller.walletAddress, sharePercent: 90 },
        { address: platformAddress, sharePercent: 5 },
      ],
      platformFeePercent: 5,
      platformFeeAddress: platformAddress,
      memo: 'Pipeline deal split',
    });
    log(`[demo] Split payment created: ${splitResult.splitPayment?.id || 'N/A'}`);
  } catch (err) {
    log(`[demo] Split creation: ${err.message}`);
  }

  // Rate counterparty
  let ratingResult = null;
  try {
    ratingResult = await buyer.rateCounterparty({
      ratedAddress: seller.walletAddress,
      score: 5,
      transactionId: quoteId,
      comment: 'Excellent service and fair pricing',
      dimensions: { quality: 5, speed: 4, communication: 5 },
    });
    log(`[demo] Buyer rated seller: ${ratingResult.feedback?.id || 'done'}`);
  } catch (err) {
    log(`[demo] Rating: ${err.message}`);
  }

  // Collect settlement info
  let settlementInfo = null;
  if (buyerSettlement) {
    const payments = commerce.a2a().listPayments({ sender_address: buyer.walletAddress });
    const settled = payments.filter((p) => p.tx_hash);
    settlementInfo = {
      chain: buyerSettlement.chainId,
      simulate: buyerSettlement.isSimulation,
      settledCount: settled.length,
    };
  }

  // Cleanup
  seller.destroy();
  buyer.destroy();

  return {
    scenario: 'escrow-deal',
    quoteId,
    quoteStatus: quoteState.status,
    escrowId: escrowResult?.escrow?.id || null,
    splitId: splitResult?.splitPayment?.id || null,
    ratingGiven: !!ratingResult,
    platformAddress,
    settlement: settlementInfo,
  };
}

// ---------------------------------------------------------------------------
// Scenario 4: RFQ Competition (5 sellers, competitive bidding)
// ---------------------------------------------------------------------------

/**
 * 5 sellers compete in an RFQ broadcast. Buyer uses marketplace service
 * to broadcast, collect, score, and award the best deal.
 *
 * @param {Object} commerce - Commerce proxy from makeCommerceProxy()
 * @param {Object} [options]
 * @param {Function} [options.log] - Logging function
 * @returns {Promise<Object>} Demo result summary
 */
export async function runRFQCompetition(commerce, options = {}) {
  const log = options.log || console.log;

  log('[demo] Starting RFQ competition scenario...');

  // Create 5 sellers with varied strategies
  const sellerConfigs = [
    { name: 'BargainBot', markup: 1.15, basePrice: 30 },
    { name: 'PremiumService', markup: 2.0, basePrice: 80 },
    { name: 'MidTier', markup: 1.4, basePrice: 50 },
    { name: 'FastDelivery', markup: 1.5, basePrice: 45 },
    { name: 'QualityFirst', markup: 1.6, basePrice: 60 },
  ];

  const sellers = sellerConfigs.map(({ name, markup, basePrice: _basePrice }) =>
    createAgentRuntime({
      name,
      walletAddress: makeWallet(),
      signingKey: makeSigningKey(),
      commerce,
      strategy: createDynamicPricingStrategy({ baseMarkup: markup }),
      budget: { daily: 5000 },
      logger: () => {},
    }),
  );

  // Create buyer
  const buyer = createAgentRuntime({
    name: 'CompetitiveBuyer',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createBudgetGatedStrategy({ markup: 1.0 }),
    budget: { daily: 1000, perTransaction: 500, startingBalance: 5000 },
    logger: () => {},
  });

  // Each seller registers a service
  const _services = sellers.map((s, i) => {
    const svc = s.registerService({
      name: `Analytics Service ${i + 1}`,
      category: 'analytics',
      description: `Analytics by ${s.name}`,
      pricingModel: 'quote',
    });
    log(`[demo] ${s.name} registered service: ${svc.name}`);
    return svc;
  });

  // Buyer broadcasts RFQ
  const rfqResult = await buyer.broadcastRFQ({
    items: [{ description: 'Analyze 50k records', quantity: 1 }],
    sellerFilter: 'analytics',
    maxResponses: 5,
    deadlineMinutes: 30,
    scoringCriteria: 'best_value',
  });
  log(`[demo] RFQ broadcast: ${rfqResult.rfq.id}, contacted ${rfqResult.sellersContacted} sellers`);

  // All sellers tick to provide quotes
  for (const s of sellers) {
    await s.tick();
  }
  log('[demo] All sellers provided quotes');

  // Collect and score responses
  const collected = buyer.collectRFQResponses(rfqResult.rfq.id);
  log(`[demo] Scored ${collected.scoredCount}/${collected.totalResponses} responses`);

  for (const r of collected.ranked) {
    log(
      `[demo]   #${r.rank} ${r.seller_address.slice(0, 10)}... score=${r.score?.toFixed(4)} price=$${r.quote?.total_decimal || '?'}`,
    );
  }

  // Award to best
  let awardResult = null;
  if (collected.scoredCount > 0) {
    awardResult = await buyer.awardRFQ(rfqResult.rfq.id);
    log(
      `[demo] Awarded to ${awardResult.winnerAddress.slice(0, 10)}... (score: ${awardResult.winnerScore?.toFixed(4)})`,
    );
  }

  // Cleanup
  sellers.forEach((s) => s.destroy());
  buyer.destroy();

  return {
    scenario: 'rfq-competition',
    rfqId: rfqResult.rfq.id,
    sellersContacted: rfqResult.sellersContacted,
    scoredCount: collected.scoredCount,
    winnerId: awardResult?.winnerId || null,
    winnerAddress: awardResult?.winnerAddress || null,
    winnerScore: awardResult?.winnerScore || null,
    losersDeclined: awardResult?.losersDeclined || 0,
  };
}

// ---------------------------------------------------------------------------
// Scenario 5: Workflow Pipeline (3-agent DAG)
// ---------------------------------------------------------------------------

/**
 * 3 agents form a data pipeline: DataFetcher → Analyzer → ReportGenerator.
 * Demonstrates multi-agent composition with cost tracking.
 *
 * @param {Object} commerce - Commerce proxy from makeCommerceProxy()
 * @param {Object} [options]
 * @param {Function} [options.log] - Logging function
 * @returns {Promise<Object>} Demo result summary
 */
export async function runWorkflowPipeline(commerce, options = {}) {
  const log = options.log || console.log;

  log('[demo] Starting workflow pipeline scenario...');

  // Create 3 pipeline agents
  const fetcher = createAgentRuntime({
    name: 'DataFetcher',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createNegotiatorStrategy({ sellerMarkup: 1.2 }),
    budget: { daily: 5000 },
    logger: () => {},
  });

  const analyzer = createAgentRuntime({
    name: 'Analyzer',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createNegotiatorStrategy({ sellerMarkup: 1.5 }),
    budget: { daily: 5000 },
    logger: () => {},
  });

  const reporter = createAgentRuntime({
    name: 'ReportGenerator',
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    strategy: createNegotiatorStrategy({ sellerMarkup: 1.3 }),
    budget: { daily: 5000 },
    logger: () => {},
  });

  // Register services
  fetcher.registerService({
    name: 'Data Fetching',
    category: 'data',
    description: 'Fetches raw data',
  });
  analyzer.registerService({
    name: 'Data Analysis',
    category: 'analytics',
    description: 'Analyzes data',
  });
  reporter.registerService({
    name: 'Report Generation',
    category: 'reporting',
    description: 'Generates reports',
  });
  log('[demo] All 3 pipeline agents registered');

  // Create workflow
  const wfResult = fetcher.createWorkflow({
    name: 'data-pipeline',
    steps: [
      {
        name: 'fetch',
        type: 'quote_request',
        agentAddress: fetcher.walletAddress,
        params: {
          description: 'Fetch stock market data',
          items: [{ description: 'Stock data fetch', quantity: 1 }],
        },
      },
      {
        name: 'analyze',
        type: 'quote_request',
        agentAddress: analyzer.walletAddress,
        params: {
          description: 'Analyze fetched data',
          items: [{ description: 'Data analysis', quantity: 1 }],
        },
        dependsOn: ['fetch'],
      },
      {
        name: 'report',
        type: 'transform',
        params: { transformType: 'aggregate' },
        dependsOn: ['fetch', 'analyze'],
      },
    ],
  });
  log(`[demo] Workflow created: ${wfResult.workflow.id} with ${wfResult.steps.length} steps`);
  log(`[demo] Execution order: ${wfResult.executionOrder.join(' → ')}`);

  // Tick agents so they can respond to quote requests
  // (In a real system, agents would be running their loops)
  for (const agent of [fetcher, analyzer, reporter]) {
    await agent.tick();
  }

  // Execute workflow
  const execResult = await fetcher.executeWorkflow(wfResult.workflow.id);
  log(`[demo] Workflow status: ${execResult.status}`);
  log(`[demo] Total cost: $${execResult.totalCost}`);
  log(`[demo] Completed steps: ${execResult.completedSteps}`);

  // Get final status
  const status = fetcher.getWorkflowStatus(wfResult.workflow.id);

  // Cleanup
  fetcher.destroy();
  analyzer.destroy();
  reporter.destroy();

  return {
    scenario: 'workflow-pipeline',
    workflowId: wfResult.workflow.id,
    status: execResult.status,
    totalCost: execResult.totalCost,
    completedSteps: execResult.completedSteps,
    stepDetails: status.steps.map((s) => ({
      name: s.name,
      type: s.type,
      status: s.status,
      cost: s.cost,
    })),
  };
}

/**
 * Run a named demo scenario.
 * @param {string} scenarioName - One of DEMO_SCENARIOS
 * @param {Object} commerce - Commerce proxy
 * @param {Object} [options]
 * @returns {Promise<Object>}
 */
export async function runDemoScenario(scenarioName, commerce, options = {}) {
  switch (scenarioName) {
    case 'basic-negotiation':
      return runBasicNegotiation(commerce, options);
    case 'marketplace':
      return runMarketplace(commerce, options);
    case 'escrow-deal':
      return runEscrowDeal(commerce, options);
    case 'rfq-competition':
      return runRFQCompetition(commerce, options);
    case 'workflow-pipeline':
      return runWorkflowPipeline(commerce, options);
    default:
      throw new Error(
        `Unknown demo scenario: ${scenarioName}. Available: ${DEMO_SCENARIOS.join(', ')}`,
      );
  }
}
