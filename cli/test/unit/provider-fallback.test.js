/**
 * Unit tests for CircuitBreaker and FallbackChain
 *
 * Tests the multi-provider failover infrastructure including:
 * - Circuit breaker state transitions (closed -> open -> half-open -> closed)
 * - FallbackChain provider ordering and failover
 * - chatWithClaudeFallback primary/fallback flow
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import {
  CircuitBreaker,
  FallbackChain,
  ModelProvider,
  resetProviderRegistry,
  getProviderRegistry,
} from '../../src/providers/base.js';

// ============================================================================
// Test Helpers
// ============================================================================

/** A mock provider that succeeds or fails on demand. */
class MockProvider extends ModelProvider {
  constructor(name, { shouldFail = false, apiKey = 'test-key' } = {}) {
    super(name, { envKey: null, models: { default: 'mock-model' }, default: 'mock-model' });
    this.shouldFail = shouldFail;
    this.callCount = 0;
    this._apiKey = apiKey;
  }

  async isAvailable() {
    return true;
  }

  async chat(messages, options = {}) {
    this.callCount++;
    if (this.shouldFail) {
      throw new Error(`${this.name} failed`);
    }
    return {
      text: `Response from ${this.name}`,
      model: 'mock-model',
      provider: this.name,
      cost: 0.001,
      usage: { inputTokens: 10, outputTokens: 20 },
    };
  }

  getApiKey() {
    return this._apiKey;
  }
}

// ============================================================================
// CircuitBreaker Tests
// ============================================================================

describe('CircuitBreaker', () => {
  it('starts all providers in closed state', () => {
    const cb = new CircuitBreaker();
    assert.strictEqual(cb.isAvailable('openai'), true);
    assert.strictEqual(cb.isAvailable('gemini'), true);
  });

  it('remains closed below failure threshold', () => {
    const cb = new CircuitBreaker({ failureThreshold: 3 });
    cb.recordFailure('openai');
    cb.recordFailure('openai');
    assert.strictEqual(
      cb.isAvailable('openai'),
      true,
      'Should still be available after 2 failures',
    );
  });

  it('opens after reaching failure threshold', () => {
    const cb = new CircuitBreaker({ failureThreshold: 3 });
    cb.recordFailure('openai');
    cb.recordFailure('openai');
    cb.recordFailure('openai');
    assert.strictEqual(cb.isAvailable('openai'), false, 'Should be unavailable after 3 failures');
  });

  it('transitions to half-open after reset timeout', async () => {
    const cb = new CircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 50 });
    cb.recordFailure('gemini');
    assert.strictEqual(cb.isAvailable('gemini'), false, 'Should be open immediately');

    await new Promise((r) => setTimeout(r, 60));
    assert.strictEqual(cb.isAvailable('gemini'), true, 'Should be half-open after timeout');
  });

  it('closes on success after half-open', async () => {
    const cb = new CircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 50 });
    cb.recordFailure('ollama');
    await new Promise((r) => setTimeout(r, 60));

    // Now half-open — record success
    cb.recordSuccess('ollama');
    const status = cb.getStatus();
    assert.strictEqual(status.ollama.state, 'closed');
    assert.strictEqual(status.ollama.failures, 0);
  });

  it('reopens on failure in half-open state', async () => {
    const cb = new CircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 50 });
    cb.recordFailure('openai');
    await new Promise((r) => setTimeout(r, 60));

    // Half-open — record another failure
    cb.recordFailure('openai');
    assert.strictEqual(cb.isAvailable('openai'), false, 'Should be open again');
  });

  it('reset() clears a specific provider', () => {
    const cb = new CircuitBreaker({ failureThreshold: 1 });
    cb.recordFailure('openai');
    assert.strictEqual(cb.isAvailable('openai'), false);

    cb.reset('openai');
    assert.strictEqual(cb.isAvailable('openai'), true);
  });

  it('resetAll() clears all providers', () => {
    const cb = new CircuitBreaker({ failureThreshold: 1 });
    cb.recordFailure('openai');
    cb.recordFailure('gemini');
    assert.strictEqual(cb.isAvailable('openai'), false);
    assert.strictEqual(cb.isAvailable('gemini'), false);

    cb.resetAll();
    assert.strictEqual(cb.isAvailable('openai'), true);
    assert.strictEqual(cb.isAvailable('gemini'), true);
  });

  it('getStatus() returns state for all tracked providers', () => {
    const cb = new CircuitBreaker({ failureThreshold: 3 });
    cb.recordFailure('openai');
    cb.recordFailure('openai');
    cb.recordSuccess('gemini');

    const status = cb.getStatus();
    assert.strictEqual(status.openai.state, 'closed');
    assert.strictEqual(status.openai.failures, 2);
    assert.strictEqual(status.gemini.state, 'closed');
    assert.strictEqual(status.gemini.failures, 0);
  });

  it('isolates circuits per provider', () => {
    const cb = new CircuitBreaker({ failureThreshold: 2 });
    cb.recordFailure('openai');
    cb.recordFailure('openai');
    assert.strictEqual(cb.isAvailable('openai'), false, 'openai should be open');
    assert.strictEqual(cb.isAvailable('gemini'), true, 'gemini should be unaffected');
  });
});

