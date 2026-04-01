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
import path from 'path';
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
import { ensureHarnessPluginsLoaded, getHarnessHookRunner } from './harness-hooks.js';
import { redactSensitive, redactObject } from './privacy.js';
import {
  buildClaudeEnv,
  createInactivityWatchdog,
  emitEvent,
  InactivityWatchdogError,
  isAbortLikeError,
  normalizeAbortController,
} from './harness-utils.js';

// Extracted modules
import {
  buildPromptReport,
  buildPromptWithHistory,
  extractCompactionSummary,
  estimateTokensFromText,
} from './conversation-history.js';
import { isRetryableError, computeRetryDelay, sleep } from './retry-helpers.js';
import { AGENTS } from './agent-definitions.js';
import { routeToAgentWithConfidence } from './agent-router.js';

const require = createRequire(import.meta.url);

function resolvePolicyStorePath(dbPath, override = null) {
  if (override) return override;
  if (process.env.STATESET_POLICY_DIR) return process.env.STATESET_POLICY_DIR;
  const resolvedDbPath = dbPath ? path.resolve(dbPath) : path.resolve('./store.db');
  return path.join(path.dirname(resolvedDbPath), '.stateset');
}

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

// Re-export AGENTS from agent-definitions for consumers that import from claude-harness
export { AGENTS };
export { routeToAgent, routeToAgentWithConfidence } from './agent-router.js';

let _sdkArgvGate = Promise.resolve();
let _claudeQueryImpl = query;

function invokeClaudeQuery(args) {
  return _claudeQueryImpl(args);
}

/**
 * Test-only override for the Claude SDK query iterator.
 */
export function __setClaudeQueryImplForTest(queryImpl = query) {
  _claudeQueryImpl = queryImpl || query;
}

export function __resetClaudeQueryImplForTest() {
  _claudeQueryImpl = query;
}

function __withSerializedArgvLock() {
  const previous = _sdkArgvGate;
  let release;
  const nextGate = new Promise((resolve) => {
    release = resolve;
  });
  _sdkArgvGate = nextGate;

  return {
    previous,
    release,
  };
}

async function __runWithCleanArgv(fn) {
  const { previous, release } = __withSerializedArgvLock();
  await previous;
  const previousArgv = process.argv;

  try {
    process.argv = previousArgv.slice(0, 2);
    return await fn();
  } finally {
    process.argv = previousArgv;
    release();
  }
}

