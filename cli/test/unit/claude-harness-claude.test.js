import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { existsSync, unlinkSync } from 'node:fs';

import {
  createAgentSession,
  createAgentStreamSession,
  runAgentLoop,
  runAgentStream,
} from '../../src/claude-harness.js';
import { X402_MCP_TOOL_NAMES } from '../../src/x402-mcp-server.js';

function newDbPath() {
  return join(
    tmpdir(),
    `stateset-claude-harness-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}.db`,
  );
}

function cleanupDb(dbPath) {
  if (!dbPath) return;
  for (const suffix of ['', '-wal', '-shm']) {
    const candidate = `${dbPath}${suffix}`;
    if (existsSync(candidate)) {
      try {
        unlinkSync(candidate);
      } catch {
        // Ignore cleanup errors.
      }
    }
  }
}

function createSessionStore() {
  const runs = [];
  const upserts = [];
  const summaries = [];
  return {
    runs,
    upserts,
    summaries,
    get() {
      return null;
    },
    recordRun(sessionId, payload) {
      runs.push({ sessionId, payload });
    },
    upsert(sessionId, payload) {
      upserts.push({ sessionId, payload });
    },
    appendSummary(sessionId, summary) {
      summaries.push({ sessionId, summary });
    },
  };
}

function createAbortError() {
  const error = new Error('aborted');
  error.name = 'AbortError';
  error.code = 'ABORT_ERR';
  return error;
}

function waitForAbort(signal) {
  return new Promise((_, reject) => {
    const keepAlive = setInterval(() => {}, 50);
    const finish = () => clearInterval(keepAlive);

    if (signal.aborted) {
      finish();
      reject(createAbortError());
      return;
    }

    signal.addEventListener(
      'abort',
      () => {
        finish();
        reject(createAbortError());
      },
      { once: true },
    );
  });
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitFor(predicate, { timeoutMs = 500, intervalMs = 5 } = {}) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    if (predicate()) return;
    await delay(intervalMs);
  }
  throw new Error('Timed out waiting for condition');
}

let claudeHarnessTestGate = Promise.resolve();

function itSerial(name, fn) {
  it(name, async (t) => {
    const previous = claudeHarnessTestGate;
    let release;
    claudeHarnessTestGate = new Promise((resolve) => {
      release = resolve;
    });

    await previous;
    try {
      return await fn(t);
    } finally {
      release();
    }
  });
}

