/**
 * Unit tests for agent runtime utils
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';

import { createStreamingHandler, resolveAgentRuntimeOptions } from '../../src/utils/agent-runtime.js';

describe('agent-runtime utils', () => {
  it('defaults to off think level and claude provider', () => {
    const opts = resolveAgentRuntimeOptions({});
    assert.strictEqual(opts.thinkLevel, 'off');
    assert.strictEqual(opts.providerName, 'claude');
    assert.strictEqual(opts.streaming, false);
    assert.strictEqual(opts.maxBudgetUsd, null);
    assert.strictEqual(opts.memoryOverride, null);
    assert.strictEqual(opts.enableX402, false);
  });

  it('resolves memory overrides', () => {
    assert.strictEqual(resolveAgentRuntimeOptions({ memory: true }).memoryOverride, true);
    assert.strictEqual(resolveAgentRuntimeOptions({ noMemory: true }).memoryOverride, false);
    assert.strictEqual(resolveAgentRuntimeOptions({ 'no-memory': true }).memoryOverride, false);
    assert.strictEqual(resolveAgentRuntimeOptions({}).memoryOverride, null);
  });

  it('throws on invalid think level', () => {
    assert.throws(() => resolveAgentRuntimeOptions({ think: 'extreme' }), /Invalid think level/);
  });

  it('creates streaming handler when enabled', () => {
    const handler = createStreamingHandler(true);
    assert.strictEqual(typeof handler, 'function');
    assert.strictEqual(createStreamingHandler(false), null);
  });
});
