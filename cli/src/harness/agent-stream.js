/**
 * Streaming generator implementation backing runAgentStream.
 *
 * Extracted from claude-harness.js. The SDK invocation gate and the
 * @stateset/embedded Commerce constructor are dependency-injected via the
 * `runtime` argument so this module holds no module-scope state.
 */

import { DEFAULT_MODEL, THINK_LEVELS } from '../config.js';
import { createStatesetMcpServer } from '../mcp-server.js';
import { createPermissionGate } from '../permissions.js';
import { loadSyncConfig, SyncConfig } from '../sync/config.js';
import { wrapCommerceWithEvents } from '../sync/capture.js';
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
  loadSessionMeta,
  applySessionMeta,
  resolveWatchdogTimeoutMs,
  resolveAbortState,
} from './run-setup.js';
import { buildBaseHistory, prepareWorkingHistory } from './context-window.js';
import {
  redactStoredText,
  normalizeResponseText,
  buildErrorFields,
  writeSessionRecord,
} from './session-persistence.js';

/**
 * @param {Object} params - runAgentStream options (already destructured by the caller).
 * @param {Object} runtime - { getCommerceCtor, executeQuery(args, queryImpl) }
 */
export async function* runAgentStreamImpl(
  {
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
    autonomousEngine = null,
    apiKey = null,
    getApiKey = null,
    queryImpl = null,
    abortController = null,
    signal = null,
    policyEngine = null,
    policyStorePath = null,
    onEvent = null,
  },
  runtime,
) {
  const effectivePolicyStorePath = resolvePolicyStorePath(dbPath, policyStorePath);
  const resolvedSettings = loadAgentSettings(settings || {});
  const watchdogSettings = { ...resolvedSettings.watchdog };
  const privacySettings = { ...resolvedSettings.privacy, ...(privacy || {}) };
  const { redactEventText, redactEventValue } = createEventRedactors(privacySettings);
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

  const sessionStoreInstance = initSessionStore({
    sessionStore,
    resolvedSettings,
    fallbackMaxSummaries: resolvedSettings.memory?.maxSummaries,
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

  if (effectiveProvider !== 'claude') {
    throw new Error(
      `runAgentStream supports only claude provider (requested: ${effectiveProvider})`,
    );
  }

  const Commerce = runtime.getCommerceCtor();
  let commerce = new Commerce(dbPath);

  // Context guard for streaming path (optional)
  const streamSessionSummary = sessionMeta?.summaries?.[0] || null;
  const { baseHistory: streamBaseHistory, historySource: streamHistorySource } = buildBaseHistory({
    conversationHistory,
    sessionSummary: streamSessionSummary,
  });
  const { workingHistory, contextGuardResult, compactionSummary } = await prepareWorkingHistory({
    baseHistory: streamBaseHistory,
    transformContext,
    effectiveSignal,
    enableContextGuard: effectiveEnableContextGuard,
    contextSettings,
    effectiveModel,
    effectiveRequest,
    hooks,
    telem: null,
    onContextWarning,
  });

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
    autonomousEngine,
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

    const storedRequest = redactStoredText(effectiveRequest, privacySettings);
    const rawResponse = normalizeResponseText(responseText, { allowNull: true });
    const storedResponse = redactStoredText(rawResponse, privacySettings);

    writeSessionRecord({
      sessionStoreInstance,
      sessionId: sessionIdToStore,
      preferRecordRun: false,
      payload: {
        provider: effectiveProvider,
        model: effectiveModel,
        thinkLevel: effectiveThinkLevel,
        slaLevel: effectiveSlaLevel,
        agent: agentName,
        lastRequest: storedRequest,
        lastResponse: storedResponse,
        ...buildErrorFields(error),
        lastRunMs: Date.now() - runStartedAt,
        promptReport,
      },
      compactionSummary,
      appendCompactionSummary,
    });
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
      for await (const message of runtime.executeQuery({ prompt: input, options }, queryImpl)) {
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
