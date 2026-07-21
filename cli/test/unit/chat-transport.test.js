import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  createChatTransport,
  shouldUsePersistentChatSession,
} from '../../src/utils/chat-transport.js';

function createPersistentSessionFactory() {
  const created = [];
  const closed = [];

  const createSessionImpl = (options) => {
    let wakeRequest = null;
    let isClosed = false;
    let turnIndex = 0;
    let lastTurnResult = null;
    const queuedRequests = [];
    const sessionId = `sess-${created.length + 1}`;
    const nextRequest = async () => {
      if (queuedRequests.length > 0) {
        return queuedRequests.shift();
      }
      return new Promise((resolve) => {
        wakeRequest = resolve;
      });
    };
    const session = {
      async *stream() {
        while (!isClosed) {
          const request = await nextRequest();
          if (request === null) {
            return;
          }

          turnIndex += 1;
          const historySource =
            turnIndex === 1
              ? Array.isArray(options.conversationHistory) && options.conversationHistory.length > 0
                ? 'conversation_history'
                : 'none'
              : 'live_session';
          options.onEvent?.({
            type: 'prompt_report',
            report: {
              historySource,
              totalInputTokens: 10 + turnIndex,
            },
          });
          options.onEvent?.({
            type: 'tool_execution_start',
            toolCallId: 'tool-1',
            toolName: 'mcp__stateset-commerce__list_orders',
            args: { request },
          });
          options.onEvent?.({
            type: 'thinking_block',
            block: { thinking: `Thinking ${request}` },
          });
          options.onEvent?.({
            type: 'message_update',
            delta: `delta:${request}`,
          });

          lastTurnResult = {
            request,
            response: `response:${request}`,
            toolResults: [
              {
                toolCall: {
                  id: 'tool-1',
                  name: 'mcp__stateset-commerce__list_orders',
                  input: { request },
                },
                result: { ok: true },
                duration: 5,
              },
            ],
            sessionId,
            provider: 'claude',
            model: options.model,
            cost: 0.42,
            budgetExceeded: false,
            usage: {
              inputTokens: 1,
              outputTokens: 2,
              totalTokens: 3,
              cacheReadTokens: null,
              cacheWriteTokens: null,
            },
            promptReport: {
              historySource,
              totalInputTokens: 10 + turnIndex,
            },
            treasury: options.treasury?.enabled
              ? {
                  requestId: `treasury-turn-${turnIndex}`,
                  charge: {
                    eventId: `evt-${turnIndex}`,
                    amount: '0.42',
                    amountSmallest: '420000',
                    token: 'USDC',
                    chainId: options.treasury.chainId || 'set_chain',
                  },
                  identity: null,
                }
              : undefined,
            error: null,
            errorCode: null,
          };

          yield { type: 'assistant' };
          yield { type: 'result', result: lastTurnResult.response };
        }
      },
      send(text) {
        if (wakeRequest) {
          const resolve = wakeRequest;
          wakeRequest = null;
          resolve(text);
          return;
        }
        queuedRequests.push(text);
      },
      close() {
        isClosed = true;
        closed.push(sessionId);
        if (wakeRequest) {
          const resolve = wakeRequest;
          wakeRequest = null;
          resolve(null);
        }
      },
      getSessionId() {
        return sessionId;
      },
      getLastPromptReport() {
        return lastTurnResult?.promptReport || null;
      },
      getLastTurnResult() {
        return lastTurnResult;
      },
    };

    created.push({ options, sessionId });
    return session;
  };

  return { createSessionImpl, created, closed };
}

describe('shouldUsePersistentChatSession', () => {
  it('uses the persistent Claude session for Claude, including treasury-backed runs', () => {
    assert.equal(shouldUsePersistentChatSession({ provider: 'claude' }), true);
    assert.equal(shouldUsePersistentChatSession({ provider: 'openai' }), false);
    assert.equal(
      shouldUsePersistentChatSession({
        provider: 'claude',
        treasury: { enabled: true },
      }),
      true,
    );
    assert.equal(
      shouldUsePersistentChatSession({
        provider: 'claude',
        treasury: { enabled: false },
      }),
      true,
    );
  });
});

