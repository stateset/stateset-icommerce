import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { printExecutionStats } from '../../src/utils/execution-stats.js';

function createOutputStub() {
  return {
    dim: (value) => value,
    bold: (value) => value,
    yellow: (value) => value,
    promptReport: (report) => `PROMPT ${report.totalInputTokens}`,
  };
}

function createConsoleCapture() {
  const logs = [];
  return {
    logs,
    log: (...args) => logs.push(args.join(' ')),
  };
}

describe('printExecutionStats', () => {
  it('prints telemetry and prompt report details', () => {
    const io = createConsoleCapture();
    const printed = printExecutionStats({
      output: createOutputStub(),
      ioConsole: io,
      result: {
        traceId: 'trace-123',
        provider: 'claude',
        cost: 0.42,
        telemetry: {
          duration: 125,
          toolCalls: { total: 2, successRate: '100%' },
          avgToolDuration: 55,
        },
        promptReport: { totalInputTokens: 321 },
      },
    });

    assert.equal(printed, true);
    assert.ok(io.logs.some((line) => line.includes('Execution Stats')));
    assert.ok(io.logs.some((line) => line.includes('Trace ID:')));
    assert.ok(io.logs.some((line) => line.includes('125ms')));
    assert.ok(io.logs.some((line) => line.includes('PROMPT 321')));
  });

  it('prints session refresh details when present', () => {
    const io = createConsoleCapture();
    printExecutionStats({
      output: createOutputStub(),
      ioConsole: io,
      result: {
        telemetry: { duration: 50 },
        sessionRefresh: {
          reason: 'treasury_budget_refresh',
          previousSessionId: 'sess-1',
          sessionId: 'sess-2',
          replayedMessages: 4,
        },
      },
    });

    assert.ok(io.logs.some((line) => line.includes('Session Refresh:')));
    assert.ok(io.logs.some((line) => line.includes('treasury budget refresh')));
    assert.ok(io.logs.some((line) => line.includes('sess-1 -> sess-2')));
    assert.ok(io.logs.some((line) => line.includes('4 prior messages')));
  });

  it('can suppress prompt report rendering', () => {
    const io = createConsoleCapture();
    printExecutionStats({
      output: createOutputStub(),
      ioConsole: io,
      includePromptReport: false,
      result: {
        telemetry: { duration: 50 },
        promptReport: { totalInputTokens: 321 },
      },
    });

    assert.ok(io.logs.some((line) => line.includes('Execution Stats')));
    assert.ok(!io.logs.some((line) => line.includes('PROMPT 321')));
  });

  it('is a no-op when no telemetry or prompt report is available', () => {
    const io = createConsoleCapture();
    const printed = printExecutionStats({
      output: createOutputStub(),
      ioConsole: io,
      result: { response: 'ok' },
    });

    assert.equal(printed, false);
    assert.deepEqual(io.logs, []);
  });
});
