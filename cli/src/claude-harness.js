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
 *
 * The heavy lifting lives in focused modules under ./harness/ — this file is
 * the thin orchestrator and keeps the public export surface unchanged.
 */

import { query } from '@anthropic-ai/claude-agent-sdk';
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
import { ModelFallback } from './model-fallback.js';
import { getMarkdownMemoryStore } from './memory/markdown-store.js';
import { getMemoryStore } from './memory/store.js';
import { loadAgentSettings } from './settings.js';
import { ensureHarnessPluginsLoaded, getHarnessHookRunner } from './harness-hooks.js';
import { redactSensitive } from './privacy.js';
import { buildClaudeEnv, emitEvent } from './harness-utils.js';

// Extracted modules
import { buildPromptReport, buildPromptWithHistory } from './conversation-history.js';
import { AGENTS } from './agent-definitions.js';
import { routeToAgentWithConfidence } from './agent-router.js';

// Harness decomposition (see cli/src/harness/)
import {
  resolvePolicyStorePath,
  createEventRedactors,
  initSessionStore,
  loadSessionMeta,
  applySessionMeta,
  resolveWatchdogTimeoutMs,
  resolveAbortState,
} from './harness/run-setup.js';
import { buildBaseHistory, prepareWorkingHistory } from './harness/context-window.js';
import {
  emptyUsageCounters,
  computeTotalTokens,
  normalizeCostUsd,
} from './harness/usage-counters.js';
import {
  resolveTreasuryConfig,
  loadDefaultTreasuryRuntime,
  initTreasuryState,
  createLoopTreasuryChargeRecorder,
} from './harness/treasury-billing.js';
import {
  redactStoredText,
  normalizeResponseText,
  buildErrorFields,
  writeSessionRecord,
} from './harness/session-persistence.js';
import { saveRunMemory } from './harness/memory-writer.js';
import { runNonClaudeProvider } from './harness/provider-run.js';
import { createRunQuery } from './harness/query-runner.js';
import { executeWithRetry } from './harness/retry-loop.js';
import { runAgentStreamImpl } from './harness/agent-stream.js';
import { createAgentStreamSessionImpl } from './harness/stream-session.js';
import { createQueueRunners } from './harness/queue-execution.js';

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

// Re-export AGENTS from agent-definitions for consumers that import from claude-harness
export { AGENTS };
export { routeToAgent, routeToAgentWithConfidence } from './agent-router.js';

let _sdkArgvGate = Promise.resolve();
let _claudeQueryImpl = query;