describe('createChatTransport', () => {
  it('reuses a persistent Claude session and translates session events into chat callbacks', async () => {
    const factory = createPersistentSessionFactory();
    const fallbackCalls = [];
    const transport = createChatTransport({
      createSessionImpl: factory.createSessionImpl,
      runAgentLoopImpl: async (options) => {
        fallbackCalls.push(options);
        return { response: 'fallback' };
      },
      settingsLoader: () => ({ memory: { enabled: false } }),
    });

    const partials = [];
    const thinking = [];
    const tools = [];
    const events = [];

    const first = await transport.query({
      request: 'first',
      provider: 'claude',
      model: 'claude-test',
      dbPath: './store.db',
      thinkLevel: 'medium',
      streaming: true,
      enableMemory: true,
      treasury: { enabled: true, chainId: 'set_chain', agentId: 'agent-1' },
      onEvent: (event) => events.push(event.type),
      onPartialMessage: (event) => partials.push(event.text),
      onThinkingBlock: (block) => thinking.push(block.thinking),
      onToolCall: (toolCall) => tools.push(toolCall.name),
    });

    const second = await transport.query({
      request: 'second',
      provider: 'claude',
      model: 'claude-test',
      dbPath: './store.db',
      thinkLevel: 'medium',
      streaming: true,
      enableMemory: true,
      treasury: { enabled: true, chainId: 'set_chain', agentId: 'agent-1' },
    });

    assert.equal(factory.created.length, 2);
    assert.equal(fallbackCalls.length, 0);
    assert.equal(first.response, 'response:first');
    assert.equal(first.sessionId, 'sess-1');
    assert.equal(first.cost, 0.42);
    assert.ok(first.traceId);
    assert.equal(first.telemetry.traceId, first.traceId);
    assert.equal(first.telemetry.toolCalls.total, 1);
    assert.equal(first.telemetry.toolCalls.successful, 1);
    assert.equal(first.telemetry.toolCalls.failed, 0);
    assert.equal(first.telemetry.toolCalls.successRate, '100.0%');
    assert.equal(first.telemetry.avgToolDuration, 5);
    assert.equal(first.telemetry.topTools[0].name, 'mcp__stateset-commerce__list_orders');
    assert.equal(first.promptReport.historySource, 'none');
    assert.deepEqual(factory.created[0].options.treasury, {
      enabled: true,
      chainId: 'set_chain',
      agentId: 'agent-1',
    });
    assert.equal(first.treasury.requestId, 'treasury-turn-1');
    assert.equal(first.treasury.charge.eventId, 'evt-1');
    assert.equal(first.sessionRefresh, null);
    assert.deepEqual(factory.created[1].options.conversationHistory, [
      { role: 'user', content: 'first' },
      { role: 'assistant', content: 'response:first' },
    ]);
    assert.equal(factory.created[1].options.sessionRefresh.reason, 'treasury_budget_refresh');
    assert.equal(factory.created[1].options.sessionRefresh.previousSessionId, 'sess-1');
    assert.equal(factory.created[1].options.sessionRefresh.replayedMessages, 2);
    assert.match(factory.created[1].options.sessionRefresh.recordedAt, /^\d{4}-\d{2}-\d{2}T/);
    assert.equal(second.response, 'response:second');
    assert.equal(second.sessionId, 'sess-2');
    assert.ok(second.traceId);
    assert.notEqual(second.traceId, first.traceId);
    assert.equal(second.telemetry.toolCalls.total, 1);
    assert.equal(second.promptReport.historySource, 'conversation_history');
    assert.equal(second.treasury.requestId, 'treasury-turn-1');
    assert.equal(second.sessionRefresh.reason, 'treasury_budget_refresh');
    assert.equal(second.sessionRefresh.previousSessionId, 'sess-1');
    assert.equal(second.sessionRefresh.replayedMessages, 2);
    assert.equal(second.sessionRefresh.sessionId, 'sess-2');
    assert.equal(
      second.sessionRefresh.recordedAt,
      factory.created[1].options.sessionRefresh.recordedAt,
    );
    assert.deepEqual(partials, ['delta:first']);
    assert.deepEqual(thinking, ['Thinking first']);
    assert.deepEqual(tools, ['mcp__stateset-commerce__list_orders']);
    assert.ok(events.includes('prompt_report'));
    assert.ok(events.includes('tool_execution_start'));
    assert.ok(events.includes('message_update'));
    assert.equal(transport.getSessionId(), 'sess-2');

    assert.deepEqual(factory.closed, ['sess-1']);
    transport.reset('test complete');
    assert.deepEqual(factory.closed, ['sess-1', 'sess-2']);
  });

  it('falls back to runAgentLoop for unsupported providers and resets the persistent session on config changes', async () => {
    const factory = createPersistentSessionFactory();
    const fallbackCalls = [];
    const transport = createChatTransport({
      createSessionImpl: factory.createSessionImpl,
      runAgentLoopImpl: async (options) => {
        fallbackCalls.push(options);
        return {
          response: `fallback:${options.request}`,
          sessionId: 'fallback-session',
          promptReport: { historySource: 'none' },
        };
      },
      settingsLoader: () => ({ memory: { enabled: false } }),
    });

    await transport.query({
      request: 'first',
      provider: 'claude',
      model: 'claude-test',
      dbPath: './store.db',
      thinkLevel: 'off',
      enableMemory: false,
    });
    await transport.query({
      request: 'second',
      provider: 'claude',
      model: 'claude-test',
      dbPath: './store.db',
      thinkLevel: 'high',
      enableMemory: false,
    });

    assert.equal(factory.created.length, 2);
    assert.deepEqual(factory.closed, ['sess-1']);

    const fallback = await transport.query({
      request: 'third',
      provider: 'openai',
      model: 'gpt-test',
      dbPath: './store.db',
      thinkLevel: 'high',
      enableMemory: true,
    });

    assert.equal(fallback.response, 'fallback:third');
    assert.equal(fallbackCalls.length, 1);
    assert.equal(transport.isPersistentActive(), false);
    assert.deepEqual(factory.closed, ['sess-1', 'sess-2']);
  });
});