describe('Claude harness paths', { concurrency: false }, () => {
  itSerial(
    'runs the Claude SDK path with tool events, prompt reporting, and session persistence',
    async () => {
      const dbPath = newDbPath();
      const sessionStore = createSessionStore();
      const events = [];

      const queryImpl = () =>
        (async function* () {
          yield {
            sessionId: 'sess-claude-success',
            type: 'assistant',
            message: {
              content: [
                { type: 'text', text: 'Working on it.' },
                {
                  type: 'tool_use',
                  id: 'tool-1',
                  name: 'mcp__stateset-commerce__list_orders',
                  input: { limit: 1 },
                },
              ],
            },
          };
          yield {
            type: 'user',
            tool_use_result: {
              tool_use_id: 'tool-1',
              name: 'mcp__stateset-commerce__list_orders',
              content: [{ type: 'text', text: '[]' }],
              is_error: false,
            },
          };
          yield {
            type: 'result',
            result: 'Final Claude response',
            total_cost_usd: 0.1234,
            usage: { input_tokens: 11, output_tokens: 7 },
          };
        })();

      const result = await runAgentLoop({
        request: 'List recent orders',
        provider: 'claude',
        model: 'claude-test',
        dbPath,
        enableSync: false,
        enableMemory: false,
        streaming: true,
        sessionStore,
        queryImpl,
        privacy: {
          redactLogs: false,
          redactMemory: false,
        },
        onEvent: (event) => events.push(event),
      });

      assert.equal(result.response, 'Final Claude response');
      assert.equal(result.sessionId, 'sess-claude-success');
      assert.equal(result.cost, 0.1234);
      assert.equal(result.toolResults.length, 1);
      assert.equal(result.toolResults[0].toolCall.id, 'tool-1');
      assert.equal(result.toolResults[0].duration >= 0, true);
      assert.ok(result.promptReport);
      assert.equal(result.promptReport.historySource, 'none');
      assert.ok(result.promptReport.totalInputTokens > 0);

      const eventTypes = events.map((event) => event.type);
      for (const expectedType of [
        'agent_start',
        'turn_start',
        'prompt_report',
        'message_start',
        'message_update',
        'tool_execution_start',
        'tool_execution_end',
        'message_end',
        'turn_end',
        'agent_end',
      ]) {
        assert.ok(eventTypes.includes(expectedType), `expected event: ${expectedType}`);
      }

      assert.equal(sessionStore.runs.length, 1);
      assert.equal(sessionStore.runs[0].sessionId, 'sess-claude-success');
      assert.equal(sessionStore.runs[0].payload.lastError, null);
      assert.equal(sessionStore.runs[0].payload.inputTokens, 11);
      assert.equal(sessionStore.runs[0].payload.outputTokens, 7);
      assert.equal(sessionStore.runs[0].payload.totalTokens, 18);
      assert.equal(sessionStore.runs[0].payload.lastCostUsd, 0.1234);
      assert.ok(sessionStore.runs[0].payload.promptReport);

      cleanupDb(dbPath);
    },
  );

  itSerial('passes autonomousEngine into the Claude MCP server for runAgentLoop', async () => {
    const dbPath = newDbPath();
    const delegated = [];

    const queryImpl = ({ options }) =>
      (async function* () {
        const delegateResult = await options.mcpServers['stateset-commerce'].executeTool(
          'delegate_to_agent',
          {
            agent_name: 'orders',
            task_description: 'List pending orders',
            context: { customerId: 'cust_001' },
          },
          { includeHooks: false },
        );

        assert.equal(delegateResult.success, true);
        assert.equal(delegateResult.status, 'success');
        assert.equal(delegateResult.result.success, true);
        assert.equal(delegateResult.result.delegatedTo, 'orders');

        yield {
          sessionId: 'sess-claude-delegate',
          type: 'result',
          result: 'Delegation complete',
        };
      })();

    const result = await runAgentLoop({
      request: 'Delegate to orders',
      provider: 'claude',
      model: 'claude-test',
      dbPath,
      allowApply: true,
      enableSync: false,
      enableMemory: false,
      queryImpl,
      autonomousEngine: {
        async executeAgentRequest(agentName, taskDescription, context) {
          delegated.push({ agentName, taskDescription, context });
          return { status: 'completed', agentName, taskDescription, context };
        },
      },
    });

    assert.equal(result.response, 'Delegation complete');
    assert.deepEqual(delegated, [
      {
        agentName: 'orders',
        taskDescription: 'List pending orders',
        context: { customerId: 'cust_001' },
      },
    ]);

    cleanupDb(dbPath);
  });

  itSerial(
    'raises a watchdog timeout on the Claude SDK path and persists the failure',
    async () => {
      const dbPath = newDbPath();
      const sessionStore = createSessionStore();
      const events = [];

      const queryImpl = ({ options }) =>
        (async function* () {
          await waitForAbort(options.abortController.signal);
        })();

      await assert.rejects(
        () =>
          runAgentLoop({
            request: 'List recent orders',
            provider: 'claude',
            model: 'claude-test',
            dbPath,
            resumeSessionId: 'sess-claude-timeout',
            enableSync: false,
            enableMemory: false,
            enableFallback: false,
            queryImpl,
            sessionStore,
            settings: {
              watchdog: {
                enabled: true,
                freshInactivityMs: 20,
                resumeInactivityMs: 20,
              },
            },
            retry: {
              enabled: false,
              maxRetries: 0,
            },
            onEvent: (event) => events.push(event),
          }),
        (error) => {
          assert.equal(error.code || error.cause?.code, 'WATCHDOG_TIMEOUT');
          assert.match(error.message, /No Claude SDK activity while resuming session after 20ms/);
          return true;
        },
      );

      const eventTypes = events.map((event) => event.type);
      assert.ok(eventTypes.includes('watchdog_timeout'));
      assert.ok(eventTypes.includes('agent_end'));
      assert.equal(sessionStore.runs.length, 1);
      assert.equal(sessionStore.runs[0].sessionId, 'sess-claude-timeout');
      assert.equal(sessionStore.runs[0].payload.lastErrorCode, 'WATCHDOG_TIMEOUT');
      assert.equal(sessionStore.runs[0].payload.abortedLastRun, true);

      cleanupDb(dbPath);
    },
  );

  itSerial(
    'runs the Claude streaming path with prompt reporting and session persistence',
    async () => {
      const dbPath = newDbPath();
      const sessionStore = createSessionStore();
      const events = [];
      const received = [];

      const queryImpl = () =>
        (async function* () {
          yield {
            sessionId: 'sess-stream-success',
            type: 'assistant',
            message: {
              content: [{ type: 'text', text: 'Streaming response' }],
            },
          };
          yield {
            type: 'result',
            result: 'Streaming response complete',
          };
        })();

      for await (const message of runAgentStream({
        request: 'Stream recent orders',
        provider: 'claude',
        model: 'claude-test',
        dbPath,
        enableSync: false,
        sessionStore,
        queryImpl,
        privacy: {
          redactLogs: false,
          redactMemory: false,
        },
        onEvent: (event) => events.push(event),
      })) {
        received.push(message);
      }

      assert.equal(received.length, 2);
      assert.equal(received[0].sessionId, 'sess-stream-success');
      assert.equal(received[1].result, 'Streaming response complete');

      const eventTypes = events.map((event) => event.type);
      for (const expectedType of [
        'prompt_report',
        'message_start',
        'message_update',
        'message_end',
        'turn_end',
        'agent_end',
      ]) {
        assert.ok(eventTypes.includes(expectedType), `expected event: ${expectedType}`);
      }

      assert.equal(sessionStore.upserts.length, 1);
      assert.equal(sessionStore.upserts[0].sessionId, 'sess-stream-success');
      assert.equal(sessionStore.upserts[0].payload.lastResponse, 'Streaming response complete');
      assert.equal(sessionStore.upserts[0].payload.lastError, null);
      assert.ok(sessionStore.upserts[0].payload.promptReport);

      cleanupDb(dbPath);
    },
  );

  itSerial('passes autonomousEngine into the Claude MCP server for runAgentStream', async () => {
    const dbPath = newDbPath();
    const received = [];
    const delegated = [];

    const queryImpl = ({ options }) =>
      (async function* () {
        const delegateResult = await options.mcpServers['stateset-commerce'].executeTool(
          'delegate_to_agent',
          {
            agent_name: 'payments',
            task_description: 'Review payment intents',
            context: { orderId: 'ord_123' },
          },
          { includeHooks: false },
        );

        assert.equal(delegateResult.success, true);
        assert.equal(delegateResult.result.success, true);
        assert.equal(delegateResult.result.delegatedTo, 'payments');

        yield {
          sessionId: 'sess-stream-delegate',
          type: 'result',
          result: 'Streaming delegation complete',
        };
      })();

    for await (const message of runAgentStream({
      request: 'Delegate streaming work',
      provider: 'claude',
      model: 'claude-test',
      dbPath,
      allowApply: true,
      enableSync: false,
      queryImpl,
      autonomousEngine: {
        async executeAgentRequest(agentName, taskDescription, context) {
          delegated.push({ agentName, taskDescription, context });
          return { status: 'completed', agentName, taskDescription, context };
        },
      },
    })) {
      received.push(message);
    }

    assert.ok(
      received.some(
        (message) =>
          message.type === 'result' && message.result === 'Streaming delegation complete',
      ),
    );
    assert.deepEqual(delegated, [
      {
        agentName: 'payments',
        taskDescription: 'Review payment intents',
        context: { orderId: 'ord_123' },
      },
    ]);

    cleanupDb(dbPath);
  });

  itSerial('passes autonomousEngine through createAgentSession queries', async () => {
    const dbPath = newDbPath();
    const delegated = [];

    const queryImpl = ({ options }) =>
      (async function* () {
        const delegateResult = await options.mcpServers['stateset-commerce'].executeTool(
          'delegate_to_agent',
          {
            agent_name: 'inventory',
            task_description: 'Check low-stock SKUs',
            context: { threshold: 5 },
          },
          { includeHooks: false },
        );

        assert.equal(delegateResult.success, true);
        assert.equal(delegateResult.result.success, true);
        assert.equal(delegateResult.result.delegatedTo, 'inventory');

        yield {
          sessionId: 'sess-query-delegate',
          type: 'result',
          result: 'Query delegation complete',
        };
      })();

    const session = createAgentSession({
      dbPath,
      provider: 'claude',
      model: 'claude-test',
      allowApply: true,
      enableSync: false,
      enableMemory: false,
      queryImpl,
      autonomousEngine: {
        async executeAgentRequest(agentName, taskDescription, context) {
          delegated.push({ agentName, taskDescription, context });
          return { status: 'completed', agentName, taskDescription, context };
        },
      },
    });

    const result = await session.query('Delegate inventory review');

    assert.equal(result.response, 'Query delegation complete');
    assert.equal(session.getSessionId(), 'sess-query-delegate');
    assert.deepEqual(delegated, [
      {
        agentName: 'inventory',
        taskDescription: 'Check low-stock SKUs',
        context: { threshold: 5 },
      },
    ]);

    cleanupDb(dbPath);
  });

  itSerial(
    'raises a watchdog timeout on the Claude streaming path and records the failure',
    async () => {
      const dbPath = newDbPath();
      const sessionStore = createSessionStore();
      const events = [];

      const queryImpl = ({ options }) =>
        (async function* () {
          await waitForAbort(options.abortController.signal);
        })();

      await assert.rejects(
        async () => {
          for await (const _message of runAgentStream({
            request: 'Resume stream recent orders',
            provider: 'claude',
            model: 'claude-test',
            dbPath,
            resumeSessionId: 'sess-stream-timeout',
            enableSync: false,
            sessionStore,
            queryImpl,
            settings: {
              watchdog: {
                enabled: true,
                freshInactivityMs: 20,
                resumeInactivityMs: 20,
              },
            },
            onEvent: (event) => events.push(event),
          })) {
            // no-op
          }
        },
        (error) => {
          assert.equal(error.code, 'WATCHDOG_TIMEOUT');
          assert.match(error.message, /No Claude SDK activity while resuming session after 20ms/);
          return true;
        },
      );

      const eventTypes = events.map((event) => event.type);
      assert.ok(eventTypes.includes('watchdog_timeout'));
      assert.ok(eventTypes.includes('agent_end'));
      assert.equal(sessionStore.upserts.length, 1);
      assert.equal(sessionStore.upserts[0].sessionId, 'sess-stream-timeout');
      assert.equal(sessionStore.upserts[0].payload.lastErrorCode, 'WATCHDOG_TIMEOUT');
      assert.equal(sessionStore.upserts[0].payload.abortedLastRun, true);

      cleanupDb(dbPath);
    },
  );

  itSerial(
    'keeps interactive sessions idle-safe between turns and persists turn accounting',
    async () => {
      const dbPath = newDbPath();
      const sessionStore = createSessionStore();
      const events = [];
      const received = [];
      const savedMemory = [];
      const savedMarkdownMemory = [];
      let queryOptions = null;

      const queryImpl = ({ prompt, options }) =>
        (async function* () {
          queryOptions = options;
          const input = prompt[Symbol.asyncIterator]();

          const firstTurn = await input.next();
          assert.equal(firstTurn.done, false);
          assert.equal(firstTurn.value.message.content[0].text, 'First interactive turn');
          yield {
            sessionId: 'sess-interactive-timeout',
            type: 'assistant',
            message: {
              content: [
                { type: 'thinking', thinking: 'Checking order history' },
                { type: 'text', text: 'First turn response' },
                {
                  type: 'tool_use',
                  id: 'tool-1',
                  name: 'mcp__stateset-commerce__list_orders',
                  input: { limit: 1 },
                },
              ],
            },
          };
          yield {
            type: 'user',
            tool_use_result: {
              tool_use_id: 'tool-1',
              name: 'mcp__stateset-commerce__list_orders',
              content: [{ type: 'text', text: '[]' }],
              is_error: false,
            },
          };
          yield {
            type: 'result',
            result: 'First turn complete',
            total_cost_usd: 0.25,
            usage: {
              input_tokens: 10,
              output_tokens: 4,
              cache_read_tokens: 1,
              cache_write_tokens: 2,
            },
          };

          const secondTurn = await input.next();
          assert.equal(secondTurn.done, false);
          assert.equal(secondTurn.value.message.content[0].text, 'Second interactive turn');
          await waitForAbort(options.abortController.signal);
        })();

      const session = createAgentStreamSession({
        provider: 'claude',
        model: 'claude-test',
        dbPath,
        enableSync: false,
        maxBudgetUsd: 4.2,
        enableX402: true,
        enableMemory: true,
        queryImpl,
        memoryStore: {
          save(entry) {
            savedMemory.push(entry);
          },
        },
        markdownMemoryStore: {
          async save(entry) {
            savedMarkdownMemory.push(entry);
          },
        },
        sessionStore,
        settings: {
          watchdog: {
            enabled: true,
            freshInactivityMs: 20,
            resumeInactivityMs: 20,
          },
        },
        onEvent: (event) => events.push(event),
      });

      const consume = (async () => {
        for await (const message of session.stream()) {
          received.push(message);
        }
      })();

      await delay(40);
      assert.equal(
        events.filter((event) => event.type === 'watchdog_timeout').length,
        0,
        'session should remain idle before the first turn',
      );
      assert.equal(
        events.filter((event) => event.type === 'prompt_report').length,
        0,
        'session should not emit prompt reports before a turn starts',
      );

      session.send('First interactive turn');
      await waitFor(() =>
        received.some(
          (message) => message.type === 'result' && message.result === 'First turn complete',
        ),
      );

      assert.equal(queryOptions.maxBudgetUsd, 4.2);
      assert.ok(queryOptions.mcpServers['stateset-commerce']);
      assert.ok(queryOptions.mcpServers['stateset-x402']);
      for (const name of X402_MCP_TOOL_NAMES) {
        assert.ok(queryOptions.allowedTools.includes(`mcp__stateset-x402__${name}`));
      }

      const promptReportsAfterFirstTurn = events
        .filter((event) => event.type === 'prompt_report')
        .map((event) => event.report);
      assert.equal(promptReportsAfterFirstTurn.length, 1);
      assert.equal(promptReportsAfterFirstTurn[0].historySource, 'none');
      assert.equal(promptReportsAfterFirstTurn[0].historyInjected, false);
      assert.ok(promptReportsAfterFirstTurn[0].totalInputTokens > 0);
      assert.equal(session.getLastPromptReport().historySource, 'none');

      const firstTurnResult = session.getLastTurnResult();
      assert.equal(firstTurnResult.request, 'First interactive turn');
      assert.equal(firstTurnResult.response, 'First turn complete');
      assert.equal(firstTurnResult.cost, 0.25);
      assert.equal(firstTurnResult.budgetExceeded, false);
      assert.equal(firstTurnResult.usage.inputTokens, 10);
      assert.equal(firstTurnResult.usage.outputTokens, 4);
      assert.equal(firstTurnResult.usage.totalTokens, 14);
      assert.equal(firstTurnResult.usage.cacheReadTokens, 1);
      assert.equal(firstTurnResult.usage.cacheWriteTokens, 2);
      assert.equal(firstTurnResult.toolResults.length, 1);
      assert.equal(firstTurnResult.toolResults[0].toolCall.id, 'tool-1');
      assert.equal(firstTurnResult.toolResults[0].duration >= 0, true);
      assert.equal(firstTurnResult.promptReport.historySource, 'none');
      assert.equal(savedMemory.length, 1);
      assert.equal(savedMarkdownMemory.length, 1);
      assert.match(savedMemory[0].summary, /First interactive turn/);
      assert.match(savedMemory[0].summary, /First turn complete/);
      assert.deepEqual(savedMemory[0].facts, ['Used tool: mcp__stateset-commerce__list_orders']);
      assert.equal(savedMemory[0].sessionId, 'sess-interactive-timeout');

      await delay(40);
      assert.equal(
        events.filter((event) => event.type === 'watchdog_timeout').length,
        0,
        'session should remain idle between completed turns',
      );

      session.send('Second interactive turn');
      await assert.rejects(consume, (error) => {
        assert.equal(error.code, 'WATCHDOG_TIMEOUT');
        assert.match(error.message, /No Claude SDK activity received after 20ms/);
        return true;
      });

      const promptReports = events
        .filter((event) => event.type === 'prompt_report')
        .map((event) => event.report);
      assert.equal(promptReports.length, 2);
      assert.equal(promptReports[1].historySource, 'live_session');
      assert.equal(promptReports[1].historyInjected, true);
      assert.equal(promptReports[1].historyMessagesInjected, 2);
      assert.equal(session.getLastPromptReport().historySource, 'live_session');

      const finalTurnResult = session.getLastTurnResult();
      assert.equal(finalTurnResult.request, 'Second interactive turn');
      assert.equal(finalTurnResult.response, null);
      assert.equal(finalTurnResult.errorCode, 'WATCHDOG_TIMEOUT');
      assert.equal(finalTurnResult.promptReport.historySource, 'live_session');
      assert.equal(finalTurnResult.usage.totalTokens, null);

      const eventTypes = events.map((event) => event.type);
      assert.ok(eventTypes.includes('thinking_block'));
      assert.ok(eventTypes.includes('watchdog_timeout'));
      assert.ok(eventTypes.includes('agent_end'));
      const firstTurnEnd = events.find((event) => event.type === 'turn_end');
      assert.equal(firstTurnEnd.cost, 0.25);
      assert.equal(firstTurnEnd.budgetExceeded, false);

      assert.equal(sessionStore.runs.length, 2);
      assert.equal(sessionStore.upserts.length, 0);
      assert.equal(sessionStore.runs[0].sessionId, 'sess-interactive-timeout');
      assert.equal(sessionStore.runs[0].payload.lastRequest, 'First interactive turn');
      assert.equal(sessionStore.runs[0].payload.lastResponse, 'First turn complete');
      assert.equal(sessionStore.runs[0].payload.lastError, null);
      assert.equal(sessionStore.runs[0].payload.lastCostUsd, 0.25);
      assert.equal(sessionStore.runs[0].payload.inputTokens, 10);
      assert.equal(sessionStore.runs[0].payload.outputTokens, 4);
      assert.equal(sessionStore.runs[0].payload.totalTokens, 14);
      assert.equal(sessionStore.runs[0].payload.cacheReadTokens, 1);
      assert.equal(sessionStore.runs[0].payload.cacheWriteTokens, 2);
      assert.ok(sessionStore.runs[0].payload.promptReport);
      assert.equal(sessionStore.runs[0].payload.promptReport.historySource, 'none');
      assert.equal(sessionStore.runs[1].sessionId, 'sess-interactive-timeout');
      assert.equal(sessionStore.runs[1].payload.lastRequest, 'Second interactive turn');
      assert.equal(sessionStore.runs[1].payload.lastResponse, null);
      assert.equal(sessionStore.runs[1].payload.lastErrorCode, 'WATCHDOG_TIMEOUT');
      assert.equal(sessionStore.runs[1].payload.abortedLastRun, true);
      assert.equal(savedMemory.length, 1);
      assert.equal(savedMarkdownMemory.length, 1);
      assert.ok(sessionStore.runs[1].payload.promptReport);
      assert.equal(sessionStore.runs[1].payload.promptReport.historySource, 'live_session');
      assert.equal(sessionStore.runs[1].payload.promptReport.historyInjected, true);

      cleanupDb(dbPath);
    },
  );

  itSerial('passes autonomousEngine into interactive session MCP servers', async () => {
    const dbPath = newDbPath();
    const received = [];
    const delegated = [];

    const queryImpl = ({ prompt, options }) =>
      (async function* () {
        const input = prompt[Symbol.asyncIterator]();
        const firstTurn = await input.next();
        assert.equal(firstTurn.done, false);
        assert.equal(firstTurn.value.message.content[0].text, 'Delegate from interactive session');

        const delegateResult = await options.mcpServers['stateset-commerce'].executeTool(
          'delegate_to_agent',
          {
            agent_name: 'inventory',
            task_description: 'Check stock for SKU-42',
            context: { sku: 'SKU-42' },
          },
          { includeHooks: false },
        );

        assert.equal(delegateResult.success, true);
        assert.equal(delegateResult.result.success, true);
        assert.equal(delegateResult.result.delegatedTo, 'inventory');

        yield {
          sessionId: 'sess-interactive-delegate',
          type: 'result',
          result: 'Interactive delegation complete',
        };
      })();

    const session = createAgentStreamSession({
      provider: 'claude',
      model: 'claude-test',
      dbPath,
      allowApply: true,
      enableSync: false,
      queryImpl,
      autonomousEngine: {
        async executeAgentRequest(agentName, taskDescription, context) {
          delegated.push({ agentName, taskDescription, context });
          return { status: 'completed', agentName, taskDescription, context };
        },
      },
    });

    const consume = (async () => {
      for await (const message of session.stream()) {
        received.push(message);
      }
    })();

    session.send('Delegate from interactive session');
    await consume;

    assert.ok(
      received.some(
        (message) =>
          message.type === 'result' && message.result === 'Interactive delegation complete',
      ),
    );
    assert.deepEqual(delegated, [
      {
        agentName: 'inventory',
        taskDescription: 'Check stock for SKU-42',
        context: { sku: 'SKU-42' },
      },
    ]);

    cleanupDb(dbPath);
  });

  itSerial(
    'injects seeded conversation history into a fresh interactive Claude session',
    async () => {
      const dbPath = newDbPath();
      const events = [];
      const received = [];
      let firstPromptText = null;

      const queryImpl = ({ prompt }) =>
        (async function* () {
          const input = prompt[Symbol.asyncIterator]();
          const firstTurn = await input.next();
          assert.equal(firstTurn.done, false);
          firstPromptText = firstTurn.value.message.content[0].text;
          yield {
            sessionId: 'sess-interactive-history',
            type: 'assistant',
            message: {
              content: [{ type: 'text', text: 'History-aware response' }],
            },
          };
          yield {
            type: 'result',
            result: 'History-aware response complete',
            usage: {
              input_tokens: 12,
              output_tokens: 3,
            },
          };
        })();

      const session = createAgentStreamSession({
        provider: 'claude',
        model: 'claude-test',
        dbPath,
        enableSync: false,
        queryImpl,
        conversationHistory: [
          { role: 'user', content: 'Earlier question' },
          { role: 'assistant', content: 'Earlier answer' },
        ],
        onEvent: (event) => events.push(event),
      });

      const consume = (async () => {
        for await (const message of session.stream()) {
          received.push(message);
        }
      })();

      session.send('Follow-up question');
      await consume;

      assert.ok(
        received.some(
          (message) =>
            message.type === 'result' && message.result === 'History-aware response complete',
        ),
      );
      assert.match(firstPromptText, /Conversation history:/);
      assert.match(firstPromptText, /USER: Earlier question/);
      assert.match(firstPromptText, /ASSISTANT: Earlier answer/);
      assert.match(firstPromptText, /Current request:\nFollow-up question/);

      const promptReport = session.getLastPromptReport();
      assert.equal(promptReport.historySource, 'conversation_history');
      assert.equal(promptReport.historyInjected, true);
      assert.equal(promptReport.historyMessagesInjected, 2);
      assert.ok(
        events.some(
          (event) =>
            event.type === 'prompt_report' && event.report.historySource === 'conversation_history',
        ),
      );

      cleanupDb(dbPath);
    },
  );

  itSerial('records treasury billing for persistent interactive Claude turns', async () => {
    const dbPath = newDbPath();
    const sessionStore = createSessionStore();
    const events = [];
    const received = [];
    const recordedFees = [];
    let queryOptions = null;
    let treasuryLoadCalls = 0;

    const treasuryRuntime = {
      async loadTreasuryContext({ dbPath: treasuryDbPath }) {
        treasuryLoadCalls += 1;
        assert.equal(treasuryDbPath, undefined);
        return {
          registry: { mocked: true },
          store: {
            getBalance({ agentId, chainId, tokenSymbol, tokenDecimals }) {
              assert.equal(agentId, 'agent-treasury');
              assert.equal(chainId, 'set_chain');
              assert.equal(tokenSymbol, 'USDC');
              assert.equal(tokenDecimals, 6);
              return { balanceSmallest: '6750000' };
            },
          },
        };
      },
      resolveToken(chainId, tokenSymbol, registry) {
        assert.equal(chainId, 'set_chain');
        assert.equal(tokenSymbol, 'USDC');
        assert.deepEqual(registry, { mocked: true });
        return { symbol: 'USDC', decimals: 6 };
      },
      async recordFee(payload, ctx) {
        recordedFees.push({ payload, ctx });
        return {
          event_id: 'evt-treasury-1',
          amount_display: '0.42',
          amount_smallest: '420000',
          token_symbol: payload.tokenSymbol,
          chain_id: payload.chainId,
        };
      },
      fromSmallestUnit(balanceSmallest, decimals) {
        assert.equal(balanceSmallest, '6750000');
        assert.equal(decimals, 6);
        return '6.75';
      },
      getIdentity() {
        throw new Error('unexpected getIdentity call');
      },
    };

    const queryImpl = ({ prompt, options }) =>
      (async function* () {
        queryOptions = options;
        const input = prompt[Symbol.asyncIterator]();
        const firstTurn = await input.next();
        assert.equal(firstTurn.done, false);
        assert.equal(firstTurn.value.message.content[0].text, 'Treasury turn');
        yield {
          sessionId: 'sess-interactive-treasury',
          type: 'assistant',
          message: {
            content: [{ type: 'text', text: 'Treasury response' }],
          },
        };
        yield {
          type: 'result',
          result: 'Treasury turn complete',
          total_cost_usd: 0.42,
          usage: {
            input_tokens: 8,
            output_tokens: 3,
          },
        };
      })();

    const session = createAgentStreamSession({
      provider: 'claude',
      model: 'claude-test',
      dbPath,
      enableSync: false,
      sessionRefresh: {
        reason: 'treasury_budget_refresh',
        previousSessionId: 'sess-old',
        replayedMessages: 2,
        recordedAt: '2026-03-23T12:34:56.000Z',
      },
      maxBudgetUsd: 10,
      queryImpl,
      treasury: {
        enabled: true,
        chainId: 'set_chain',
        tokenSymbol: 'USDC',
        agentId: 'agent-treasury',
        chargeLlm: true,
      },
      treasuryRuntime,
      sessionStore,
      onEvent: (event) => events.push(event),
    });

    const consume = (async () => {
      for await (const message of session.stream()) {
        received.push(message);
      }
    })();

    session.send('Treasury turn');
    await consume;

    assert.ok(
      received.some(
        (message) => message.type === 'result' && message.result === 'Treasury turn complete',
      ),
    );
    assert.equal(treasuryLoadCalls, 1);
    assert.equal(queryOptions.maxBudgetUsd, 6.75);
    assert.equal(recordedFees.length, 1);
    assert.equal(recordedFees[0].payload.agentId, 'agent-treasury');
    assert.equal(recordedFees[0].payload.chainId, 'set_chain');
    assert.equal(recordedFees[0].payload.tokenSymbol, 'USDC');
    assert.equal(recordedFees[0].payload.amount, 0.42);
    assert.equal(recordedFees[0].payload.source, 'llm');
    assert.equal(recordedFees[0].payload.sessionId, 'sess-interactive-treasury');
    assert.equal(recordedFees[0].payload.toolName, 'llm_inference');
    assert.equal(recordedFees[0].payload.metadata.provider, 'claude');
    assert.equal(recordedFees[0].payload.metadata.model, 'claude-test');
    assert.equal(recordedFees[0].payload.metadata.costUsd, 0.42);
    assert.equal(recordedFees[0].payload.metadata.usage.inputTokens, 8);
    assert.equal(recordedFees[0].payload.metadata.usage.outputTokens, 3);
    assert.equal(recordedFees[0].payload.metadata.usage.totalTokens, 11);
    assert.ok(recordedFees[0].payload.requestId);
    assert.equal(recordedFees[0].payload.taskId, recordedFees[0].payload.requestId);

    const turnResult = session.getLastTurnResult();
    assert.equal(turnResult.request, 'Treasury turn');
    assert.equal(turnResult.response, 'Treasury turn complete');
    assert.equal(turnResult.cost, 0.42);
    assert.equal(turnResult.treasury.requestId, recordedFees[0].payload.requestId);
    assert.equal(turnResult.treasury.charge.eventId, 'evt-treasury-1');
    assert.equal(turnResult.treasury.charge.amount, '0.42');
    assert.equal(turnResult.treasury.charge.token, 'USDC');
    assert.equal(turnResult.treasury.charge.chainId, 'set_chain');
    assert.equal(turnResult.treasury.identity, null);
    assert.deepEqual(turnResult.sessionRefresh, {
      reason: 'treasury_budget_refresh',
      previousSessionId: 'sess-old',
      replayedMessages: 2,
      recordedAt: '2026-03-23T12:34:56.000Z',
    });

    const turnEnd = events.find((event) => event.type === 'turn_end');
    assert.equal(turnEnd.treasury.requestId, recordedFees[0].payload.requestId);
    assert.equal(turnEnd.treasury.charge.eventId, 'evt-treasury-1');
    assert.equal(turnEnd.sessionRefresh.reason, 'treasury_budget_refresh');
    assert.equal(turnEnd.sessionRefresh.previousSessionId, 'sess-old');
    const agentEnd = [...events].reverse().find((event) => event.type === 'agent_end');
    assert.equal(agentEnd.treasury.charge.eventId, 'evt-treasury-1');
    assert.equal(agentEnd.sessionRefresh.reason, 'treasury_budget_refresh');

    assert.equal(sessionStore.runs.length, 1);
    assert.equal(sessionStore.runs[0].sessionId, 'sess-interactive-treasury');
    assert.equal(sessionStore.runs[0].payload.lastCostUsd, 0.42);
    assert.deepEqual(sessionStore.runs[0].payload.sessionRefresh, {
      reason: 'treasury_budget_refresh',
      previousSessionId: 'sess-old',
      replayedMessages: 2,
      recordedAt: '2026-03-23T12:34:56.000Z',
    });

    cleanupDb(dbPath);
  });
});
