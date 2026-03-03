/**
 * Agent Runtime — Autonomous AI Agent Lifecycle Manager
 *
 * Wraps an A2A service with budget enforcement, pluggable negotiation
 * strategies, service registration/discovery, and an event-driven
 * service loop for processing incoming quotes and payment requests.
 *
 * @example
 * ```javascript
 * import { createAgentRuntime, makeCommerceProxy } from './agent-runtime.js';
 * import { createBudgetGatedStrategy } from './strategies.js';
 * import { A2AStore } from './store.js';
 *
 * const store = new A2AStore({ dbPath: './demo-a2a.db' });
 * store.init();
 * const commerce = makeCommerceProxy(store);
 *
 * const runtime = createAgentRuntime({
 *   name: 'DataForge AI',
 *   walletAddress: '0x1234...',
 *   signingKey: { privateKey: '...', publicKey: '...' },
 *   commerce,
 *   budget: { perTransaction: 100, daily: 500 },
 *   strategy: createBudgetGatedStrategy({ markup: 1.3 }),
 * });
 *
 * runtime.registerService({ name: 'Sentiment Analysis', category: 'analytics', ... });
 * await runtime.tick(); // process one cycle
 * ```
 */

import { randomUUID } from 'node:crypto';
import { EventEmitter } from 'node:events';
import { createA2AService } from './index.js';
import { createAlwaysAcceptStrategy } from './strategies.js';
import { createReputationService } from './reputation.js';
import { createA2ASubscriptionService } from './subscriptions.js';
import { createSplitPaymentService } from './splits.js';
import { createMarketplaceService } from './marketplace.js';
import { createSLAService } from './sla.js';
import { createWorkflowService } from './workflows.js';

// ---------------------------------------------------------------------------
// Commerce Proxy Helper — shared across all demos and tests
// ---------------------------------------------------------------------------

/**
 * Build a commerce proxy object from an A2AStore instance.
 * This is the same shape that mcp-server.js builds internally.
 *
 * @param {import('./store.js').A2AStore} a2aStore
 * @returns {Object} Commerce-compatible proxy
 */
