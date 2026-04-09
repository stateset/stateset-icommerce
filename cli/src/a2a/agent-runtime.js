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
import { createA2ASubscriptionService, computeNextBillingDate } from './subscriptions.js';
import { createSplitPaymentService } from './splits.js';
import { createMarketplaceService } from './marketplace.js';
import { createSLAService } from './sla.js';
import { createWorkflowService } from './workflows.js';
import {
  DEFAULT_ASSET,
  DEFAULT_NETWORK,
  fromSmallestUnit,
  getAssetDecimals,
  getDefaultAssetForNetwork,
  toSmallestUnit,
} from './assets.js';
import { adaptCommerceApis } from '../commerce.js';

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
  return adaptCommerceApis({
    a2a: () => ({
      // Payments
      createPayment: (p) => a2aStore.createPayment(p),
      getPayment: (id) => a2aStore.getPayment(id),
      updatePayment: (id, u) => a2aStore.updatePayment(id, u),
      listPayments: (f) => a2aStore.listPayments(f),
      sumPayments: (f) => a2aStore.sumPayments(f),
      summarizePayments: (f) => a2aStore.summarizePayments(f),
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
  });
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

const BILLING_INTERVAL_DAYS = {
  weekly: 7,
  biweekly: 14,
  monthly: 30,
  quarterly: 90,
  annual: 365,
};

function countPastDueCycles(sinceIso, interval) {
  const since = new Date(sinceIso).getTime();
  const now = Date.now();
  const daysPassed = (now - since) / (24 * 60 * 60 * 1000);
  const intervalDays = BILLING_INTERVAL_DAYS[interval] || 30;
  return Math.floor(daysPassed / intervalDays);
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
    commerce: initialCommerce,
    budget: budgetConfig = {},
    strategy: initialStrategy,
    pollIntervalMs = 5000,
    logger = console.debug,
    autoRegisterCard = false,
    agentDescription = '',
    agentSkills = ['buy', 'sell', 'quote'],
    supportedNetworks,
    supportedAssets,
    settlement: initialSettlement = null,
  } = params;
  let commerce = initialCommerce;

  if (!walletAddress) throw new Error('walletAddress is required');
  if (!commerce) throw new Error('commerce is required');
  commerce = adaptCommerceApis(commerce, ['a2a', 'x402']);

  let settlement = null;
  const settlementServices = new Map();
  const configuredSupportedNetworks =
    Array.isArray(supportedNetworks) && supportedNetworks.length > 0 ? supportedNetworks : null;
  const configuredSupportedAssets =
    Array.isArray(supportedAssets) && supportedAssets.length > 0 ? supportedAssets : null;

  function registerSettlementService(nextSettlement, options = {}) {
    if (!nextSettlement || !nextSettlement.chainId) {
      return null;
    }
    settlementServices.set(nextSettlement.chainId, nextSettlement);
    if (options.makeDefault !== false) {
      settlement = nextSettlement;
    }
    return nextSettlement;
  }

  function clearSettlementServices() {
    settlement = null;
    settlementServices.clear();
  }

  function getSettlementService(chainId = null) {
    if (chainId) {
      return settlementServices.get(chainId) || null;
    }
    return settlement;
  }

  function getSettlementChains() {
    return [...settlementServices.keys()];
  }

  if (initialSettlement) {
    registerSettlementService(initialSettlement);
  }

  function getRuntimeDefaultNetwork() {
    return settlement?.chainId || DEFAULT_NETWORK;
  }

  function getRuntimeDefaultAsset() {
    return getDefaultAssetForNetwork(getRuntimeDefaultNetwork()) || DEFAULT_ASSET;
  }

  function getRuntimeSupportedNetworks() {
    if (configuredSupportedNetworks) {
      return configuredSupportedNetworks;
    }
    const settlementChains = getSettlementChains();
    return settlementChains.length > 0 ? settlementChains : [getRuntimeDefaultNetwork()];
  }

  function getRuntimeSupportedAssets() {
    if (configuredSupportedAssets) {
      return configuredSupportedAssets;
    }
    const settlementChains = getSettlementChains();
    if (settlementChains.length === 0) {
      return [getRuntimeDefaultAsset()];
    }
    return [...new Set(settlementChains.map((chainId) => getDefaultAssetForNetwork(chainId)))];
  }

  function normalizeAcceptedNetworks(value, fallback = getRuntimeDefaultNetwork()) {
    if (Array.isArray(value) && value.length > 0) {
      return value;
    }
    if (typeof value === 'string' && value.length > 0) {
      try {
        const parsed = JSON.parse(value);
        if (Array.isArray(parsed) && parsed.length > 0) {
          return parsed;
        }
      } catch (_err) {
        void _err;
      }
      return [value];
    }
    return [fallback];
  }

  function parseMetadata(value) {
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

  function parsePaymentAddresses(value) {
    const parsed = parseMetadata(value);
    if (!parsed || Array.isArray(parsed)) return null;
    return parsed;
  }

  async function getOwnReceiveAddress(network = getRuntimeDefaultNetwork()) {
    const settlementService = getSettlementService(network);
    if (settlementService) {
      try {
        const address = await settlementService.getAddress();
        if (address) return address;
      } catch {
        // Fall back to the identity wallet when settlement address derivation
        // is unavailable.
      }
    }
    return walletAddress;
  }

  function getFinalityTracker() {
    return commerce?._finalityTracker || null;
  }

  function getTrackedFinality(intentId) {
    const tracker = getFinalityTracker();
    if (!tracker || !intentId) return null;
    try {
      return tracker.getSettlementStatus(intentId);
    } catch {
      return null;
    }
  }

  function syncTrackedSettlement({
    intentId,
    txHash,
    chainId,
    blockNumber = 0,
    confirmations = 0,
  }) {
    const tracker = getFinalityTracker();
    if (!tracker || !intentId || !txHash || !chainId) {
      return null;
    }

    if (!getTrackedFinality(intentId)) {
      try {
        tracker.trackSettlement(intentId, txHash, chainId, blockNumber || 0);
      } catch {
        // Ignore duplicate/invalid tracking attempts and continue.
      }
    }

    const safeConfirmations = Math.max(0, Number(confirmations || 0));
    const latestBlock =
      blockNumber && safeConfirmations > 0 ? blockNumber + safeConfirmations - 1 : blockNumber || 0;

    try {
      tracker.updateConfirmations(intentId, safeConfirmations, latestBlock);
    } catch {
      // Best-effort only.
    }

    return getTrackedFinality(intentId);
  }

  function markTrackedSettlementFailed(intentId, reason) {
    const tracker = getFinalityTracker();
    if (!tracker || !intentId) {
      return null;
    }

    if (!getTrackedFinality(intentId)) {
      return null;
    }

    try {
      return tracker.markFailed(intentId, reason);
    } catch {
      return getTrackedFinality(intentId);
    }
  }

  function formatStoredPayment(payment) {
    if (!payment) return null;
    const decimals = getAssetDecimals(payment.asset);
    const metadata = parseMetadata(payment.metadata);
    return {
      id: payment.id,
      status: payment.status,
      from: payment.sender_address,
      to: payment.recipient_address,
      amount:
        typeof payment.amount_decimal === 'number'
          ? payment.amount_decimal
          : fromSmallestUnit(payment.amount, decimals),
      asset: payment.asset,
      network: payment.network,
      memo: payment.memo,
      txHash: payment.tx_hash,
      blockNumber: payment.block_number ?? null,
      explorerUrl: metadata?.explorer_url || null,
      confirmations:
        metadata?.confirmations !== undefined && metadata?.confirmations !== null
          ? Number(metadata.confirmations)
          : null,
      chainId: metadata?.chain_id || payment.network || null,
      simulated: metadata?.simulated ?? null,
      createdAt: payment.created_at,
      completedAt: payment.completed_at,
    };
  }

  async function triggerCallback(url, payload) {
    if (typeof fetch !== 'function') {
      return;
    }

    try {
      await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
    } catch (error) {
      logger(`[${name}] Callback failed: ${error.message}`);
    }
  }

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
    defaultAsset: getRuntimeDefaultAsset(),
    defaultNetwork: getRuntimeDefaultNetwork(),
    receiveAddressForNetwork: (network) => getOwnReceiveAddress(network),
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
    byRail: new Map(),
    lastDayReset: todayKey(),
    lastMonthReset: monthKey(),
    history: [],
  };

  function normalizeBudgetAsset(asset, network = null) {
    if (asset) {
      return String(asset).toUpperCase();
    }
    if (network) {
      return getDefaultAssetForNetwork(network) || getRuntimeDefaultAsset();
    }
    return getRuntimeDefaultAsset();
  }

  function normalizeBudgetNetwork(network = null) {
    return network || getRuntimeDefaultNetwork();
  }

  function getBudgetRailScope(options = {}) {
    const network = normalizeBudgetNetwork(options.network || null);
    const asset = normalizeBudgetAsset(options.asset || null, network);
    return { asset, network };
  }

  function getBudgetRailKey(options = {}) {
    const scope = getBudgetRailScope(options);
    return `${scope.asset}:${scope.network}`;
  }

  function createBudgetRailState(options = {}) {
    const scope = getBudgetRailScope(options);
    return {
      asset: scope.asset,
      network: scope.network,
      spentToday: 0,
      spentThisMonth: 0,
      balance: budgetConfig.startingBalance ?? null,
    };
  }

  function getBudgetRailState(options = {}, { createIfMissing = true } = {}) {
    const key = getBudgetRailKey(options);
    let railState = budgetState.byRail.get(key) || null;
    if (!railState && createIfMissing) {
      railState = createBudgetRailState(options);
      budgetState.byRail.set(key, railState);
    }
    return railState;
  }

  function evaluateBudget(amount, options = {}) {
    rolloverBudget();
    const scope = getBudgetRailScope(options);
    const railState =
      getBudgetRailState(scope, { createIfMissing: false }) || createBudgetRailState(scope);

    const evaluation = {
      allowed: true,
      asset: scope.asset,
      network: scope.network,
      attempted: amount,
      spentToday: railState.spentToday,
      spentThisMonth: railState.spentThisMonth,
      balance: railState.balance,
      remainingDaily: budgetLimits.daily - railState.spentToday,
      remainingMonthly: budgetLimits.monthly - railState.spentThisMonth,
      remainingBalance: railState.balance,
      limit: null,
      remaining: null,
      type: null,
    };

    if (amount > budgetLimits.perTransaction) {
      return {
        ...evaluation,
        allowed: false,
        type: 'perTransaction',
        limit: budgetLimits.perTransaction,
        remaining: budgetLimits.perTransaction,
      };
    }

    if (railState.spentToday + amount > budgetLimits.daily) {
      return {
        ...evaluation,
        allowed: false,
        type: 'daily',
        limit: budgetLimits.daily,
        remaining: Math.max(0, budgetLimits.daily - railState.spentToday),
      };
    }

    if (railState.spentThisMonth + amount > budgetLimits.monthly) {
      return {
        ...evaluation,
        allowed: false,
        type: 'monthly',
        limit: budgetLimits.monthly,
        remaining: Math.max(0, budgetLimits.monthly - railState.spentThisMonth),
      };
    }

    if (railState.balance !== null && amount > railState.balance) {
      return {
        ...evaluation,
        allowed: false,
        type: 'balance',
        limit: railState.balance,
        remaining: Math.max(0, railState.balance),
      };
    }

    return evaluation;
  }

  function emitBudgetExceeded(evaluation, metadata = {}) {
    if (!evaluation || evaluation.allowed) {
      return evaluation;
    }
    emitter.emit('budget:exceeded', {
      type: evaluation.type,
      asset: evaluation.asset,
      network: evaluation.network,
      limit: evaluation.limit,
      attempted: evaluation.attempted,
      spentToday: evaluation.spentToday,
      spentThisMonth: evaluation.spentThisMonth,
      balance: evaluation.balance,
      remaining: evaluation.remaining,
      ...metadata,
    });
    return evaluation;
  }

  function toBudgetLeaf(railState) {
    return {
      asset: railState.asset,
      network: railState.network,
      spentToday: railState.spentToday,
      spentThisMonth: railState.spentThisMonth,
      balance: railState.balance,
      remainingDaily: budgetLimits.daily - railState.spentToday,
      remainingMonthly: budgetLimits.monthly - railState.spentThisMonth,
    };
  }

  function buildBudgetBreakdown(railStates) {
    const breakdownByAsset = {};

    for (const railState of railStates) {
      const assetBucket = breakdownByAsset[railState.asset] || {
        spentToday: null,
        spentThisMonth: null,
        balance: null,
        remainingDaily: null,
        remainingMonthly: null,
        aggregateTotalsMeaningful: false,
        trackedNetworks: [],
        networks: {},
      };
      assetBucket.networks[railState.network] = toBudgetLeaf(railState);
      breakdownByAsset[railState.asset] = assetBucket;
    }

    const assets = Object.keys(breakdownByAsset).sort();
    for (const asset of assets) {
      const assetBucket = breakdownByAsset[asset];
      const trackedNetworks = Object.keys(assetBucket.networks).sort();
      const orderedNetworks = {};
      for (const network of trackedNetworks) {
        orderedNetworks[network] = assetBucket.networks[network];
      }
      assetBucket.networks = orderedNetworks;
      assetBucket.trackedNetworks = trackedNetworks;
      assetBucket.aggregateTotalsMeaningful = trackedNetworks.length <= 1;
      if (trackedNetworks.length === 1) {
        const [network] = trackedNetworks;
        const leaf = assetBucket.networks[network];
        assetBucket.spentToday = leaf.spentToday;
        assetBucket.spentThisMonth = leaf.spentThisMonth;
        assetBucket.balance = leaf.balance;
        assetBucket.remainingDaily = leaf.remainingDaily;
        assetBucket.remainingMonthly = leaf.remainingMonthly;
      }
    }

    return { assets, breakdownByAsset };
  }

  function rolloverBudget() {
    const today = todayKey();
    const month = monthKey();
    if (budgetState.lastDayReset !== today) {
      for (const railState of budgetState.byRail.values()) {
        railState.spentToday = 0;
      }
      budgetState.lastDayReset = today;
    }
    if (budgetState.lastMonthReset !== month) {
      for (const railState of budgetState.byRail.values()) {
        railState.spentThisMonth = 0;
      }
      budgetState.lastMonthReset = month;
    }
  }

  function canAfford(amount, options = {}) {
    return evaluateBudget(amount, options).allowed;
  }

  function recordSpend(amount, metadata = {}) {
    rolloverBudget();
    const railState = getBudgetRailState(metadata);
    railState.spentToday += amount;
    railState.spentThisMonth += amount;
    if (railState.balance !== null) {
      railState.balance -= amount;
    }
    const scope = getBudgetRailScope(metadata);
    budgetState.history.push({
      amount,
      asset: scope.asset,
      network: scope.network,
      timestamp: new Date().toISOString(),
      ...metadata,
    });

    // Budget warning at 80%
    if (budgetLimits.daily !== Infinity && railState.spentToday > budgetLimits.daily * 0.8) {
      emitter.emit('budget:warning', {
        type: 'daily',
        asset: scope.asset,
        network: scope.network,
        spent: railState.spentToday,
        limit: budgetLimits.daily,
      });
    }
  }

  function getBudget(filter = {}) {
    rolloverBudget();
    const matchingRails = [...budgetState.byRail.values()].filter((railState) => {
      if (filter.asset && railState.asset !== normalizeBudgetAsset(filter.asset)) {
        return false;
      }
      if (filter.network && railState.network !== normalizeBudgetNetwork(filter.network)) {
        return false;
      }
      return true;
    });
    const effectiveRails =
      matchingRails.length > 0 ? matchingRails : [createBudgetRailState(filter)];
    const breakdown = buildBudgetBreakdown(effectiveRails);
    const singleRail = effectiveRails.length === 1 ? toBudgetLeaf(effectiveRails[0]) : null;
    return {
      ...budgetLimits,
      asset: singleRail?.asset || null,
      network: singleRail?.network || null,
      spentToday: singleRail?.spentToday ?? null,
      spentThisMonth: singleRail?.spentThisMonth ?? null,
      balance: singleRail?.balance ?? null,
      remainingDaily: singleRail?.remainingDaily ?? null,
      remainingMonthly: singleRail?.remainingMonthly ?? null,
      assets: breakdown.assets,
      aggregateAsset: filter.asset
        ? normalizeBudgetAsset(filter.asset)
        : breakdown.assets.length === 1
          ? breakdown.assets[0]
          : null,
      aggregateTotalsMeaningful: effectiveRails.length <= 1,
      breakdownByAsset: breakdown.breakdownByAsset,
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

  async function requestQuote(params) {
    const network = params.network || getRuntimeDefaultNetwork();
    const asset = params.asset || getDefaultAssetForNetwork(network) || getRuntimeDefaultAsset();
    return a2a.requestQuote({
      ...params,
      asset,
      network,
    });
  }

  async function requestPayment(params) {
    const network = params.network || getRuntimeDefaultNetwork();
    const asset = params.asset || getDefaultAssetForNetwork(network) || getRuntimeDefaultAsset();
    return a2a.requestPayment({
      ...params,
      asset,
      network,
    });
  }

  async function pay(params) {
    const {
      to,
      amount,
      asset: requestedAsset,
      network: requestedNetwork,
      memo,
      referenceType,
      referenceId,
      idempotencyKey,
      eventContext = {},
    } = params;

    if (!to) {
      throw new Error('Recipient (to) is required');
    }
    if (amount === undefined || amount === null) {
      throw new Error('Amount is required');
    }
    if (amount <= 0) {
      throw new Error('Amount must be positive');
    }

    const network = requestedNetwork || getRuntimeDefaultNetwork();
    const asset = requestedAsset || getDefaultAssetForNetwork(network) || getRuntimeDefaultAsset();
    const settlementService = getSettlementService(network);

    const budgetCheck = evaluateBudget(amount, { asset, network });
    if (!budgetCheck.allowed) {
      emitBudgetExceeded(budgetCheck, {
        referenceType: referenceType || null,
        referenceId: referenceId || null,
        operation: referenceType || 'payment',
      });
      throw new Error(
        `Cannot afford payment amount ${amount} (${budgetCheck.type} budget exceeded)`,
      );
    }

    if (settlementServices.size > 0 && !settlementService) {
      const errorMessage = `No settlement service configured for network ${network}`;
      emitter.emit('settlement:failed', {
        ...eventContext,
        phase: 'selection',
        error: errorMessage,
      });
      throw new Error(errorMessage);
    }

    if (settlementService) {
      await ensureSettlementFunds(settlementService, amount, {
        ...eventContext,
        referenceType: referenceType || null,
        referenceId: referenceId || null,
      });
    }

    const payResult = await a2a.pay({
      to,
      amount,
      asset,
      network,
      memo,
      referenceType,
      referenceId,
      idempotencyKey,
    });

    if (!payResult || payResult.success === false || !payResult.payment) {
      throw new Error(payResult?.error || 'Payment returned unsuccessful');
    }

    let storedPayment = await commerce.a2a().getPayment(payResult.payment.id);
    let settlementResult = null;
    if (settlementService) {
      const objectRecipient =
        to && typeof to === 'object'
          ? to.wallet_address || to.walletAddress || to.address || null
          : null;
      settlementResult = await settleRecordedPayment({
        payment: storedPayment || {
          id: payResult.payment.id,
          recipient_address: payResult.payment.to,
          asset,
          memo: memo || null,
        },
        amount,
        asset,
        settlementService,
        memo,
        counterpartyAgentId:
          storedPayment?.recipient_agent_id ||
          (typeof to === 'object' ? to.id || to.agentId || null : null),
        identityAddress:
          storedPayment?.recipient_address || payResult.payment.to || objectRecipient || null,
        eventContext: {
          ...eventContext,
          referenceType: referenceType || null,
          referenceId: referenceId || null,
        },
      });
      storedPayment = await commerce.a2a().getPayment(payResult.payment.id);
    }

    recordSpend(amount, {
      type: referenceType || 'payment',
      paymentId: payResult.payment.id,
      referenceType: referenceType || null,
      referenceId: referenceId || null,
      network,
      asset,
    });

    return {
      ...payResult,
      payment: formatStoredPayment(storedPayment) || payResult.payment,
      settlement: settlementResult?.settlement || null,
      viaRuntime: true,
    };
  }

  async function payRequest(requestId, options = {}) {
    const request = await commerce.a2a().getPaymentRequest(requestId);
    if (!request) {
      throw new Error('Payment request not found');
    }

    if (request.status === 'paid') {
      throw new Error('Payment request already paid');
    }
    if (request.status === 'expired' || new Date(request.expires_at) < new Date()) {
      throw new Error('Payment request has expired');
    }
    if (request.status === 'cancelled') {
      throw new Error('Payment request was cancelled');
    }

    const decimals = getAssetDecimals(request.asset);
    const amountToPay = options.amount
      ? toSmallestUnit(options.amount, decimals)
      : request.amount - request.amount_paid;

    if (amountToPay <= 0) {
      throw new Error('Invalid payment amount');
    }

    if (!request.allow_partial && amountToPay < request.amount - request.amount_paid) {
      throw new Error('Partial payments not allowed for this request');
    }

    const requestMetadata = parseMetadata(request.metadata);
    const paymentTarget = request.requester_agent_id
      ? {
          id: request.requester_agent_id,
          wallet_address: request.requester_address,
          paymentAddress: requestMetadata?.requester_payment_address,
        }
      : requestMetadata?.requester_payment_address || request.requester_address;

    const amountDecimal = fromSmallestUnit(amountToPay, decimals);
    const paymentResult = await pay({
      to: paymentTarget,
      amount: amountDecimal,
      asset: request.asset,
      network: normalizeAcceptedNetworks(request.accepted_networks)[0],
      memo: `Payment for: ${request.description}`,
      referenceType: 'payment_request',
      referenceId: requestId,
      eventContext: { requestId },
    });

    const newAmountPaid = request.amount_paid + amountToPay;
    const isFullyPaid = newAmountPaid >= request.amount;

    await commerce.a2a().updatePaymentRequest(requestId, {
      status: isFullyPaid ? 'paid' : 'processing',
      amount_paid: newAmountPaid,
      payment_ids: [...(request.payment_ids || []), paymentResult.payment.id],
      paid_at: isFullyPaid ? new Date().toISOString() : null,
    });

    if (request.callback_url && isFullyPaid) {
      triggerCallback(request.callback_url, {
        event: 'payment_request.paid',
        request_id: requestId,
        payment_id: paymentResult.payment.id,
        amount: amountToPay,
        total_paid: newAmountPaid,
      }).catch(() => {}); // Fire and forget.
    }

    return {
      success: true,
      payment: paymentResult.payment,
      request: {
        id: requestId,
        status: isFullyPaid ? 'paid' : 'processing',
        amountPaid: fromSmallestUnit(newAmountPaid, decimals),
        amountRemaining: fromSmallestUnit(request.amount - newAmountPaid, decimals),
        fullyPaid: isFullyPaid,
      },
      settlement: paymentResult.settlement || null,
      viaRuntime: true,
    };
  }

  async function acceptQuote(quoteId) {
    const quote = await commerce.a2a().getQuote(quoteId);
    if (!quote) {
      throw new Error('Quote not found');
    }

    if (quote.status !== 'quoted') {
      throw new Error(`Cannot accept quote in status: ${quote.status}`);
    }

    if (new Date(quote.expires_at) < new Date()) {
      throw new Error('Quote has expired');
    }

    if (quote.buyer_address !== walletAddress) {
      throw new Error('Only the buyer can accept a quote');
    }

    const decimals = getAssetDecimals(quote.asset);
    const quoteMetadata = parseMetadata(quote.metadata);
    const paymentTarget = quote.seller_agent_id
      ? {
          id: quote.seller_agent_id,
          wallet_address: quote.seller_address,
          paymentAddress: quoteMetadata?.seller_payment_address,
        }
      : quoteMetadata?.seller_payment_address || quote.seller_address;
    const amount =
      typeof quote.total_decimal === 'number'
        ? quote.total_decimal
        : fromSmallestUnit(quote.total, decimals);

    const paymentResult = await pay({
      to: paymentTarget,
      amount,
      asset: quote.asset,
      network: normalizeAcceptedNetworks(quote.accepted_networks)[0],
      memo: `Payment for quote ${quoteId}`,
      referenceType: 'quote',
      referenceId: quoteId,
      eventContext: { quoteId },
    });

    await commerce.a2a().updateQuote(quoteId, {
      status: 'accepted',
      payment_id: paymentResult.payment.id,
      accepted_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    return {
      success: true,
      payment: paymentResult.payment,
      quote: {
        id: quoteId,
        status: 'accepted',
        total: amount,
        asset: quote.asset,
        network: normalizeAcceptedNetworks(quote.accepted_networks)[0],
      },
      settlement: paymentResult.settlement || null,
      viaRuntime: true,
    };
  }

  async function getPayment(paymentId) {
    return a2a.getPayment(paymentId);
  }

  async function refreshPayment(paymentId) {
    return a2a.refreshPayment(paymentId);
  }

  const settlementAwareA2A = {
    ...a2a,
    requestQuote,
    requestPayment,
    pay,
    payRequest,
    acceptQuote,
    getPayment,
    refreshPayment,
  };

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
    if (!marketplaceSvc) {
      marketplaceSvc = createMarketplaceService(commerce.a2a(), settlementAwareA2A);
    }
    return marketplaceSvc;
  }

  function getSLASvc() {
    if (!slaSvc) slaSvc = createSLAService(commerce.a2a());
    return slaSvc;
  }

  function getWorkflowSvc() {
    if (!workflowSvc) {
      workflowSvc = createWorkflowService(commerce.a2a(), settlementAwareA2A);
    }
    return workflowSvc;
  }

  // ── Escrow ──

  async function createEscrowDeal(params) {
    const {
      sellerAddress,
      amount,
      asset,
      network,
      quoteId,
      conditions,
      expiresInHours = 72,
      memo,
    } = params;
    const budgetCheck = evaluateBudget(amount, { asset, network });
    if (!budgetCheck.allowed) {
      emitBudgetExceeded(budgetCheck, {
        operation: 'escrow:create',
        referenceType: quoteId ? 'quote' : 'escrow',
        referenceId: quoteId || null,
        sellerAddress,
      });
      throw new Error(`Cannot afford amount ${amount} (budget exceeded: ${budgetCheck.type})`);
    }
    const result = await a2a.createConditionalPayment({
      sellerAddress,
      amount,
      asset: asset || getRuntimeDefaultAsset(),
      network: network || getRuntimeDefaultNetwork(),
      quoteId,
      conditions,
      expiresInHours,
      memo,
    });
    recordSpend(amount, {
      type: 'escrow',
      escrowId: result.escrow?.id,
      asset: asset || getRuntimeDefaultAsset(),
      network: network || getRuntimeDefaultNetwork(),
    });
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
    const {
      providerAddress,
      planName,
      amount,
      asset,
      network,
      interval = 'monthly',
      trialDays = 0,
    } = params;
    const budgetCheck = evaluateBudget(amount, { asset, network });
    if (!budgetCheck.allowed) {
      emitBudgetExceeded(budgetCheck, {
        operation: 'subscription:create',
        planName,
        providerAddress,
      });
      throw new Error(
        `Cannot afford subscription amount ${amount} (${budgetCheck.type} budget exceeded)`,
      );
    }
    const result = await getSubscriptionSvc().createSubscription({
      subscriberAddress: walletAddress,
      providerAddress,
      planName,
      amount,
      asset: asset || getRuntimeDefaultAsset(),
      network: network || getRuntimeDefaultNetwork(),
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
    const now = new Date();
    const nowIso = now.toISOString();
    const store = commerce.a2a();

    let processed = 0;
    let billed = 0;
    let failed = 0;
    let cancelled = 0;
    let trialsActivated = 0;
    let totalBilled = 0;

    const dueSubscriptions = (await store.getDueSubscriptions(nowIso)).filter(
      (sub) => sub.subscriber_address === walletAddress,
    );
    const expiredTrials = (await store.getExpiredTrials(nowIso)).filter(
      (trial) => trial.subscriber_address === walletAddress,
    );

    async function markPastDue(sub, errorMessage) {
      const updates = {};
      if (!sub.past_due_since) {
        updates.past_due_since = nowIso;
      }
      updates.next_billing_date = computeNextBillingDate(now, sub.billing_interval);
      await store.updateSubscription(sub.id, updates);
      emitter.emit('subscription:billing_failed', {
        subscriptionId: sub.id,
        error: errorMessage,
        pastDueSince: sub.past_due_since || nowIso,
      });
    }

    for (const sub of dueSubscriptions) {
      processed++;

      if (
        sub.cancel_at_period_end &&
        sub.current_period_end &&
        new Date(sub.current_period_end) <= now
      ) {
        await store.updateSubscription(sub.id, {
          status: 'cancelled',
          cancelled_at: nowIso,
          cancel_at_period_end: false,
        });
        cancelled++;
        emitter.emit('subscription:cancelled', {
          subscriptionId: sub.id,
          reason: 'cancel_at_period_end',
        });
        continue;
      }

      if (sub.past_due_since) {
        const pastDueCycles = countPastDueCycles(sub.past_due_since, sub.billing_interval);
        const maxCycles = sub.max_past_due_cycles || 3;
        if (pastDueCycles >= maxCycles) {
          await store.updateSubscription(sub.id, {
            status: 'cancelled',
            cancelled_at: nowIso,
          });
          cancelled++;
          emitter.emit('subscription:cancelled', {
            subscriptionId: sub.id,
            reason: 'max_past_due_cycles_exceeded',
            pastDueCycles,
          });
          continue;
        }
      }

      const amount = sub.amount_decimal || 0;
      const network = sub.network || DEFAULT_NETWORK;
      const asset = sub.asset || getDefaultAssetForNetwork(network);

      const budgetCheck = evaluateBudget(amount, { asset, network });
      if (!budgetCheck.allowed) {
        emitBudgetExceeded(budgetCheck, {
          operation: 'subscription:billing',
          referenceType: 'subscription',
          referenceId: sub.id,
          subscriptionId: sub.id,
          providerAddress: sub.provider_address,
        });
        failed++;
        await markPastDue(sub, `Budget exceeded for subscription amount ${amount}`);
        continue;
      }

      try {
        const payResult = await pay({
          to: sub.provider_address,
          amount,
          asset,
          network,
          memo: `Subscription billing: ${sub.plan_name} (${sub.billing_interval})`,
          referenceType: 'subscription',
          referenceId: sub.id,
          idempotencyKey: `sub-${sub.id}-${nowIso}`,
          eventContext: { subscriptionId: sub.id },
        });

        const newTotalBilled = (sub.total_billed || 0) + sub.amount;
        const newTotalBilledDecimal = (sub.total_billed_decimal || 0) + amount;
        const newBillingCount = (sub.billing_count || 0) + 1;
        const nextBilling = computeNextBillingDate(now, sub.billing_interval);

        await store.updateSubscription(sub.id, {
          total_billed: newTotalBilled,
          total_billed_decimal: newTotalBilledDecimal,
          billing_count: newBillingCount,
          last_payment_id: payResult.payment.id,
          current_period_start: nowIso,
          current_period_end: nextBilling,
          next_billing_date: nextBilling,
          past_due_since: null,
        });

        billed++;
        totalBilled += amount;
      } catch (err) {
        failed++;
        await markPastDue(sub, err.message);
      }
    }

    for (const trial of expiredTrials) {
      processed++;
      try {
        const nextBilling = computeNextBillingDate(now, trial.billing_interval);
        await store.updateSubscription(trial.id, {
          status: 'active',
          current_period_start: nowIso,
          current_period_end: nextBilling,
          next_billing_date: nextBilling,
        });
        trialsActivated++;
      } catch {
        failed++;
      }
    }

    if (billed > 0) {
      emitter.emit('subscription:billed', {
        count: billed,
        total: totalBilled,
      });
    }

    return {
      processed,
      succeeded: billed + trialsActivated,
      failed,
      cancelled,
      billed,
      billingCount: billed,
      totalBilled,
      trialsActivated,
    };
  }

  // ── Split Payments ──

  async function createSplitDeal(params) {
    const {
      totalAmount,
      recipients,
      asset,
      network,
      splitType = 'percentage',
      platformFeePercent,
      platformFeeAddress,
      memo,
    } = params;
    const budgetCheck = evaluateBudget(totalAmount, { asset, network });
    if (!budgetCheck.allowed) {
      emitBudgetExceeded(budgetCheck, {
        operation: 'split:create',
        recipientCount: Array.isArray(recipients) ? recipients.length : 0,
      });
      throw new Error(
        `Cannot afford split amount ${totalAmount} (${budgetCheck.type} budget exceeded)`,
      );
    }
    const result = await getSplitsSvc().createSplitPayment({
      senderAddress: walletAddress,
      totalAmount,
      asset: asset || getRuntimeDefaultAsset(),
      network: network || getRuntimeDefaultNetwork(),
      splitType,
      platformFeePercent: platformFeePercent || 0,
      platformFeeAddress: platformFeeAddress || null,
      recipients,
      memo: memo || '',
    });
    emitter.emit('split:created', { splitPayment: result.splitPayment });
    return result;
  }

  async function executeSplitDeal(splitId) {
    const split = await getSplitsSvc().getSplitPayment(splitId);
    if (!split) {
      throw new Error('Split payment not found');
    }
    const budgetCheck = evaluateBudget(split.totalAmountDecimal, {
      asset: split.asset,
      network: split.network,
    });
    if (!budgetCheck.allowed) {
      emitBudgetExceeded(budgetCheck, {
        operation: 'split:execute',
        referenceType: 'split',
        referenceId: splitId,
        splitId,
      });
      throw new Error(
        `Cannot afford split amount ${split.totalAmountDecimal} (${budgetCheck.type} budget exceeded)`,
      );
    }

    const result = await getSplitsSvc().executeSplitPayment(
      splitId,
      async (to, amount, asset, network, splitMemo) => {
        const payResult = await pay({
          to,
          amount,
          asset,
          network,
          memo: splitMemo,
          referenceType: 'split',
          referenceId: splitId,
          eventContext: {
            splitPaymentId: splitId,
            recipientAddress: to,
          },
        });

        return payResult.payment;
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
      supported_networks: getRuntimeSupportedNetworks(),
      supported_assets: getRuntimeSupportedAssets(),
      a2a_skills: agentSkills,
      description: agentDescription,
      trust_level: 'sandbox',
    });
    cachedCard = card;
    emitter.emit('card:registered', { card });
    return { card, created: true };
  }

  async function syncAgentCard(options = {}) {
    const { card } = ensureAgentCard();
    const x402 = commerce.x402();
    const settlementPaymentAddresses = {};
    for (const [chainId, settlementService] of settlementServices.entries()) {
      try {
        const address =
          options.settlementAddress !== undefined && chainId === getRuntimeDefaultNetwork()
            ? options.settlementAddress
            : await settlementService.getAddress();
        if (address) {
          settlementPaymentAddresses[chainId] = address;
        }
      } catch {
        // Ignore settlement address lookup failures during card sync.
      }
    }
    const paymentAddresses = {
      ...(parsePaymentAddresses(card?.payment_addresses) || {}),
      ...settlementPaymentAddresses,
      ...(options.paymentAddresses || {}),
    };
    const settlementAddress =
      options.settlementAddress !== undefined
        ? options.settlementAddress
        : await getOwnReceiveAddress(getRuntimeDefaultNetwork());

    if (settlementAddress && getRuntimeDefaultNetwork()) {
      paymentAddresses[getRuntimeDefaultNetwork()] = settlementAddress;
    }

    const updated = x402.updateAgent(card.id, {
      supported_networks: getRuntimeSupportedNetworks(),
      supported_assets: getRuntimeSupportedAssets(),
      payment_addresses: JSON.stringify(paymentAddresses),
      updated_at: new Date().toISOString(),
    });

    cachedCard = updated;
    emitter.emit('card:synced', { card: updated, paymentAddresses });
    return updated;
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

  async function resolveCounterpartyPaymentAddress(params) {
    const {
      agentId: counterpartyAgentId,
      identityAddress,
      network,
      metadata,
      metadataKey,
    } = params;
    const metadataRecord = parseMetadata(metadata);
    const fallbackAddress = metadataRecord?.[metadataKey] || identityAddress;

    let agent = null;
    if (counterpartyAgentId) {
      agent = await commerce.x402().getAgent(counterpartyAgentId);
    } else if (identityAddress) {
      agent = await commerce.x402().getAgentByWallet(identityAddress);
    }

    if (!agent) {
      return fallbackAddress;
    }

    const paymentAddresses = parsePaymentAddresses(agent?.payment_addresses);
    return paymentAddresses?.[network] || fallbackAddress;
  }

  async function ensureSettlementFunds(settlementService, amount, eventContext = {}) {
    if (!settlementService) {
      return null;
    }

    try {
      const fundCheck = await settlementService.hasSufficientFunds(amount);
      if (!fundCheck.sufficient) {
        emitter.emit('settlement:insufficient_funds', {
          ...eventContext,
          required: amount,
          available: fundCheck.balance,
          symbol: fundCheck.symbol,
          chainId: settlementService.chainId,
        });
        throw new Error(
          `Insufficient on-chain funds: need ${amount}, have ${fundCheck.balance} ${fundCheck.symbol}`,
        );
      }
      return fundCheck;
    } catch (prefErr) {
      if (!prefErr.message?.startsWith('Insufficient on-chain funds:')) {
        emitter.emit('settlement:failed', {
          ...eventContext,
          phase: 'preflight',
          error: prefErr.message,
        });
      }
      throw prefErr;
    }
  }

  async function settleRecordedPayment(params) {
    const {
      payment,
      amount,
      asset,
      settlementService,
      memo,
      counterpartyAgentId,
      identityAddress,
      metadata,
      metadataKey,
      eventContext = {},
    } = params;

    if (!payment || !settlementService) {
      return { success: true, settled: false };
    }

    try {
      const payoutAddress =
        (await resolveCounterpartyPaymentAddress({
          agentId: counterpartyAgentId,
          identityAddress,
          network: settlementService.chainId,
          metadata,
          metadataKey,
        })) ||
        payment.recipient_address ||
        identityAddress;

      emitter.emit('settlement:pending', {
        ...eventContext,
        paymentId: payment.id,
        amount,
        toAddress: payoutAddress,
        chainId: settlementService.chainId,
      });

      const sResult = await settlementService.settle({
        toAddress: payoutAddress,
        amount,
        asset: asset || payment.asset || getDefaultAssetForNetwork(settlementService.chainId),
        memo: memo || null,
        paymentId: payment.id,
      });

      if (!sResult.success) {
        throw new Error(sResult.error || 'Settlement failed');
      }

      commerce.a2a().updatePayment(payment.id, {
        status: 'completed',
        tx_hash: sResult.txHash || null,
        block_number: sResult.blockNumber || null,
        completed_at: new Date().toISOString(),
        metadata: JSON.stringify({
          explorer_url: sResult.explorerUrl,
          confirmations: sResult.confirmations,
          chain_id: settlementService.chainId,
          simulated: sResult.simulated || false,
        }),
      });

      if (!sResult.simulated && sResult.txHash) {
        syncTrackedSettlement({
          intentId: payment.id,
          txHash: sResult.txHash,
          chainId: settlementService.chainId,
          blockNumber: sResult.blockNumber || 0,
          confirmations: sResult.confirmations || 0,
        });
      }

      emitter.emit('settlement:confirmed', {
        ...eventContext,
        paymentId: payment.id,
        txHash: sResult.txHash,
        blockNumber: sResult.blockNumber,
        explorerUrl: sResult.explorerUrl,
        confirmations: sResult.confirmations,
        simulated: sResult.simulated || false,
      });

      return { success: true, settled: true, settlement: sResult };
    } catch (settleErr) {
      commerce.a2a().updatePayment(payment.id, {
        status: 'failed',
        metadata: JSON.stringify({ settlement_error: settleErr.message }),
      });
      markTrackedSettlementFailed(payment.id, settleErr.message);

      emitter.emit('settlement:failed', {
        ...eventContext,
        paymentId: payment.id,
        error: settleErr.message,
      });
      throw settleErr;
    }
  }

  // Auto-register card on creation if requested
  if (autoRegisterCard) {
    try {
      ensureAgentCard();
      if (settlement) {
        Promise.resolve(syncAgentCard()).catch((err) => {
          logger(`[${name}] Agent card sync failed: ${err.message}`);
        });
      }
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
            const quoteNetwork = normalizeAcceptedNetworks(
              quote.accepted_networks,
              quote.network || getRuntimeDefaultNetwork(),
            )[0];
            const quoteSettlement = getSettlementService(quoteNetwork);

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

            const quoteAsset =
              quote.asset || getDefaultAssetForNetwork(quoteNetwork) || getRuntimeDefaultAsset();
            const budgetCheck = evaluateBudget(total, {
              asset: quoteAsset,
              network: quoteNetwork,
            });
            if (!budgetCheck.allowed) {
              emitBudgetExceeded(budgetCheck, {
                operation: 'quote:accept',
                referenceType: 'quote',
                referenceId: quote.id,
                quoteId: quote.id,
              });
              continue;
            }

            // Pre-flight on-chain balance check
            if (settlementServices.size > 0 && !quoteSettlement) {
              emitter.emit('settlement:failed', {
                quoteId: quote.id,
                phase: 'selection',
                error: `No settlement service configured for network ${quoteNetwork}`,
              });
              logger(
                `[${name}] Missing settlement service for quote ${quote.id} on ${quoteNetwork}`,
              );
              continue;
            }

            if (quoteSettlement) {
              try {
                const fundCheck = await quoteSettlement.hasSufficientFunds(total);
                if (!fundCheck.sufficient) {
                  emitter.emit('settlement:insufficient_funds', {
                    quoteId: quote.id,
                    required: total,
                    available: fundCheck.balance,
                    symbol: fundCheck.symbol,
                    chainId: quoteSettlement.chainId,
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

            const result = await acceptQuote(quote.id);
            if (circuitBreakerSvc) circuitBreakerSvc.recordSuccess(name, total);
            emitter.emit('quote:accepted', { quote, payment: result.payment });
            emitter.emit('payment:sent', { payment: result.payment });

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

      // 6. Subscriptions: process billing for subscriptions where we are the subscriber
      try {
        const billing = await processSubscriptionBilling();
        if (billing.processed > 0) {
          processed += billing.processed;
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
          if (!expireErr.message?.includes('not a function')) {
            console.debug('marketplace expireRFQs failed:', expireErr.message || expireErr);
          }
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
    a2a: settlementAwareA2A,

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
    requestQuote,
    requestPayment,
    pay,
    payRequest,
    acceptQuote,
    getPayment,
    refreshPayment,

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
    get settlement() {
      return settlement;
    },
    set settlement(nextSettlement) {
      if (!nextSettlement) {
        clearSettlementServices();
        return;
      }
      registerSettlementService(nextSettlement);
    },
    setSettlement(nextSettlement) {
      if (!nextSettlement) {
        clearSettlementServices();
        return null;
      }
      return registerSettlementService(nextSettlement);
    },
    getSettlement(chainId) {
      return getSettlementService(chainId);
    },
    listSettlementChains() {
      return getSettlementChains();
    },
    getDefaultPaymentConfig() {
      return {
        asset: getRuntimeDefaultAsset(),
        network: getRuntimeDefaultNetwork(),
      };
    },
    async getOnChainBalance(chainId) {
      const settlementService = getSettlementService(chainId);
      if (!settlementService) return null;
      return settlementService.getBalance();
    },
    async getChainWalletAddress(chainId) {
      const settlementService = getSettlementService(chainId);
      if (!settlementService) return null;
      return settlementService.getAddress();
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
    syncAgentCard,
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
