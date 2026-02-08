/**
 * Tests for cli/src/model-fallback.js
 *
 * Covers: DEFAULT_FALLBACK_CHAIN, CooldownTracker (via ModelFallback),
 * ModelFallback (execute, executeWithModel, getAvailableModels, getStatus),
 * createFallbackCaller.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

import {
  DEFAULT_FALLBACK_CHAIN,
  ModelFallback,
  createFallbackCaller,
} from '../../src/model-fallback.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Minimal 2-model chain for fast tests (no real env keys needed) */
function testChain() {
  return [
    {
      id: 'model-a',
      provider: 'test',
      model: 'test-a',
      envKey: null, // no env key required
      priority: 1,
      capabilities: ['tools', 'streaming'],
    },
    {
      id: 'model-b',
      provider: 'test',
      model: 'test-b',
      envKey: null,
      priority: 2,
      capabilities: ['streaming'],
    },
  ];
}

// ---------------------------------------------------------------------------
// DEFAULT_FALLBACK_CHAIN
// ---------------------------------------------------------------------------

describe('DEFAULT_FALLBACK_CHAIN', () => {
  it('is an array of at least 2 models', () => {
    assert.ok(Array.isArray(DEFAULT_FALLBACK_CHAIN));
    assert.ok(DEFAULT_FALLBACK_CHAIN.length >= 2);
  });

  it('each model has required fields', () => {
    for (const m of DEFAULT_FALLBACK_CHAIN) {
      assert.ok(m.id, 'model.id');
      assert.ok(m.provider, 'model.provider');
      assert.ok(m.model, 'model.model');
      assert.ok(typeof m.priority === 'number');
      assert.ok(Array.isArray(m.capabilities));
    }
  });

  it('first model is claude-sonnet', () => {
    assert.equal(DEFAULT_FALLBACK_CHAIN[0].id, 'claude-sonnet');
  });

  it('models are ordered by priority', () => {
    for (let i = 1; i < DEFAULT_FALLBACK_CHAIN.length; i++) {
      assert.ok(DEFAULT_FALLBACK_CHAIN[i].priority > DEFAULT_FALLBACK_CHAIN[i - 1].priority);
    }
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — constructor
// ---------------------------------------------------------------------------

describe('ModelFallback constructor', () => {
  it('uses default chain when none specified', () => {
    const fb = new ModelFallback();
    assert.equal(fb.chain.length, DEFAULT_FALLBACK_CHAIN.length);
  });

  it('accepts custom chain', () => {
    const fb = new ModelFallback({ chain: testChain() });
    assert.equal(fb.chain.length, 2);
  });

  it('defaults maxRetries to 3', () => {
    const fb = new ModelFallback();
    assert.equal(fb.maxRetries, 3);
  });

  it('filters chain by requiredCapabilities', () => {
    const fb = new ModelFallback({
      chain: testChain(),
      requiredCapabilities: ['tools'],
    });
    assert.equal(fb.chain.length, 1);
    assert.equal(fb.chain[0].id, 'model-a');
  });

  it('empty chain when no model has required capability', () => {
    const fb = new ModelFallback({
      chain: testChain(),
      requiredCapabilities: ['vision'],
    });
    assert.equal(fb.chain.length, 0);
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — getAvailableModels
// ---------------------------------------------------------------------------

describe('ModelFallback.getAvailableModels', () => {
  it('returns all models when none in cooldown and no envKey required', () => {
    const fb = new ModelFallback({ chain: testChain() });
    assert.equal(fb.getAvailableModels().length, 2);
  });

  it('excludes models in cooldown', () => {
    const fb = new ModelFallback({ chain: testChain() });
    fb.setModelCooldown('model-a', 60000, 'test');
    const available = fb.getAvailableModels();
    assert.equal(available.length, 1);
    assert.equal(available[0].id, 'model-b');
  });

  it('excludes models missing env key', () => {
    const chain = [
      { id: 'm1', provider: 'test', model: 'x', envKey: 'MISSING_KEY_XYZ', priority: 1, capabilities: [] },
    ];
    const fb = new ModelFallback({ chain });
    assert.equal(fb.getAvailableModels().length, 0);
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — execute (success)
// ---------------------------------------------------------------------------

describe('ModelFallback.execute — success path', () => {
  it('returns result from first model on success', async () => {
    const fb = new ModelFallback({ chain: testChain(), retryDelayMs: 1 });
    const { result, model } = await fb.execute(async (m) => `ok-${m.id}`);
    assert.equal(result, 'ok-model-a');
    assert.equal(model.id, 'model-a');
  });

  it('clears cooldown on success', async () => {
    const fb = new ModelFallback({ chain: testChain(), retryDelayMs: 1 });
    fb.setModelCooldown('model-a', 1, 'old');
    // Wait for cooldown to expire
    await new Promise((r) => setTimeout(r, 5));
    const { model } = await fb.execute(async (m) => 'ok');
    assert.equal(model.id, 'model-a');
  });

  it('includes attempts array', async () => {
    const fb = new ModelFallback({ chain: testChain(), retryDelayMs: 1 });
    const { attempts } = await fb.execute(async () => 'ok');
    assert.equal(attempts.length, 1);
    assert.equal(attempts[0].success, true);
    assert.equal(attempts[0].model, 'model-a');
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — execute (fallback)
// ---------------------------------------------------------------------------

describe('ModelFallback.execute — fallback', () => {
  it('falls back to next model on transient error', async () => {
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 1,
      retryDelayMs: 1,
    });
    let callCount = 0;
    const { result, model } = await fb.execute(async (m) => {
      callCount++;
      if (m.id === 'model-a') throw new Error('connection timeout');
      return 'ok-b';
    });
    assert.equal(result, 'ok-b');
    assert.equal(model.id, 'model-b');
  });

  it('calls onFallback callback', async () => {
    const fallbacks = [];
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 1,
      retryDelayMs: 1,
      onFallback: (info) => fallbacks.push(info),
    });
    await fb.execute(async (m) => {
      if (m.id === 'model-a') throw new Error('oops');
      return 'ok';
    });
    assert.equal(fallbacks.length, 1);
    assert.equal(fallbacks[0].from.id, 'model-a');
    assert.equal(fallbacks[0].to.id, 'model-b');
  });

  it('retries within same model before falling back', async () => {
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 3,
      retryDelayMs: 1,
    });
    let aAttempts = 0;
    const { model } = await fb.execute(async (m) => {
      if (m.id === 'model-a') {
        aAttempts++;
        throw new Error('flaky');
      }
      return 'ok';
    });
    assert.equal(aAttempts, 3); // all retries exhausted
    assert.equal(model.id, 'model-b');
  });

  it('throws when all models exhausted', async () => {
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 1,
      retryDelayMs: 1,
    });
    await assert.rejects(
      () => fb.execute(async () => { throw new Error('always fail'); }),
      (e) => {
        assert.ok(e.message.includes('All models failed'));
        assert.ok(e.message.includes('model-a'));
        assert.ok(e.message.includes('model-b'));
        return true;
      },
    );
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — execute (rate limit detection)
// ---------------------------------------------------------------------------

describe('ModelFallback.execute — rate limit handling', () => {
  it('puts model in cooldown on rate limit error', async () => {
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 1,
      retryDelayMs: 1,
      baseCooldownMs: 10000,
    });
    await fb.execute(async (m) => {
      if (m.id === 'model-a') throw new Error('429 too many requests');
      return 'ok';
    });
    // model-a should be in cooldown now
    const available = fb.getAvailableModels();
    assert.equal(available.length, 1);
    assert.equal(available[0].id, 'model-b');
  });

  it('detects rate_limit pattern', async () => {
    const cooldowns = [];
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 1,
      retryDelayMs: 1,
      onCooldown: (info) => cooldowns.push(info),
    });
    await fb.execute(async (m) => {
      if (m.id === 'model-a') throw new Error('Rate limit exceeded');
      return 'ok';
    });
    assert.equal(cooldowns.length, 1);
    assert.equal(cooldowns[0].permanent, false);
  });

  it('detects overloaded pattern', async () => {
    const cooldowns = [];
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 1,
      retryDelayMs: 1,
      onCooldown: (info) => cooldowns.push(info),
    });
    await fb.execute(async (m) => {
      if (m.id === 'model-a') throw new Error('Server overloaded');
      return 'ok';
    });
    assert.equal(cooldowns.length, 1);
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — execute (permanent failure detection)
// ---------------------------------------------------------------------------

describe('ModelFallback.execute — permanent failures', () => {
  it('puts model in long cooldown on auth error', async () => {
    const cooldowns = [];
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 3,
      retryDelayMs: 1,
      onCooldown: (info) => cooldowns.push(info),
    });
    await fb.execute(async (m) => {
      if (m.id === 'model-a') throw new Error('Invalid API key');
      return 'ok';
    });
    assert.equal(cooldowns.length, 1);
    assert.equal(cooldowns[0].permanent, true);
  });

  it('does not retry on permanent failure', async () => {
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 3,
      retryDelayMs: 1,
    });
    let aAttempts = 0;
    await fb.execute(async (m) => {
      if (m.id === 'model-a') {
        aAttempts++;
        throw new Error('401 Unauthorized');
      }
      return 'ok';
    });
    // Should not retry permanent errors — only 1 attempt
    assert.equal(aAttempts, 1);
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — execute (preferred model)
// ---------------------------------------------------------------------------

describe('ModelFallback.execute — preferred model', () => {
  it('tries preferred model first', async () => {
    const fb = new ModelFallback({ chain: testChain(), retryDelayMs: 1 });
    const { model } = await fb.execute(async (m) => 'ok', {
      preferredModel: 'model-b',
    });
    assert.equal(model.id, 'model-b');
  });

  it('falls back if preferred model fails', async () => {
    const fb = new ModelFallback({
      chain: testChain(),
      maxRetries: 1,
      retryDelayMs: 1,
    });
    const { model } = await fb.execute(
      async (m) => {
        if (m.id === 'model-b') throw new Error('fail');
        return 'ok';
      },
      { preferredModel: 'model-b' },
    );
    assert.equal(model.id, 'model-a');
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — execute (no models available)
// ---------------------------------------------------------------------------

describe('ModelFallback.execute — no models available', () => {
  it('throws helpful error when all in cooldown', async () => {
    const fb = new ModelFallback({ chain: testChain() });
    fb.setModelCooldown('model-a', 60000, 'rate limit');
    fb.setModelCooldown('model-b', 60000, 'rate limit');
    await assert.rejects(
      () => fb.execute(async () => 'ok'),
      (e) => {
        assert.ok(e.message.includes('cooldown'));
        return true;
      },
    );
  });

  it('throws helpful error when all missing keys', async () => {
    const chain = [
      { id: 'm1', provider: 'test', model: 'x', envKey: 'MISSING_A_XYZ', priority: 1, capabilities: [] },
      { id: 'm2', provider: 'test', model: 'y', envKey: 'MISSING_B_XYZ', priority: 2, capabilities: [] },
    ];
    const fb = new ModelFallback({ chain });
    await assert.rejects(
      () => fb.execute(async () => 'ok'),
      (e) => {
        assert.ok(e.message.includes('API key') || e.message.includes('No models'));
        return true;
      },
    );
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — executeWithModel
// ---------------------------------------------------------------------------

describe('ModelFallback.executeWithModel', () => {
  it('runs operation with specific model', async () => {
    const fb = new ModelFallback({ chain: testChain() });
    const result = await fb.executeWithModel('model-b', async (m) => `hi-${m.id}`);
    assert.equal(result, 'hi-model-b');
  });

  it('resolves by model name as well as ID', async () => {
    const fb = new ModelFallback({ chain: testChain() });
    const result = await fb.executeWithModel('test-b', async (m) => m.id);
    assert.equal(result, 'model-b');
  });

  it('throws for unknown model', async () => {
    const fb = new ModelFallback({ chain: testChain() });
    await assert.rejects(
      () => fb.executeWithModel('nonexistent', async () => 'ok'),
      /Unknown model/,
    );
  });

  it('throws when model is in cooldown', async () => {
    const fb = new ModelFallback({ chain: testChain() });
    fb.setModelCooldown('model-a', 60000, 'rate limit');
    await assert.rejects(
      () => fb.executeWithModel('model-a', async () => 'ok'),
      /cooldown/,
    );
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — getStatus
// ---------------------------------------------------------------------------

describe('ModelFallback.getStatus', () => {
  it('returns status for all models', () => {
    const fb = new ModelFallback({ chain: testChain() });
    const status = fb.getStatus();
    assert.equal(status.length, 2);
    assert.equal(status[0].id, 'model-a');
    assert.equal(status[0].available, true);
    assert.equal(status[0].inCooldown, false);
  });

  it('reflects cooldown state', () => {
    const fb = new ModelFallback({ chain: testChain() });
    fb.setModelCooldown('model-a', 60000, 'test');
    const status = fb.getStatus();
    assert.equal(status[0].available, false);
    assert.equal(status[0].inCooldown, true);
    assert.ok(status[0].cooldownRemainingMs > 0);
  });
});

// ---------------------------------------------------------------------------
// ModelFallback — cooldown management
// ---------------------------------------------------------------------------

describe('ModelFallback cooldown management', () => {
  it('clearModelCooldown removes cooldown', () => {
    const fb = new ModelFallback({ chain: testChain() });
    fb.setModelCooldown('model-a', 60000, 'test');
    assert.equal(fb.getAvailableModels().length, 1);
    fb.clearModelCooldown('model-a');
    assert.equal(fb.getAvailableModels().length, 2);
  });

  it('setModelCooldown accepts duration and reason', () => {
    const fb = new ModelFallback({ chain: testChain() });
    fb.setModelCooldown('model-a', 5000, 'too hot');
    const status = fb.getStatus().find((s) => s.id === 'model-a');
    assert.equal(status.inCooldown, true);
  });
});

// ---------------------------------------------------------------------------
// createFallbackCaller
// ---------------------------------------------------------------------------

describe('createFallbackCaller', () => {
  it('returns object with call, fallback, getStatus', () => {
    const caller = createFallbackCaller({
      claudeCall: async () => 'claude',
      openaiCall: async () => 'openai',
      geminiCall: async () => 'gemini',
    });
    assert.ok(typeof caller.call === 'function');
    assert.ok(caller.fallback instanceof ModelFallback);
    assert.ok(typeof caller.getStatus === 'function');
  });

  it('filters by requiredCapabilities', () => {
    const caller = createFallbackCaller({
      claudeCall: async () => 'claude',
      requiredCapabilities: ['thinking'],
    });
    // Only claude-sonnet has 'thinking' capability in DEFAULT_FALLBACK_CHAIN
    const status = caller.getStatus();
    assert.equal(status.length, 1);
    assert.equal(status[0].id, 'claude-sonnet');
  });
});