export function makeCommerceProxy(a2aStore) {
  return {
    a2a: () => ({
      // Payments
      createPayment: (p) => a2aStore.createPayment(p),
      getPayment: (id) => a2aStore.getPayment(id),
      updatePayment: (id, u) => a2aStore.updatePayment(id, u),
      listPayments: (f) => a2aStore.listPayments(f),
      sumPayments: (f) => a2aStore.sumPayments(f),
      // Payment Requests
      createPaymentRequest: (r) => a2aStore.createPaymentRequest(r),
      getPaymentRequest: (id) => a2aStore.getPaymentRequest(id),
      updatePaymentRequest: (id, u) => a2aStore.updatePaymentRequest(id, u),
      listPaymentRequests: (f) => a2aStore.listPaymentRequests(f),
      // Quotes
      createQuote: (q) => a2aStore.createQuote(q),
      getQuote: (id) => a2aStore.getQuote(id),
      updateQuote: (id, u) => a2aStore.updateQuote(id, u),
      listQuotes: (f) => a2aStore.listQuotes(f),
      // Escrow
      createEscrow: (e) => a2aStore.createEscrow(e),
      getEscrow: (id) => a2aStore.getEscrow(id),
      updateEscrow: (id, u) => a2aStore.updateEscrow(id, u),
      listEscrows: (f) => a2aStore.listEscrows(f),
      // Feedback / Reputation
      createFeedback: (f) => a2aStore.createFeedback(f),
      getFeedback: (id) => a2aStore.getFeedback(id),
      updateFeedback: (id, u) => a2aStore.updateFeedback(id, u),
      listFeedback: (f) => a2aStore.listFeedback(f),
      getReputationScore: (addr) => a2aStore.getReputationScore(addr),
      upsertReputationScore: (s) => a2aStore.upsertReputationScore(s),
      // Services
      createService: (s) => a2aStore.createService(s),
      getService: (id) => a2aStore.getService(id),
      updateService: (id, u) => a2aStore.updateService(id, u),
      listServices: (f) => a2aStore.listServices(f),
      // Disputes
      createDispute: (d) => a2aStore.createDispute(d),
      getDispute: (id) => a2aStore.getDispute(id),
      updateDispute: (id, u) => a2aStore.updateDispute(id, u),
      listDisputes: (f) => a2aStore.listDisputes(f),
      createEvidence: (e) => a2aStore.createEvidence(e),
      getEvidence: (id) => a2aStore.getEvidence(id),
      listEvidenceByDispute: (id) => a2aStore.listEvidenceByDispute(id),
      // Subscriptions
      createSubscription: (s) => a2aStore.createSubscription(s),
      getSubscription: (id) => a2aStore.getSubscription(id),
      updateSubscription: (id, u) => a2aStore.updateSubscription(id, u),
      listSubscriptions: (f) => a2aStore.listSubscriptions(f),
      getDueSubscriptions: (now) => a2aStore.getDueSubscriptions(now),
      getExpiredTrials: (now) => a2aStore.getExpiredTrials(now),
      // Splits
      createSplitPayment: (s) => a2aStore.createSplitPayment(s),
      getSplitPayment: (id) => a2aStore.getSplitPayment(id),
      updateSplitPayment: (id, u) => a2aStore.updateSplitPayment(id, u),
      listSplitPayments: (f) => a2aStore.listSplitPayments(f),
      createSplitRecipient: (r) => a2aStore.createSplitRecipient(r),
      getSplitRecipient: (id) => a2aStore.getSplitRecipient(id),
      updateSplitRecipient: (id, u) => a2aStore.updateSplitRecipient(id, u),
      listSplitRecipients: (f) => a2aStore.listSplitRecipients(f),
      // Notifications
      createNotificationLog: (n) => a2aStore.createNotificationLog(n),
      getNotificationLog: (id) => a2aStore.getNotificationLog(id),
      updateNotificationLog: (id, u) => a2aStore.updateNotificationLog(id, u),
      listNotificationLog: (f) => a2aStore.listNotificationLog(f),
      getPendingNotifications: (max, lim) => a2aStore.getPendingNotifications(max, lim),
      upsertWebhookConfig: (c) => a2aStore.upsertWebhookConfig(c),
      getWebhookConfig: (addr) => a2aStore.getWebhookConfig(addr),
      listWebhookConfigs: (f) => a2aStore.listWebhookConfigs(f),
      // Events
      createEventSubscription: (s) => a2aStore.createEventSubscription(s),
      getEventSubscription: (id) => a2aStore.getEventSubscription(id),
      updateEventSubscription: (id, u) => a2aStore.updateEventSubscription(id, u),
      listEventSubscriptions: (f) => a2aStore.listEventSubscriptions(f),
      createEventLog: (e) => a2aStore.createEventLog(e),
      getEventLog: (id) => a2aStore.getEventLog(id),
      listEventLog: (f) => a2aStore.listEventLog(f),
      // RFQs
      createRFQ: (r) => a2aStore.createRFQ(r),
      getRFQ: (id) => a2aStore.getRFQ(id),
      updateRFQ: (id, u) => a2aStore.updateRFQ(id, u),
      listRFQs: (f) => a2aStore.listRFQs(f),
      createRFQResponse: (r) => a2aStore.createRFQResponse(r),
      getRFQResponse: (id) => a2aStore.getRFQResponse(id),
      updateRFQResponse: (id, u) => a2aStore.updateRFQResponse(id, u),
      listRFQResponses: (f) => a2aStore.listRFQResponses(f),
      // SLAs
      createSLADefinition: (s) => a2aStore.createSLADefinition(s),
      getSLADefinition: (id) => a2aStore.getSLADefinition(id),
      updateSLADefinition: (id, u) => a2aStore.updateSLADefinition(id, u),
      listSLADefinitions: (f) => a2aStore.listSLADefinitions(f),
      createSLAViolation: (v) => a2aStore.createSLAViolation(v),
      getSLAViolation: (id) => a2aStore.getSLAViolation(id),
      updateSLAViolation: (id, u) => a2aStore.updateSLAViolation(id, u),
      listSLAViolations: (f) => a2aStore.listSLAViolations(f),
      // Workflows
      createWorkflow: (w) => a2aStore.createWorkflow(w),
      getWorkflow: (id) => a2aStore.getWorkflow(id),
      updateWorkflow: (id, u) => a2aStore.updateWorkflow(id, u),
      listWorkflows: (f) => a2aStore.listWorkflows(f),
      createWorkflowStep: (s) => a2aStore.createWorkflowStep(s),
      getWorkflowStep: (id) => a2aStore.getWorkflowStep(id),
      updateWorkflowStep: (id, u) => a2aStore.updateWorkflowStep(id, u),
      listWorkflowSteps: (f) => a2aStore.listWorkflowSteps(f),
    }),
    // x402 — agent card methods proxied from A2AStore
    x402: () => ({
      getAgent: (id) => a2aStore.getAgent?.(id) ?? null,
      getAgentByWallet: (addr) => a2aStore.getAgentByWallet?.(addr) ?? null,
      registerAgent: (card) => a2aStore.registerAgent?.(card) ?? null,
      listAgents: (f) => a2aStore.listAgents?.(f) ?? [],
      discoverAgents: (...args) => a2aStore.discoverAgents?.(...args) ?? [],
      verifyAgent: (id) => a2aStore.verifyAgent?.(id) ?? null,
      updateAgent: (id, u) => a2aStore.updateAgent?.(id, u) ?? null,
    }),
  };
}

// ---------------------------------------------------------------------------
// Budget Helpers
// ---------------------------------------------------------------------------

function todayKey() {
  return new Date().toISOString().slice(0, 10); // YYYY-MM-DD
}

function monthKey() {
  return new Date().toISOString().slice(0, 7); // YYYY-MM
}

// ---------------------------------------------------------------------------
// Agent Runtime Factory
// ---------------------------------------------------------------------------

