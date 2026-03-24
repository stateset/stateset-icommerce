import { randomUUID } from 'node:crypto';
import { createAgentStreamSession, runAgentLoop } from '../claude-harness.js';

export function shouldUsePersistentChatSession({ provider } = {}) {
  return provider === 'claude';
}

function cloneConversationHistory(history = []) {
  return Array.isArray(history) ? history.map((entry) => ({ ...entry })) : [];
}

function buildPersistentSessionKey(options = {}) {
  return JSON.stringify({
    provider: options.provider || 'claude',
    dbPath: options.dbPath || './store.db',
    model: options.model || null,
    allowApply: Boolean(options.allowApply),
    maxTurns: options.maxTurns ?? 10,
    thinkLevel: options.thinkLevel ?? 'off',
    maxBudgetUsd: options.maxBudgetUsd ?? null,
    enableX402: Boolean(options.enableX402),
    enableMemory: options.enableMemory ?? null,
    useMarkdownMemory: options.useMarkdownMemory ?? null,
    treasury: options.treasury || null,
  });
}

function buildPersistentTelemetry({ traceId, startedAt, turnResult }) {
  const toolResults = Array.isArray(turnResult?.toolResults) ? turnResult.toolResults : [];
  const total = toolResults.length;
  const successful = toolResults.filter(
    (entry) => !entry?.result?.is_error && !entry?.result?.isError && !entry?.result?.error,
  ).length;
  const failed = total - successful;
  const durations = toolResults
    .map((entry) => Number(entry?.duration))
    .filter((value) => Number.isFinite(value) && value >= 0);
  const topToolMap = new Map();

  for (const entry of toolResults) {
    const name = entry?.toolCall?.name || 'unknown';
    const duration = Number(entry?.duration);
    const current = topToolMap.get(name) || { name, count: 0, totalMs: 0, avgMs: 0 };
    current.count += 1;
    if (Number.isFinite(duration) && duration >= 0) {
      current.totalMs += duration;
      current.avgMs = current.totalMs / current.count;
    }
    topToolMap.set(name, current);
  }

  return {
    traceId,
    duration: startedAt ? Date.now() - startedAt : 0,
    spanCount: 0,
    toolCalls: {
      total,
      successful,
      failed,
      successRate: total > 0 ? `${((successful / total) * 100).toFixed(1)}%` : 'N/A',
    },
    avgToolDuration:
      durations.length > 0
        ? Math.round(durations.reduce((sum, value) => sum + value, 0) / durations.length)
        : 0,
    topTools: Array.from(topToolMap.values())
      .sort((a, b) => b.count - a.count)
      .slice(0, 5),
  };
}

function createPersistentChatResult({ turnResult, session, options, turn }) {
  const traceId = turn?.traceId || randomUUID();
  const sessionRefresh = turnResult?.sessionRefresh || turn?.sessionRefresh || null;
  return {
    response: turnResult?.response || '',
    toolResults: Array.isArray(turnResult?.toolResults) ? turnResult.toolResults : [],
    sessionId: session.getSessionId(),
    provider: options.provider,
    thinkLevel: options.thinkLevel,
    cost: turnResult?.cost ?? null,
    budgetExceeded: Boolean(turnResult?.budgetExceeded),
    promptReport: session.getLastPromptReport(),
    usedModel: turnResult?.model || options.model,
    treasury: turnResult?.treasury,
    sessionRefresh: sessionRefresh
      ? {
          ...sessionRefresh,
          sessionId: session.getSessionId(),
        }
      : null,
    telemetry: buildPersistentTelemetry({
      traceId,
      startedAt: turn?.startedAt,
      turnResult,
    }),
    traceId,
  };
}

