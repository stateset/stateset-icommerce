/**
 * Queued multi-turn streaming session implementation backing
 * createAgentStreamSession.
 *
 * Extracted from claude-harness.js. The SDK invocation gate and the
 * @stateset/embedded Commerce constructor are dependency-injected via the
 * `runtime` argument so this module holds no module-scope state.
 */

import { DEFAULT_MODEL, THINK_LEVELS } from '../config.js';
import { createStatesetMcpServer } from '../mcp-server.js';
import { createX402McpServer, X402_MCP_TOOL_NAMES } from '../x402-mcp-server.js';
import { createPermissionGate } from '../permissions.js';
import { loadSyncConfig, SyncConfig } from '../sync/config.js';
import { wrapCommerceWithEvents } from '../sync/capture.js';
import { getMarkdownMemoryStore } from '../memory/markdown-store.js';
import { getMemoryStore } from '../memory/store.js';
import { loadAgentSettings } from '../settings.js';
import { ensureHarnessPluginsLoaded, getHarnessHookRunner } from '../harness-hooks.js';
import {
  emitEvent,
  createInactivityWatchdog,
  isAbortLikeError,
  buildClaudeEnv,
} from '../harness-utils.js';
import { buildPromptReport, buildPromptWithHistory } from '../conversation-history.js';
import { AGENTS } from '../agent-definitions.js';
import { routeToAgentWithConfidence } from '../agent-router.js';
import {
  resolvePolicyStorePath,
  createEventRedactors,
  initSessionStore,
  resolveWatchdogTimeoutMs,
  resolveAbortState,
} from './run-setup.js';
import { emptyUsageCounters, mergeUsageCounters, computeTotalTokens } from './usage-counters.js';
import {
  resolveTreasuryConfig,
  loadDefaultTreasuryRuntime,
  initTreasuryState,
  createStreamTreasuryChargeRecorder,
} from './treasury-billing.js';
import { cloneTurnResult, summarizeToolResultsForEvent } from './turn-result.js';
import { createTurnQueue } from './turn-queue.js';
import {
  redactStoredText,
  normalizeResponseText,
  buildErrorFields,
  writeSessionRecord,
} from './session-persistence.js';
import { normalizeCostUsd } from './usage-counters.js';
import { saveRunMemory } from './memory-writer.js';

/**
 * @param {Object} options - createAgentStreamSession options.
 * @param {Object} runtime - { getCommerceCtor, executeQuery(args, queryImpl) }
 */
