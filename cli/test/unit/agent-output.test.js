/**
 * Unit tests for agent output utils
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

import {
  buildAgentOutputData,
  resolveOutputFormat,
  writeAgentOutputFile,
} from '../../src/utils/agent-output.js';

describe('agent-output utils', () => {
  describe('resolveOutputFormat', () => {
    it('defaults to json when --json set and no --format', () => {
      const format = resolveOutputFormat({
        format: 'table',
        json: true,
        argv: ['node', 'cli'],
      });
      assert.strictEqual(format, 'json');
    });

    it('respects explicit --format over --json', () => {
      const format = resolveOutputFormat({
        format: 'table',
        json: true,
        argv: ['node', 'cli', '--format', 'csv'],
      });
      assert.strictEqual(format, 'csv');
    });

    it('handles --format=value', () => {
      const format = resolveOutputFormat({
        format: 'table',
        json: false,
        argv: ['node', 'cli', '--format=yaml'],
      });
      assert.strictEqual(format, 'yaml');
    });
  });

  describe('buildAgentOutputData', () => {
    it('builds structured output with optional fields', () => {
      const output = buildAgentOutputData({
        agent: 'orders',
        request: 'list orders',
        allowApply: true,
        result: {
          sessionId: 'sess-123',
          traceId: 'trace-abc',
          response: 'ok',
          provider: 'anthropic',
          usedModel: 'claude-test',
          cost: 0.42,
          budgetExceeded: false,
          toolResults: [
            {
              toolCall: { name: 'mcp__stateset-commerce__list_orders', input: { limit: 1 } },
              result: [{ id: 'ord-1' }],
            },
          ],
        },
      });

      assert.strictEqual(output.agent, 'orders');
      assert.strictEqual(output.request, 'list orders');
      assert.strictEqual(output.allowApply, true);
      assert.strictEqual(output.sessionId, 'sess-123');
      assert.strictEqual(output.traceId, 'trace-abc');
      assert.strictEqual(output.provider, 'anthropic');
      assert.strictEqual(output.model, 'claude-test');
      assert.strictEqual(output.cost, 0.42);
      assert.strictEqual(output.budgetExceeded, false);
      assert.deepStrictEqual(output.toolResults, [
        {
          tool: 'mcp__stateset-commerce__list_orders',
          input: { limit: 1 },
          result: [{ id: 'ord-1' }],
        },
      ]);
    });

    it('adds telemetry and prompt reports only when requested', () => {
      const result = {
        sessionId: 'sess-123',
        traceId: 'trace-abc',
        response: 'ok',
        telemetry: { duration: 125 },
        promptReport: { totalInputTokens: 321 },
        toolResults: [],
      };

      const baseline = buildAgentOutputData({
        agent: 'orders',
        request: 'list orders',
        allowApply: false,
        result,
      });
      const withStats = buildAgentOutputData({
        agent: 'orders',
        request: 'list orders',
        allowApply: false,
        result,
        includeTelemetry: true,
        includePromptReport: true,
      });

      assert.strictEqual(baseline.telemetry, undefined);
      assert.strictEqual(baseline.promptReport, undefined);
      assert.deepStrictEqual(withStats.telemetry, { duration: 125 });
      assert.deepStrictEqual(withStats.promptReport, { totalInputTokens: 321 });
    });
  });

  describe('writeAgentOutputFile', () => {
    it('writes formatted output to disk', async () => {
      const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'ss-cli-'));
      const filePath = path.join(dir, 'output.json');
      await writeAgentOutputFile(filePath, { foo: 1 }, 'json');
      const contents = await fs.readFile(filePath, 'utf8');
      assert.ok(contents.includes('"foo": 1'));
    });
  });
});