/**
 * Create an autonomous agent runtime.
 *
 * @param {Object} params
 * @param {string} params.name - Human-readable agent name
 * @param {string} params.walletAddress - Agent wallet address (0x...)
 * @param {Object} params.signingKey - { privateKey, publicKey } hex strings
 * @param {string} [params.agentId] - UUID (auto-generated if omitted)
 * @param {Object} params.commerce - Commerce proxy from makeCommerceProxy()
 * @param {Object} [params.budget] - Budget constraints
 * @param {number} [params.budget.perTransaction=Infinity] - Max per-txn
 * @param {number} [params.budget.daily=Infinity] - Max daily spend
 * @param {number} [params.budget.monthly=Infinity] - Max monthly spend
 * @param {number} [params.budget.startingBalance=null] - Starting balance
 * @param {Object} [params.strategy] - Negotiation strategy (from strategies.js)
 * @param {number} [params.pollIntervalMs=5000] - Service loop poll interval
 * @param {Function} [params.logger] - Logging function
 * @param {boolean} [params.autoRegisterCard=false] - Auto-register agent card on creation
 * @param {string} [params.agentDescription] - Description for the agent card
 * @param {string[]} [params.agentSkills] - A2A skills for the agent card
 * @param {string[]} [params.supportedNetworks] - Supported networks for the card
 * @param {string[]} [params.supportedAssets] - Supported assets for the card
 * @returns {Object} Agent runtime instance
 */
