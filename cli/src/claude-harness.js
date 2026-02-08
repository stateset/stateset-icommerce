/**
 * Claude Agent SDK integration for StateSet iCommerce CLI
 * Supports multiple specialized agents with domain-specific tools and prompts
 *
 * v0.4.0 Enhancements:
 * - Lane-based command queue for session serialization
 * - Context window guard with automatic compaction
 * - Model fallback chain for resilience
 * - Dual memory storage (SQLite + Markdown)
 * - Semantic browser snapshots
 */

import { query } from '@anthropic-ai/claude-agent-sdk';
import { randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { DEFAULT_MODEL, THINK_LEVELS } from './config.js';
import { createStatesetMcpServer } from './mcp-server.js';
import { createX402McpServer, X402_MCP_TOOL_NAMES } from './x402-mcp-server.js';
import { AgentTelemetry, noOpTelemetry } from './telemetry.js';
import { createPermissionGate } from './permissions.js';
import { loadSyncConfig, SyncConfig } from './sync/config.js';
import { wrapCommerceWithEvents } from './sync/capture.js';
import { createSyncEngine } from './sync/engine.js';

// v0.4.0: New modules for enhanced reliability
import { getCommandQueue } from './command-queue.js';
import { ContextGuard } from './context-guard.js';
import { ModelFallback } from './model-fallback.js';
import { getMarkdownMemoryStore } from './memory/markdown-store.js';
import { getMemoryStore } from './memory/store.js';
import { loadAgentSettings } from './settings.js';
import { getAgentSessionStore } from './agent-session-store.js';
import { resolveProviderApiKey } from './credentials.js';
import { ensureHarnessPluginsLoaded, getHarnessHookRunner } from './harness-hooks.js';
import { redactSensitive, redactObject } from './privacy.js';

// Extracted modules
import {
  buildPromptWithHistory,
  extractCompactionSummary,
  estimateTokensFromText,
} from './conversation-history.js';
import { isRetryableError, computeRetryDelay, sleep } from './retry-helpers.js';
import { AGENTS } from './agent-definitions.js';

const require = createRequire(import.meta.url);

/** @type {any} */
let _CommerceCtor = null;

function getCommerceCtor() {
  if (_CommerceCtor) return _CommerceCtor;
  let mod;
  try {
    mod = require('@stateset/embedded');
  } catch (err) {
    const msg = err && typeof err.message === 'string' ? err.message : String(err);
    throw new Error(`Failed to load @stateset/embedded. ${msg}`);
  }

  const CommerceCtor = mod.Commerce || mod.default?.Commerce || mod.default;
  if (!CommerceCtor) {
    throw new Error('Failed to resolve Commerce export from @stateset/embedded.');
  }

  _CommerceCtor = CommerceCtor;
  return CommerceCtor;
}

function buildClaudeEnv({ env: envOverrides = null, apiKey = null } = {}) {
  const env = { ...process.env, ...(envOverrides || {}) };
  if (apiKey) {
    env.ANTHROPIC_API_KEY = apiKey;
    return env;
  }
  if (!env.ANTHROPIC_API_KEY) {
    const storedKey = resolveProviderApiKey('claude');
    if (storedKey) env.ANTHROPIC_API_KEY = storedKey;
  }
  return env;
}

function normalizeAbortController({ abortController = null, signal = null } = {}) {
  if (abortController) return abortController;
  if (!signal) return null;
  const controller = new AbortController();
  if (signal.aborted) {
    controller.abort(signal.reason);
    return controller;
  }
  signal.addEventListener('abort', () => controller.abort(signal.reason), { once: true });
  return controller;
}

function emitEvent(onEvent, event) {
  if (typeof onEvent !== 'function') return;
  try {
    const result = onEvent(event);
    if (result && typeof result.catch === 'function') {
      result.catch((err) => {
        console.error('[Harness] onEvent error:', err?.message || err);
      });
    }
  } catch (err) {
    console.error('[Harness] onEvent error:', err?.message || err);
  }
}

// Re-export AGENTS from agent-definitions for consumers that import from claude-harness
export { AGENTS };

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

// ============================================================================
// Main Agent Loop
// ============================================================================

/**
 * Run the Claude agent loop
 * @param {Object} options
 * @param {string} options.request - Natural language request
 * @param {string} options.dbPath - Path to SQLite database
 * @param {string} options.model - Claude model to use
 * @param {boolean} options.allowApply - Whether to allow write operations
 * @param {number} options.maxTurns - Maximum conversation turns
 * @param {string} options.resumeSessionId - Session ID to resume
 * @param {string} options.agent - Specific agent to use (optional, auto-routes if not specified)
 * @param {Function} options.onToolCall - Callback for tool invocations
 * @param {Function} options.onMessage - Callback for assistant messages
 * @param {boolean} options.verbose - Enable verbose telemetry output
 * @param {Object} options.guardrails - Custom guardrails configuration
 * @param {Function} options.onConfirmRequired - Callback for confirmation prompts
 * @param {AgentTelemetry} options.telemetry - Custom telemetry instance
 * @param {PermissionGate} options.permissionGate - Custom permission gate instance
 * @param {boolean} options.enableSync - Enable VES sync event capture (default: auto-detect from config)
 * @param {boolean} options.autoSyncPush - Auto-push events after mutations (default: false)
 * @param {Function} options.onSyncEvent - Callback when sync event is captured
 * @param {string} options.thinkLevel - Extended thinking level: off|low|medium|high
 * @param {boolean} options.streaming - Enable streaming/partial messages
 * @param {number|null} options.maxBudgetUsd - Maximum budget in USD per query
 * @param {string} options.provider - AI provider: claude|openai|gemini|ollama
 * @param {Function} options.onPartialMessage - Callback for streaming tokens
 * @param {Function} options.onThinkingBlock - Callback for thinking content blocks
 * @param {boolean} options.enableFallback - Enable automatic model fallback (default: true)
 * @param {boolean} options.enableContextGuard - Enable context window guard (default: true)
 * @param {boolean} options.enableMemory - Enable memory persistence (default: true)
 * @param {boolean} options.useMarkdownMemory - Use markdown memory store (default: true)
 * @param {object[]} options.conversationHistory - Existing conversation history for context
 * @param {Function} options.transformContext - Optional transform for conversation history before prompt build
 * @param {Function} options.onContextWarning - Callback when context approaches limit
 * @param {Function} options.onFallback - Callback when falling back to alternative model
 * @param {boolean} options.enableX402 - Enable x402 MCP server tools (default: false)
 * @param {string} options.apiKey - Override Claude API key for this run
 * @param {Function} options.getApiKey - Resolve API key dynamically for this run
 * @param {AbortController} options.abortController - Abort controller for cancelling the run
 * @param {AbortSignal} options.signal - Abort signal for cancelling the run
 * @param {Object} options.settings - Settings overrides (merged with defaults)
 * @param {Object} options.retry - Retry settings overrides
 * @param {Object} options.privacy - Privacy/redaction overrides
 * @param {Object} options.sessionStore - Custom session store instance
 * @param {Object} options.hookRunner - HookRunner instance for prompt/tool hooks
 * @param {boolean} options.enablePlugins - Enable harness plugin loading
 * @param {Object} options.contextGuardOptions - Override context guard thresholds
 * @param {Function} options.onEvent - Event callback for agent lifecycle events
 */
export async function runAgentLoop({
  request,
  dbPath = './store.db',
  model,
  allowApply = false,
  maxTurns = 10,
  resumeSessionId,
  agent,
  onToolCall,
  onMessage,
  verbose = false,
  guardrails = null,
  onConfirmRequired = null,
  telemetry = null,
  permissionGate = null,
  enableSync = null,
  autoSyncPush = false,
  onSyncEvent = null,
  thinkLevel,
  streaming = false,
  maxBudgetUsd = null,
  provider,
  onPartialMessage = null,
  onThinkingBlock = null,
  // v0.4.0: New options
  enableFallback = true,
  enableContextGuard = null,
  enableMemory = null,
  useMarkdownMemory = null,
  conversationHistory = [],
  transformContext = null,
  onContextWarning = null,
  onFallback = null,
  enableX402 = false,
  apiKey = null,
  getApiKey = null,
  abortController = null,
  signal = null,
  settings = null,
  retry = null,
  privacy = null,
  sessionStore = null,
  hookRunner = null,
  enablePlugins = null,
  contextGuardOptions = null,
  onEvent = null,
  treasury = null,
}) {
  const resolvedSettings = loadAgentSettings(settings || {});
  const retrySettings = { ...resolvedSettings.retry, ...(retry || {}) };
  const privacySettings = { ...resolvedSettings.privacy, ...(privacy || {}) };
  const eventRedact = privacySettings.redactLogs;
  const redactEventText = (text) => (eventRedact ? redactSensitive(text, privacySettings) : text);
  const redactEventValue = (value) => (eventRedact ? redactObject(value, privacySettings) : value);
  const contextSettings = { ...resolvedSettings.contextGuard, ...(contextGuardOptions || {}) };
  const memorySettings = { ...resolvedSettings.memory };
  const effectiveEnableContextGuard = enableContextGuard ?? contextSettings.enabled;
  const effectiveEnableMemory = enableMemory ?? memorySettings.enabled;
  const effectiveUseMarkdownMemory = useMarkdownMemory ?? memorySettings.useMarkdown;
  const pluginsEnabled = enablePlugins ?? resolvedSettings.plugins?.enabled ?? false;
  const pluginsVerbose = resolvedSettings.plugins?.verbose ?? false;
  const effectiveGuardrails = guardrails
    ? { ...resolvedSettings.guardrails, ...guardrails }
    : { ...resolvedSettings.guardrails };
  const envTreasuryEnabled = process.env.TREASURY_BILLING === 'true';
  const envTreasuryChain = process.env.TREASURY_CHAIN || null;
  const envTreasuryToken = process.env.TREASURY_TOKEN || null;
  const envTreasuryAgent = process.env.TREASURY_AGENT || 'default';
  const envTreasuryDb = process.env.TREASURY_DB || null;
  const envTreasuryLlm = process.env.TREASURY_LLM_BILLING === 'true';
  const envTreasuryRegistry = process.env.TREASURY_ERC8004_REGISTRY || null;
  const envTreasuryRegistryDb = process.env.TREASURY_ERC8004_DB || null;

  // Determine provider/model/think level with session restore
  let effectiveProvider = provider || resolvedSettings.provider?.default || 'claude';
  let effectiveModel = model || resolvedSettings.model?.default || DEFAULT_MODEL;
  let effectiveThinkLevel = thinkLevel ?? resolvedSettings.thinkLevel?.default ?? 'off';

  const treasuryConfig = treasury
    ? { ...treasury }
    : envTreasuryEnabled
      ? {
          enabled: true,
          chainId: envTreasuryChain || 'set_chain',
          tokenSymbol: envTreasuryToken || 'USDC',
          agentId: envTreasuryAgent,
          dbPath: envTreasuryDb,
          chargeLlm: envTreasuryLlm || envTreasuryEnabled,
        }
      : null;

  if (treasuryConfig) {
    if (!treasuryConfig.chainId && envTreasuryChain) {
      treasuryConfig.chainId = envTreasuryChain;
    }
    if (!treasuryConfig.tokenSymbol && envTreasuryToken) {
      treasuryConfig.tokenSymbol = envTreasuryToken;
    }
    if (!treasuryConfig.agentId && envTreasuryAgent) {
      treasuryConfig.agentId = envTreasuryAgent;
    }
    if (!treasuryConfig.dbPath && envTreasuryDb) {
      treasuryConfig.dbPath = envTreasuryDb;
    }
    if (treasuryConfig.chargeLlm === undefined) {
      treasuryConfig.chargeLlm = envTreasuryLlm || envTreasuryEnabled || true;
    }
    if (!treasuryConfig.erc8004Registry && envTreasuryRegistry) {
      treasuryConfig.erc8004Registry = envTreasuryRegistry;
    }
    if (!treasuryConfig.erc8004DbPath && envTreasuryRegistryDb) {
      treasuryConfig.erc8004DbPath = envTreasuryRegistryDb;
    }
  }

  let treasuryState = null;
  let treasuryCharge = null;
  let effectiveMaxBudgetUsd = maxBudgetUsd;

  const useSessionStore = resolvedSettings.sessionStore?.enabled !== false;
  let sessionStoreInstance = sessionStore || null;
  if (!sessionStoreInstance && useSessionStore) {
    try {
      sessionStoreInstance = getAgentSessionStore({
        dbPath: resolvedSettings.sessionStore?.dbPath || undefined,
        maxSummaries:
          resolvedSettings.sessionStore?.maxSummaries || memorySettings.maxSummaries || 5,
      });
    } catch (err) {
      console.warn('[Harness] Session store unavailable:', err.message);
      sessionStoreInstance = null;
    }
  }

  let sessionMeta = null;
  if (resumeSessionId && sessionStoreInstance && resolvedSettings.model?.preferSession !== false) {
    try {
      sessionMeta = sessionStoreInstance.get(resumeSessionId);
    } catch (err) {
      console.warn('[Harness] Session store read failed:', err.message);
      sessionMeta = null;
    }
  }

  if (sessionMeta) {
    if (!provider && sessionMeta.provider) effectiveProvider = sessionMeta.provider;
    if (!model && sessionMeta.model) effectiveModel = sessionMeta.model;
    if ((thinkLevel === null || thinkLevel === undefined) && sessionMeta.thinkLevel) {
      effectiveThinkLevel = sessionMeta.thinkLevel;
    }
    if (!agent && sessionMeta.agent) agent = sessionMeta.agent;
  }

  if (pluginsEnabled) {
    try {
      await ensureHarnessPluginsLoaded({ verbose: pluginsVerbose || verbose });
    } catch (err) {
      console.warn('[Harness] Plugin load failed:', err.message);
    }
  }
  const hooks = hookRunner || getHarnessHookRunner();

  let effectiveRequest = request;
  let systemPromptOverride = null;
  if (hooks?.hasHooks?.('before_agent_start')) {
    const hookResult = await hooks.run('before_agent_start', {
      request: effectiveRequest,
      agent,
      model: effectiveModel,
      provider: effectiveProvider,
      thinkLevel: effectiveThinkLevel,
      guardrails: effectiveGuardrails,
      allowApply,
      conversationHistory,
      systemPrompt: AGENTS[agent]?.systemPrompt,
    });
    if (hookResult?.request) effectiveRequest = hookResult.request;
    if (hookResult?.agent) agent = hookResult.agent;
    if (hookResult?.model) effectiveModel = hookResult.model;
    if (hookResult?.provider) effectiveProvider = hookResult.provider;
    if (hookResult?.thinkLevel) effectiveThinkLevel = hookResult.thinkLevel;
    if (hookResult?.systemPrompt) {
      systemPromptOverride = hookResult.systemPrompt;
    } else if (hookResult?.systemPromptAppend && AGENTS[agent]?.systemPrompt) {
      systemPromptOverride = `${AGENTS[agent].systemPrompt}\n\n${hookResult.systemPromptAppend}`;
    }
  }

  const resolvedAbortController = normalizeAbortController({ abortController, signal });
  const effectiveSignal = resolvedAbortController?.signal || signal || null;

  const safeRequestForLogs = privacySettings.redactLogs
    ? redactSensitive(effectiveRequest, privacySettings)
    : effectiveRequest;

  // Initialize telemetry
  const telem = telemetry || (verbose ? new AgentTelemetry({ verbose }) : noOpTelemetry);
  const mainSpan = telem.startSpan('agent_run', {
    request: safeRequestForLogs.slice(0, 100),
    agent,
  });

  // Initialize permission gate
  const gate =
    permissionGate ||
    createPermissionGate({
      apply: allowApply,
      guardrails: effectiveGuardrails,
      onConfirmRequired,
    });

  // Emit initial lifecycle events
  emitEvent(onEvent, { type: 'agent_start' });
  emitEvent(onEvent, { type: 'turn_start' });
  const userEventMessage = { role: 'user', content: redactEventText(effectiveRequest) };
  emitEvent(onEvent, { type: 'message_start', message: userEventMessage });
  emitEvent(onEvent, { type: 'message_end', message: userEventMessage });

  // -------------------------------------------------------------------------
  // v0.4.0: Context Guard - Check context window before proceeding
  // -------------------------------------------------------------------------
  const sessionSummary = sessionMeta?.summaries?.[0] || null;
  const baseHistory =
    conversationHistory.length > 0
      ? conversationHistory
      : sessionSummary
        ? [
            { role: 'user', content: sessionSummary },
            {
              role: 'assistant',
              content: 'Understood. I have the context from our earlier conversation.',
            },
          ]
        : [];
  let workingHistory = [...baseHistory];
  let contextGuardResult = null;
  let compactionSummary = null;

  if (typeof transformContext === 'function') {
    try {
      const transformed = await transformContext(workingHistory, effectiveSignal);
      if (Array.isArray(transformed)) {
        workingHistory = transformed;
      }
    } catch (err) {
      console.warn('[Harness] transformContext failed:', err.message);
    }
  }

  if (effectiveEnableContextGuard && workingHistory.length > 0) {
    const contextGuard = ContextGuard.forModel(effectiveModel, {
      warningThreshold: contextSettings.warningThreshold,
      compactThreshold: contextSettings.compactThreshold,
      abortThreshold: contextSettings.abortThreshold,
      reserveTokens: contextSettings.reserveTokens,
    });
    contextGuardResult = contextGuard.check(
      workingHistory,
      '', // System prompt will be added by SDK
      effectiveRequest,
    );

    if (!contextGuardResult.safe && contextGuardResult.action === 'abort') {
      telem.logCustomEvent('context_overflow', {
        tokens: contextGuardResult.usage.tokens,
        percent: contextGuardResult.usage.percent,
      });
      throw new Error(contextGuardResult.message);
    }

    if (contextGuardResult.action === 'compact') {
      let historyForCompaction = workingHistory;
      if (hooks?.hasHooks?.('before_compaction')) {
        const hookResult = await hooks.run('before_compaction', {
          history: historyForCompaction,
          usage: contextGuardResult.usage,
          request: effectiveRequest,
        });
        if (hookResult?.history) {
          historyForCompaction = hookResult.history;
        }
      }

      if (historyForCompaction !== workingHistory) {
        workingHistory = historyForCompaction;
        contextGuardResult = contextGuard.check(workingHistory, '', effectiveRequest);
        if (!contextGuardResult.safe && contextGuardResult.action === 'abort') {
          telem.logCustomEvent('context_overflow', {
            tokens: contextGuardResult.usage.tokens,
            percent: contextGuardResult.usage.percent,
          });
          throw new Error(contextGuardResult.message);
        }
      }

      if (contextGuardResult.action !== 'compact') {
        // Recheck after hook no longer requires compaction.
      } else {
        workingHistory = contextGuardResult.compactedHistory;
        compactionSummary = extractCompactionSummary(contextGuardResult.compactedHistory);
        telem.logCustomEvent('context_compacted', {
          originalTokens: contextGuardResult.usage.tokens,
          compactedTokens: contextGuardResult.usage.afterCompaction?.tokens,
          tokensSaved: contextGuardResult.usage.afterCompaction?.tokensSaved,
        });
        if (hooks?.hasHooks?.('after_compaction')) {
          await hooks.run('after_compaction', {
            summary: compactionSummary,
            usage: contextGuardResult.usage,
          });
        }
      }
    }

    if (contextGuardResult.action === 'warn' && onContextWarning) {
      onContextWarning(contextGuardResult);
    }
  }

  // Only inject history into the prompt when not resuming an SDK session.
  // Resumed sessions already carry server-side context.
  const shouldIncludeHistory = workingHistory.length > 0 && !resumeSessionId;
  const requestWithHistory = shouldIncludeHistory
    ? buildPromptWithHistory(effectiveRequest, workingHistory, {
        redactHistory: privacySettings.redactHistory,
        redactOptions: privacySettings,
      })
    : effectiveRequest;

  // -------------------------------------------------------------------------
  // v0.4.0: Model Fallback - Set up fallback chain
  // -------------------------------------------------------------------------
  let modelFallback = null;
  if (enableFallback && effectiveProvider === 'claude') {
    modelFallback = new ModelFallback({
      requiredCapabilities: effectiveThinkLevel !== 'off' ? ['tools', 'thinking'] : ['tools'],
      onFallback: (info) => {
        telem.logCustomEvent('model_fallback', {
          from: info.from.id,
          to: info.to.id,
          reason: info.reason,
        });
        if (onFallback) onFallback(info);
      },
      onCooldown: (info) => {
        telem.logCustomEvent('model_cooldown', {
          model: info.model.id,
          reason: info.reason,
          permanent: info.permanent,
        });
      },
    });
  }

  // -------------------------------------------------------------------------
  // v0.4.0: Memory stores initialization
  // -------------------------------------------------------------------------
  let memoryStore = null;
  let markdownMemory = null;

  if (effectiveEnableMemory) {
    try {
      memoryStore = getMemoryStore();
      if (effectiveUseMarkdownMemory) {
        markdownMemory = getMarkdownMemoryStore();
      }
    } catch (e) {
      telem.logCustomEvent('memory_init_failed', { error: e.message });
    }
  }

  // Initialize commerce instance
  const Commerce = getCommerceCtor();
  let commerce = new Commerce(dbPath);
  let syncEngine = null;
  let syncConfig = null;

  // Check if sync is configured and should be enabled
  const rawSyncConfig = loadSyncConfig();
  const shouldEnableSync = enableSync !== null ? enableSync : rawSyncConfig !== null;

  if (shouldEnableSync && rawSyncConfig) {
    syncConfig = new SyncConfig(rawSyncConfig);

    // Wrap commerce with event capture
    commerce = wrapCommerceWithEvents(commerce, syncConfig);

    // Log sync enablement
    telem.logCustomEvent('sync_enabled', {
      tenantId: syncConfig.tenantId,
      storeId: syncConfig.storeId,
      agentId: syncConfig.agentId,
    });

    // Set up sync event callback if provided
    if (onSyncEvent && commerce._capture) {
      const originalCapture = commerce._capture.capture.bind(commerce._capture);
      commerce._capture.capture = (resourceMethod, entityId, payload, options) => {
        originalCapture(resourceMethod, entityId, payload, options);
        onSyncEvent({ resourceMethod, entityId, payload, options });
      };
    }

    // Initialize sync engine if auto-push is enabled
    if (autoSyncPush) {
      try {
        syncEngine = createSyncEngine({ db: commerce.db, config: syncConfig });
        await syncEngine.initialize();
      } catch (error) {
        // Log but don't fail - sync is optional
        telem.logCustomEvent('sync_init_failed', { error: error.message });
      }
    }
  }

  // Create MCP server with telemetry and permissions
  const mcpServer = createStatesetMcpServer({
    commerce,
    dbPath,
    allowApply,
    telemetry: telem,
    permissionGate: gate,
    hookRunner: hooks,
    treasury: treasuryConfig,
  });

  const mcpServers = {
    'stateset-commerce': mcpServer,
  };

  // Determine which agent to use
  const routingResult = routeToAgentWithConfidence(effectiveRequest);
  let agentName = agent || routingResult.primary.agent;
  if (!agent && routingResult.primary.level === 'default' && resolvedSettings.agent?.default) {
    agentName = resolvedSettings.agent.default;
  }
  const agentConfig = AGENTS[agentName] || AGENTS['customer-service'];

  const allowedTools = [...agentConfig.tools];

  const shouldEnableX402 = Boolean(
    enableX402 || process.env.X402_ENABLE === '1' || process.env.X402_SEQUENCER_URL,
  );

  if (shouldEnableX402) {
    const configDir = process.env.STATESET_CONFIG_DIR || '.stateset';
    const x402Server = createX402McpServer({ env: process.env, configDir });
    mcpServers['stateset-x402'] = x402Server;
    allowedTools.push(...X402_MCP_TOOL_NAMES.map((name) => `mcp__stateset-x402__${name}`));
  }

  // Log routing decision
  telem.logAgentRouting(
    safeRequestForLogs,
    agentName,
    routingResult.primary.confidence,
    routingResult.alternatives,
  );

  if (treasuryConfig?.enabled) {
    try {
      const { loadTreasuryContext, resolveToken } = await import('./treasury/index.js');
      const { fromSmallestUnit } = await import('./chains/config.js');
      const ctx = await loadTreasuryContext({
        dbPath: treasuryConfig.dbPath || undefined,
      });
      const chainId = treasuryConfig.chainId || 'set_chain';
      const tokenSymbol = treasuryConfig.tokenSymbol || 'USDC';
      let agentId = treasuryConfig.agentId || 'default';
      let erc8004Identity = null;
      const erc8004Registry = treasuryConfig.erc8004Registry || null;
      if (erc8004Registry) {
        const { getIdentity } = await import('./erc8004/index.js');
        const identityDbPath = treasuryConfig.erc8004DbPath || dbPath;
        erc8004Identity = getIdentity(identityDbPath, erc8004Registry, agentId);
        if (!erc8004Identity) {
          throw new Error(`ERC-8004 identity not found for ${erc8004Registry}:${agentId}`);
        }
        agentId = erc8004Identity.agent_id;
      }
      const token = resolveToken(chainId, tokenSymbol, ctx.registry);
      if (!token) {
        throw new Error(`Unknown treasury token ${tokenSymbol} on ${chainId}.`);
      }
      const balance = ctx.store.getBalance({
        agentId,
        chainId,
        tokenSymbol: token.symbol,
        tokenDecimals: token.decimals,
      });
      const balanceDisplay = fromSmallestUnit(balance.balanceSmallest, token.decimals);
      const balanceUsd = Number.parseFloat(balanceDisplay);
      if (!Number.isFinite(balanceUsd) || balanceUsd <= 0) {
        throw new Error(`Treasury balance is empty for ${token.symbol} on ${chainId}.`);
      }
      const resolvedBudget = maxBudgetUsd ? Math.min(Number(maxBudgetUsd), balanceUsd) : balanceUsd;
      if (!Number.isFinite(resolvedBudget) || resolvedBudget <= 0) {
        throw new Error(`Treasury budget unavailable for ${token.symbol} on ${chainId}.`);
      }
      effectiveMaxBudgetUsd = resolvedBudget;
      treasuryState = {
        enabled: true,
        chargeLlm: treasuryConfig.chargeLlm !== false,
        ctx,
        agentId,
        chainId,
        token,
        balanceUsd,
        requestId: randomUUID(),
        erc8004Registry,
        erc8004Identity,
      };
    } catch (error) {
      throw new Error(`Treasury billing failed: ${error.message}`);
    }
  }

  const recordTreasuryLlmCharge = async ({
    costUsd,
    sessionId: chargeSessionId,
    provider: chargeProvider,
    model: chargeModel,
    usage,
  }) => {
    if (!treasuryState?.enabled || !treasuryState.chargeLlm) return null;
    const amount = Number(costUsd);
    if (!Number.isFinite(amount) || amount <= 0) return null;
    try {
      const { recordFee } = await import('./treasury/index.js');
      const erc8004Meta = treasuryState.erc8004Identity
        ? {
            erc8004: {
              registry: treasuryState.erc8004Registry,
              agentId: treasuryState.erc8004Identity.agent_id,
              wallet: treasuryState.erc8004Identity.agent_wallet,
              owner: treasuryState.erc8004Identity.owner_address,
            },
          }
        : {};
      const entry = await recordFee(
        {
          agentId: treasuryState.agentId,
          chainId: treasuryState.chainId,
          tokenSymbol: treasuryState.token.symbol,
          amount,
          source: 'llm',
          metadata: {
            provider: chargeProvider,
            model: chargeModel,
            usage: usage || null,
            costUsd: amount,
            ...erc8004Meta,
          },
          taskId: treasuryState.requestId,
          sessionId: chargeSessionId || null,
          toolName: 'llm_inference',
          requestId: treasuryState.requestId,
        },
        treasuryState.ctx,
      );
      telem.logCustomEvent('treasury_llm_charge', {
        amount,
        token: treasuryState.token.symbol,
        chainId: treasuryState.chainId,
        provider: chargeProvider,
        model: chargeModel,
        sessionId: chargeSessionId || null,
        requestId: treasuryState.requestId,
      });
      return {
        eventId: entry.event_id,
        amount: entry.amount_display,
        amountSmallest: entry.amount_smallest,
        token: entry.token_symbol,
        chainId: entry.chain_id,
      };
    } catch (err) {
      telem.logCustomEvent('treasury_llm_charge_failed', {
        error: err.message,
        amount,
        provider: chargeProvider,
        model: chargeModel,
      });
      return null;
    }
  };

  // Build options
  const thinkTokens = THINK_LEVELS[effectiveThinkLevel] || 0;
  const systemPrompt = systemPromptOverride || agentConfig.systemPrompt;
  let apiKeyOverride = apiKey;
  if (!apiKeyOverride && typeof getApiKey === 'function' && effectiveProvider === 'claude') {
    apiKeyOverride = await getApiKey(effectiveProvider);
  }
  const claudeEnv =
    effectiveProvider === 'claude' ? buildClaudeEnv({ apiKey: apiKeyOverride }) : null;
  const options = {
    model: effectiveModel,
    systemPrompt,
    mcpServers,
    allowedTools,
    maxTurns,
    // Allow MCP tools to run without prompting for permission
    permissionMode: 'bypassPermissions',
    allowDangerouslySkipPermissions: true,
    // v0.2.8: Extended thinking
    ...(thinkTokens > 0 ? { maxThinkingTokens: thinkTokens } : {}),
    // v0.2.8: Streaming partial messages
    ...(streaming ? { includePartialMessages: true } : {}),
    // v0.2.8: Budget controls
    ...(effectiveMaxBudgetUsd ? { maxBudgetUsd: parseFloat(effectiveMaxBudgetUsd) } : {}),
    ...(claudeEnv ? { env: claudeEnv } : {}),
    ...(resolvedAbortController ? { abortController: resolvedAbortController } : {}),
  };

  // Track results
  const toolResults = [];
  let sessionId = resumeSessionId;
  let response = '';

  // Save process.argv to restore later (prevent our CLI args from being passed to Claude Code)
  const savedArgv = process.argv;

  try {
    // If resuming, add session ID to options
    if (resumeSessionId) {
      options.resume = resumeSessionId;
    }

    // Clean process.argv before SDK call
    process.argv = process.argv.slice(0, 2); // Keep only node and script path

    // v0.2.8: Non-Claude provider path
    if (effectiveProvider !== 'claude') {
      const { getProviderRegistry } = await import('./providers/base.js');
      const providerInstance = getProviderRegistry().get(effectiveProvider);
      if (!providerInstance) {
        throw new Error(
          `Unknown provider: ${effectiveProvider}. Available: ${getProviderRegistry().list().join(', ')}`,
        );
      }
      if (!(await providerInstance.isAvailable())) {
        const providerConfig = (await import('./config.js')).PROVIDERS[effectiveProvider];
        throw new Error(
          `Provider "${effectiveProvider}" is not available. ${providerConfig?.envKey ? `Set ${providerConfig.envKey} environment variable.` : ''}`,
        );
      }
      const messages = [
        { role: 'system', content: systemPrompt },
        { role: 'user', content: requestWithHistory },
      ];
      let providerApiKey = apiKey;
      if (!providerApiKey && typeof getApiKey === 'function') {
        providerApiKey = await getApiKey(effectiveProvider);
      }
      let providerMaxTokens = null;
      let treasuryBudgetUsd = null;
      if (treasuryState?.enabled) {
        treasuryBudgetUsd = effectiveMaxBudgetUsd ?? treasuryState.balanceUsd;
        if (!Number.isFinite(treasuryBudgetUsd) || treasuryBudgetUsd <= 0) {
          throw new Error(
            `Treasury balance is empty for ${treasuryState.token.symbol} on ${treasuryState.chainId}.`,
          );
        }

        if (typeof providerInstance.estimateCost === 'function') {
          const inputEstimate =
            estimateTokensFromText(systemPrompt) + estimateTokensFromText(requestWithHistory);
          const inputTokensEstimate = Math.ceil(inputEstimate * 1.3) + 32;
          const safetyBudget = treasuryBudgetUsd * 0.95;
          const defaultMaxTokens = 4096;

          const estimateCost = (outputTokens) =>
            providerInstance.estimateCost(
              { inputTokens: inputTokensEstimate, outputTokens },
              effectiveModel,
            );

          const baseCost = estimateCost(0);
          if (baseCost === null || baseCost === undefined) {
            telem.logCustomEvent('treasury_llm_estimate_unavailable', {
              provider: effectiveProvider,
              model: effectiveModel,
            });
          } else if (baseCost > safetyBudget) {
            throw new Error(
              `Treasury balance ${treasuryBudgetUsd.toFixed(4)} is insufficient for estimated input tokens.`,
            );
          } else {
            let low = 0;
            let high = defaultMaxTokens;
            let best = 0;
            while (low <= high) {
              const mid = Math.floor((low + high) / 2);
              const cost = estimateCost(mid);
              if (cost === null || cost === undefined) break;
              if (cost <= safetyBudget) {
                best = mid;
                low = mid + 1;
              } else {
                high = mid - 1;
              }
            }
            if (best <= 0) {
              const oneTokenCost = estimateCost(1);
              if (
                oneTokenCost !== null &&
                oneTokenCost !== undefined &&
                oneTokenCost <= safetyBudget
              ) {
                providerMaxTokens = 1;
              } else {
                throw new Error(
                  `Treasury balance ${treasuryBudgetUsd.toFixed(4)} is insufficient for any output tokens.`,
                );
              }
            } else {
              providerMaxTokens = best;
            }
          }
        } else {
          telem.logCustomEvent('treasury_llm_estimate_unavailable', {
            provider: effectiveProvider,
            model: effectiveModel,
          });
        }
      }
      let assistantStarted = false;
      let partialText = '';
      const handlePartialMessage = (data) => {
        if (!assistantStarted) {
          assistantStarted = true;
          emitEvent(onEvent, {
            type: 'message_start',
            message: { role: 'assistant', content: '' },
          });
        }
        if (data?.text) {
          partialText += data.text;
          emitEvent(onEvent, {
            type: 'message_update',
            message: { role: 'assistant', content: redactEventText(partialText) },
            delta: redactEventText(data.text),
          });
        }
        if (onPartialMessage) onPartialMessage(data);
      };

      const providerResult = await providerInstance.chat(messages, {
        model: effectiveModel,
        stream: streaming,
        onPartialMessage: streaming ? handlePartialMessage : onPartialMessage,
        apiKey: providerApiKey,
        signal: effectiveSignal,
        ...(providerMaxTokens ? { maxTokens: providerMaxTokens } : {}),
      });
      process.argv = savedArgv;
      let providerResponse = providerResult.text;
      let budgetExceeded = false;
      if (
        treasuryState?.enabled &&
        providerResult.cost !== null &&
        providerResult.cost !== undefined &&
        treasuryBudgetUsd !== null &&
        treasuryBudgetUsd !== undefined
      ) {
        budgetExceeded = providerResult.cost > treasuryBudgetUsd;
      }
      if (hooks?.hasHooks?.('before_send')) {
        const hookResult = await hooks.run('before_send', {
          request: effectiveRequest,
          response: providerResponse,
          agent: agentName,
          model: effectiveModel,
          provider: effectiveProvider,
          toolResults: [],
        });
        if (hookResult?.response) {
          providerResponse = hookResult.response;
        }
      }
      const responseForUser = privacySettings.redactResponse
        ? redactSensitive(providerResponse, privacySettings)
        : providerResponse;
      if (onMessage) {
        onMessage(responseForUser);
      }
      if (!assistantStarted) {
        emitEvent(onEvent, {
          type: 'message_start',
          message: { role: 'assistant', content: '' },
        });
      }
      emitEvent(onEvent, {
        type: 'message_end',
        message: { role: 'assistant', content: redactEventText(providerResponse) },
      });
      emitEvent(onEvent, {
        type: 'turn_end',
        response: redactEventText(providerResponse),
        toolResults: [],
      });
      emitEvent(onEvent, {
        type: 'agent_end',
        response: redactEventText(providerResponse),
        toolResults: [],
        sessionId: null,
        agent: agentName,
        provider: effectiveProvider,
        model: effectiveModel,
        cost: providerResult.cost || null,
        budgetExceeded,
      });
      treasuryCharge = await recordTreasuryLlmCharge({
        costUsd: providerResult.cost,
        sessionId: null,
        provider: effectiveProvider,
        model: providerResult.model || effectiveModel,
        usage: providerResult.usage,
      });
      return {
        response: responseForUser,
        toolResults: [],
        sessionId: null,
        agent: agentName,
        routing: routingResult,
        provider: effectiveProvider,
        cost: providerResult.cost || null,
        thinkLevel: effectiveThinkLevel,
        budgetExceeded,
        treasury: treasuryState
          ? {
              requestId: treasuryState.requestId,
              charge: treasuryCharge,
              identity: treasuryState.erc8004Identity,
            }
          : undefined,
      };
    }

    // -------------------------------------------------------------------------
    // v0.4.0: Run query with optional model fallback
    // -------------------------------------------------------------------------
    let budgetExceeded = false;
    let totalCost = null;
    let usedModel = effectiveModel;
    let fallbackAttempts = [];

    // Helper function to run the actual query
    const runQuery = async (queryModel) => {
      const queryOptions = { ...options, model: queryModel };
      const results = {
        toolResults: [],
        response: '',
        sessionId: null,
        budgetExceeded: false,
        totalCost: null,
        error: null,
        errorType: null,
      };
      const pendingToolCalls = new Map();
      let assistantStarted = false;
      let assistantText = '';

      for await (const message of query({ prompt: requestWithHistory, options: queryOptions })) {
        // Capture session ID
        if (message.sessionId && !results.sessionId) {
          results.sessionId = message.sessionId;
        }

        // Handle different message types
        if (message.type === 'assistant') {
          const content = message.message?.content || message.content;
          if (content) {
            if (!assistantStarted) {
              assistantStarted = true;
              emitEvent(onEvent, {
                type: 'message_start',
                message: { role: 'assistant', content: '' },
              });
            }
            for (const block of content) {
              if (block.type === 'tool_use') {
                const toolCall = {
                  id: block.id,
                  name: block.name,
                  input: block.input,
                  startTime: Date.now(),
                };
                const entry = { toolCall, result: null };
                results.toolResults.push(entry);
                if (toolCall.id) {
                  pendingToolCalls.set(toolCall.id, entry);
                }
                emitEvent(onEvent, {
                  type: 'tool_execution_start',
                  toolCallId: toolCall.id,
                  toolName: toolCall.name,
                  args: redactEventValue(toolCall.input),
                });
                if (onToolCall) {
                  onToolCall(toolCall);
                }
              } else if (block.type === 'text') {
                results.response += block.text;
                assistantText = results.response;
                if (streaming) {
                  emitEvent(onEvent, {
                    type: 'message_update',
                    message: { role: 'assistant', content: redactEventText(assistantText) },
                    delta: redactEventText(block.text),
                  });
                }
              } else if (block.type === 'thinking' && onThinkingBlock) {
                onThinkingBlock(block);
              }
            }
          }
        } else if (message.type === 'result') {
          if (message.result) {
            results.response = message.result;
            assistantText = message.result;
          }
          if (message.total_cost_usd !== null && message.total_cost_usd !== undefined) {
            results.totalCost = message.total_cost_usd;
          }
          if (message.subtype === 'error_max_budget_usd') {
            results.budgetExceeded = true;
          }
          if (message.subtype && message.subtype.startsWith('error_')) {
            results.errorType = message.subtype;
            results.error =
              message.errors && message.errors.length > 0
                ? message.errors.join('; ')
                : message.subtype;
          }
          if (!assistantStarted) {
            assistantStarted = true;
            emitEvent(onEvent, {
              type: 'message_start',
              message: { role: 'assistant', content: '' },
            });
          }
          if (assistantText) {
            emitEvent(onEvent, {
              type: 'message_end',
              message: { role: 'assistant', content: redactEventText(assistantText) },
            });
          }
        } else if (message.type === 'user') {
          const toolUseId =
            message.parent_tool_use_id ||
            message.tool_use_id ||
            message.tool_use_result?.tool_use_id ||
            message.tool_use_result?.tool_use_id;
          const pending = toolUseId
            ? pendingToolCalls.get(toolUseId)
            : results.toolResults.find((tr) => tr.result === null);
          if (pending && message.tool_use_result) {
            pending.result = message.tool_use_result;
            pending.endTime = Date.now();
            pending.duration = pending.endTime - pending.toolCall.startTime;
            if (hooks?.hasHooks?.('tool_result_persist')) {
              const hookResult = await hooks.run('tool_result_persist', {
                tool: pending.toolCall.name,
                toolCall: pending.toolCall,
                result: pending.result,
              });
              if (hookResult?.result) {
                pending.result = hookResult.result;
              }
            }
            const logInput = privacySettings.redactLogs
              ? redactObject(pending.toolCall.input, privacySettings)
              : pending.toolCall.input;
            const logResult = privacySettings.redactLogs
              ? redactObject(pending.result, privacySettings)
              : pending.result;
            telem.logToolCall(pending.toolCall.name, logInput, logResult, pending.duration);
            emitEvent(onEvent, {
              type: 'tool_execution_end',
              toolCallId: pending.toolCall.id,
              toolName: pending.toolCall.name,
              result: redactEventValue(pending.result),
              isError: Boolean(pending.result?.is_error || pending.result?.isError),
            });
            if (toolUseId) {
              pendingToolCalls.delete(toolUseId);
            }
          }
        }

        if (
          streaming &&
          onPartialMessage &&
          message.type !== 'assistant' &&
          message.type !== 'result' &&
          message.type !== 'user'
        ) {
          onPartialMessage(message);
        }
      }

      return results;
    };

    const executeOnce = async () => {
      if (modelFallback && enableFallback) {
        const fallbackResult = await modelFallback.execute(
          async (modelConfig) => {
            usedModel = modelConfig.model;
            return runQuery(modelConfig.model);
          },
          { preferredModel: effectiveModel },
        );
        fallbackAttempts = fallbackResult.attempts;
        return fallbackResult.result;
      }
      return runQuery(effectiveModel);
    };

    let queryResult;
    let attempt = 0;
    while (true) {
      attempt++;
      try {
        queryResult = await executeOnce();
        if (queryResult.error) {
          if (queryResult.errorType === 'error_max_budget_usd') {
            break;
          }
          const err = new Error(queryResult.error);
          err.code = queryResult.errorType;
          throw err;
        }
        break;
      } catch (err) {
        const errorType = queryResult?.errorType;
        const nonRetryable = errorType && errorType.startsWith('error_max');
        const canRetry =
          retrySettings?.enabled &&
          attempt <= (retrySettings.maxRetries || 0) &&
          !nonRetryable &&
          isRetryableError(err, retrySettings);

        if (!canRetry) {
          throw err;
        }

        const delayMs = computeRetryDelay(attempt, retrySettings);
        telem.logCustomEvent('auto_retry', {
          attempt,
          delayMs,
          error: err?.message || String(err),
        });
        await sleep(delayMs);
      }
    }

    // Extract results
    const {
      toolResults: queryToolResults,
      response: queryResponse,
      sessionId: querySessionId,
      budgetExceeded: queryBudgetExceeded,
      totalCost: queryTotalCost,
    } = queryResult;
    toolResults.push(...queryToolResults);
    response = queryResponse;
    if (querySessionId) sessionId = querySessionId;
    budgetExceeded = queryBudgetExceeded;
    totalCost = queryTotalCost;

    // Restore process.argv
    process.argv = savedArgv;

    if (hooks?.hasHooks?.('before_send')) {
      const hookResult = await hooks.run('before_send', {
        request: effectiveRequest,
        response,
        agent: agentName,
        model: usedModel || effectiveModel,
        provider: effectiveProvider,
        toolResults,
      });
      if (hookResult?.response) {
        response = hookResult.response;
      }
    }

    const responseForUser = privacySettings.redactResponse
      ? redactSensitive(response, privacySettings)
      : response;
    const logResponse = privacySettings.redactLogs
      ? redactSensitive(response, privacySettings)
      : response;

    // Log assistant response
    telem.logAssistantMessage(logResponse);

    if (onMessage) {
      onMessage(responseForUser);
    }

    emitEvent(onEvent, {
      type: 'turn_end',
      response: redactEventText(response),
      toolResults: redactEventValue(toolResults),
    });

    // Auto-push sync events if enabled
    let syncResult = null;
    if (syncEngine && autoSyncPush && allowApply) {
      try {
        const pendingCount = commerce._outbox?.getPendingCount() || 0;
        if (pendingCount > 0) {
          telem.logCustomEvent('sync_push_start', { pendingCount });
          syncResult = await syncEngine.push();
          telem.logCustomEvent('sync_push_complete', {
            pushed: syncResult.pushed,
            rejected: syncResult.rejected,
          });
        }
      } catch (error) {
        telem.logCustomEvent('sync_push_failed', { error: error.message });
      }
    }

    // Shutdown sync engine
    if (syncEngine) {
      await syncEngine.shutdown();
    }

    // -------------------------------------------------------------------------
    // v0.4.0: Save to memory stores
    // -------------------------------------------------------------------------
    if (effectiveEnableMemory && response) {
      try {
        // Extract key facts from the conversation
        const facts = [];
        for (const tr of toolResults) {
          if (tr.toolCall?.name) {
            facts.push(`Used tool: ${tr.toolCall.name}`);
          }
        }

        if (compactionSummary) {
          facts.push('Context compaction applied');
        }

        const summaryRequest = privacySettings.redactMemory
          ? redactSensitive(effectiveRequest, privacySettings)
          : effectiveRequest;
        const summaryResponse = privacySettings.redactMemory
          ? redactSensitive(response, privacySettings)
          : response;

        const memoryEntry = {
          summary: `${summaryRequest.slice(0, 100)}${summaryRequest.length > 100 ? '...' : ''} → ${summaryResponse.slice(0, 150)}${summaryResponse.length > 150 ? '...' : ''}`,
          facts,
          agent: agentName,
          sessionId,
          channel: 'cli',
          senderId: 'local',
        };

        // Save to SQLite memory store
        if (memoryStore) {
          memoryStore.save(memoryEntry);
          telem.logCustomEvent('memory_saved', { store: 'sqlite' });
        }

        // Save to markdown memory store
        if (markdownMemory) {
          await markdownMemory.save(memoryEntry);
          telem.logCustomEvent('memory_saved', { store: 'markdown' });
        }

        if (compactionSummary) {
          const compactionEntry = {
            summary: `[Compaction] ${summaryRequest.slice(0, 80)}...`,
            facts: [`Summary: ${compactionSummary.slice(0, 200)}`],
            agent: agentName,
            sessionId,
            channel: 'cli',
            senderId: 'local',
          };
          if (memoryStore) memoryStore.save(compactionEntry);
          if (markdownMemory) await markdownMemory.save(compactionEntry);
        }
      } catch (e) {
        telem.logCustomEvent('memory_save_failed', { error: e.message });
      }
    }

    // Persist session metadata
    const sessionIdToStore = sessionId || resumeSessionId;
    if (sessionStoreInstance && sessionIdToStore) {
      const storedRequest = privacySettings.redactMemory
        ? redactSensitive(effectiveRequest, privacySettings)
        : effectiveRequest;
      const storedResponse = privacySettings.redactMemory
        ? redactSensitive(response, privacySettings)
        : response;
      try {
        sessionStoreInstance.upsert(sessionIdToStore, {
          provider: effectiveProvider,
          model: usedModel || effectiveModel,
          thinkLevel: effectiveThinkLevel,
          agent: agentName,
          lastRequest: storedRequest,
          lastResponse: storedResponse,
        });
        if (compactionSummary) {
          sessionStoreInstance.appendSummary(sessionIdToStore, compactionSummary);
        }
      } catch (err) {
        console.warn('[Harness] Session store write failed:', err.message);
      }
    }

    if (hooks?.hasHooks?.('agent_end')) {
      await hooks.run('agent_end', {
        request: effectiveRequest,
        response: responseForUser,
        agent: agentName,
        model: usedModel || effectiveModel,
        provider: effectiveProvider,
        toolResults,
        cost: totalCost,
        budgetExceeded,
      });
    }

    treasuryCharge = await recordTreasuryLlmCharge({
      costUsd: totalCost,
      sessionId,
      provider: effectiveProvider,
      model: usedModel || effectiveModel,
      usage: null,
    });

    emitEvent(onEvent, {
      type: 'agent_end',
      response: redactEventText(response),
      toolResults: redactEventValue(toolResults),
      sessionId,
      agent: agentName,
      provider: effectiveProvider,
      model: usedModel || effectiveModel,
      cost: totalCost,
      budgetExceeded,
    });

    // End main span
    telem.endSpanRef(mainSpan, 'ok', { toolCallCount: toolResults.length });

    return {
      response: responseForUser,
      toolResults,
      sessionId,
      agent: agentName,
      routing: routingResult,
      telemetry: telem.getSummary(),
      traceId: telem.traceId,
      treasury: treasuryState
        ? {
            requestId: treasuryState.requestId,
            charge: treasuryCharge,
            identity: treasuryState.erc8004Identity,
          }
        : undefined,
      provider: effectiveProvider,
      cost: totalCost,
      thinkLevel: effectiveThinkLevel,
      budgetExceeded,
      // v0.4.0: New result fields
      usedModel,
      fallbackAttempts: fallbackAttempts.length > 1 ? fallbackAttempts : undefined,
      contextGuard: contextGuardResult
        ? {
            action: contextGuardResult.action,
            usage: contextGuardResult.usage,
          }
        : undefined,
      sync: syncResult
        ? {
            enabled: true,
            pushed: syncResult.pushed,
            rejected: syncResult.rejected,
            receipt: syncResult.receipt,
          }
        : shouldEnableSync
          ? { enabled: true, pushed: 0 }
          : null,
    };
  } catch (error) {
    // Restore process.argv on error
    process.argv = savedArgv;
    // Cleanup sync engine on error
    if (syncEngine) {
      try {
        await syncEngine.shutdown();
      } catch {
        /* ignore */
      }
    }
    emitEvent(onEvent, {
      type: 'agent_end',
      error: error?.message || String(error),
    });
    telem.logError(error, { agent: agentName, request: safeRequestForLogs.slice(0, 100) });
    telem.endSpanRef(mainSpan, 'error', { error: error.message });
    throw new Error(`Agent error: ${error.message}`);
  }
}

/**
 * Create a streaming generator for interactive use
 * @param {Object} options
 * @param {boolean} options.enableSync - Enable VES sync event capture
 */
export async function* runAgentStream({
  request,
  dbPath = './store.db',
  model,
  allowApply = false,
  maxTurns = 10,
  resumeSessionId,
  agent,
  enableSync = null,
  guardrails = null,
  onConfirmRequired = null,
  permissionGate = null,
  conversationHistory = [],
  transformContext = null,
  enableContextGuard = null,
  onContextWarning = null,
  settings = null,
  privacy = null,
  hookRunner = null,
  enablePlugins = null,
  sessionStore = null,
  contextGuardOptions = null,
  provider,
  thinkLevel,
  apiKey = null,
  getApiKey = null,
  abortController = null,
  signal = null,
  onEvent = null,
}) {
  const resolvedSettings = loadAgentSettings(settings || {});
  const privacySettings = { ...resolvedSettings.privacy, ...(privacy || {}) };
  const eventRedact = privacySettings.redactLogs;
  const redactEventText = (text) => (eventRedact ? redactSensitive(text, privacySettings) : text);
  const redactEventValue = (value) => (eventRedact ? redactObject(value, privacySettings) : value);
  const contextSettings = { ...resolvedSettings.contextGuard, ...(contextGuardOptions || {}) };
  const effectiveEnableContextGuard = enableContextGuard ?? contextSettings.enabled;
  const pluginsEnabled = enablePlugins ?? resolvedSettings.plugins?.enabled ?? false;
  const pluginsVerbose = resolvedSettings.plugins?.verbose ?? false;
  const effectiveGuardrails = guardrails
    ? { ...resolvedSettings.guardrails, ...guardrails }
    : { ...resolvedSettings.guardrails };
  let effectiveProvider = provider || resolvedSettings.provider?.default || 'claude';
  let effectiveModel = model || resolvedSettings.model?.default || DEFAULT_MODEL;
  let effectiveThinkLevel = thinkLevel ?? resolvedSettings.thinkLevel?.default ?? 'off';

  const useSessionStore = resolvedSettings.sessionStore?.enabled !== false;
  let sessionStoreInstance = sessionStore || null;
  if (!sessionStoreInstance && useSessionStore) {
    try {
      sessionStoreInstance = getAgentSessionStore({
        dbPath: resolvedSettings.sessionStore?.dbPath || undefined,
        maxSummaries:
          resolvedSettings.sessionStore?.maxSummaries || resolvedSettings.memory?.maxSummaries || 5,
      });
    } catch (err) {
      console.warn('[Harness] Session store unavailable:', err.message);
      sessionStoreInstance = null;
    }
  }
  let sessionMeta = null;
  if (resumeSessionId && sessionStoreInstance && resolvedSettings.model?.preferSession !== false) {
    try {
      sessionMeta = sessionStoreInstance.get(resumeSessionId);
    } catch (err) {
      console.warn('[Harness] Session store read failed:', err.message);
      sessionMeta = null;
    }
  }
  if (sessionMeta) {
    if (!provider && sessionMeta.provider) effectiveProvider = sessionMeta.provider;
    if (!model && sessionMeta.model) effectiveModel = sessionMeta.model;
    if ((thinkLevel === null || thinkLevel === undefined) && sessionMeta.thinkLevel) {
      effectiveThinkLevel = sessionMeta.thinkLevel;
    }
    if (!agent && sessionMeta.agent) agent = sessionMeta.agent;
  }

  if (pluginsEnabled) {
    try {
      await ensureHarnessPluginsLoaded({ verbose: pluginsVerbose });
    } catch (err) {
      console.warn('[Harness] Plugin load failed:', err.message);
    }
  }
  const hooks = hookRunner || getHarnessHookRunner();

  let effectiveRequest = request;
  if (hooks?.hasHooks?.('before_agent_start')) {
    const hookResult = await hooks.run('before_agent_start', {
      request: effectiveRequest,
      agent,
      model: effectiveModel,
      provider: effectiveProvider,
      thinkLevel: effectiveThinkLevel,
      guardrails: effectiveGuardrails,
      allowApply,
      conversationHistory,
    });
    if (hookResult?.request) effectiveRequest = hookResult.request;
    if (hookResult?.agent) agent = hookResult.agent;
    if (hookResult?.model) effectiveModel = hookResult.model;
    if (hookResult?.provider) effectiveProvider = hookResult.provider;
    if (hookResult?.thinkLevel) effectiveThinkLevel = hookResult.thinkLevel;
  }

  const resolvedAbortController = normalizeAbortController({ abortController, signal });
  const effectiveSignal = resolvedAbortController?.signal || signal || null;

  if (effectiveProvider !== 'claude') {
    throw new Error(
      `runAgentStream supports only claude provider (requested: ${effectiveProvider})`,
    );
  }

  const Commerce = getCommerceCtor();
  let commerce = new Commerce(dbPath);

  // Context guard for streaming path (optional)
  const streamSessionSummary = sessionMeta?.summaries?.[0] || null;
  const streamBaseHistory =
    conversationHistory.length > 0
      ? conversationHistory
      : streamSessionSummary
        ? [
            { role: 'user', content: streamSessionSummary },
            {
              role: 'assistant',
              content: 'Understood. I have the context from our earlier conversation.',
            },
          ]
        : [];
  let workingHistory = [...streamBaseHistory];
  let contextGuardResult = null;
  let compactionSummary = null;
  if (typeof transformContext === 'function') {
    try {
      const transformed = await transformContext(workingHistory, effectiveSignal);
      if (Array.isArray(transformed)) {
        workingHistory = transformed;
      }
    } catch (err) {
      console.warn('[Harness] transformContext failed:', err.message);
    }
  }
  if (effectiveEnableContextGuard && workingHistory.length > 0) {
    const contextGuard = ContextGuard.forModel(effectiveModel, {
      warningThreshold: contextSettings.warningThreshold,
      compactThreshold: contextSettings.compactThreshold,
      abortThreshold: contextSettings.abortThreshold,
      reserveTokens: contextSettings.reserveTokens,
    });
    contextGuardResult = contextGuard.check(
      workingHistory,
      '', // System prompt will be added by SDK
      effectiveRequest,
    );

    if (!contextGuardResult.safe && contextGuardResult.action === 'abort') {
      throw new Error(contextGuardResult.message);
    }

    if (contextGuardResult.action === 'compact') {
      let historyForCompaction = workingHistory;
      if (hooks?.hasHooks?.('before_compaction')) {
        const hookResult = await hooks.run('before_compaction', {
          history: historyForCompaction,
          usage: contextGuardResult.usage,
          request: effectiveRequest,
        });
        if (hookResult?.history) {
          historyForCompaction = hookResult.history;
        }
      }

      if (historyForCompaction !== workingHistory) {
        workingHistory = historyForCompaction;
        contextGuardResult = contextGuard.check(workingHistory, '', effectiveRequest);
        if (!contextGuardResult.safe && contextGuardResult.action === 'abort') {
          throw new Error(contextGuardResult.message);
        }
      }

      if (contextGuardResult.action !== 'compact') {
        // Recheck after hook no longer requires compaction.
      } else {
        workingHistory = contextGuardResult.compactedHistory;
        compactionSummary = extractCompactionSummary(contextGuardResult.compactedHistory);
        if (hooks?.hasHooks?.('after_compaction')) {
          await hooks.run('after_compaction', {
            summary: compactionSummary,
            usage: contextGuardResult.usage,
          });
        }
      }
    }

    if (contextGuardResult.action === 'warn' && onContextWarning) {
      onContextWarning(contextGuardResult);
    }
  }

  // Check if sync is configured
  const rawSyncConfig = loadSyncConfig();
  const shouldEnableSync = enableSync !== null ? enableSync : rawSyncConfig !== null;

  if (shouldEnableSync && rawSyncConfig) {
    const syncConfig = new SyncConfig(rawSyncConfig);
    commerce = wrapCommerceWithEvents(commerce, syncConfig);
  }

  const gate =
    permissionGate ||
    createPermissionGate({
      apply: allowApply,
      guardrails: effectiveGuardrails,
      onConfirmRequired,
    });

  const mcpServer = createStatesetMcpServer({
    commerce,
    dbPath,
    allowApply,
    permissionGate: gate,
    hookRunner: hooks,
  });

  // Determine which agent to use
  const routingResult = routeToAgentWithConfidence(effectiveRequest);
  let agentName = agent || routingResult.primary.agent;
  if (!agent && routingResult.primary.level === 'default' && resolvedSettings.agent?.default) {
    agentName = resolvedSettings.agent.default;
  }
  const agentConfig = AGENTS[agentName] || AGENTS['customer-service'];

  const shouldIncludeHistory = workingHistory.length > 0 && !resumeSessionId;
  const requestWithHistory = shouldIncludeHistory
    ? buildPromptWithHistory(effectiveRequest, workingHistory, {
        redactHistory: privacySettings.redactHistory,
        redactOptions: privacySettings,
      })
    : effectiveRequest;

  const streamThinkTokens = THINK_LEVELS[effectiveThinkLevel] || 0;
  let apiKeyOverride = apiKey;
  if (!apiKeyOverride && typeof getApiKey === 'function' && effectiveProvider === 'claude') {
    apiKeyOverride = await getApiKey(effectiveProvider);
  }
  const claudeEnv = buildClaudeEnv({ apiKey: apiKeyOverride });
  const options = {
    model: effectiveModel,
    systemPrompt: agentConfig.systemPrompt,
    mcpServers: {
      'stateset-commerce': mcpServer,
    },
    allowedTools: agentConfig.tools,
    maxTurns,
    // Allow MCP tools to run without SDK permission prompts (guarded by PermissionGate)
    permissionMode: 'bypassPermissions',
    allowDangerouslySkipPermissions: true,
    ...(streamThinkTokens > 0 ? { maxThinkingTokens: streamThinkTokens } : {}),
    ...(claudeEnv ? { env: claudeEnv } : {}),
    ...(resolvedAbortController ? { abortController: resolvedAbortController } : {}),
  };

  const input = resumeSessionId
    ? { sessionId: resumeSessionId, prompt: requestWithHistory }
    : { prompt: requestWithHistory };

  let streamSessionId = resumeSessionId || null;
  let lastResponse = null;
  let assistantStarted = false;
  let assistantText = '';

  try {
    for await (const message of query({ prompt: input, options })) {
      if (message.sessionId && !streamSessionId) {
        streamSessionId = message.sessionId;
      }
      if (message.type === 'assistant') {
        const content = message.message?.content || message.content;
        if (content) {
          if (!assistantStarted) {
            assistantStarted = true;
            emitEvent(onEvent, {
              type: 'message_start',
              message: { role: 'assistant', content: '' },
            });
          }
          for (const block of content) {
            if (block.type === 'tool_use') {
              emitEvent(onEvent, {
                type: 'tool_execution_start',
                toolCallId: block.id,
                toolName: block.name,
                args: redactEventValue(block.input),
              });
            } else if (block.type === 'text') {
              assistantText += block.text;
              emitEvent(onEvent, {
                type: 'message_update',
                message: { role: 'assistant', content: redactEventText(assistantText) },
                delta: redactEventText(block.text),
              });
            }
          }
        }
      } else if (message.type === 'user' && message.tool_use_result) {
        let toolResult = message.tool_use_result;
        if (hooks?.hasHooks?.('tool_result_persist')) {
          const hookResult = await hooks.run('tool_result_persist', {
            tool: toolResult?.name,
            toolCall: {
              id: toolResult?.tool_use_id,
              name: toolResult?.name,
              input: toolResult?.content,
            },
            result: toolResult,
          });
          if (hookResult?.result) {
            toolResult = hookResult.result;
          }
        }
        emitEvent(onEvent, {
          type: 'tool_execution_end',
          toolCallId: toolResult?.tool_use_id,
          toolName: toolResult?.name,
          result: redactEventValue(toolResult),
          isError: Boolean(toolResult?.is_error || toolResult?.isError),
        });
      }
      if (message.type === 'result' && message.result) {
        lastResponse = message.result;
        assistantText = message.result;
        if (hooks?.hasHooks?.('before_send')) {
          const hookResult = await hooks.run('before_send', {
            request: effectiveRequest,
            response: lastResponse,
            agent: agentName,
            model: effectiveModel,
            provider: effectiveProvider,
            toolResults: [],
          });
          if (hookResult?.response) {
            lastResponse = hookResult.response;
          }
        }
        if (!assistantStarted) {
          assistantStarted = true;
          emitEvent(onEvent, {
            type: 'message_start',
            message: { role: 'assistant', content: '' },
          });
        }
        emitEvent(onEvent, {
          type: 'message_end',
          message: { role: 'assistant', content: redactEventText(lastResponse) },
        });
        emitEvent(onEvent, {
          type: 'turn_end',
          response: redactEventText(lastResponse),
          toolResults: [],
        });
      }
      yield message;
    }

    if (sessionStoreInstance && streamSessionId) {
      const storedRequest = privacySettings.redactMemory
        ? redactSensitive(effectiveRequest, privacySettings)
        : effectiveRequest;
      const storedResponse =
        privacySettings.redactMemory && lastResponse
          ? redactSensitive(lastResponse, privacySettings)
          : lastResponse;
      try {
        sessionStoreInstance.upsert(streamSessionId, {
          provider: effectiveProvider,
          model: effectiveModel,
          thinkLevel: effectiveThinkLevel,
          agent: agentName,
          lastRequest: storedRequest,
          lastResponse: storedResponse,
        });
        if (compactionSummary) {
          sessionStoreInstance.appendSummary(streamSessionId, compactionSummary);
        }
      } catch (err) {
        console.warn('[Harness] Session store write failed:', err.message);
      }
    }

    if (hooks?.hasHooks?.('agent_end')) {
      await hooks.run('agent_end', {
        request: effectiveRequest,
        response: lastResponse,
        agent: agentName,
        model: effectiveModel,
        provider: effectiveProvider,
        toolResults: [],
        cost: null,
        budgetExceeded: false,
      });
    }

    emitEvent(onEvent, {
      type: 'agent_end',
      response: lastResponse ? redactEventText(lastResponse) : null,
      toolResults: [],
      sessionId: streamSessionId,
      agent: agentName,
      provider: effectiveProvider,
      model: effectiveModel,
      cost: null,
      budgetExceeded: false,
    });
  } catch (error) {
    emitEvent(onEvent, {
      type: 'agent_end',
      error: error?.message || String(error),
    });
    throw error;
  }
}

/**
 * Create a streaming agent session with queued inputs.
 * Messages are queued and delivered in order once the model finishes a turn.
 *
 * @param {Object} options
 * @returns {{ stream: () => AsyncGenerator, send: (text: string) => void, followUp: (text: string) => void, steer: (text: string) => void, close: () => void, abort: (reason?: any) => void, getSessionId: () => string|null }}
 */
export function createAgentStreamSession(options = {}) {
  const {
    dbPath = './store.db',
    model,
    allowApply = false,
    maxTurns = 10,
    agent,
    enableSync = null,
    guardrails = null,
    onConfirmRequired = null,
    permissionGate = null,
    settings = null,
    privacy = null,
    hookRunner = null,
    enablePlugins = null,
    sessionStore = null,
    contextGuardOptions: _contextGuardOptions = null,
    provider,
    thinkLevel,
    apiKey = null,
    getApiKey = null,
    abortController = null,
    signal = null,
    onEvent = null,
  } = options;

  const resolvedSettings = loadAgentSettings(settings || {});
  const privacySettings = { ...resolvedSettings.privacy, ...(privacy || {}) };
  const eventRedact = privacySettings.redactLogs;
  const redactEventText = (text) => (eventRedact ? redactSensitive(text, privacySettings) : text);
  const redactEventValue = (value) => (eventRedact ? redactObject(value, privacySettings) : value);
  const pluginsEnabled = enablePlugins ?? resolvedSettings.plugins?.enabled ?? false;
  const pluginsVerbose = resolvedSettings.plugins?.verbose ?? false;
  const effectiveGuardrails = guardrails
    ? { ...resolvedSettings.guardrails, ...guardrails }
    : { ...resolvedSettings.guardrails };
  const effectiveProvider = provider || resolvedSettings.provider?.default || 'claude';
  const effectiveModel = model || resolvedSettings.model?.default || DEFAULT_MODEL;
  const effectiveThinkLevel = thinkLevel ?? resolvedSettings.thinkLevel?.default ?? 'off';

  if (pluginsEnabled) {
    ensureHarnessPluginsLoaded({ verbose: pluginsVerbose }).catch((err) => {
      console.warn('[Harness] Plugin load failed:', err.message);
    });
  }

  if (effectiveProvider !== 'claude') {
    throw new Error(
      `createAgentStreamSession supports only claude provider (requested: ${effectiveProvider})`,
    );
  }

  const hooks = hookRunner || getHarnessHookRunner();
  const useSessionStore = resolvedSettings.sessionStore?.enabled !== false;
  let sessionStoreInstance = sessionStore || null;
  if (!sessionStoreInstance && useSessionStore) {
    try {
      sessionStoreInstance = getAgentSessionStore({
        dbPath: resolvedSettings.sessionStore?.dbPath || undefined,
        maxSummaries:
          resolvedSettings.sessionStore?.maxSummaries || resolvedSettings.memory?.maxSummaries || 5,
      });
    } catch (err) {
      console.warn('[Harness] Session store unavailable:', err.message);
      sessionStoreInstance = null;
    }
  }

  const routingResult = routeToAgentWithConfidence('');
  const agentName =
    agent || resolvedSettings.agent?.default || routingResult.primary.agent || 'customer-service';
  const agentConfig = AGENTS[agentName] || AGENTS['customer-service'];

  const gate =
    permissionGate ||
    createPermissionGate({
      apply: allowApply,
      guardrails: effectiveGuardrails,
      onConfirmRequired,
    });

  const Commerce = getCommerceCtor();
  let commerce = new Commerce(dbPath);
  const rawSyncConfig = loadSyncConfig();
  const shouldEnableSync = enableSync !== null ? enableSync : rawSyncConfig !== null;
  if (shouldEnableSync && rawSyncConfig) {
    const syncConfig = new SyncConfig(rawSyncConfig);
    commerce = wrapCommerceWithEvents(commerce, syncConfig);
  }

  const mcpServer = createStatesetMcpServer({
    commerce,
    dbPath,
    allowApply,
    permissionGate: gate,
    hookRunner: hooks,
  });

  const streamThinkTokens = THINK_LEVELS[effectiveThinkLevel] || 0;
  const resolvedAbortController = normalizeAbortController({ abortController, signal });
  const baseOptionsForQuery = {
    model: effectiveModel,
    systemPrompt: agentConfig.systemPrompt,
    mcpServers: {
      'stateset-commerce': mcpServer,
    },
    allowedTools: agentConfig.tools,
    maxTurns,
    permissionMode: 'bypassPermissions',
    allowDangerouslySkipPermissions: true,
    ...(streamThinkTokens > 0 ? { maxThinkingTokens: streamThinkTokens } : {}),
  };

  const queue = [];
  const followUpQueue = [];
  const steerQueue = [];
  let inTurn = false;
  let closed = false;
  let sessionId = null;
  let wakeInput = null;
  let agentStarted = false;
  let assistantStarted = false;
  let assistantText = '';

  const notify = () => {
    if (wakeInput) {
      wakeInput();
      wakeInput = null;
    }
  };

  const enqueue = (text, mode = 'followUp') => {
    if (!text) return;
    if (mode === 'steer') {
      steerQueue.push(text);
    } else if (mode === 'followUp') {
      followUpQueue.push(text);
    } else {
      queue.push(text);
    }
    notify();
  };

  const nextMessage = async () => {
    while (!closed) {
      if (steerQueue.length > 0 && !inTurn) return steerQueue.shift();
      if (!inTurn && followUpQueue.length > 0) return followUpQueue.shift();
      if (!inTurn && queue.length > 0) return queue.shift();
      await new Promise((resolve) => {
        wakeInput = resolve;
      });
    }
    return null;
  };

  async function* inputStream() {
    while (!closed) {
      const next = await nextMessage();
      if (!next) continue;
      inTurn = true;
      assistantStarted = false;
      assistantText = '';
      if (!agentStarted) {
        agentStarted = true;
        emitEvent(onEvent, { type: 'agent_start' });
      }
      emitEvent(onEvent, { type: 'turn_start' });
      const userEventMessage = { role: 'user', content: redactEventText(next) };
      emitEvent(onEvent, { type: 'message_start', message: userEventMessage });
      emitEvent(onEvent, { type: 'message_end', message: userEventMessage });
      yield {
        type: 'user',
        session_id: sessionId || '',
        message: {
          role: 'user',
          content: [{ type: 'text', text: next }],
        },
        parent_tool_use_id: null,
      };
    }
  }

  async function* stream() {
    let lastResponse = null;
    const apiKeyOverride =
      typeof getApiKey === 'function' ? await getApiKey(effectiveProvider) : apiKey;
    const claudeEnv = buildClaudeEnv({ apiKey: apiKeyOverride });
    const optionsForQuery = {
      ...baseOptionsForQuery,
      ...(claudeEnv ? { env: claudeEnv } : {}),
      ...(resolvedAbortController ? { abortController: resolvedAbortController } : {}),
    };

    try {
      for await (const message of query({ prompt: inputStream(), options: optionsForQuery })) {
        if (message.sessionId && !sessionId) {
          sessionId = message.sessionId;
        }
        if (message.type === 'assistant') {
          const content = message.message?.content || message.content;
          if (content) {
            if (!assistantStarted) {
              assistantStarted = true;
              emitEvent(onEvent, {
                type: 'message_start',
                message: { role: 'assistant', content: '' },
              });
            }
            for (const block of content) {
              if (block.type === 'tool_use') {
                emitEvent(onEvent, {
                  type: 'tool_execution_start',
                  toolCallId: block.id,
                  toolName: block.name,
                  args: redactEventValue(block.input),
                });
              } else if (block.type === 'text') {
                assistantText += block.text;
                emitEvent(onEvent, {
                  type: 'message_update',
                  message: { role: 'assistant', content: redactEventText(assistantText) },
                  delta: redactEventText(block.text),
                });
              }
            }
          }
        } else if (message.type === 'user' && message.tool_use_result) {
          let toolResult = message.tool_use_result;
          if (hooks?.hasHooks?.('tool_result_persist')) {
            const hookResult = await hooks.run('tool_result_persist', {
              tool: toolResult?.name,
              toolCall: {
                id: toolResult?.tool_use_id,
                name: toolResult?.name,
                input: toolResult?.content,
              },
              result: toolResult,
            });
            if (hookResult?.result) {
              toolResult = hookResult.result;
            }
          }
          emitEvent(onEvent, {
            type: 'tool_execution_end',
            toolCallId: toolResult?.tool_use_id,
            toolName: toolResult?.name,
            result: redactEventValue(toolResult),
            isError: Boolean(toolResult?.is_error || toolResult?.isError),
          });
        }
        if (message.type === 'result') {
          inTurn = false;
          lastResponse = message.result || lastResponse;
          if (lastResponse && hooks?.hasHooks?.('before_send')) {
            const hookResult = await hooks.run('before_send', {
              request: null,
              response: lastResponse,
              agent: agentName,
              model: effectiveModel,
              provider: effectiveProvider,
              toolResults: [],
            });
            if (hookResult?.response) {
              lastResponse = hookResult.response;
            }
          }
          if (!assistantStarted) {
            assistantStarted = true;
            emitEvent(onEvent, {
              type: 'message_start',
              message: { role: 'assistant', content: '' },
            });
          }
          if (lastResponse !== null && lastResponse !== undefined) {
            emitEvent(onEvent, {
              type: 'message_end',
              message: { role: 'assistant', content: redactEventText(lastResponse) },
            });
            emitEvent(onEvent, {
              type: 'turn_end',
              response: redactEventText(lastResponse),
              toolResults: [],
            });
          }
          notify();
        }
        yield message;
      }

      if (sessionStoreInstance && sessionId) {
        const storedResponse =
          privacySettings.redactMemory && lastResponse
            ? redactSensitive(lastResponse, privacySettings)
            : lastResponse;
        try {
          sessionStoreInstance.upsert(sessionId, {
            provider: effectiveProvider,
            model: effectiveModel,
            thinkLevel: effectiveThinkLevel,
            agent: agentName,
            lastRequest: null,
            lastResponse: storedResponse,
          });
        } catch (err) {
          console.warn('[Harness] Session store write failed:', err.message);
        }
      }

      emitEvent(onEvent, {
        type: 'agent_end',
        response: lastResponse ? redactEventText(lastResponse) : null,
        toolResults: [],
        sessionId,
        agent: agentName,
        provider: effectiveProvider,
        model: effectiveModel,
        cost: null,
        budgetExceeded: false,
      });
    } catch (error) {
      emitEvent(onEvent, {
        type: 'agent_end',
        error: error?.message || String(error),
      });
      throw error;
    }
  }

  return {
    stream,
    send: (text) => enqueue(text, 'followUp'),
    followUp: (text) => enqueue(text, 'followUp'),
    steer: (text) => enqueue(text, 'steer'),
    close: () => {
      closed = true;
      notify();
    },
    abort: (reason) => {
      closed = true;
      if (resolvedAbortController) {
        try {
          resolvedAbortController.abort(reason);
        } catch (err) {
          console.warn('[harness] Abort controller error:', err.message);
        }
      }
      notify();
    },
    getSessionId: () => sessionId,
  };
}

/**
 * Create an agent session for multi-turn conversations
 */
export function createAgentSession({
  dbPath = './store.db',
  model,
  allowApply = false,
  maxTurns = 10,
  agent,
  resumeSessionId = null,
  conversationHistory = [],
  ...rest
}) {
  let sessionId = resumeSessionId;
  let currentAgent = agent;
  let history = Array.isArray(conversationHistory) ? conversationHistory.slice() : [];

  return {
    async query(message, { onToolCall = null, onText = null } = {}) {
      const result = await runAgentLoop({
        request: message,
        dbPath,
        model,
        allowApply,
        maxTurns,
        resumeSessionId: sessionId,
        agent: currentAgent,
        onToolCall,
        onMessage: onText,
        conversationHistory: history,
        ...rest,
      });

      // Update session ID for subsequent queries
      if (result.sessionId) {
        sessionId = result.sessionId;
      }

      // Track which agent was used
      if (result.agent) {
        currentAgent = result.agent;
      }

      if (message) {
        history.push({ role: 'user', content: message });
      }
      if (result.response) {
        history.push({ role: 'assistant', content: result.response });
      }

      return result;
    },

    getSessionId() {
      return sessionId;
    },

    getAgent() {
      return currentAgent;
    },

    setAgent(name) {
      if (AGENTS[name]) {
        currentAgent = name;
      } else {
        throw new Error(`Unknown agent: ${name}. Available: ${Object.keys(AGENTS).join(', ')}`);
      }
    },

    getHistory() {
      return history.slice();
    },

    clearHistory() {
      history = [];
    },
  };
}

/**
 * List available agents
 */
export function listAgents() {
  return Object.entries(AGENTS).map(([id, config]) => ({
    id,
    name: config.name,
    description: config.description,
    toolCount: config.tools.length,
  }));
}

// ============================================================================
// v0.4.0: Queue-Wrapped Agent Execution
// ============================================================================

/**
 * Run agent loop with lane-based serialization.
 * Operations for the same session execute serially to prevent race conditions.
 *
 * @param {Object} options - Same options as runAgentLoop plus:
 * @param {boolean} options.useQueue - Enable queue-based serialization (default: true)
 * @param {string} options.laneId - Custom lane ID (default: sessionId or 'default')
 * @returns {Promise<Object>} - Same result as runAgentLoop
 */
export async function runAgentLoopQueued(options) {
  const { useQueue = true, laneId, ...loopOptions } = options;

  // Determine lane ID - use session ID for serialization
  const effectiveLaneId = laneId || options.resumeSessionId || 'default';

  if (!useQueue) {
    return runAgentLoop(loopOptions);
  }

  // Get the command queue singleton
  const queue = getCommandQueue();

  // Enqueue the operation in the appropriate lane
  return queue.enqueue(
    effectiveLaneId,
    async () => {
      return runAgentLoop(loopOptions);
    },
    {
      request: options.request?.slice(0, 50),
      agent: options.agent,
    },
  );
}

/**
 * Run multiple agent requests in parallel lanes.
 * Each request gets its own lane for concurrent execution.
 *
 * @param {Object[]} requests - Array of runAgentLoop options
 * @returns {Promise<Object[]>} - Array of results
 */
export async function runAgentLoopParallel(requests) {
  const queue = getCommandQueue();

  return Promise.all(
    requests.map((options, index) =>
      queue.enqueueParallel(
        'parallel',
        async () => {
          return runAgentLoop(options);
        },
        { index },
      ),
    ),
  );
}

/**
 * Get queue statistics for monitoring.
 * @returns {Object}
 */
export function getQueueStats() {
  return getCommandQueue().getStats();
}

// ============================================================================
// Re-exports for convenience
// ============================================================================

export { AgentTelemetry, noOpTelemetry } from './telemetry.js';
export {
  PermissionGate,
  createPermissionGate,
  PERMISSION_LEVELS,
  TOOL_PERMISSIONS,
} from './permissions.js';
export { RichOutput, ICONS, createOutput } from './output.js';

// Sync (Verifiable Event Sync)
export { loadSyncConfig, saveSyncConfig, SyncConfig, isSyncConfigured } from './sync/config.js';
export { createOutbox, Outbox } from './sync/outbox.js';
export { createSyncEngine, SyncEngine } from './sync/engine.js';
export { wrapCommerceWithEvents, EventCapture } from './sync/capture.js';
export { createSequencerClient, SequencerClient } from './sync/client.js';

// v0.4.0: New modules for enhanced reliability
export { CommandQueue, getCommandQueue, resetCommandQueue } from './command-queue.js';
export {
  ContextGuard,
  ConversationSummarizer,
  estimateTokens,
  estimateHistoryTokens,
  guardContext,
} from './context-guard.js';
export { ModelFallback, DEFAULT_FALLBACK_CHAIN, createFallbackCaller } from './model-fallback.js';
export { MarkdownMemoryStore, getMarkdownMemoryStore } from './memory/markdown-store.js';
export { MemoryStore, getMemoryStore } from './memory/store.js';
