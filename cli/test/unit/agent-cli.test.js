import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { buildAgentCliOptions, runAgentCli } from '../../src/utils/agent-cli.js';

function createConsoleCapture() {
  const logs = [];
  const errors = [];

  return {
    logs,
    errors,
    log: (...args) => logs.push(args.join(' ')),
    error: (...args) => errors.push(args.join(' ')),
  };
}

function createOutputStub() {
  return {
    toolCall: (name, input) =>
      `TOOL ${name.replace('mcp__stateset-commerce__', '')}(${JSON.stringify(input)})`,
    promptReport: (report) => `PROMPT ${report.totalInputTokens}`,
    dim: (value) => value,
    bold: (value) => value,
    yellow: (value) => value,
  };
}

describe('agent-cli utils', () => {
  it('builds write-capable and read-only option sets', () => {
    const writeOptions = buildAgentCliOptions({ allowApply: true });
    const readOnlyOptions = buildAgentCliOptions({ allowApply: false });

    assert.ok(writeOptions.apply);
    assert.ok(writeOptions.yes);
    assert.ok(writeOptions.stats);
    assert.ok(!readOnlyOptions.apply);
    assert.ok(!readOnlyOptions.yes);
    assert.ok(readOnlyOptions.stats);
  });

  it('includes telemetry and prompt reports in json output when --stats is enabled', async () => {
    const io = createConsoleCapture();
    const captured = {};

    const exitCode = await runAgentCli(
      {
        agent: 'orders',
        commandName: 'stateset-orders',
        title: 'StateSet Orders Agent',
        icon: '📦',
        help: 'HELP',
      },
      {
        argv: ['node', 'stateset-orders', '--json', '--stats', 'list', 'orders'],
        console: io,
        stdin: { isTTY: false },
        runAgentLoopFn: async (options) => {
          captured.options = options;
          return {
            sessionId: 'sess-123',
            traceId: 'trace-abc',
            response: 'ok',
            provider: 'anthropic',
            usedModel: 'claude-test',
            cost: 0.42,
            budgetExceeded: false,
            telemetry: {
              duration: 125,
              toolCalls: { total: 1, successRate: '100%' },
              avgToolDuration: 55,
            },
            promptReport: { totalInputTokens: 321 },
            toolResults: [
              {
                toolCall: { name: 'mcp__stateset-commerce__list_orders', input: { limit: 1 } },
                result: [{ id: 'ord-1' }],
              },
            ],
          };
        },
        createConfirmHandlerFn: (options) => {
          captured.confirm = options;
          return 'confirm-handler';
        },
        createOutputFn: createOutputStub,
      },
    );

    assert.equal(exitCode, 0);
    assert.equal(captured.options.allowApply, false);
    assert.equal(captured.options.onConfirmRequired, 'confirm-handler');
    assert.equal(captured.confirm.assumeYes, false);

    const payload = JSON.parse(io.logs.at(-1));
    assert.equal(payload.telemetry.duration, 125);
    assert.equal(payload.promptReport.totalInputTokens, 321);
    assert.equal(payload.model, 'claude-test');
  });

  it('preserves read-only agent behavior without confirm handlers', async () => {
    const io = createConsoleCapture();
    let confirmCalled = false;
    let capturedOptions;

    const exitCode = await runAgentCli(
      {
        agent: 'analytics',
        commandName: 'stateset-analytics',
        title: 'StateSet Analytics Agent',
        icon: '📊',
        help: 'HELP',
        allowApply: false,
        modeLabel: '👁️  Read-only (analytics)',
      },
      {
        argv: ['node', 'stateset-analytics', 'show', 'revenue'],
        console: io,
        stdin: { isTTY: false },
        runAgentLoopFn: async (options) => {
          capturedOptions = options;
          return {
            response: 'done',
            toolResults: [],
          };
        },
        createConfirmHandlerFn: () => {
          confirmCalled = true;
          return 'confirm-handler';
        },
        createOutputFn: createOutputStub,
      },
    );

    assert.equal(exitCode, 0);
    assert.equal(confirmCalled, false);
    assert.equal(capturedOptions.allowApply, false);
    assert.equal(capturedOptions.onConfirmRequired, undefined);
    assert.ok(io.logs.some((line) => line.includes('Read-only (analytics)')));
  });

  it('prints human-readable stats and prompt budget when requested', async () => {
    const io = createConsoleCapture();

    const exitCode = await runAgentCli(
      {
        agent: 'orders',
        commandName: 'stateset-orders',
        title: 'StateSet Orders Agent',
        icon: '📦',
        help: 'HELP',
      },
      {
        argv: ['node', 'stateset-orders', '--stats', 'list', 'orders'],
        console: io,
        stdin: { isTTY: false },
        runAgentLoopFn: async () => ({
          sessionId: 'sess-123',
          traceId: 'trace-abc',
          response: 'ok',
          provider: 'anthropic',
          telemetry: {
            duration: 125,
            toolCalls: { total: 1, successRate: '100%' },
            avgToolDuration: 55,
          },
          promptReport: { totalInputTokens: 321 },
          toolResults: [],
        }),
        createConfirmHandlerFn: () => 'confirm-handler',
        createOutputFn: createOutputStub,
      },
    );

    assert.equal(exitCode, 0);
    assert.ok(io.logs.some((line) => line.includes('Execution Stats')));
    assert.ok(io.logs.some((line) => line.includes('PROMPT 321')));
    assert.ok(io.logs.some((line) => line.includes('continue this conversation')));
  });
});
