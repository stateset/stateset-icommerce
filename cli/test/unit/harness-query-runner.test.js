import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { createRunQuery } from '../../src/harness/query-runner.js';

function makeDeps({ messages, hooks = null, overrides = {} }) {
  const events = [];
  const toolCalls = [];
  const state = { response: null, sessionId: undefined, usage: null, attemptStarts: 0 };
  const telemEvents = [];
  const toolLogs = [];
  const deps = {
    executeQuery: async function* () {
      for (const message of messages) {
        yield message;
      }
    },
    options: { model: 'base-model', maxTurns: 5 },
    requestWithHistory: 'hello',
    resumeSessionId: null,
    watchdogTimeoutMs: null,
    effectiveAbortController: null,
    effectiveProvider: 'claude',
    telem: {
      logCustomEvent: (type, data) => telemEvents.push({ type, data }),
      logToolCall: (name, input, result, duration) =>
        toolLogs.push({ name, input, result, duration }),
    },
    onEvent: (event) => events.push(event),
    redactEventText: (text) => text,
    redactEventValue: (value) => value,
    hooks,
    privacySettings: { redactLogs: false },
    streaming: false,
    onToolCall: (call) => toolCalls.push(call),
    onPartialMessage: null,
    onThinkingBlock: null,
    syncState: {
      onAttemptStart: () => {
        state.attemptStarts++;
        state.response = '';
        state.sessionId = null;
      },
      getSessionId: () => state.sessionId,
      setSessionId: (id) => {
        state.sessionId = id;
      },
      setResponse: (text) => {
        state.response = text;
      },
      setUsage: (usage) => {
        state.usage = usage;
      },
    },
    ...overrides,
  };
  return { deps, events, toolCalls, state, telemEvents, toolLogs };
}