export function createAgentStreamSessionImpl(options, runtime) {
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
    autonomousEngine = null,
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
    queryImpl = null,
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
  const { redactEventText, redactEventValue } = createEventRedactors(privacySettings);
  const pluginsEnabled = enablePlugins ?? resolvedSettings.plugins?.enabled ?? false;
  const pluginsVerbose = resolvedSettings.plugins?.verbose ?? false;
  const effectiveGuardrails = guardrails
    ? { ...resolvedSettings.guardrails, ...guardrails }
    : { ...resolvedSettings.guardrails };
  const effectiveProvider = provider || resolvedSettings.provider?.default || 'claude';
  const effectiveModel = model || resolvedSettings.model?.default || DEFAULT_MODEL;
  const effectiveThinkLevel = thinkLevel ?? resolvedSettings.thinkLevel?.default ?? 'off';
  const effectiveSlaLevel = slaLevel ?? resolvedSettings.agent?.slaLevel ?? null;
  const effectiveEnableMemory = enableMemory ?? memorySettings.enabled;
  const effectiveUseMarkdownMemory = useMarkdownMemory ?? memorySettings.useMarkdown;
  const watchdogTimeoutMs = resolveWatchdogTimeoutMs({
    watchdogSettings,
    resumeSessionId: null,
    effectiveProvider,
  });
  const { effectiveAbortController } = resolveAbortState({
    abortController,
    signal,
    watchdogTimeoutMs,
  });

  const treasuryConfig = resolveTreasuryConfig(treasury);

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
  const sessionStoreInstance = initSessionStore({
    sessionStore,
    resolvedSettings,
    fallbackMaxSummaries: resolvedSettings.memory?.maxSummaries,
  });

  let memoryStoreInstance = memoryStoreOverride || null;
  let markdownMemoryStoreInstance = markdownMemoryStoreOverride || null;
  if (effectiveEnableMemory) {
    try {
      if (!memoryStoreInstance) {
        memoryStoreInstance = getMemoryStore({ dbPath: memorySettings.dbPath || undefined });
      }
      if (effectiveUseMarkdownMemory && !markdownMemoryStoreInstance) {
        markdownMemoryStoreInstance = getMarkdownMemoryStore({
          memoryDir: memorySettings.dir || undefined,
        });
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

  const Commerce = runtime.getCommerceCtor();
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
    autonomousEngine,
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

  const initializeTreasuryState = async () => {
    if (!treasuryConfig?.enabled || treasuryState) return;
    let treasuryRuntimeImpl;
    try {
      treasuryRuntimeImpl = treasuryRuntime || (await loadDefaultTreasuryRuntime());
    } catch (error) {
      throw new Error(`Treasury billing failed: ${error.message}`);
    }
    const initialized = await initTreasuryState({
      treasuryConfig,
      dbPath,
      maxBudgetUsd: configuredMaxBudgetUsd,
      runtime: treasuryRuntimeImpl,
      includeRuntimeInState: true,
    });
    treasuryState = initialized.treasuryState;
    effectiveStreamMaxBudgetUsd = initialized.effectiveBudgetUsd;
  };

  const recordTreasuryLlmCharge = createStreamTreasuryChargeRecorder({
    getTreasuryState: () => treasuryState,
  });

  const turnQueue = createTurnQueue();
  let sessionId = null;
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

  const persistStreamSession = ({
    responseText = lastTurnResult?.response ?? lastResponse,
    error = null,
    promptReport = lastPromptReport,
    lastCostUsd = lastTurnResult?.cost ?? null,
    usage = lastTurnResult?.usage ?? emptyUsageCounters(),
    modelUsed = lastTurnResult?.model || effectiveModel,
  } = {}) => {
    if (!sessionStoreInstance || !sessionId) return;

    const storedRequest = redactStoredText(lastRequest, privacySettings);
    const rawResponse = normalizeResponseText(responseText, { allowNull: true });
    const storedResponse = redactStoredText(rawResponse, privacySettings);
    const usageCounters = usage || emptyUsageCounters();

    writeSessionRecord({
      sessionStoreInstance,
      sessionId,
      payload: {
        provider: effectiveProvider,
        model: modelUsed || effectiveModel,
        thinkLevel: effectiveThinkLevel,
        slaLevel: effectiveSlaLevel,
        agent: agentName,
        lastRequest: storedRequest,
        lastResponse: storedResponse,
        ...buildErrorFields(error),
        lastRunMs: currentTurnStartedAt ? Date.now() - currentTurnStartedAt : null,
        lastCostUsd: normalizeCostUsd(lastCostUsd),
        inputTokens: usageCounters.inputTokens,
        outputTokens: usageCounters.outputTokens,
        totalTokens: computeTotalTokens(usageCounters),
        cacheReadTokens: usageCounters.cacheReadTokens,
        cacheWriteTokens: usageCounters.cacheWriteTokens,
        compactionCount: 0,
        promptReport,
        sessionRefresh: lastTurnResult?.sessionRefresh ?? null,
      },
    });
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
    if (!effectiveEnableMemory) return;
    await saveRunMemory({
      request,
      response,
      toolResults,
      agentName,
      sessionId: turnSessionId,
      memoryStore: memoryStoreInstance,
      markdownMemory: markdownMemoryStoreInstance,
      privacySettings,
      onError: (err) => {
        console.warn('[Harness] Memory save failed:', err.message);
      },
    });
  };

  async function* inputStream() {
    while (!turnQueue.isClosed()) {
      const next = await turnQueue.nextMessage();
      if (!next) continue;
      turnQueue.setInTurn(true);
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
        for await (const message of runtime.executeQuery(
          { prompt: inputStream(), options: optionsForQuery },
          queryImpl,
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
            turnQueue.setInTurn(false);
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
            turnQueue.notify();
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
      turnQueue.setInTurn(false);
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
    send: (text) => turnQueue.enqueue(text, 'followUp'),
    followUp: (text) => turnQueue.enqueue(text, 'followUp'),
    steer: (text) => turnQueue.enqueue(text, 'steer'),
    close: () => {
      turnQueue.close();
    },
    abort: (reason) => {
      turnQueue.close();
      if (effectiveAbortController) {
        try {
          effectiveAbortController.abort(reason);
        } catch (err) {
          console.warn('[harness] Abort controller error:', err.message);
        }
      }
    },
    getSessionId: () => sessionId,
    getLastPromptReport: () => (lastPromptReport ? { ...lastPromptReport } : null),
    getLastTurnResult: () => cloneTurnResult(lastTurnResult),
  };
}