// ============================================================================
// FallbackChain Tests
// ============================================================================

describe('FallbackChain', () => {
  beforeEach(() => {
    resetProviderRegistry();
  });

  it('uses the preferred provider when available', async () => {
    const registry = getProviderRegistry();
    const mock = new MockProvider('openai');
    registry.register(mock);

    const chain = new FallbackChain({ order: ['openai', 'gemini'] });
    const result = await chain.chat([{ role: 'user', content: 'hello' }], {
      preferredProvider: 'openai',
    });

    assert.strictEqual(result.provider, 'openai');
    assert.strictEqual(result.failedOver, false);
    assert.strictEqual(mock.callCount, 1);
  });

  it('falls over to next provider on failure', async () => {
    const registry = getProviderRegistry();
    const failing = new MockProvider('openai', { shouldFail: true });
    const working = new MockProvider('gemini');
    registry.register(failing);
    registry.register(working);

    const chain = new FallbackChain({ order: ['openai', 'gemini'] });
    const result = await chain.chat([{ role: 'user', content: 'hello' }], {
      preferredProvider: 'openai',
    });

    assert.strictEqual(result.provider, 'gemini');
    assert.strictEqual(result.failedOver, true);
    assert.deepStrictEqual(result.attemptedProviders, ['openai', 'gemini']);
  });

  it('throws when all providers fail', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('openai', { shouldFail: true }));
    registry.register(new MockProvider('gemini', { shouldFail: true }));

    const chain = new FallbackChain({ order: ['openai', 'gemini'] });

    await assert.rejects(
      () => chain.chat([{ role: 'user', content: 'hello' }]),
      /All providers failed/,
    );
  });

  it('skips providers with open circuits', async () => {
    const registry = getProviderRegistry();
    const openai = new MockProvider('openai', { shouldFail: true });
    const gemini = new MockProvider('gemini');
    registry.register(openai);
    registry.register(gemini);

    const chain = new FallbackChain({
      order: ['openai', 'gemini'],
      failureThreshold: 1,
    });

    // First call: openai fails, gemini succeeds
    await chain.chat([{ role: 'user', content: 'hello' }]);
    assert.strictEqual(openai.callCount, 1);

    // Second call: openai circuit is open, goes straight to gemini
    openai.callCount = 0;
    await chain.chat([{ role: 'user', content: 'hello again' }]);
    assert.strictEqual(openai.callCount, 0, 'Should not attempt openai');
  });

  it('tracks failover count', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('openai', { shouldFail: true }));
    registry.register(new MockProvider('gemini'));

    const chain = new FallbackChain({ order: ['openai', 'gemini'] });
    assert.strictEqual(chain.getFailoverCount(), 0);

    await chain.chat([{ role: 'user', content: 'hello' }]);
    assert.strictEqual(chain.getFailoverCount(), 1);
  });

  it('tracks last used provider', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('gemini'));

    const chain = new FallbackChain({ order: ['gemini'] });
    await chain.chat([{ role: 'user', content: 'hello' }]);
    assert.strictEqual(chain.getLastUsedProvider(), 'gemini');
  });

  it('getCircuitStatus() reflects breaker state', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('openai', { shouldFail: true }));
    registry.register(new MockProvider('gemini'));

    const chain = new FallbackChain({
      order: ['openai', 'gemini'],
      failureThreshold: 1,
    });

    await chain.chat([{ role: 'user', content: 'hello' }]);
    const status = chain.getCircuitStatus();
    assert.strictEqual(status.openai.state, 'open');
  });

  it('resetCircuit() allows retrying a failed provider', async () => {
    const registry = getProviderRegistry();
    const openai = new MockProvider('openai', { shouldFail: true });
    const gemini = new MockProvider('gemini');
    registry.register(openai);
    registry.register(gemini);

    const chain = new FallbackChain({
      order: ['openai', 'gemini'],
      failureThreshold: 1,
    });

    await chain.chat([{ role: 'user', content: 'hello' }]);
    assert.strictEqual(chain.getCircuitStatus().openai.state, 'open');

    // Fix the provider and reset circuit
    openai.shouldFail = false;
    chain.resetCircuit('openai');

    const result = await chain.chat([{ role: 'user', content: 'hello' }]);
    assert.strictEqual(result.provider, 'openai');
  });
});