export function createChatTransport({
  createSessionImpl = createAgentStreamSession,
  runAgentLoopImpl = runAgentLoop,
} = {}) {
  let persistentSession = null;
  let persistentConsumer = null;
  let persistentSessionKey = null;
  let persistentConversationHistory = [];
  let pendingTreasuryRefresh = false;
  let activeTurn = null;

  const appendPersistentConversationTurn = ({ request, response }) => {
    if (request) {
      persistentConversationHistory.push({ role: 'user', content: request });
    }
    if (response !== null && response !== undefined && response !== '') {
      persistentConversationHistory.push({ role: 'assistant', content: response });
    }
  };

  const resetPersistentSession = (reason = 'reset', { clearHistory = true } = {}) => {
    const pendingTurn = activeTurn;
    activeTurn = null;

    if (persistentSession) {
      try {
        persistentSession.close();
      } catch {
        // Ignore close errors during transport reset.
      }
    }

    persistentSession = null;
    persistentConsumer = null;
    persistentSessionKey = null;
    pendingTreasuryRefresh = false;
    if (clearHistory) {
      persistentConversationHistory = [];
    }

    if (pendingTurn) {
      const error = new Error(`Chat session closed: ${reason}`);
      error.code = 'CHAT_SESSION_RESET';
      pendingTurn.reject(error);
    }
  };

  const handlePersistentEvent = (event) => {
    if (!activeTurn) return;

    if (typeof activeTurn.onEvent === 'function') {
      activeTurn.onEvent(event);
    }

    if (
      event?.type === 'message_update' &&
      activeTurn.streaming &&
      typeof activeTurn.onPartialMessage === 'function'
    ) {
      const deltaText =
        typeof event.delta === 'string'
          ? event.delta
          : typeof event.delta?.text === 'string'
            ? event.delta.text
            : null;
      if (deltaText) {
        activeTurn.onPartialMessage({
          text: deltaText,
          content: deltaText,
          delta: { text: deltaText },
        });
      }
    }

    if (event?.type === 'thinking_block' && typeof activeTurn.onThinkingBlock === 'function') {
      activeTurn.onThinkingBlock(event.block);
    }

    if (event?.type === 'tool_execution_start' && typeof activeTurn.onToolCall === 'function') {
      activeTurn.onToolCall({
        id: event.toolCallId,
        name: event.toolName,
        input: event.args,
      });
    }
  };

  const ensurePersistentSession = (options, sessionRefresh = null) => {
    const nextKey = buildPersistentSessionKey(options);
    if (persistentSession && persistentSessionKey !== nextKey) {
      resetPersistentSession('config changed');
    }
    if (persistentSession) {
      return persistentSession;
    }

    persistentSession = createSessionImpl({
      dbPath: options.dbPath,
      model: options.model,
      allowApply: options.allowApply,
      maxTurns: options.maxTurns,
      agent: options.agent,
      slaLevel: options.slaLevel,
      enableSync: options.enableSync,
      guardrails: options.guardrails,
      onConfirmRequired: options.onConfirmRequired,
      permissionGate: options.permissionGate,
      settings: options.settings,
      privacy: options.privacy,
      hookRunner: options.hookRunner,
      enablePlugins: options.enablePlugins,
      provider: options.provider,
      thinkLevel: options.thinkLevel,
      maxBudgetUsd: options.maxBudgetUsd,
      enableX402: options.enableX402,
      enableMemory: options.enableMemory,
      useMarkdownMemory: options.useMarkdownMemory,
      memoryStore: options.memoryStore,
      markdownMemoryStore: options.markdownMemoryStore,
      conversationHistory: cloneConversationHistory(persistentConversationHistory),
      sessionRefresh: sessionRefresh ? { ...sessionRefresh } : null,
      treasury: options.treasury,
      treasuryRuntime: options.treasuryRuntime,
      apiKey: options.apiKey,
      getApiKey: options.getApiKey,
      abortController: options.abortController,
      signal: options.signal,
      policyEngine: options.policyEngine,
      policyStorePath: options.policyStorePath,
      onEvent: handlePersistentEvent,
    });
    persistentSessionKey = nextKey;

    persistentConsumer = (async () => {
      try {
        for await (const message of persistentSession.stream()) {
          if (message?.type !== 'result' || !activeTurn) continue;
          const turn = activeTurn;
          activeTurn = null;
          const turnResult = persistentSession.getLastTurnResult();
          if (turnResult?.error && turnResult?.errorCode !== 'error_max_budget_usd') {
            const error = new Error(turnResult.error);
            error.code = turnResult.errorCode;
            turn.reject(error);
            continue;
          }
          appendPersistentConversationTurn({
            request: turn.options.request,
            response: turnResult?.response,
          });
          if (turn.options?.treasury?.enabled) {
            pendingTreasuryRefresh = true;
          }
          turn.resolve(
            createPersistentChatResult({
              turnResult,
              session: persistentSession,
              options: turn.options,
              turn,
            }),
          );
        }
      } catch (error) {
        const pendingTurn = activeTurn;
        activeTurn = null;
        persistentSession = null;
        persistentConsumer = null;
        persistentSessionKey = null;
        if (pendingTurn) {
          pendingTurn.reject(error);
        }
      }
    })();

    return persistentSession;
  };

  return {
    async query(options = {}) {
      const usePersistentSession = shouldUsePersistentChatSession({
        provider: options.provider,
        treasury: options.treasury,
      });
      const sessionRefresh =
        pendingTreasuryRefresh && options.treasury?.enabled
          ? {
              reason: 'treasury_budget_refresh',
              previousSessionId: persistentSession?.getSessionId?.() || null,
              replayedMessages: persistentConversationHistory.length,
              recordedAt: new Date().toISOString(),
            }
          : null;

      if (sessionRefresh) {
        resetPersistentSession('treasury budget refresh', { clearHistory: false });
      }

      if (!usePersistentSession) {
        if (persistentSession) {
          resetPersistentSession('fallback transport');
        }
        return runAgentLoopImpl(options);
      }

      const session = ensurePersistentSession(options, sessionRefresh);
      if (activeTurn) {
        const error = new Error('Chat session is already processing a turn');
        error.code = 'CHAT_SESSION_BUSY';
        throw error;
      }

      return new Promise((resolve, reject) => {
        activeTurn = {
          resolve,
          reject,
          options,
          traceId: randomUUID(),
          startedAt: Date.now(),
          streaming: Boolean(options.streaming),
          onEvent: options.onEvent,
          onPartialMessage: options.onPartialMessage,
          onThinkingBlock: options.onThinkingBlock,
          onToolCall: options.onToolCall,
          sessionRefresh,
        };
        session.send(options.request);
      });
    },
    reset: resetPersistentSession,
    getSessionId: () => persistentSession?.getSessionId?.() || null,
    isPersistentActive: () => Boolean(persistentSession),
  };
}