describe('harness/query-runner createRunQuery', () => {
  it('assembles text blocks and final result, syncing state for error paths', async () => {
    const messages = [
      {
        type: 'assistant',
        sessionId: 'sess-1',
        message: { content: [{ type: 'text', text: 'partial ' }] },
      },
      { type: 'result', result: 'final answer', total_cost_usd: 0.05, usage: { input_tokens: 10 } },
    ];
    const { deps, events, state } = makeDeps({ messages });
    const runQuery = createRunQuery(deps);
    const results = await runQuery('model-x');

    assert.equal(results.response, 'final answer');
    assert.equal(results.sessionId, 'sess-1');
    assert.equal(results.totalCost, 0.05);
    assert.equal(results.usage.inputTokens, 10);
    assert.equal(state.response, 'final answer');
    assert.equal(state.sessionId, 'sess-1');
    assert.equal(state.attemptStarts, 1);
    const types = events.map((e) => e.type);
    assert.deepEqual(types, ['message_start', 'message_end']);
  });

  it('pairs tool calls with tool results by tool_use_id and emits execution events', async () => {
    const messages = [
      {
        type: 'assistant',
        message: {
          content: [{ type: 'tool_use', id: 'tu-1', name: 'get_order', input: { id: '42' } }],
        },
      },
      {
        type: 'user',
        parent_tool_use_id: 'tu-1',
        tool_use_result: { content: 'order data', is_error: false },
      },
      { type: 'result', result: 'done' },
    ];
    const { deps, events, toolCalls, toolLogs } = makeDeps({ messages });
    const results = await createRunQuery(deps)('model-x');

    assert.equal(results.toolResults.length, 1);
    assert.equal(results.toolResults[0].toolCall.name, 'get_order');
    assert.deepEqual(results.toolResults[0].result, { content: 'order data', is_error: false });
    assert.equal(typeof results.toolResults[0].duration, 'number');
    assert.equal(toolCalls.length, 1);
    assert.equal(toolLogs.length, 1);
    const types = events.map((e) => e.type);
    assert.deepEqual(types, [
      'message_start',
      'tool_execution_start',
      'tool_execution_end',
      'message_end',
    ]);
  });

  it('lets the tool_result_persist hook replace a tool result', async () => {
    const messages = [
      {
        type: 'assistant',
        message: { content: [{ type: 'tool_use', id: 'tu-1', name: 'get_order', input: {} }] },
      },
      { type: 'user', parent_tool_use_id: 'tu-1', tool_use_result: { content: 'raw' } },
      { type: 'result', result: 'done' },
    ];
    const hooks = {
      hasHooks: (name) => name === 'tool_result_persist',
      run: async (name, ctx) => {
        assert.equal(name, 'tool_result_persist');
        assert.equal(ctx.tool, 'get_order');
        return { result: { content: 'sanitized' } };
      },
    };
    const { deps } = makeDeps({ messages, hooks });
    const results = await createRunQuery(deps)('model-x');
    assert.deepEqual(results.toolResults[0].result, { content: 'sanitized' });
  });

  it('flags budgetExceeded and error fields for error_max_budget_usd results', async () => {
    const messages = [
      { type: 'result', subtype: 'error_max_budget_usd', errors: ['budget blown'] },
    ];
    const { deps } = makeDeps({ messages });
    const results = await createRunQuery(deps)('model-x');
    assert.equal(results.budgetExceeded, true);
    assert.equal(results.errorType, 'error_max_budget_usd');
    assert.equal(results.error, 'budget blown');
  });

  it('falls back to the subtype when an error result has no error list', async () => {
    const messages = [{ type: 'result', subtype: 'error_max_turns' }];
    const { deps } = makeDeps({ messages });
    const results = await createRunQuery(deps)('model-x');
    assert.equal(results.errorType, 'error_max_turns');
    assert.equal(results.error, 'error_max_turns');
    assert.equal(results.budgetExceeded, false);
  });

  it('matches an unkeyed tool result to the first pending tool call', async () => {
    const messages = [
      {
        type: 'assistant',
        message: {
          content: [
            { type: 'tool_use', id: 'tu-1', name: 'first', input: {} },
            { type: 'tool_use', id: 'tu-2', name: 'second', input: {} },
          ],
        },
      },
      { type: 'user', tool_use_result: { content: 'for first' } },
      { type: 'result', result: 'done' },
    ];
    const { deps } = makeDeps({ messages });
    const results = await createRunQuery(deps)('model-x');
    assert.deepEqual(results.toolResults[0].result, { content: 'for first' });
    assert.equal(results.toolResults[1].result, null);
  });

  it('emits streaming message_update deltas and forwards unknown messages to onPartialMessage', async () => {
    const partials = [];
    const messages = [
      { type: 'assistant', message: { content: [{ type: 'text', text: 'chunk' }] } },
      { type: 'system', subtype: 'status' },
      { type: 'result', result: 'chunk' },
    ];
    const { deps, events } = makeDeps({
      messages,
      overrides: { streaming: true, onPartialMessage: (m) => partials.push(m) },
    });
    await createRunQuery(deps)('model-x');
    assert.ok(events.some((e) => e.type === 'message_update' && e.delta === 'chunk'));
    assert.equal(partials.length, 1);
    assert.equal(partials[0].type, 'system');
  });

  it('emits message_start even for a result-only stream with no assistant text', async () => {
    const { deps, events } = makeDeps({ messages: [{ type: 'result' }] });
    const results = await createRunQuery(deps)('model-x');
    assert.equal(results.response, '');
    assert.deepEqual(
      events.map((e) => e.type),
      ['message_start'],
    );
  });

  it('routes thinking blocks to onThinkingBlock', async () => {
    const thinking = [];
    const messages = [
      { type: 'assistant', message: { content: [{ type: 'thinking', thinking: 'hmm' }] } },
      { type: 'result', result: 'ok' },
    ];
    const { deps } = makeDeps({
      messages,
      overrides: { onThinkingBlock: (block) => thinking.push(block) },
    });
    await createRunQuery(deps)('model-x');
    assert.equal(thinking.length, 1);
    assert.equal(thinking[0].thinking, 'hmm');
  });
});