// ============================================================================
// chatWithClaudeFallback Tests
// ============================================================================

describe('FallbackChain.chatWithClaudeFallback', () => {
  beforeEach(() => {
    resetProviderRegistry();
  });

  it('uses Claude when it succeeds', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('openai'));

    const chain = new FallbackChain({ order: ['claude', 'openai'] });

    const result = await chain.chatWithClaudeFallback(
      async () => ({
        text: 'Claude response',
        model: 'claude-sonnet',
        usage: { inputTokens: 5, outputTokens: 10 },
      }),
      [{ role: 'user', content: 'hello' }],
    );

    assert.strictEqual(result.provider, 'claude');
    assert.strictEqual(result.failedOver, false);
    assert.strictEqual(result.text, 'Claude response');
  });

  it('falls back to other providers when Claude fails', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('openai'));

    const chain = new FallbackChain({ order: ['claude', 'openai'] });

    const result = await chain.chatWithClaudeFallback(async () => {
      throw new Error('Claude down');
    }, [{ role: 'user', content: 'hello' }]);

    assert.strictEqual(result.failedOver, true);
    assert.strictEqual(result.provider, 'openai');
  });

  it('records Claude failure in circuit breaker', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('openai'));

    const chain = new FallbackChain({
      order: ['claude', 'openai'],
      failureThreshold: 1,
    });

    await chain.chatWithClaudeFallback(async () => {
      throw new Error('Claude down');
    }, [{ role: 'user', content: 'hello' }]);

    const status = chain.getCircuitStatus();
    assert.strictEqual(status.claude.state, 'open');
  });

  it('skips Claude when its circuit is open', async () => {
    const registry = getProviderRegistry();
    registry.register(new MockProvider('openai'));

    const chain = new FallbackChain({
      order: ['claude', 'openai'],
      failureThreshold: 1,
    });

    // Trip Claude's circuit
    await chain.chatWithClaudeFallback(async () => {
      throw new Error('Claude down');
    }, [{ role: 'user', content: 'hello' }]);

    let claudeCalled = false;
    const result = await chain.chatWithClaudeFallback(async () => {
      claudeCalled = true;
      return { text: 'ok' };
    }, [{ role: 'user', content: 'hello again' }]);

    assert.strictEqual(claudeCalled, false, 'Claude should not be called when circuit is open');
    assert.strictEqual(result.provider, 'openai');
  });
});
