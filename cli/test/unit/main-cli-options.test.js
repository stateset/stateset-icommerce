import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { buildRunAgentLoopOptions } from '../../src/main-cli-options.js';

describe('buildRunAgentLoopOptions', () => {
  it('preserves safety, routing, and billing flags across execution modes', () => {
    const onConfirmRequired = () => {};
    const options = buildRunAgentLoopOptions({
      request: 'sync orders',
      config: {
        db: './store.db',
        model: 'claude-sonnet-4-5-20250929',
        apply: true,
        verbose: true,
      },
      values: {
        agent: 'sync',
        stream: false,
        budget: '12.50',
        x402: true,
      },
      treasuryConfig: { enabled: true, chainId: 'base' },
      onConfirmRequired,
      resumeSessionId: 'session-123',
      thinkLevel: 'high',
      providerName: 'openai',
      memoryOverride: false,
    });

    assert.equal(options.request, 'sync orders');
    assert.equal(options.dbPath, './store.db');
    assert.equal(options.model, 'claude-sonnet-4-5-20250929');
    assert.equal(options.allowApply, true);
    assert.equal(options.resumeSessionId, 'session-123');
    assert.equal(options.agent, 'sync');
    assert.equal(options.verbose, true);
    assert.equal(options.treasury.chainId, 'base');
    assert.equal(options.onConfirmRequired, onConfirmRequired);
    assert.equal(options.thinkLevel, 'high');
    assert.equal(options.streaming, false);
    assert.equal(options.maxBudgetUsd, '12.50');
    assert.equal(options.provider, 'openai');
    assert.equal(options.enableMemory, false);
    assert.equal(options.enableX402, true);
  });

  it('omits optional callbacks when they are not supplied', () => {
    const options = buildRunAgentLoopOptions({
      request: 'list customers',
      config: {
        db: './store.db',
        model: 'claude-sonnet-4-5-20250929',
        apply: false,
        verbose: false,
      },
      values: {
        agent: 'customer-service',
        stream: false,
        budget: null,
        x402: false,
      },
      treasuryConfig: null,
      onConfirmRequired: () => {},
    });

    assert.ok(!('resumeSessionId' in options));
    assert.ok(!('onPartialMessage' in options));
    assert.ok(!('onThinkingBlock' in options));
    assert.ok(!('onToolCall' in options));
    assert.equal(options.enableMemory, null);
    assert.equal(options.provider, 'claude');
    assert.equal(options.maxBudgetUsd, null);
  });
});