function invokeClaudeQuery(args, queryImpl = null) {
  return (queryImpl || _claudeQueryImpl)(args);
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

/**
 * Runtime injected into the extracted harness modules: SDK invocation gate
 * (with the test queryImpl override) plus the Commerce constructor loader.
 */
const harnessRuntime = {
  getCommerceCtor,
  executeQuery: (args, queryImpl = null) =>
    __runQueryWithCleanArgv(() => invokeClaudeQuery(args, queryImpl)),
};

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
 * @param {Object} options.autonomousEngine - Optional autonomous engine used by delegate_to_agent
 * @param {string} options.apiKey - Override Claude API key for this run
 * @param {Function} options.getApiKey - Resolve API key dynamically for this run
 * @param {Function} options.queryImpl - Test-only Claude SDK query implementation override
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
  autonomousEngine = null,
  apiKey = null,
  getApiKey = null,
  queryImpl = null,
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
  const { redactEventText, redactEventValue } = createEventRedactors(privacySettings);
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

  // Determine provider/model/think level with session restore
  let effectiveProvider = provider || resolvedSettings.provider?.default || 'claude';
  let effectiveModel = model || resolvedSettings.model?.default || DEFAULT_MODEL;
  let effectiveThinkLevel = thinkLevel ?? resolvedSettings.thinkLevel?.default ?? 'off';
  let effectiveSlaLevel = slaLevel ?? resolvedSettings.agent?.slaLevel ?? null;

  const treasuryConfig = resolveTreasuryConfig(treasury);

  let treasuryState = null;
  let treasuryCharge = null;
  let effectiveMaxBudgetUsd = maxBudgetUsd;

  const sessionStoreInstance = initSessionStore({
    sessionStore,
    resolvedSettings,
    fallbackMaxSummaries: memorySettings.maxSummaries,
  });
  const sessionMeta = loadSessionMeta({ resumeSessionId, sessionStoreInstance, resolvedSettings });
  ({ effectiveProvider, effectiveModel, effectiveThinkLevel, effectiveSlaLevel, agent } =
    applySessionMeta({
      sessionMeta,
      provider,
      model,
      thinkLevel,
      agent,
      effectiveProvider,
      effectiveModel,
      effectiveThinkLevel,
      effectiveSlaLevel,
    }));

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

  const watchdogTimeoutMs = resolveWatchdogTimeoutMs({
    watchdogSettings,
    resumeSessionId,
    effectiveProvider,
  });
  const { effectiveAbortController, effectiveSignal } = resolveAbortState({
    abortController,
    signal,
    watchdogTimeoutMs,
  });

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
  const { baseHistory, historySource } = buildBaseHistory({ conversationHistory, sessionSummary });
  const { workingHistory, contextGuardResult, compactionSummary } = await prepareWorkingHistory({
    baseHistory,
    transformContext,
    effectiveSignal,
    enableContextGuard: effectiveEnableContextGuard,
    contextSettings,
    effectiveModel,
    effectiveRequest,
    hooks,
    telem,
    onContextWarning,
  });

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
  if (enableFallback && effectiveProvider === 'claude' && !queryImpl) {
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
      memoryStore = getMemoryStore({ dbPath: memorySettings.dbPath || undefined });
      if (effectiveUseMarkdownMemory) {
        markdownMemory = getMarkdownMemoryStore({ memoryDir: memorySettings.dir || undefined });
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
    autonomousEngine,
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
    let treasuryRuntime;
    try {
      treasuryRuntime = await loadDefaultTreasuryRuntime();
    } catch (error) {
      throw new Error(`Treasury billing failed: ${error.message}`);
    }
    const initialized = await initTreasuryState({
      treasuryConfig,
      dbPath,
      maxBudgetUsd,
      runtime: treasuryRuntime,
      includeRequestId: true,
    });
    treasuryState = initialized.treasuryState;
    effectiveMaxBudgetUsd = initialized.effectiveBudgetUsd;
  }

  const recordTreasuryLlmCharge = createLoopTreasuryChargeRecorder({
    getTreasuryState: () => treasuryState,
    telem,
  });

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

    const rawResponse = normalizeResponseText(responseText);
    const storedRequest = redactStoredText(effectiveRequest, privacySettings);
    const storedResponse = redactStoredText(rawResponse, privacySettings);
    const usageCounters = usage || emptyUsageCounters();

    writeSessionRecord({
      sessionStoreInstance,
      sessionId: sessionIdToStore,
      payload: {
        provider: effectiveProvider,
        model: modelUsed || effectiveModel,
        thinkLevel: effectiveThinkLevel,
        slaLevel: effectiveSlaLevel,
        agent: agentName,
        lastRequest: storedRequest,
        lastResponse: storedResponse,
        ...buildErrorFields(error),
        lastRunMs: Date.now() - runStartedAt,
        lastCostUsd: normalizeCostUsd(lastCostUsd),
        inputTokens: usageCounters.inputTokens,
        outputTokens: usageCounters.outputTokens,
        totalTokens: computeTotalTokens(usageCounters),
        cacheReadTokens: usageCounters.cacheReadTokens,
        cacheWriteTokens: usageCounters.cacheWriteTokens,
        compactionCount: compactionSummary ? 1 : 0,
        promptReport,
      },
      compactionSummary,
      appendCompactionSummary,
    });
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
      return __runWithCleanArgv(() =>
        runNonClaudeProvider({
          effectiveProvider,
          effectiveModel,
          effectiveThinkLevel,
          effectiveSlaLevel,
          effectiveRequest,
          requestWithHistory,
          systemPrompt,
          agentName,
          routingResult,
          promptReport,
          streaming,
          onMessage,
          onPartialMessage,
          onEvent,
          redactEventText,
          hooks,
          privacySettings,
          apiKey,
          getApiKey,
          effectiveSignal,
          treasuryState,
          effectiveMaxBudgetUsd,
          telem,
          recordTreasuryLlmCharge,
        }),
      );
    }

    // -------------------------------------------------------------------------
    // v0.4.0: Run query with optional model fallback
    // -------------------------------------------------------------------------

    // Helper function to run the actual query
    const runQuery = createRunQuery({
      executeQuery: (args) => harnessRuntime.executeQuery(args, queryImpl),
      options,
      requestWithHistory,
      resumeSessionId,
      watchdogTimeoutMs,
      effectiveAbortController,
      effectiveProvider,
      telem,
      onEvent,
      redactEventText,
      redactEventValue,
      hooks,
      privacySettings,
      streaming,
      onToolCall,
      onPartialMessage,
      onThinkingBlock,
      syncState: {
        onAttemptStart: () => {
          response = '';
          latestUsage = emptyUsageCounters();
          if (!resumeSessionId) {
            sessionId = null;
          }
        },
        getSessionId: () => sessionId,
        setSessionId: (id) => {
          sessionId = id;
        },
        setResponse: (text) => {
          response = text;
        },
        setUsage: (usage) => {
          latestUsage = usage;
        },
      },
    });

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

    const queryResult = await executeWithRetry({ executeOnce, retrySettings, telem });

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
    if (effectiveEnableMemory) {
      await saveRunMemory({
        request: effectiveRequest,
        response,
        toolResults,
        compactionSummary,
        agentName,
        sessionId,
        memoryStore,
        markdownMemory,
        privacySettings,
        onSaved: (store) => telem.logCustomEvent('memory_saved', { store }),
        onError: (e) => telem.logCustomEvent('memory_save_failed', { error: e.message }),
      });
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
export async function* runAgentStream(options = {}) {
  yield* runAgentStreamImpl(options, harnessRuntime);
}

/**
 * Create a streaming agent session with queued inputs.
 * Messages are queued and delivered in order once the model finishes a turn.
 *
 * @param {Object} options
 * @param {Object|null} [options.autonomousEngine=null] Optional autonomous engine forwarded to runtime tools such as delegate_to_agent.
 * @returns {{ stream: () => AsyncGenerator, send: (text: string) => void, followUp: (text: string) => void, steer: (text: string) => void, close: () => void, abort: (reason?: any) => void, getSessionId: () => string|null, getLastPromptReport: () => object|null, getLastTurnResult: () => object|null }}
 */
export function createAgentStreamSession(options = {}) {
  return createAgentStreamSessionImpl(options, harnessRuntime);
}

/**
 * Create an agent session for multi-turn conversations.
 *
 * Extra options are forwarded to runAgentLoop(), including autonomousEngine for
 * runtime delegation support.
 *
 * @param {Object} options
 * @param {Object|null} [options.autonomousEngine=null] Optional autonomous engine forwarded to delegate_to_agent and related runtime features.
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
// v0.4.0: Queue-Wrapped Agent Execution (see ./harness/queue-execution.js)
// ============================================================================

const { runAgentLoopQueued, runAgentLoopParallel } = createQueueRunners({ runAgentLoop });
export { runAgentLoopQueued, runAgentLoopParallel };
export { removeQueueLane, clearQueueLanes, getQueueStats } from './harness/queue-execution.js';

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