export function createAgentRuntime(params) {
  const {
    name,
    walletAddress,
    signingKey,
    agentId = randomUUID(),
    commerce,
    budget: budgetConfig = {},
    strategy: initialStrategy,
    pollIntervalMs = 5000,
    logger = console.debug,
    autoRegisterCard = false,
    agentDescription = '',
    agentSkills = ['buy', 'sell', 'quote'],
    supportedNetworks = ['set_chain'],
    supportedAssets = ['USDC'],
    settlement = null,
  } = params;

  if (!walletAddress) throw new Error('walletAddress is required');
  if (!commerce) throw new Error('commerce is required');

  // -------------------------------------------------------------------------
  // Event emitter
  // -------------------------------------------------------------------------
  const emitter = new EventEmitter();

  // -------------------------------------------------------------------------
  // A2A service (underlying payment/quote layer)
  // -------------------------------------------------------------------------
  const a2a = createA2AService(commerce, {
    agentId,
    walletAddress,
    signingKey: signingKey || { privateKey: '', publicKey: '' },
    defaultAsset: 'USDC',
    defaultNetwork: 'set_chain',
  });

  // -------------------------------------------------------------------------
  // Budget state
  // -------------------------------------------------------------------------
  const budgetLimits = {
    perTransaction: budgetConfig.perTransaction ?? Infinity,
    daily: budgetConfig.daily ?? Infinity,
    monthly: budgetConfig.monthly ?? Infinity,
  };

  const budgetState = {
    spentToday: 0,
    spentThisMonth: 0,
    balance: budgetConfig.startingBalance ?? null,
    lastDayReset: todayKey(),
    lastMonthReset: monthKey(),
    history: [],
  };

  function rolloverBudget() {
    const today = todayKey();
    const month = monthKey();
    if (budgetState.lastDayReset !== today) {
      budgetState.spentToday = 0;
      budgetState.lastDayReset = today;
    }
    if (budgetState.lastMonthReset !== month) {
      budgetState.spentThisMonth = 0;
      budgetState.lastMonthReset = month;
    }
  }

  function canAfford(amount) {
    rolloverBudget();
    if (amount > budgetLimits.perTransaction) return false;
    if (budgetState.spentToday + amount > budgetLimits.daily) return false;
    if (budgetState.spentThisMonth + amount > budgetLimits.monthly) return false;
    if (budgetState.balance !== null && amount > budgetState.balance) return false;
    return true;
  }

  function recordSpend(amount, metadata = {}) {
    rolloverBudget();
    budgetState.spentToday += amount;
    budgetState.spentThisMonth += amount;
    if (budgetState.balance !== null) {
      budgetState.balance -= amount;
    }
    budgetState.history.push({
      amount,
      timestamp: new Date().toISOString(),
      ...metadata,
    });

    // Budget warning at 80%
    if (budgetLimits.daily !== Infinity && budgetState.spentToday > budgetLimits.daily * 0.8) {
      emitter.emit('budget:warning', {
        type: 'daily',
        spent: budgetState.spentToday,
        limit: budgetLimits.daily,
      });
    }
  }

  function getBudget() {
    rolloverBudget();
    return {
      ...budgetLimits,
      spentToday: budgetState.spentToday,
      spentThisMonth: budgetState.spentThisMonth,
      balance: budgetState.balance,
      remainingDaily: budgetLimits.daily - budgetState.spentToday,
      remainingMonthly: budgetLimits.monthly - budgetState.spentThisMonth,
    };
  }

  // -------------------------------------------------------------------------
  // Strategy
  // -------------------------------------------------------------------------
  let strategy = initialStrategy || createAlwaysAcceptStrategy();

  // -------------------------------------------------------------------------
  // Service management
  // -------------------------------------------------------------------------
  const registeredServiceIds = new Set();

  function registerService(config) {
    const service = commerce.a2a().createService({
      id: config.id || randomUUID(),
      agent_address: walletAddress,
      name: config.name,
      description: config.description || '',
      category: config.category || 'other',
      pricing_model: config.pricingModel || 'quote',
      pricing_details: config.pricingDetails ? JSON.stringify(config.pricingDetails) : null,
      active: 1,
      input_schema: config.inputSchema ? JSON.stringify(config.inputSchema) : null,
      output_schema: config.outputSchema ? JSON.stringify(config.outputSchema) : null,
      endpoint_url: config.endpointUrl || null,
      avg_response_time: config.avgResponseTime || null,
      success_rate: config.successRate || null,
      transaction_count: 0,
      metadata: config.metadata ? JSON.stringify(config.metadata) : null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });
    registeredServiceIds.add(service.id);
    emitter.emit('service:registered', { service });
    return service;
  }

  function listMyServices() {
    return commerce.a2a().listServices({ agent_address: walletAddress });
  }

  function discoverServices(filter = {}) {
    return commerce.a2a().listServices({ ...filter, active: 1 });
  }

  // -------------------------------------------------------------------------
  // Decision making (delegates to strategy)
  // -------------------------------------------------------------------------
  const ctx = () => ({
    runtime: { canAfford, getBudget: getBudget },
    budget: getBudget(),
  });

  function evaluateQuote(quote) {
    return strategy.evaluateReceivedQuote(quote, ctx());
  }

  function evaluatePaymentRequest(request) {
    return strategy.evaluatePaymentRequest(request, ctx());
  }

  // -------------------------------------------------------------------------
  // Advanced Services (lazy-initialized)
  // -------------------------------------------------------------------------
  let reputationSvc = null;
  let subscriptionSvc = null;
  let splitsSvc = null;
  let marketplaceSvc = null;
  let slaSvc = null;
  let workflowSvc = null;
  const circuitBreakerSvc = params.circuitBreaker || null;

  function getReputationSvc() {
    if (!reputationSvc) reputationSvc = createReputationService(commerce.a2a());
    return reputationSvc;
  }

  function getSubscriptionSvc() {
    if (!subscriptionSvc) subscriptionSvc = createA2ASubscriptionService(commerce.a2a());
    return subscriptionSvc;
  }

  function getSplitsSvc() {
    if (!splitsSvc) splitsSvc = createSplitPaymentService(commerce.a2a());
    return splitsSvc;
  }

  function getMarketplaceSvc() {
    if (!marketplaceSvc) marketplaceSvc = createMarketplaceService(commerce.a2a(), a2a);
    return marketplaceSvc;
  }

  function getSLASvc() {
    if (!slaSvc) slaSvc = createSLAService(commerce.a2a());
    return slaSvc;
  }

  function getWorkflowSvc() {
    if (!workflowSvc) workflowSvc = createWorkflowService(commerce.a2a(), a2a);
    return workflowSvc;
  }

  // ── Escrow ──

  async function createEscrowDeal(params) {
    const { sellerAddress, amount, conditions, expiresInHours = 72 } = params;
    if (!canAfford(amount)) {
      throw new Error(`Cannot afford $${amount} (budget exceeded)`);
    }
    const result = await a2a.createConditionalPayment({
      sellerAddress,
      amount,
      conditions,
      expiresInHours,
    });
    recordSpend(amount, { type: 'escrow', escrowId: result.escrow?.id });
    emitter.emit('escrow:created', { escrow: result.escrow, amount });
    return result;
  }

  // ── Reputation ──

  async function rateCounterparty(params) {
    const { ratedAddress, score, transactionId, comment, dimensions } = params;
    const feedback = await getReputationSvc().rateAgent({
      agentAddress: ratedAddress,
      reviewerAddress: walletAddress,
      transactionType: 'quote',
      transactionId: transactionId || randomUUID(),
      score: score,
      dimensions: dimensions || {},
      comment: comment || '',
    });
    emitter.emit('reputation:rated', { ratedAddress, score, feedback });
    return { feedback };
  }

  async function getReputation(address) {
    const rep = await getReputationSvc().getReputation(address);
    const summary = await getReputationSvc().getFeedbackSummary(address);
    return { reputation: rep, summary };
  }

  // ── Subscriptions ──

  async function subscribeTo(params) {
    const { providerAddress, planName, amount, interval = 'monthly', trialDays = 0 } = params;
    if (!canAfford(amount)) {
      throw new Error(`Cannot afford subscription of $${amount}`);
    }
    const result = await getSubscriptionSvc().createSubscription({
      subscriberAddress: walletAddress,
      providerAddress,
      planName,
      amount,
      asset: 'USDC',
      network: 'set_chain',
      billingInterval: interval,
      trialDays,
    });
    emitter.emit('subscription:created', { subscription: result.subscription });
    return result;
  }

  async function pauseSubscription(id) {
    const result = await getSubscriptionSvc().pauseSubscription(id);
    emitter.emit('subscription:paused', { subscriptionId: id });
    return result;
  }

  async function resumeSubscription(id) {
    const result = await getSubscriptionSvc().resumeSubscription(id);
    emitter.emit('subscription:resumed', { subscriptionId: id });
    return result;
  }

  async function cancelSubscription(id, opts = {}) {
    const result = await getSubscriptionSvc().cancelSubscription(id, opts);
    emitter.emit('subscription:cancelled', { subscriptionId: id });
    return result;
  }

  async function processSubscriptionBilling() {
    const result = await getSubscriptionSvc().processBilling();
    if (result.totalBilled > 0) {
      emitter.emit('subscription:billed', {
        count: result.billingCount,
        total: result.totalBilled,
      });
    }
    return result;
  }

  // ── Split Payments ──

  async function createSplitDeal(params) {
    const { totalAmount, recipients, platformFeePercent, platformFeeAddress, memo } = params;
    if (!canAfford(totalAmount)) {
      throw new Error(`Cannot afford split of $${totalAmount}`);
    }
    const result = await getSplitsSvc().createSplitPayment({
      senderAddress: walletAddress,
      totalAmount,
      asset: 'USDC',
      network: 'set_chain',
      splitType: 'percentage',
      platformFeePercent: platformFeePercent || 0,
      platformFeeAddress: platformFeeAddress || null,
      recipients,
      memo: memo || '',
    });
    emitter.emit('split:created', { splitPayment: result.splitPayment });
    return result;
  }

  async function executeSplitDeal(splitId) {
    const result = await getSplitsSvc().executeSplitPayment(
      splitId,
      async (to, amount, asset, splitMemo) => {
        return a2a.pay({ to, amount, asset, memo: splitMemo });
      },
    );
    emitter.emit('split:executed', { splitId, result });
    return result;
  }

  // -------------------------------------------------------------------------
  // Agent Card Management
  // -------------------------------------------------------------------------
  let cachedCard = null;

  /**
   * Ensure an agent card exists for this runtime. Creates one if missing.
   * @returns {{ card: Object, created: boolean }}
   */
  function ensureAgentCard() {
    const x402 = commerce.x402();
    const existing = x402.getAgentByWallet(walletAddress);
    if (existing) {
      cachedCard = existing;
      emitter.emit('card:exists', { card: existing });
      return { card: existing, created: false };
    }
    const card = x402.registerAgent({
      id: agentId,
      name,
      wallet_address: walletAddress,
      public_key: signingKey?.publicKey || null,
      supported_networks: supportedNetworks,
      supported_assets: supportedAssets,
      a2a_skills: agentSkills,
      description: agentDescription,
      trust_level: 'sandbox',
    });
    cachedCard = card;
    emitter.emit('card:registered', { card });
    return { card, created: true };
  }

  /**
   * Check if the agent card is active (not suspended).
   * @returns {{ active: boolean, card: Object|null, reason?: string }}
   */
  function checkCardActive() {
    const x402 = commerce.x402();
    const card = x402.getAgentByWallet(walletAddress);
    if (!card) {
      emitter.emit('card:missing', { walletAddress });
      return { active: false, card: null, reason: 'card_not_found' };
    }
    if (!card.active || card.suspended_at) {
      emitter.emit('card:suspended', { card });
      return { active: false, card, reason: 'suspended' };
    }
    cachedCard = card;
    return { active: true, card };
  }

  /**
   * Get the cached or freshly fetched agent card.
   * @returns {Object|null}
   */
  function getAgentCard() {
    if (cachedCard) return cachedCard;
    const x402 = commerce.x402();
    cachedCard = x402.getAgentByWallet(walletAddress) || null;
    return cachedCard;
  }

  // Auto-register card on creation if requested
  if (autoRegisterCard) {
    try {
      ensureAgentCard();
    } catch (err) {
      logger(`[${name}] Auto-register card failed: ${err.message}`);
    }
  }

  // -------------------------------------------------------------------------
  // Service Loop
  // -------------------------------------------------------------------------
  let loopTimer = null;
  let running = false;
  // Track which quote IDs we've already processed to avoid re-processing
  const processedQuoteIds = new Set();

  /**
   * Process one cycle of the service loop.
   * This is the core autonomous behavior.
   */
  async function tick() {
    let processed = 0;

    // Check card status before processing (skip cycle if suspended)
    if (autoRegisterCard) {
      const cardStatus = checkCardActive();
      if (!cardStatus.active) {
        emitter.emit('loop:tick', { processed: 0, skipped: true, reason: cardStatus.reason });
        return 0;
      }
    }

    try {
      // 1. Seller: respond to incoming quote requests
      const pendingRequests = commerce.a2a().listQuotes({
        seller_address: walletAddress,
        status: 'requested',
      });

      for (const quote of pendingRequests) {
        if (processedQuoteIds.has(quote.id + ':requested')) continue;
        processedQuoteIds.add(quote.id + ':requested');

        try {
          emitter.emit('quote:received', { quote, role: 'seller' });
          const pricing = strategy.evaluateIncomingQuote(quote, ctx());
          await a2a.provideQuote(quote.id, {
            total: pricing.total,
            fees: pricing.fees || 0,
            tax: pricing.tax || 0,
            terms: pricing.terms || '',
            message: pricing.message || '',
          });
          emitter.emit('quote:provided', { quoteId: quote.id, pricing });
          processed++;
        } catch (err) {
          logger(`[${name}] Error providing quote ${quote.id}: ${err.message}`);
          emitter.emit('loop:error', { error: err, context: 'provideQuote', quoteId: quote.id });
        }
      }

      // 2. Seller: handle counter-offers
      const counterOffers = commerce.a2a().listQuotes({
        seller_address: walletAddress,
        status: 'counter_offered',
      });

      for (const quote of counterOffers) {
        if (processedQuoteIds.has(quote.id + ':counter_offered')) continue;
        processedQuoteIds.add(quote.id + ':counter_offered');

        try {
          const decision = strategy.evaluateCounterOffer(quote, ctx());
          if (decision.action === 'accept') {
            // Accept the buyer's counter — provide quote at their price
            await a2a.reviseQuote(quote.id, {
              total: quote.total_decimal ?? quote.total,
              message: 'Accepted your offer.',
            });
          } else if (decision.action === 'revise') {
            await a2a.reviseQuote(quote.id, {
              total: decision.total,
              fees: decision.fees || 0,
              tax: decision.tax || 0,
              message: decision.message || '',
            });
          } else {
            await a2a.declineQuote(quote.id, decision.reason || 'Offer too low');
            emitter.emit('quote:declined', { quote, reason: decision.reason });
          }
          processed++;
        } catch (err) {
          logger(`[${name}] Error handling counter ${quote.id}: ${err.message}`);
          emitter.emit('loop:error', { error: err, context: 'counterOffer', quoteId: quote.id });
        }
      }

      // 3. Buyer: evaluate received quotes
      const receivedQuotes = commerce.a2a().listQuotes({
        buyer_address: walletAddress,
        status: 'quoted',
      });

      for (const quote of receivedQuotes) {
        if (processedQuoteIds.has(quote.id + ':quoted')) continue;
        processedQuoteIds.add(quote.id + ':quoted');

        try {
          emitter.emit('quote:received', { quote, role: 'buyer' });
          const decision = strategy.evaluateReceivedQuote(quote, ctx());

          if (decision.action === 'accept') {
            const total = quote.total_decimal ?? quote.total ?? 0;

            // Circuit breaker pre-check
            if (circuitBreakerSvc) {
              const cbCheck = circuitBreakerSvc.checkTransaction(name, total);
              if (!cbCheck.allowed) {
                emitter.emit('circuit:blocked', {
                  quoteId: quote.id,
                  amount: total,
                  reason: cbCheck.reason,
                  state: cbCheck.state,
                });
                logger(`[${name}] Circuit breaker blocked: ${cbCheck.reason}`);
                continue;
              }
            }

            if (!canAfford(total)) {
              emitter.emit('budget:exceeded', {
                type: 'perTransaction',
                limit: budgetLimits.perTransaction,
                attempted: total,
              });
              continue;
            }

            // Pre-flight on-chain balance check
            if (settlement) {
              try {
                const fundCheck = await settlement.hasSufficientFunds(total);
                if (!fundCheck.sufficient) {
                  emitter.emit('settlement:insufficient_funds', {
                    quoteId: quote.id,
                    required: total,
                    available: fundCheck.balance,
                    symbol: fundCheck.symbol,
                    chainId: settlement.chainId,
                  });
                  logger(
                    `[${name}] Insufficient on-chain funds: need ${total}, have ${fundCheck.balance} ${fundCheck.symbol}`,
                  );
                  continue;
                }
              } catch (prefErr) {
                emitter.emit('settlement:failed', {
                  quoteId: quote.id,
                  phase: 'preflight',
                  error: prefErr.message,
                });
                logger(`[${name}] Settlement pre-flight failed: ${prefErr.message}`);
                continue;
              }
            }

            const result = await a2a.acceptQuote(quote.id);
            recordSpend(total, { type: 'quote', quoteId: quote.id });
            if (circuitBreakerSvc) circuitBreakerSvc.recordSuccess(name, total);
            emitter.emit('quote:accepted', { quote, payment: result.payment });
            emitter.emit('payment:sent', { payment: result.payment });

            // Settle on-chain after payment record created
            if (settlement && result.payment) {
              try {
                emitter.emit('settlement:pending', {
                  paymentId: result.payment.id,
                  quoteId: quote.id,
                  amount: total,
                  toAddress: quote.seller_address,
                  chainId: settlement.chainId,
                });

                const sResult = await settlement.settle({
                  toAddress: quote.seller_address,
                  amount: total,
                  asset: quote.asset || 'USDC',
                  memo: `A2A quote payment: ${quote.id}`,
                  paymentId: result.payment.id,
                });

                if (sResult.success) {
                  commerce.a2a().updatePayment(result.payment.id, {
                    status: 'completed',
                    tx_hash: sResult.txHash || null,
                    block_number: sResult.blockNumber || null,
                    completed_at: new Date().toISOString(),
                    metadata: JSON.stringify({
                      explorer_url: sResult.explorerUrl,
                      confirmations: sResult.confirmations,
                      chain_id: settlement.chainId,
                      simulated: sResult.simulated || false,
                    }),
                  });

                  emitter.emit('settlement:confirmed', {
                    paymentId: result.payment.id,
                    quoteId: quote.id,
                    txHash: sResult.txHash,
                    blockNumber: sResult.blockNumber,
                    explorerUrl: sResult.explorerUrl,
                    confirmations: sResult.confirmations,
                    simulated: sResult.simulated || false,
                  });
                } else {
                  commerce.a2a().updatePayment(result.payment.id, {
                    status: 'failed',
                    metadata: JSON.stringify({ settlement_error: sResult.error }),
                  });

                  emitter.emit('settlement:failed', {
                    paymentId: result.payment.id,
                    quoteId: quote.id,
                    error: sResult.error,
                  });
                }
              } catch (settleErr) {
                emitter.emit('settlement:failed', {
                  paymentId: result.payment.id,
                  quoteId: quote.id,
                  error: settleErr.message,
                });
                logger(`[${name}] Settlement failed: ${settleErr.message}`);
              }
            }

            processed++;
          } else if (decision.action === 'counter') {
            await a2a.counterQuote(quote.id, {
              total: decision.total,
              message: decision.message || '',
            });
            emitter.emit('quote:countered', {
              quote,
              counterAmount: decision.total,
            });
            processed++;
          } else if (decision.action === 'decline') {
            await a2a.declineQuote(quote.id, decision.reason);
            emitter.emit('quote:declined', { quote, reason: decision.reason });
            processed++;
          }
          // action === 'defer' → skip for now (used by BestOfN)
        } catch (err) {
          if (circuitBreakerSvc) circuitBreakerSvc.recordFailure(name, 0, err.message);
          logger(`[${name}] Error evaluating quote ${quote.id}: ${err.message}`);
          emitter.emit('loop:error', { error: err, context: 'evaluateQuote', quoteId: quote.id });
        }
      }

      // 4. Seller: auto-fulfill accepted quotes
      const acceptedQuotes = commerce.a2a().listQuotes({
        seller_address: walletAddress,
        status: 'accepted',
      });

      for (const quote of acceptedQuotes) {
        if (processedQuoteIds.has(quote.id + ':accepted')) continue;
        processedQuoteIds.add(quote.id + ':accepted');

        try {
          const shouldFulfill = strategy.shouldFulfill
            ? strategy.shouldFulfill(quote, ctx())
            : true;

          if (shouldFulfill) {
            await a2a.fulfillQuote(quote.id);
            emitter.emit('service:fulfilled', { quoteId: quote.id });

            // Auto-rate buyer after fulfillment (if strategy supports it)
            if (strategy.postFulfillmentRating) {
              try {
                const rating = strategy.postFulfillmentRating(quote, ctx());
                if (rating && rating.score > 0) {
                  await rateCounterparty({
                    ratedAddress: quote.buyer_address,
                    score: rating.score,
                    transactionId: quote.id,
                    comment: rating.comment || 'Auto-rated after fulfillment',
                  });
                }
              } catch (ratingErr) {
                logger(`[${name}] Auto-rating failed: ${ratingErr.message}`);
              }
            }
            processed++;
          }
        } catch (err) {
          logger(`[${name}] Error fulfilling quote ${quote.id}: ${err.message}`);
          emitter.emit('loop:error', { error: err, context: 'fulfill', quoteId: quote.id });
        }
      }
      // 5. Escrow: auto-settle fulfilled escrows where we are the buyer
      try {
        const activeEscrows = commerce.a2a().listEscrows({
          buyer_address: walletAddress,
          status: 'active',
        });
        for (const escrow of activeEscrows) {
          if (processedQuoteIds.has(escrow.id + ':escrow-settle')) continue;
          try {
            const conditions = await a2a.checkPaymentConditions(escrow.id);
            if (conditions.allMet) {
              processedQuoteIds.add(escrow.id + ':escrow-settle');
              await a2a.settleConditionalPayment(escrow.id);
              emitter.emit('escrow:settled', { escrowId: escrow.id });
              processed++;
            }
          } catch (escrowErr) {
            logger(`[${name}] Escrow settle error ${escrow.id}: ${escrowErr.message}`);
          }
        }
      } catch (escrowListErr) {
        // listEscrows may not be available — silently skip
        if (!escrowListErr.message?.includes('not a function')) {
          logger(`[${name}] Escrow check error: ${escrowListErr.message}`);
        }
      }

      // 6. Subscriptions: process billing for subscriptions where we are the provider
      try {
        const billing = await processSubscriptionBilling();
        if (billing.billingCount > 0) {
          processed += billing.billingCount;
        }
      } catch (billingErr) {
        // Subscription billing may not be available — silently skip
        if (!billingErr.message?.includes('not a function')) {
          logger(`[${name}] Billing error: ${billingErr.message}`);
        }
      }

      // 7. SLA: check for breaches on registered services
      try {
        for (const svcId of registeredServiceIds) {
          const breaches = getSLASvc().detectBreaches(svcId);
          if (breaches.breaches?.length > 0) {
            emitter.emit('sla:breach', { serviceId: svcId, breaches: breaches.breaches });
          }
        }
        // Also expire any open RFQs
        try {
          getMarketplaceSvc().expireRFQs();
        } catch (expireErr) {
          console.debug('marketplace expireRFQs not available:', expireErr.message);
        }
      } catch (slaErr) {
        if (!slaErr.message?.includes('not a function')) {
          logger(`[${name}] SLA check error: ${slaErr.message}`);
        }
      }
    } catch (err) {
      logger(`[${name}] Service loop error: ${err.message}`);
      emitter.emit('loop:error', { error: err, context: 'tick' });
    }

    emitter.emit('loop:tick', { processed });
    return processed;
  }

  function start() {
    if (running) return;
    running = true;
    loopTimer = setInterval(() => {
      tick().catch((err) => {
        emitter.emit('loop:error', { error: err, context: 'interval' });
      });
    }, pollIntervalMs);
    if (loopTimer.unref) loopTimer.unref();
    logger(`[${name}] Service loop started (${pollIntervalMs}ms interval)`);
  }

  function stop() {
    if (!running) return;
    running = false;
    if (loopTimer) {
      clearInterval(loopTimer);
      loopTimer = null;
    }
    logger(`[${name}] Service loop stopped`);
  }

  function isRunning() {
    return running;
  }

  function destroy() {
    stop();
    emitter.removeAllListeners();
    processedQuoteIds.clear();
  }

  // -------------------------------------------------------------------------
  // Public API
  // -------------------------------------------------------------------------
  return {
    // Identity
    name,
    agentId,
    walletAddress,

    // Underlying A2A service
    a2a,

    // Budget
    canAfford,
    recordSpend,
    getBudget,

    // Strategy
    setStrategy(s) {
      strategy = s;
    },
    getStrategy() {
      return strategy;
    },
    evaluateQuote,
    evaluatePaymentRequest,

    // Services
    registerService,
    listMyServices,
    discoverServices,

    // Escrow
    createEscrowDeal,

    // Reputation
    rateCounterparty,
    getReputation,

    // Subscriptions
    subscribeTo,
    pauseSubscription,
    resumeSubscription,
    cancelSubscription,
    processSubscriptionBilling,

    // Splits
    createSplitDeal,
    executeSplitDeal,

    // Settlement
    settlement,
    async getOnChainBalance() {
      if (!settlement) return null;
      return settlement.getBalance();
    },
    async getChainWalletAddress() {
      if (!settlement) return null;
      return settlement.getAddress();
    },

    // Marketplace
    async broadcastRFQ(params) {
      const result = await getMarketplaceSvc().broadcastRFQ({
        ...params,
        buyerAddress: walletAddress,
        buyerAgentId: agentId,
      });
      emitter.emit('rfq:broadcast', {
        rfqId: result.rfq.id,
        sellersContacted: result.sellersContacted,
      });
      return result;
    },
    collectRFQResponses(rfqId) {
      return getMarketplaceSvc().collectRFQResponses(rfqId);
    },
    async awardRFQ(rfqId, winnerId) {
      const result = await getMarketplaceSvc().awardRFQ(rfqId, winnerId);
      emitter.emit('rfq:awarded', {
        rfqId,
        winnerId: result.winnerId,
        winnerAddress: result.winnerAddress,
      });
      return result;
    },
    getMarketMetrics(serviceId) {
      return getMarketplaceSvc().getServiceMetrics(serviceId);
    },

    // SLA
    attachSLA(params) {
      const result = getSLASvc().attachSLA(params);
      emitter.emit('sla:attached', { slaId: result.sla.id, serviceId: params.serviceId });
      return result;
    },
    checkSLACompliance(serviceId) {
      return getSLASvc().checkCompliance(serviceId);
    },

    // Workflows
    createWorkflow(params) {
      const result = getWorkflowSvc().createWorkflow(params);
      return result;
    },
    async executeWorkflow(workflowId, context) {
      emitter.emit('workflow:started', { workflowId });
      const result = await getWorkflowSvc().executeWorkflow(workflowId, context);
      if (result.status === 'completed') {
        emitter.emit('workflow:completed', { workflowId, totalCost: result.totalCost });
      }
      return result;
    },
    getWorkflowStatus(workflowId) {
      return getWorkflowSvc().getWorkflowStatus(workflowId);
    },

    // Circuit Breaker
    getCircuitBreakerState() {
      return circuitBreakerSvc?.getState(name) ?? { state: 'closed', reason: null };
    },
    tripBreaker(reason) {
      if (circuitBreakerSvc) {
        circuitBreakerSvc.trip(name, reason);
        emitter.emit('circuit:tripped', { agentName: name, reason });
      }
    },
    emergencyHalt(reason) {
      if (circuitBreakerSvc) {
        circuitBreakerSvc.tripAll(reason);
        emitter.emit('circuit:kill_switch', { reason });
      }
    },

    // Agent Card
    ensureAgentCard,
    checkCardActive,
    getAgentCard,

    // Service Loop
    start,
    stop,
    isRunning,
    tick,

    // Events (delegate to emitter)
    on: emitter.on.bind(emitter),
    off: emitter.off.bind(emitter),
    once: emitter.once.bind(emitter),
    emit: emitter.emit.bind(emitter),

    // Lifecycle
    destroy,
  };
}