async function* __runQueryWithCleanArgv(generatorFactory) {
  const { previous, release } = __withSerializedArgvLock();
  await previous;
  const previousArgv = process.argv;

  try {
    process.argv = previousArgv.slice(0, 2);
    for await (const message of generatorFactory()) {
      yield message;
    }
  } finally {
    process.argv = previousArgv;
    release();
  }
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
 * @param {string} options.slaLevel - Optional routing SLA: standard|expedited|critical
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
 * @param {PolicyEngine} options.policyEngine - Custom policy engine for MCP tools
 * @param {string} options.policyStorePath - Path to policy store directory
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
  slaLevel = null,
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
  policyEngine = null,
  policyStorePath = null,
  onEvent = null,
  treasury = null,
}) {
  const effectivePolicyStorePath = resolvePolicyStorePath(dbPath, policyStorePath);
  const resolvedSettings = loadAgentSettings(settings || {});
  const retrySettings = { ...resolvedSettings.retry, ...(retry || {}) };
  const watchdogSettings = { ...resolvedSettings.watchdog };
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
  let effectiveSlaLevel = slaLevel ?? resolvedSettings.agent?.slaLevel ?? null;

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
    if ((effectiveSlaLevel === null || effectiveSlaLevel === undefined) && sessionMeta.slaLevel) {
      effectiveSlaLevel = sessionMeta.slaLevel;
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
      slaLevel: effectiveSlaLevel,
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
    if (hookResult?.slaLevel !== undefined) effectiveSlaLevel = hookResult.slaLevel;
    if (hookResult?.systemPrompt) {
      systemPromptOverride = hookResult.systemPrompt;
    } else if (hookResult?.systemPromptAppend && AGENTS[agent]?.systemPrompt) {
      systemPromptOverride = `${AGENTS[agent].systemPrompt}\n\n${hookResult.systemPromptAppend}`;
    }
  }

  const configuredWatchdogTimeoutMs = resumeSessionId
    ? Number(watchdogSettings.resumeInactivityMs)
    : Number(watchdogSettings.freshInactivityMs);
  const watchdogTimeoutMs =
    effectiveProvider === 'claude' &&
    watchdogSettings.enabled !== false &&
    Number.isFinite(configuredWatchdogTimeoutMs) &&
    configuredWatchdogTimeoutMs > 0
      ? configuredWatchdogTimeoutMs
      : null;
  const resolvedAbortController = normalizeAbortController({ abortController, signal });
  const effectiveAbortController =
    resolvedAbortController || (watchdogTimeoutMs ? new AbortController() : null);
  const effectiveSignal = effectiveAbortController?.signal || signal || null;

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
  emitEvent(onEvent, { type: 'agent_start', slaLevel: effectiveSlaLevel });
  emitEvent(onEvent, { type: 'turn_start' });
  const userEventMessage = { role: 'user', content: redactEventText(effectiveRequest) };
  emitEvent(onEvent, { type: 'message_start', message: userEventMessage });
  emitEvent(onEvent, { type: 'message_end', message: userEventMessage });

  // -------------------------------------------------------------------------
  // v0.4.0: Context Guard - Check context window before proceeding
  // -------------------------------------------------------------------------
  const sessionSummary = sessionMeta?.summaries?.[0] || null;
  const historySource =
    conversationHistory.length > 0
      ? 'conversation_history'
      : sessionSummary
        ? 'session_summary'
        : 'none';
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
  let rawSyncConfig = null;
  try {
    rawSyncConfig = loadSyncConfig();
  } catch (syncErr) {
    console.debug('sync config not available (standalone mode):', syncErr.message);
  }
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
    policyEngine,
    policyStorePath: effectivePolicyStorePath,
    treasury: treasuryConfig,
  });

  const mcpServers = {
    'stateset-commerce': mcpServer,
  };

  // Determine which agent to use
  const routingResult = routeToAgentWithConfidence(effectiveRequest, {
    slaLevel: effectiveSlaLevel,
  });
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
    const x402Server = createX402McpServer({
      env: process.env,
      configDir,
      policyEngine,
      policyStorePath: effectivePolicyStorePath,
    });
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
  const promptReport = buildPromptReport({
    request: effectiveRequest,
    history: workingHistory,
    systemPrompt,
    includeHistory: shouldIncludeHistory,
    resumeSession: Boolean(resumeSessionId),
    historySource,
    compactionSummary,
    contextGuardResult,
    redactOptions: privacySettings,
    redactHistory: privacySettings.redactHistory,
  });
  emitEvent(onEvent, { type: 'prompt_report', report: promptReport });
  telem.logCustomEvent('prompt_report', promptReport);
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
    ...(effectiveAbortController ? { abortController: effectiveAbortController } : {}),
  };

  // Track results
  const toolResults = [];
  let sessionId = resumeSessionId;
  let response = '';
  const runStartedAt = Date.now();

  const emptyUsageCounters = () => ({
    inputTokens: null,
    outputTokens: null,
    totalTokens: null,
    cacheReadTokens: null,
    cacheWriteTokens: null,
  });

  const readUsageCounter = (source, keys) => {
    if (!source || typeof source !== 'object') return null;
    for (const key of keys) {
      const value = source[key];
      if (value === null || value === undefined || value === '') continue;
      const numeric = Number(value);
      if (Number.isFinite(numeric)) {
        return Math.trunc(numeric);
      }
    }
    return null;
  };

  const readAnyUsageCounter = (sources, keys) => {
    for (const source of sources) {
      const value = readUsageCounter(source, keys);
      if (value !== null) return value;
    }
    return null;
  };

  const mergeUsageCounters = (currentUsage, message) => {
    const nextUsage = currentUsage ? { ...currentUsage } : emptyUsageCounters();
    const direct = message && typeof message === 'object' ? message : null;
    const usageSources = [direct, direct?.usage, direct?.result_usage, direct?.resultUsage];

    const inputTokens = readAnyUsageCounter(usageSources, ['input_tokens', 'inputTokens']);
    const outputTokens = readAnyUsageCounter(usageSources, ['output_tokens', 'outputTokens']);
    const totalTokens = readAnyUsageCounter(usageSources, ['total_tokens', 'totalTokens']);
    const cacheReadTokens = readAnyUsageCounter(usageSources, [
      'cache_read_tokens',
      'cacheReadTokens',
      'cache_read_input_tokens',
      'cacheReadInputTokens',
    ]);
    const cacheWriteTokens = readAnyUsageCounter(usageSources, [
      'cache_write_tokens',
      'cacheWriteTokens',
      'cache_creation_input_tokens',
      'cacheCreationInputTokens',
    ]);

    if (inputTokens !== null) nextUsage.inputTokens = inputTokens;
    if (outputTokens !== null) nextUsage.outputTokens = outputTokens;
    if (totalTokens !== null) nextUsage.totalTokens = totalTokens;
    if (cacheReadTokens !== null) nextUsage.cacheReadTokens = cacheReadTokens;
    if (cacheWriteTokens !== null) nextUsage.cacheWriteTokens = cacheWriteTokens;
    if (
      nextUsage.totalTokens === null &&
      nextUsage.inputTokens !== null &&
      nextUsage.outputTokens !== null
    ) {
      nextUsage.totalTokens = nextUsage.inputTokens + nextUsage.outputTokens;
    }
    return nextUsage;
  };

  let latestUsage = emptyUsageCounters();

  const persistSessionRun = ({
    responseText = response,
    error = null,
    modelUsed = effectiveModel,
    sessionIdOverride = null,
    lastCostUsd = null,
    usage = latestUsage,
    appendCompactionSummary = !error,
  } = {}) => {
    const sessionIdToStore = sessionIdOverride || sessionId || resumeSessionId;
    if (!sessionStoreInstance || !sessionIdToStore) return;

    const rawResponse =
      typeof responseText === 'string' ? responseText : String(responseText || '');
    const storedRequest = privacySettings.redactMemory
      ? redactSensitive(effectiveRequest, privacySettings)
      : effectiveRequest;
    const storedResponse = privacySettings.redactMemory
      ? redactSensitive(rawResponse, privacySettings)
      : rawResponse;
    const usageCounters = usage || emptyUsageCounters();
    const totalTokens =
      usageCounters.totalTokens ??
      (usageCounters.inputTokens !== null && usageCounters.outputTokens !== null
        ? usageCounters.inputTokens + usageCounters.outputTokens
        : null);
    const normalizedLastCostUsd =
      lastCostUsd === null || lastCostUsd === undefined || lastCostUsd === ''
        ? null
        : Number(lastCostUsd);
    const payload = {
      provider: effectiveProvider,
      model: modelUsed || effectiveModel,
      thinkLevel: effectiveThinkLevel,
      slaLevel: effectiveSlaLevel,
      agent: agentName,
      lastRequest: storedRequest,
      lastResponse: storedResponse,
      lastError: error ? error?.message || String(error) : null,
      lastErrorCode: error?.code || null,
      lastErrorAt: error ? Date.now() : null,
      abortedLastRun: error
        ? error instanceof InactivityWatchdogError || isAbortLikeError(error)
        : false,
      lastRunMs: Date.now() - runStartedAt,
      lastCostUsd: Number.isFinite(normalizedLastCostUsd) ? normalizedLastCostUsd : null,
      inputTokens: usageCounters.inputTokens,
      outputTokens: usageCounters.outputTokens,
      totalTokens,
      cacheReadTokens: usageCounters.cacheReadTokens,
      cacheWriteTokens: usageCounters.cacheWriteTokens,
      compactionCount: compactionSummary ? 1 : 0,
      promptReport,
    };

    try {
      if (typeof sessionStoreInstance.recordRun === 'function') {
        sessionStoreInstance.recordRun(sessionIdToStore, payload);
      } else if (typeof sessionStoreInstance.upsert === 'function') {
        sessionStoreInstance.upsert(sessionIdToStore, payload);
      }
      if (
        appendCompactionSummary &&
        compactionSummary &&
        typeof sessionStoreInstance.appendSummary === 'function'
      ) {
        sessionStoreInstance.appendSummary(sessionIdToStore, compactionSummary);
      }
    } catch (err) {
      console.warn('[Harness] Session store write failed:', err.message);
    }
  };

  let budgetExceeded = false;
  let totalCost = null;
  let usedModel = effectiveModel;
  let fallbackAttempts = [];

  try {
    // If resuming, add session ID to options
    if (resumeSessionId) {
      options.resume = resumeSessionId;
    }

    // v0.2.8: Non-Claude provider path
    if (effectiveProvider !== 'claude') {
      return __runWithCleanArgv(async () => {
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
            slaLevel: effectiveSlaLevel,
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
          slaLevel: effectiveSlaLevel,
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
          slaLevel: effectiveSlaLevel,
          routing: routingResult,
          provider: effectiveProvider,
          cost: providerResult.cost || null,
          thinkLevel: effectiveThinkLevel,
          budgetExceeded,
          promptReport,
          treasury: treasuryState
            ? {
                requestId: treasuryState.requestId,
                charge: treasuryCharge,
                identity: treasuryState.erc8004Identity,
              }
            : undefined,
        };
      });
    }

    // -------------------------------------------------------------------------
    // v0.4.0: Run query with optional model fallback
    // -------------------------------------------------------------------------

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
        usage: emptyUsageCounters(),
      };
      const pendingToolCalls = new Map();
      let assistantStarted = false;
      let assistantText = '';
      response = '';
      latestUsage = emptyUsageCounters();
      if (!resumeSessionId) {
        sessionId = null;
      }
      const watchdog = watchdogTimeoutMs
        ? createInactivityWatchdog({
            timeoutMs: watchdogTimeoutMs,
            abortController: effectiveAbortController,
            message: resumeSessionId
              ? `No Claude SDK activity while resuming session after ${watchdogTimeoutMs}ms`
              : `No Claude SDK activity received after ${watchdogTimeoutMs}ms`,
            onTimeout: (watchdogError) => {
              const currentSessionId = results.sessionId || sessionId || resumeSessionId || null;
              telem.logCustomEvent('watchdog_timeout', {
                timeoutMs: watchdogTimeoutMs,
                elapsedMs: watchdogError.elapsedMs,
                provider: effectiveProvider,
                model: queryModel,
                sessionId: currentSessionId,
              });
              emitEvent(onEvent, {
                type: 'watchdog_timeout',
                timeoutMs: watchdogTimeoutMs,
                elapsedMs: watchdogError.elapsedMs,
                provider: effectiveProvider,
                model: queryModel,
                sessionId: currentSessionId,
              });
            },
          })
        : null;

      try {
        for await (const message of __runQueryWithCleanArgv(() =>
          invokeClaudeQuery({ prompt: requestWithHistory, options: queryOptions }),
        )) {
          watchdog?.touch();
          results.usage = mergeUsageCounters(results.usage, message);
          latestUsage = results.usage;

          if (message.sessionId && !results.sessionId) {
            results.sessionId = message.sessionId;
          }
          if (message.sessionId) {
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
                  response = results.response;
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
            results.usage = mergeUsageCounters(results.usage, message);
            latestUsage = results.usage;
            if (message.result) {
              results.response = message.result;
              assistantText = message.result;
              response = message.result;
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
      } catch (error) {
        if (watchdog?.timedOut && isAbortLikeError(error)) {
          throw watchdog.error || error;
        }
        throw error;
      } finally {
        watchdog?.stop();
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
        const nonRetryable =
          (errorType && errorType.startsWith('error_max')) ||
          err?.code === 'WATCHDOG_TIMEOUT' ||
          err instanceof InactivityWatchdogError;
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
      usage: queryUsage,
    } = queryResult;
    toolResults.push(...queryToolResults);
    response = queryResponse;
    if (querySessionId) sessionId = querySessionId;
    budgetExceeded = queryBudgetExceeded;
    totalCost = queryTotalCost;
    latestUsage = queryUsage || latestUsage;

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

    if (hooks?.hasHooks?.('agent_end')) {
      await hooks.run('agent_end', {
        request: effectiveRequest,
        response: responseForUser,
        agent: agentName,
        slaLevel: effectiveSlaLevel,
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
      usage: latestUsage,
    });

    persistSessionRun({
      responseText: response,
      modelUsed: usedModel || effectiveModel,
      lastCostUsd: totalCost,
      usage: latestUsage,
    });

    emitEvent(onEvent, {
      type: 'agent_end',
      response: redactEventText(response),
      toolResults: redactEventValue(toolResults),
      sessionId,
      agent: agentName,
      slaLevel: effectiveSlaLevel,
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
      slaLevel: effectiveSlaLevel,
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
      promptReport,
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
    // Cleanup sync engine on error
    if (syncEngine) {
      try {
        await syncEngine.shutdown();
      } catch (err) {
        console.debug('[harness] Sync engine shutdown failed:', err.message || err);
      }
    }
    persistSessionRun({
      responseText: response,
      error,
      modelUsed: usedModel || effectiveModel,
      lastCostUsd: totalCost,
      usage: latestUsage,
      appendCompactionSummary: false,
    });
    const errorMessage = error?.message || String(error);
    emitEvent(onEvent, {
      type: 'agent_end',
      error: errorMessage,
    });
    telem.logError(error, { agent: agentName, request: safeRequestForLogs.slice(0, 100) });
    telem.endSpanRef(mainSpan, 'error', { error: errorMessage });
    const wrappedError = new Error(`Agent error: ${errorMessage}`);
    if (error?.code) wrappedError.code = error.code;
    if (error !== undefined) wrappedError.cause = error;
    throw wrappedError;
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
  slaLevel = null,
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
  policyEngine = null,
  policyStorePath = null,
  onEvent = null,
}) {
  const effectivePolicyStorePath = resolvePolicyStorePath(dbPath, policyStorePath);
  const resolvedSettings = loadAgentSettings(settings || {});
  const watchdogSettings = { ...resolvedSettings.watchdog };
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
  let effectiveSlaLevel = slaLevel ?? resolvedSettings.agent?.slaLevel ?? null;

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
    if ((effectiveSlaLevel === null || effectiveSlaLevel === undefined) && sessionMeta.slaLevel) {
      effectiveSlaLevel = sessionMeta.slaLevel;
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
      slaLevel: effectiveSlaLevel,
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
    if (hookResult?.slaLevel !== undefined) effectiveSlaLevel = hookResult.slaLevel;
  }

  const configuredWatchdogTimeoutMs = resumeSessionId
    ? Number(watchdogSettings.resumeInactivityMs)
    : Number(watchdogSettings.freshInactivityMs);
  const watchdogTimeoutMs =
    effectiveProvider === 'claude' &&
    watchdogSettings.enabled !== false &&
    Number.isFinite(configuredWatchdogTimeoutMs) &&
    configuredWatchdogTimeoutMs > 0
      ? configuredWatchdogTimeoutMs
      : null;
  const resolvedAbortController = normalizeAbortController({ abortController, signal });
  const effectiveAbortController =
    resolvedAbortController || (watchdogTimeoutMs ? new AbortController() : null);
  const effectiveSignal = effectiveAbortController?.signal || signal || null;

  if (effectiveProvider !== 'claude') {
    throw new Error(
      `runAgentStream supports only claude provider (requested: ${effectiveProvider})`,
    );
  }

  const Commerce = getCommerceCtor();
  let commerce = new Commerce(dbPath);

  // Context guard for streaming path (optional)
  const streamSessionSummary = sessionMeta?.summaries?.[0] || null;
  const streamHistorySource =
    conversationHistory.length > 0
      ? 'conversation_history'
      : streamSessionSummary
        ? 'session_summary'
        : 'none';
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
  let rawSyncConfig2 = null;
  try {
    rawSyncConfig2 = loadSyncConfig();
  } catch (syncErr) {
    console.debug('sync config not available (standalone mode):', syncErr.message);
  }
  const shouldEnableSync = enableSync !== null ? enableSync : rawSyncConfig2 !== null;

  if (shouldEnableSync && rawSyncConfig2) {
    const syncConfig = new SyncConfig(rawSyncConfig2);
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
    policyEngine,
    policyStorePath: effectivePolicyStorePath,
  });

  // Determine which agent to use
  const routingResult = routeToAgentWithConfidence(effectiveRequest, {
    slaLevel: effectiveSlaLevel,
  });
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
  const promptReport = buildPromptReport({
    request: effectiveRequest,
    history: workingHistory,
    systemPrompt: agentConfig.systemPrompt,
    includeHistory: shouldIncludeHistory,
    resumeSession: Boolean(resumeSessionId),
    historySource: streamHistorySource,
    compactionSummary,
    contextGuardResult,
    redactOptions: privacySettings,
    redactHistory: privacySettings.redactHistory,
  });
  emitEvent(onEvent, { type: 'prompt_report', report: promptReport });

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
    ...(effectiveAbortController ? { abortController: effectiveAbortController } : {}),
  };

  const input = resumeSessionId
    ? { sessionId: resumeSessionId, prompt: requestWithHistory }
    : { prompt: requestWithHistory };

  let streamSessionId = resumeSessionId || null;
  let lastResponse = null;
  let assistantStarted = false;
  let assistantText = '';
  const runStartedAt = Date.now();

  const persistStreamSession = ({
    responseText = lastResponse,
    error = null,
    appendCompactionSummary = !error,
  } = {}) => {
    const sessionIdToStore = streamSessionId || resumeSessionId;
    if (!sessionStoreInstance || !sessionIdToStore) return;

    const storedRequest = privacySettings.redactMemory
      ? redactSensitive(effectiveRequest, privacySettings)
      : effectiveRequest;
    const rawResponse =
      responseText === null || responseText === undefined
        ? null
        : typeof responseText === 'string'
          ? responseText
          : String(responseText);
    const storedResponse =
      privacySettings.redactMemory && rawResponse
        ? redactSensitive(rawResponse, privacySettings)
        : rawResponse;

    try {
      sessionStoreInstance.upsert(sessionIdToStore, {
        provider: effectiveProvider,
        model: effectiveModel,
        thinkLevel: effectiveThinkLevel,
        slaLevel: effectiveSlaLevel,
        agent: agentName,
        lastRequest: storedRequest,
        lastResponse: storedResponse,
        lastError: error ? error?.message || String(error) : null,
        lastErrorCode: error?.code || null,
        lastErrorAt: error ? Date.now() : null,
        abortedLastRun: error
          ? error instanceof InactivityWatchdogError || isAbortLikeError(error)
          : false,
        lastRunMs: Date.now() - runStartedAt,
        promptReport,
      });
      if (
        appendCompactionSummary &&
        compactionSummary &&
        typeof sessionStoreInstance.appendSummary === 'function'
      ) {
        sessionStoreInstance.appendSummary(sessionIdToStore, compactionSummary);
      }
    } catch (err) {
      console.warn('[Harness] Session store write failed:', err.message);
    }
  };

  try {
    const watchdog = watchdogTimeoutMs
      ? createInactivityWatchdog({
          timeoutMs: watchdogTimeoutMs,
          abortController: effectiveAbortController,
          message: resumeSessionId
            ? `No Claude SDK activity while resuming session after ${watchdogTimeoutMs}ms`
            : `No Claude SDK activity received after ${watchdogTimeoutMs}ms`,
          onTimeout: (watchdogError) => {
            emitEvent(onEvent, {
              type: 'watchdog_timeout',
              timeoutMs: watchdogTimeoutMs,
              elapsedMs: watchdogError.elapsedMs,
              provider: effectiveProvider,
              model: effectiveModel,
              sessionId: streamSessionId || resumeSessionId || null,
            });
          },
        })
      : null;

    try {
      for await (const message of __runQueryWithCleanArgv(() =>
        invokeClaudeQuery({ prompt: input, options }),
      )) {
        watchdog?.touch();

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
    } catch (error) {
      if (watchdog?.timedOut && isAbortLikeError(error)) {
        throw watchdog.error || error;
      }
      throw error;
    } finally {
      watchdog?.stop();
    }

    persistStreamSession({ responseText: lastResponse });

    if (hooks?.hasHooks?.('agent_end')) {
      await hooks.run('agent_end', {
        request: effectiveRequest,
        response: lastResponse,
        agent: agentName,
        slaLevel: effectiveSlaLevel,
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
      slaLevel: effectiveSlaLevel,
      provider: effectiveProvider,
      model: effectiveModel,
      cost: null,
      budgetExceeded: false,
    });
  } catch (error) {
    persistStreamSession({
      responseText: lastResponse,
      error,
      appendCompactionSummary: false,
    });
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
 * @returns {{ stream: () => AsyncGenerator, send: (text: string) => void, followUp: (text: string) => void, steer: (text: string) => void, close: () => void, abort: (reason?: any) => void, getSessionId: () => string|null, getLastPromptReport: () => object|null, getLastTurnResult: () => object|null }}
 */
export function createAgentStreamSession(options = {}) {
  const {
    dbPath = './store.db',
    model,
    allowApply = false,
    maxTurns = 10,
    agent,
    slaLevel = null,
    enableSync = null,
    guardrails = null,
    onConfirmRequired = null,
    permissionGate = null,
    settings = null,
    privacy = null,
    hookRunner = null,
    enablePlugins = null,
    sessionStore = null,
    conversationHistory: initialConversationHistory = [],
    sessionRefresh = null,
    contextGuardOptions: _contextGuardOptions = null,
    provider,
    thinkLevel,
    maxBudgetUsd = null,
    enableX402 = false,
    enableMemory = null,
    useMarkdownMemory = null,
    memoryStore: memoryStoreOverride = null,
    markdownMemoryStore: markdownMemoryStoreOverride = null,
    treasury = null,
    treasuryRuntime = null,
    apiKey = null,
    getApiKey = null,
    abortController = null,
    signal = null,
    policyEngine = null,
    policyStorePath = null,
    onEvent = null,
  } = options;

  const effectivePolicyStorePath = resolvePolicyStorePath(dbPath, policyStorePath);
  const resolvedSettings = loadAgentSettings(settings || {});
  const watchdogSettings = { ...resolvedSettings.watchdog };
  const memorySettings = { ...resolvedSettings.memory };
  const privacySettings = { ...resolvedSettings.privacy, ...(privacy || {}) };
  const eventRedact = privacySettings.redactLogs;
  const redactEventText = (text) => (eventRedact ? redactSensitive(text, privacySettings) : text);
  const redactEventValue = (value) => (eventRedact ? redactObject(value, privacySettings) : value);
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
  const effectiveProvider = provider || resolvedSettings.provider?.default || 'claude';
  const effectiveModel = model || resolvedSettings.model?.default || DEFAULT_MODEL;
  const effectiveThinkLevel = thinkLevel ?? resolvedSettings.thinkLevel?.default ?? 'off';
  const effectiveSlaLevel = slaLevel ?? resolvedSettings.agent?.slaLevel ?? null;
  const effectiveEnableMemory = enableMemory ?? memorySettings.enabled;
  const effectiveUseMarkdownMemory = useMarkdownMemory ?? memorySettings.useMarkdown;
  const configuredWatchdogTimeoutMs = Number(watchdogSettings.freshInactivityMs);
  const watchdogTimeoutMs =
    effectiveProvider === 'claude' &&
    watchdogSettings.enabled !== false &&
    Number.isFinite(configuredWatchdogTimeoutMs) &&
    configuredWatchdogTimeoutMs > 0
      ? configuredWatchdogTimeoutMs
      : null;
  const resolvedAbortController = normalizeAbortController({ abortController, signal });
  const effectiveAbortController =
    resolvedAbortController || (watchdogTimeoutMs ? new AbortController() : null);

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

  let memoryStoreInstance = memoryStoreOverride || null;
  let markdownMemoryStoreInstance = markdownMemoryStoreOverride || null;
  if (effectiveEnableMemory) {
    try {
      if (!memoryStoreInstance) {
        memoryStoreInstance = getMemoryStore();
      }
      if (effectiveUseMarkdownMemory && !markdownMemoryStoreInstance) {
        markdownMemoryStoreInstance = getMarkdownMemoryStore();
      }
    } catch (err) {
      console.warn('[Harness] Memory store unavailable:', err.message);
      memoryStoreInstance = null;
      markdownMemoryStoreInstance = null;
    }
  }

  const routingResult = routeToAgentWithConfidence('', {
    slaLevel: effectiveSlaLevel,
  });
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
  let rawSyncConfig3 = null;
  try {
    rawSyncConfig3 = loadSyncConfig();
  } catch (syncErr) {
    console.debug('sync config not available (standalone mode):', syncErr.message);
  }
  const shouldEnableSync = enableSync !== null ? enableSync : rawSyncConfig3 !== null;
  if (shouldEnableSync && rawSyncConfig3) {
    const syncConfig = new SyncConfig(rawSyncConfig3);
    commerce = wrapCommerceWithEvents(commerce, syncConfig);
  }

  const mcpServer = createStatesetMcpServer({
    commerce,
    dbPath,
    allowApply,
    permissionGate: gate,
    hookRunner: hooks,
    policyEngine,
    policyStorePath: effectivePolicyStorePath,
    treasury: treasuryConfig,
  });

  const mcpServers = {
    'stateset-commerce': mcpServer,
  };
  const allowedTools = [...agentConfig.tools];
  const shouldEnableX402 = Boolean(
    enableX402 || process.env.X402_ENABLE === '1' || process.env.X402_SEQUENCER_URL,
  );

  if (shouldEnableX402) {
    const configDir = process.env.STATESET_CONFIG_DIR || '.stateset';
    const x402Server = createX402McpServer({
      env: process.env,
      configDir,
      policyEngine,
      policyStorePath: effectivePolicyStorePath,
    });
    mcpServers['stateset-x402'] = x402Server;
    allowedTools.push(...X402_MCP_TOOL_NAMES.map((name) => `mcp__stateset-x402__${name}`));
  }

  const parsedMaxBudgetUsd = Number(maxBudgetUsd);
  const configuredMaxBudgetUsd =
    Number.isFinite(parsedMaxBudgetUsd) && parsedMaxBudgetUsd > 0 ? parsedMaxBudgetUsd : null;
  let treasuryState = null;
  let effectiveStreamMaxBudgetUsd = configuredMaxBudgetUsd;
  const streamThinkTokens = THINK_LEVELS[effectiveThinkLevel] || 0;

  const resolveTreasuryRuntime = async () => {
    if (treasuryRuntime) {
      return treasuryRuntime;
    }
    const treasuryModule = await import('./treasury/index.js');
    const chainsModule = await import('./chains/config.js');
    const erc8004Module = await import('./erc8004/index.js');
    return {
      loadTreasuryContext: treasuryModule.loadTreasuryContext,
      resolveToken: treasuryModule.resolveToken,
      recordFee: treasuryModule.recordFee,
      fromSmallestUnit: chainsModule.fromSmallestUnit,
      getIdentity: erc8004Module.getIdentity,
    };
  };

  const initializeTreasuryState = async () => {
    if (!treasuryConfig?.enabled || treasuryState) return;

    try {
      const runtime = await resolveTreasuryRuntime();
      const ctx = await runtime.loadTreasuryContext({
        dbPath: treasuryConfig.dbPath || undefined,
      });
      const chainId = treasuryConfig.chainId || 'set_chain';
      const tokenSymbol = treasuryConfig.tokenSymbol || 'USDC';
      let agentId = treasuryConfig.agentId || 'default';
      let erc8004Identity = null;
      const erc8004Registry = treasuryConfig.erc8004Registry || null;
      if (erc8004Registry) {
        const identityDbPath = treasuryConfig.erc8004DbPath || dbPath;
        erc8004Identity = runtime.getIdentity(identityDbPath, erc8004Registry, agentId);
        if (!erc8004Identity) {
          throw new Error(`ERC-8004 identity not found for ${erc8004Registry}:${agentId}`);
        }
        agentId = erc8004Identity.agent_id;
      }
      const token = runtime.resolveToken(chainId, tokenSymbol, ctx.registry);
      if (!token) {
        throw new Error(`Unknown treasury token ${tokenSymbol} on ${chainId}.`);
      }
      const balance = ctx.store.getBalance({
        agentId,
        chainId,
        tokenSymbol: token.symbol,
        tokenDecimals: token.decimals,
      });
      const balanceDisplay = runtime.fromSmallestUnit(balance.balanceSmallest, token.decimals);
      const balanceUsd = Number.parseFloat(balanceDisplay);
      if (!Number.isFinite(balanceUsd) || balanceUsd <= 0) {
        throw new Error(`Treasury balance is empty for ${token.symbol} on ${chainId}.`);
      }
      const resolvedBudget = configuredMaxBudgetUsd
        ? Math.min(configuredMaxBudgetUsd, balanceUsd)
        : balanceUsd;
      if (!Number.isFinite(resolvedBudget) || resolvedBudget <= 0) {
        throw new Error(`Treasury budget unavailable for ${token.symbol} on ${chainId}.`);
      }
      effectiveStreamMaxBudgetUsd = resolvedBudget;
      treasuryState = {
        enabled: true,
        chargeLlm: treasuryConfig.chargeLlm !== false,
        ctx,
        agentId,
        chainId,
        token,
        balanceUsd,
        erc8004Registry,
        erc8004Identity,
        runtime,
      };
    } catch (error) {
      throw new Error(`Treasury billing failed: ${error.message}`);
    }
  };

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
      const requestId = randomUUID();
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
      const entry = await treasuryState.runtime.recordFee(
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
          taskId: requestId,
          sessionId: chargeSessionId || null,
          toolName: 'llm_inference',
          requestId,
        },
        treasuryState.ctx,
      );
      return {
        requestId,
        charge: {
          eventId: entry.event_id,
          amount: entry.amount_display,
          amountSmallest: entry.amount_smallest,
          token: entry.token_symbol,
          chainId: entry.chain_id,
        },
        identity: treasuryState.erc8004Identity,
      };
    } catch (err) {
      console.warn('[Harness] Treasury charge failed:', err.message);
      return null;
    }
  };

  const emptyUsageCounters = () => ({
    inputTokens: null,
    outputTokens: null,
    totalTokens: null,
    cacheReadTokens: null,
    cacheWriteTokens: null,
  });

  const readUsageCounter = (source, keys) => {
    if (!source || typeof source !== 'object') return null;
    for (const key of keys) {
      const value = source[key];
      if (value === null || value === undefined || value === '') continue;
      const numeric = Number(value);
      if (Number.isFinite(numeric)) {
        return Math.trunc(numeric);
      }
    }
    return null;
  };

  const readAnyUsageCounter = (sources, keys) => {
    for (const source of sources) {
      const value = readUsageCounter(source, keys);
      if (value !== null) return value;
    }
    return null;
  };

  const mergeUsageCounters = (currentUsage, message) => {
    const nextUsage = currentUsage ? { ...currentUsage } : emptyUsageCounters();
    const direct = message && typeof message === 'object' ? message : null;
    const usageSources = [direct, direct?.usage, direct?.result_usage, direct?.resultUsage];

    const inputTokens = readAnyUsageCounter(usageSources, ['input_tokens', 'inputTokens']);
    const outputTokens = readAnyUsageCounter(usageSources, ['output_tokens', 'outputTokens']);
    const totalTokens = readAnyUsageCounter(usageSources, ['total_tokens', 'totalTokens']);
    const cacheReadTokens = readAnyUsageCounter(usageSources, [
      'cache_read_tokens',
      'cacheReadTokens',
      'cache_read_input_tokens',
      'cacheReadInputTokens',
    ]);
    const cacheWriteTokens = readAnyUsageCounter(usageSources, [
      'cache_write_tokens',
      'cacheWriteTokens',
      'cache_creation_input_tokens',
      'cacheCreationInputTokens',
    ]);

    if (inputTokens !== null) nextUsage.inputTokens = inputTokens;
    if (outputTokens !== null) nextUsage.outputTokens = outputTokens;
    if (totalTokens !== null) nextUsage.totalTokens = totalTokens;
    if (cacheReadTokens !== null) nextUsage.cacheReadTokens = cacheReadTokens;
    if (cacheWriteTokens !== null) nextUsage.cacheWriteTokens = cacheWriteTokens;
    if (
      nextUsage.totalTokens === null &&
      nextUsage.inputTokens !== null &&
      nextUsage.outputTokens !== null
    ) {
      nextUsage.totalTokens = nextUsage.inputTokens + nextUsage.outputTokens;
    }
    return nextUsage;
  };

  const cloneTurnResult = (result) => {
    if (!result) return null;
    return {
      ...result,
      usage: result.usage ? { ...result.usage } : null,
      promptReport: result.promptReport ? { ...result.promptReport } : null,
      sessionRefresh: result.sessionRefresh ? { ...result.sessionRefresh } : null,
      treasury: result.treasury
        ? {
            ...result.treasury,
            charge: result.treasury.charge ? { ...result.treasury.charge } : null,
            identity: result.treasury.identity ? { ...result.treasury.identity } : null,
          }
        : result.treasury,
      toolResults: Array.isArray(result.toolResults)
        ? result.toolResults.map((entry) => ({
            ...entry,
            toolCall: entry.toolCall ? { ...entry.toolCall } : entry.toolCall,
          }))
        : [],
    };
  };

  const summarizeToolResultsForEvent = (toolResults = []) =>
    toolResults.map((entry) => ({
      toolCall: entry.toolCall
        ? {
            id: entry.toolCall.id,
            name: entry.toolCall.name,
            input: entry.toolCall.input,
          }
        : null,
      result: entry.result,
      duration: entry.duration ?? null,
    }));

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
  let lastRequest = null;
  let lastResponse = null;
  let lastPromptReport = null;
  let lastTurnResult = null;
  let activeWatchdog = null;
  const conversationHistory = Array.isArray(initialConversationHistory)
    ? initialConversationHistory.map((entry) => ({ ...entry }))
    : [];
  const seededConversationHistoryLength = conversationHistory.length;
  let pendingSessionRefresh = sessionRefresh ? { ...sessionRefresh } : null;
  let currentTurnStartedAt = null;
  let pendingToolCalls = new Map();

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

  const persistStreamSession = ({
    responseText = lastTurnResult?.response ?? lastResponse,
    error = null,
    promptReport = lastPromptReport,
    lastCostUsd = lastTurnResult?.cost ?? null,
    usage = lastTurnResult?.usage ?? emptyUsageCounters(),
    modelUsed = lastTurnResult?.model || effectiveModel,
  } = {}) => {
    if (!sessionStoreInstance || !sessionId) return;

    const storedRequest =
      privacySettings.redactMemory && lastRequest
        ? redactSensitive(lastRequest, privacySettings)
        : lastRequest;
    const rawResponse =
      responseText === null || responseText === undefined
        ? null
        : typeof responseText === 'string'
          ? responseText
          : String(responseText);
    const storedResponse =
      privacySettings.redactMemory && rawResponse
        ? redactSensitive(rawResponse, privacySettings)
        : rawResponse;
    const usageCounters = usage || emptyUsageCounters();
    const totalTokens =
      usageCounters.totalTokens ??
      (usageCounters.inputTokens !== null && usageCounters.outputTokens !== null
        ? usageCounters.inputTokens + usageCounters.outputTokens
        : null);
    const normalizedLastCostUsd =
      lastCostUsd === null || lastCostUsd === undefined || lastCostUsd === ''
        ? null
        : Number(lastCostUsd);
    const payload = {
      provider: effectiveProvider,
      model: modelUsed || effectiveModel,
      thinkLevel: effectiveThinkLevel,
      slaLevel: effectiveSlaLevel,
      agent: agentName,
      lastRequest: storedRequest,
      lastResponse: storedResponse,
      lastError: error ? error?.message || String(error) : null,
      lastErrorCode: error?.code || null,
      lastErrorAt: error ? Date.now() : null,
      abortedLastRun: error
        ? error instanceof InactivityWatchdogError || isAbortLikeError(error)
        : false,
      lastRunMs: currentTurnStartedAt ? Date.now() - currentTurnStartedAt : null,
      lastCostUsd: Number.isFinite(normalizedLastCostUsd) ? normalizedLastCostUsd : null,
      inputTokens: usageCounters.inputTokens,
      outputTokens: usageCounters.outputTokens,
      totalTokens,
      cacheReadTokens: usageCounters.cacheReadTokens,
      cacheWriteTokens: usageCounters.cacheWriteTokens,
      compactionCount: 0,
      promptReport,
      sessionRefresh: lastTurnResult?.sessionRefresh ?? null,
    };

    try {
      if (typeof sessionStoreInstance.recordRun === 'function') {
        sessionStoreInstance.recordRun(sessionId, payload);
      } else if (typeof sessionStoreInstance.upsert === 'function') {
        sessionStoreInstance.upsert(sessionId, payload);
      }
    } catch (err) {
      console.warn('[Harness] Session store write failed:', err.message);
    }
  };

  const stopActiveWatchdog = () => {
    if (!activeWatchdog) return;
    activeWatchdog.stop();
    activeWatchdog = null;
  };

  const startTurnWatchdog = () => {
    stopActiveWatchdog();
    if (!watchdogTimeoutMs) return;

    activeWatchdog = createInactivityWatchdog({
      timeoutMs: watchdogTimeoutMs,
      abortController: effectiveAbortController,
      message: `No Claude SDK activity received after ${watchdogTimeoutMs}ms`,
      onTimeout: (watchdogError) => {
        emitEvent(onEvent, {
          type: 'watchdog_timeout',
          timeoutMs: watchdogTimeoutMs,
          elapsedMs: watchdogError.elapsedMs,
          provider: effectiveProvider,
          model: effectiveModel,
          sessionId,
        });
      },
    });
  };

  const buildTurnPromptReport = (request) => {
    const seededHistoryOnly =
      seededConversationHistoryLength > 0 &&
      conversationHistory.length === seededConversationHistoryLength;
    const report = buildPromptReport({
      request,
      history: conversationHistory,
      systemPrompt: agentConfig.systemPrompt,
      includeHistory: conversationHistory.length > 0,
      resumeSession: false,
      historySource:
        conversationHistory.length === 0
          ? 'none'
          : seededHistoryOnly
            ? 'conversation_history'
            : 'live_session',
      redactOptions: privacySettings,
      redactHistory: privacySettings.redactHistory,
    });
    lastPromptReport = report;
    if (lastTurnResult) {
      lastTurnResult.promptReport = report;
    }
    return report;
  };

  const saveTurnMemory = async ({ request, response, toolResults, sessionId: turnSessionId }) => {
    if (!effectiveEnableMemory || !response) return;
    try {
      const facts = [];
      for (const toolResult of toolResults || []) {
        if (toolResult?.toolCall?.name) {
          facts.push(`Used tool: ${toolResult.toolCall.name}`);
        }
      }

      const summaryRequest = privacySettings.redactMemory
        ? redactSensitive(request, privacySettings)
        : request;
      const summaryResponse = privacySettings.redactMemory
        ? redactSensitive(response, privacySettings)
        : response;
      const memoryEntry = {
        summary: `${summaryRequest.slice(0, 100)}${summaryRequest.length > 100 ? '...' : ''} → ${summaryResponse.slice(0, 150)}${summaryResponse.length > 150 ? '...' : ''}`,
        facts,
        agent: agentName,
        sessionId: turnSessionId,
        channel: 'cli',
        senderId: 'local',
      };

      if (memoryStoreInstance) {
        memoryStoreInstance.save(memoryEntry);
      }
      if (markdownMemoryStoreInstance) {
        await markdownMemoryStoreInstance.save(memoryEntry);
      }
    } catch (err) {
      console.warn('[Harness] Memory save failed:', err.message);
    }
  };

  async function* inputStream() {
    while (!closed) {
      const next = await nextMessage();
      if (!next) continue;
      inTurn = true;
      assistantStarted = false;
      assistantText = '';
      lastRequest = next;
      lastResponse = null;
      currentTurnStartedAt = Date.now();
      pendingToolCalls = new Map();
      lastTurnResult = {
        request: next,
        response: null,
        toolResults: [],
        sessionId,
        provider: effectiveProvider,
        model: effectiveModel,
        cost: null,
        budgetExceeded: false,
        usage: emptyUsageCounters(),
        promptReport: null,
        sessionRefresh: pendingSessionRefresh ? { ...pendingSessionRefresh } : null,
        treasury: treasuryState
          ? {
              requestId: null,
              charge: null,
              identity: treasuryState.erc8004Identity,
            }
          : undefined,
        error: null,
        errorCode: null,
      };
      pendingSessionRefresh = null;
      startTurnWatchdog();
      if (!agentStarted) {
        agentStarted = true;
        emitEvent(onEvent, { type: 'agent_start', slaLevel: effectiveSlaLevel });
      }
      const promptReport = buildTurnPromptReport(next);
      const requestText =
        conversationHistory.length > 0 && !sessionId
          ? buildPromptWithHistory(next, conversationHistory, {
              redactHistory: privacySettings.redactHistory,
              redactOptions: privacySettings,
            })
          : next;
      emitEvent(onEvent, { type: 'turn_start' });
      emitEvent(onEvent, { type: 'prompt_report', report: promptReport });
      const userEventMessage = { role: 'user', content: redactEventText(next) };
      emitEvent(onEvent, { type: 'message_start', message: userEventMessage });
      emitEvent(onEvent, { type: 'message_end', message: userEventMessage });
      yield {
        type: 'user',
        session_id: sessionId || '',
        message: {
          role: 'user',
          content: [{ type: 'text', text: requestText }],
        },
        parent_tool_use_id: null,
      };
    }
  }

  async function* stream() {
    await initializeTreasuryState();
    let apiKeyOverride = apiKey;
    if (!apiKeyOverride && typeof getApiKey === 'function') {
      apiKeyOverride = await getApiKey(effectiveProvider);
    }
    const claudeEnv = buildClaudeEnv({ apiKey: apiKeyOverride });
    const optionsForQuery = {
      model: effectiveModel,
      systemPrompt: agentConfig.systemPrompt,
      mcpServers,
      allowedTools,
      maxTurns,
      permissionMode: 'bypassPermissions',
      allowDangerouslySkipPermissions: true,
      ...(streamThinkTokens > 0 ? { maxThinkingTokens: streamThinkTokens } : {}),
      ...(effectiveStreamMaxBudgetUsd ? { maxBudgetUsd: effectiveStreamMaxBudgetUsd } : {}),
      ...(claudeEnv ? { env: claudeEnv } : {}),
      ...(effectiveAbortController ? { abortController: effectiveAbortController } : {}),
    };

    try {
      try {
        for await (const message of __runQueryWithCleanArgv(() =>
          invokeClaudeQuery({ prompt: inputStream(), options: optionsForQuery }),
        )) {
          activeWatchdog?.touch();
          if (lastTurnResult) {
            lastTurnResult.usage = mergeUsageCounters(lastTurnResult.usage, message);
          }

          if (message.sessionId && !sessionId) {
            sessionId = message.sessionId;
          }
          if (message.sessionId && lastTurnResult) {
            lastTurnResult.sessionId = message.sessionId;
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
                  const toolCall = {
                    id: block.id,
                    name: block.name,
                    input: block.input,
                    startTime: Date.now(),
                  };
                  const entry = { toolCall, result: null };
                  if (lastTurnResult) {
                    lastTurnResult.toolResults.push(entry);
                  }
                  if (toolCall.id) {
                    pendingToolCalls.set(toolCall.id, entry);
                  }
                  emitEvent(onEvent, {
                    type: 'tool_execution_start',
                    toolCallId: toolCall.id,
                    toolName: toolCall.name,
                    args: redactEventValue(toolCall.input),
                  });
                } else if (block.type === 'text') {
                  assistantText += block.text;
                  if (lastTurnResult) {
                    lastTurnResult.response = assistantText;
                  }
                  emitEvent(onEvent, {
                    type: 'message_update',
                    message: { role: 'assistant', content: redactEventText(assistantText) },
                    delta: redactEventText(block.text),
                  });
                } else if (block.type === 'thinking') {
                  emitEvent(onEvent, {
                    type: 'thinking_block',
                    block: redactEventValue(block),
                  });
                }
              }
            }
          } else if (message.type === 'user' && message.tool_use_result) {
            let toolResult = message.tool_use_result;
            const toolUseId =
              message.parent_tool_use_id ||
              message.tool_use_id ||
              message.tool_use_result?.tool_use_id ||
              message.tool_use_result?.tool_use_id;
            const pending = toolUseId
              ? pendingToolCalls.get(toolUseId)
              : lastTurnResult?.toolResults.find((entry) => entry.result === null) || null;
            if (pending) {
              pending.result = toolResult;
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
                  toolResult = hookResult.result;
                }
              }
              emitEvent(onEvent, {
                type: 'tool_execution_end',
                toolCallId: pending.toolCall.id,
                toolName: pending.toolCall.name,
                result: redactEventValue(toolResult),
                isError: Boolean(toolResult?.is_error || toolResult?.isError),
              });
              if (toolUseId) {
                pendingToolCalls.delete(toolUseId);
              }
            }
          }
          if (message.type === 'result') {
            inTurn = false;
            if (lastTurnResult) {
              lastTurnResult.usage = mergeUsageCounters(lastTurnResult.usage, message);
              if (message.result !== null && message.result !== undefined) {
                lastTurnResult.response = message.result;
                assistantText = message.result;
              }
              if (lastTurnResult.response !== null && lastTurnResult.response !== undefined) {
                lastResponse = lastTurnResult.response;
              }
              if (message.total_cost_usd !== null && message.total_cost_usd !== undefined) {
                lastTurnResult.cost = Number(message.total_cost_usd);
              }
              if (message.subtype === 'error_max_budget_usd') {
                lastTurnResult.budgetExceeded = true;
              }
              if (message.subtype && message.subtype.startsWith('error_')) {
                lastTurnResult.errorCode = message.subtype;
                lastTurnResult.error =
                  message.errors && message.errors.length > 0
                    ? message.errors.join('; ')
                    : message.subtype;
              }
            }
            stopActiveWatchdog();
            if (lastTurnResult?.response && hooks?.hasHooks?.('before_send')) {
              const hookResult = await hooks.run('before_send', {
                request: lastRequest,
                response: lastTurnResult.response,
                agent: agentName,
                model: lastTurnResult.model,
                provider: effectiveProvider,
                toolResults: lastTurnResult.toolResults,
              });
              if (hookResult?.response) {
                lastTurnResult.response = hookResult.response;
                lastResponse = hookResult.response;
                assistantText = hookResult.response;
              }
            }
            if (treasuryState) {
              const treasuryCharge = await recordTreasuryLlmCharge({
                costUsd: lastTurnResult?.cost ?? null,
                sessionId,
                provider: effectiveProvider,
                model: lastTurnResult?.model || effectiveModel,
                usage: lastTurnResult?.usage ?? emptyUsageCounters(),
              });
              lastTurnResult.treasury = treasuryCharge || {
                requestId: null,
                charge: null,
                identity: treasuryState.erc8004Identity,
              };
            }
            if (!assistantStarted) {
              assistantStarted = true;
              emitEvent(onEvent, {
                type: 'message_start',
                message: { role: 'assistant', content: '' },
              });
            }
            if (lastTurnResult?.response !== null && lastTurnResult?.response !== undefined) {
              emitEvent(onEvent, {
                type: 'message_end',
                message: { role: 'assistant', content: redactEventText(lastTurnResult.response) },
              });
            }
            emitEvent(onEvent, {
              type: 'turn_end',
              response:
                lastTurnResult?.response !== null && lastTurnResult?.response !== undefined
                  ? redactEventText(lastTurnResult.response)
                  : null,
              toolResults: redactEventValue(
                summarizeToolResultsForEvent(lastTurnResult?.toolResults || []),
              ),
              cost: lastTurnResult?.cost ?? null,
              budgetExceeded: lastTurnResult?.budgetExceeded ?? false,
              treasury: lastTurnResult?.treasury,
              sessionRefresh: lastTurnResult?.sessionRefresh || null,
            });
            persistStreamSession({
              responseText: lastTurnResult?.response ?? null,
              promptReport: lastTurnResult?.promptReport ?? lastPromptReport,
              lastCostUsd: lastTurnResult?.cost ?? null,
              usage: lastTurnResult?.usage ?? emptyUsageCounters(),
              modelUsed: lastTurnResult?.model || effectiveModel,
            });
            await saveTurnMemory({
              request: lastRequest,
              response: lastTurnResult?.response ?? null,
              toolResults: lastTurnResult?.toolResults || [],
              sessionId,
            });
            if (lastRequest !== null && lastRequest !== undefined) {
              conversationHistory.push({ role: 'user', content: lastRequest });
            }
            if (lastTurnResult?.response !== null && lastTurnResult?.response !== undefined) {
              conversationHistory.push({ role: 'assistant', content: lastTurnResult.response });
            }
            pendingToolCalls = new Map();
            notify();
          }
          yield message;
        }
      } catch (error) {
        if (activeWatchdog?.timedOut && isAbortLikeError(error)) {
          throw activeWatchdog.error || error;
        }
        throw error;
      } finally {
        stopActiveWatchdog();
      }

      emitEvent(onEvent, {
        type: 'agent_end',
        response: lastResponse ? redactEventText(lastResponse) : null,
        toolResults: redactEventValue(
          summarizeToolResultsForEvent(lastTurnResult?.toolResults || []),
        ),
        sessionId,
        agent: agentName,
        slaLevel: effectiveSlaLevel,
        provider: effectiveProvider,
        model: lastTurnResult?.model || effectiveModel,
        cost: lastTurnResult?.cost ?? null,
        budgetExceeded: lastTurnResult?.budgetExceeded ?? false,
        treasury: lastTurnResult?.treasury,
        sessionRefresh: lastTurnResult?.sessionRefresh || null,
      });
    } catch (error) {
      inTurn = false;
      if (lastTurnResult) {
        lastTurnResult.error = error?.message || String(error);
        lastTurnResult.errorCode = error?.code || null;
        lastTurnResult.sessionId = sessionId;
      }
      persistStreamSession({
        responseText: lastTurnResult?.response ?? null,
        error,
        promptReport: lastTurnResult?.promptReport ?? lastPromptReport,
        lastCostUsd: lastTurnResult?.cost ?? null,
        usage: lastTurnResult?.usage ?? emptyUsageCounters(),
        modelUsed: lastTurnResult?.model || effectiveModel,
      });
      emitEvent(onEvent, {
        type: 'agent_end',
        error: error?.message || String(error),
        sessionId,
        agent: agentName,
        slaLevel: effectiveSlaLevel,
        provider: effectiveProvider,
        model: lastTurnResult?.model || effectiveModel,
        cost: lastTurnResult?.cost ?? null,
        budgetExceeded: lastTurnResult?.budgetExceeded ?? false,
        treasury: lastTurnResult?.treasury,
        sessionRefresh: lastTurnResult?.sessionRefresh || null,
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
      if (effectiveAbortController) {
        try {
          effectiveAbortController.abort(reason);
        } catch (err) {
          console.warn('[harness] Abort controller error:', err.message);
        }
      }
      notify();
    },
    getSessionId: () => sessionId,
    getLastPromptReport: () => (lastPromptReport ? { ...lastPromptReport } : null),
    getLastTurnResult: () => cloneTurnResult(lastTurnResult),
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

function getConfiguredCommandQueue(settingsOverrides = null) {
  const resolvedSettings = loadAgentSettings(settingsOverrides || {});
  const queueSettings = resolvedSettings.queue || {};
  return getCommandQueue({
    maxLanes: queueSettings.maxLanes,
    laneTimeoutMs: queueSettings.laneTimeoutMs ?? queueSettings.laneTimeout,
    maxQueueSize: queueSettings.maxQueueSize,
    idleCleanupMs: queueSettings.idleCleanupMs,
    parallelConcurrency: queueSettings.parallelConcurrency,
    waitWarningMs: queueSettings.waitWarningMs,
    runningWarningMs: queueSettings.runningWarningMs,
    warningThrottleMs: queueSettings.warningThrottleMs,
    monitorIntervalMs: queueSettings.monitorIntervalMs,
    emitWarnings: queueSettings.emitWarnings,
  });
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
  const queue = getConfiguredCommandQueue(options.settings || null);

  // Enqueue the operation in the appropriate lane
  return queue.enqueue(
    effectiveLaneId,
    async () => {
      return runAgentLoop(loopOptions);
    },
    {
      request: typeof options.request === 'string' ? options.request.slice(0, 50) : '',
      agent: options.agent,
    },
  );
}

/**
 * Remove a specific queue lane.
 *
 * @param {string} laneId
 * @param {{ force?: boolean }} [options]
 */
export function removeQueueLane(laneId, options = {}) {
  const queue = getCommandQueue();
  return queue.removeLane(laneId, options);
}

/**
 * Clear queue lanes.
 *
 * @param {{ force?: boolean }} [options]
 */
export function clearQueueLanes(options = {}) {
  const queue = getCommandQueue();
  return queue.clearLanes(options);
}

/**
 * Run multiple agent requests in parallel lanes.
 * Each request gets its own lane for concurrent execution.
 *
 * @param {Object[]} requests - Array of runAgentLoop options
 * @returns {Promise<Object[]>} - Array of results
 */
export async function runAgentLoopParallel(requests) {
  const queue = getConfiguredCommandQueue(
    requests.find((options) => options?.settings)?.settings || null,
  );

  return Promise.all(
    requests.map((options, index) => {
      const laneId =
        options?.laneId || options?.resumeSessionId || options?.sessionId || `parallel:${index}`;
      return queue.enqueueParallel(
        laneId,
        async () => {
          return runAgentLoop(options);
        },
        { index, resumeSessionId: options?.resumeSessionId || null },
      );
    }),
  );
}

/**
 * Get queue statistics for monitoring.
 * @returns {Object}
 */
export function getQueueStats(laneId = null) {
  const queue = getCommandQueue();
  if (!laneId) {
    return queue.getStats();
  }

  const laneStats = queue.getLaneStats(laneId);
  if (!laneStats) return null;
  return { laneId, ...laneStats };
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
