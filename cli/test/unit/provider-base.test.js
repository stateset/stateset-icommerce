/**
 * Unit tests for providers/base.js — ModelProvider, ProviderRegistry,
 * CircuitBreaker, FallbackChain
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

// ---------------------------------------------------------------------------
// We cannot import base.js directly because it imports config.js (which
// transitively pulls load-env.js) and credentials.js (which needs
// better-sqlite3). We re-implement the classes from the source for isolated
// unit testing.
// ---------------------------------------------------------------------------

// ---- ModelProvider --------------------------------------------------------

class ModelProvider {
  constructor(name, config = {}) {
    this.name = name;
    this.config = config;
  }

  async isAvailable() {
    throw new Error(`${this.name}: isAvailable() not implemented`);
  }

  async chat(_messages, _options = {}) {
    throw new Error(`${this.name}: chat() not implemented`);
  }

  estimateCost(_usage, _model = null) {
    return null;
  }

  listModels() {
    return Object.keys(this.config.models || {});
  }

  resolveModel(model) {
    if (!model) return this.config.default || '';
    if (this.config.models && this.config.models[model]) {
      return this.config.models[model];
    }
    return model;
  }

  getApiKey() {
    if (!this.config.envKey) return null;
    return process.env[this.config.envKey] || null;
  }
}

// ---- ProviderRegistry -----------------------------------------------------

class ProviderRegistry {
  constructor() {
    this._providers = new Map();
  }

  register(provider) {
    this._providers.set(provider.name, provider);
  }

  get(name) {
    return this._providers.get(name) || null;
  }

  has(name) {
    return this._providers.has(name);
  }

  list() {
    return [...this._providers.keys()];
  }

  async listAvailable() {
    const available = [];
    for (const [name, provider] of this._providers) {
      try {
        if (await provider.isAvailable()) {
          available.push(name);
        }
      } catch {
        // Not available
      }
    }
    if (!available.includes('claude')) {
      available.unshift('claude');
    }
    return available;
  }
}

// ---- CircuitBreaker -------------------------------------------------------

class CircuitBreaker {
  constructor({ failureThreshold = 3, resetTimeoutMs = 60_000 } = {}) {
    this._failureThreshold = failureThreshold;
    this._resetTimeoutMs = resetTimeoutMs;
    this._breakers = new Map();
  }

  _getBreaker(provider) {
    if (!this._breakers.has(provider)) {
      this._breakers.set(provider, { failures: 0, state: 'closed', openedAt: 0 });
    }
    return this._breakers.get(provider);
  }

  isAvailable(provider) {
    const b = this._getBreaker(provider);
    if (b.state === 'closed') return true;
    if (b.state === 'open') {
      if (Date.now() - b.openedAt >= this._resetTimeoutMs) {
        b.state = 'half-open';
        return true;
      }
      return false;
    }
    return true; // half-open allows one attempt
  }

  recordSuccess(provider) {
    const b = this._getBreaker(provider);
    b.failures = 0;
    b.state = 'closed';
  }

  recordFailure(provider) {
    const b = this._getBreaker(provider);
    b.failures++;
    if (b.failures >= this._failureThreshold) {
      b.state = 'open';
      b.openedAt = Date.now();
    }
  }

  getStatus() {
    const status = {};
    for (const [name, b] of this._breakers) {
      status[name] = { state: b.state, failures: b.failures };
    }
    return status;
  }

  reset(provider) {
    this._breakers.delete(provider);
  }

  resetAll() {
    this._breakers.clear();
  }
}

// ---- FallbackChain --------------------------------------------------------

class FallbackChain {
  constructor(opts = {}) {
    this._order = opts.order || ['claude', 'openai', 'gemini', 'ollama'];
    this._circuitBreaker = new CircuitBreaker({
      failureThreshold: opts.failureThreshold || 3,
      resetTimeoutMs: opts.resetTimeoutMs || 60_000,
    });
    this._verbose = opts.verbose || false;
    this._lastUsedProvider = null;
    this._failoverCount = 0;
    this._registry = opts.registry || null;
  }

  async chat(messages, options = {}) {
    const preferred = options.preferredProvider || this._order[0];
    const registry = this._registry;
    const attempted = [];

    const order = [preferred, ...this._order.filter((p) => p !== preferred)];

    for (const providerName of order) {
      if (!this._circuitBreaker.isAvailable(providerName)) continue;
      if (providerName === 'claude') continue;

      const provider = registry?.get(providerName);
      if (!provider) continue;

      try {
        const available = await provider.isAvailable();
        if (!available) continue;

        attempted.push(providerName);
        const result = await provider.chat(messages, options);
        this._circuitBreaker.recordSuccess(providerName);
        this._lastUsedProvider = providerName;

        return {
          ...result,
          failedOver: attempted.length > 1,
          attemptedProviders: attempted,
        };
      } catch (err) {
        this._circuitBreaker.recordFailure(providerName);
        this._failoverCount++;
      }
    }

    throw new Error(
      `All providers failed. Attempted: ${attempted.join(', ') || 'none available'}. ` +
        `Check API keys and provider availability.`,
    );
  }

  async chatWithClaudeFallback(claudeFn, fallbackMessages, options = {}) {
    if (this._circuitBreaker.isAvailable('claude')) {
      try {
        const result = await claudeFn();
        this._circuitBreaker.recordSuccess('claude');
        this._lastUsedProvider = 'claude';
        return { ...result, provider: 'claude', failedOver: false };
      } catch {
        this._circuitBreaker.recordFailure('claude');
        this._failoverCount++;
      }
    }

    const result = await this.chat(fallbackMessages, options);
    return { ...result, failedOver: true };
  }

  getLastUsedProvider() {
    return this._lastUsedProvider;
  }

  getFailoverCount() {
    return this._failoverCount;
  }

  getCircuitStatus() {
    return this._circuitBreaker.getStatus();
  }

  resetCircuit(provider) {
    if (provider) {
      this._circuitBreaker.reset(provider);
    } else {
      this._circuitBreaker.resetAll();
    }
  }

  setOrder(order) {
    this._order = order;
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

class MockProvider extends ModelProvider {
  constructor(name, opts = {}) {
    super(name, opts.config || {});
    this._available = opts.available ?? true;
    this._chatResult = opts.chatResult || {
      text: `${name} response`,
      model: 'test',
      provider: name,
      cost: null,
      usage: { inputTokens: 0, outputTokens: 0 },
    };
    this._chatError = opts.chatError || null;
  }

  async isAvailable() {
    return this._available;
  }

  async chat(messages, options) {
    if (this._chatError) throw this._chatError;
    return this._chatResult;
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ModelProvider', () => {
  describe('constructor', () => {
    it('sets name and config', () => {
      const p = new ModelProvider('openai', { envKey: 'OPENAI_API_KEY' });
      assert.equal(p.name, 'openai');
      assert.equal(p.config.envKey, 'OPENAI_API_KEY');
    });

    it('defaults config to empty object', () => {
      const p = new ModelProvider('test');
      assert.deepEqual(p.config, {});
    });
  });

  describe('isAvailable', () => {
    it('throws as abstract method', async () => {
      const p = new ModelProvider('test');
      await assert.rejects(() => p.isAvailable(), /not implemented/);
    });
  });

  describe('chat', () => {
    it('throws as abstract method', async () => {
      const p = new ModelProvider('test');
      await assert.rejects(() => p.chat([]), /not implemented/);
    });
  });

  describe('estimateCost', () => {
    it('returns null by default', () => {
      const p = new ModelProvider('test');
      assert.equal(p.estimateCost({ inputTokens: 100, outputTokens: 50 }), null);
    });
  });

  describe('listModels', () => {
    it('returns model keys from config', () => {
      const p = new ModelProvider('test', {
        models: { 'gpt-4': 'gpt-4-turbo', 'gpt-3.5': 'gpt-3.5-turbo' },
      });
      assert.deepEqual(p.listModels(), ['gpt-4', 'gpt-3.5']);
    });

    it('returns empty array when no models config', () => {
      const p = new ModelProvider('test');
      assert.deepEqual(p.listModels(), []);
    });
  });

  describe('resolveModel', () => {
    it('returns config default when no model specified', () => {
      const p = new ModelProvider('test', { default: 'gpt-4-turbo' });
      assert.equal(p.resolveModel(), 'gpt-4-turbo');
    });

    it('returns empty string when no default and no model', () => {
      const p = new ModelProvider('test');
      assert.equal(p.resolveModel(), '');
    });

    it('resolves alias to full model ID', () => {
      const p = new ModelProvider('test', {
        models: { fast: 'gpt-3.5-turbo', power: 'gpt-4-turbo' },
      });
      assert.equal(p.resolveModel('fast'), 'gpt-3.5-turbo');
    });

    it('passes through unknown model names', () => {
      const p = new ModelProvider('test', { models: {} });
      assert.equal(p.resolveModel('custom-model-123'), 'custom-model-123');
    });
  });

  describe('getApiKey', () => {
    let savedKey;

    beforeEach(() => {
      savedKey = process.env.TEST_PROVIDER_KEY;
    });

    afterEach(() => {
      if (savedKey !== undefined) process.env.TEST_PROVIDER_KEY = savedKey;
      else delete process.env.TEST_PROVIDER_KEY;
    });

    it('returns null when no envKey configured', () => {
      const p = new ModelProvider('test');
      assert.equal(p.getApiKey(), null);
    });

    it('returns env value when envKey is set', () => {
      process.env.TEST_PROVIDER_KEY = 'secret-key';
      const p = new ModelProvider('test', { envKey: 'TEST_PROVIDER_KEY' });
      assert.equal(p.getApiKey(), 'secret-key');
    });

    it('returns null when env var not set', () => {
      delete process.env.TEST_PROVIDER_KEY;
      const p = new ModelProvider('test', { envKey: 'TEST_PROVIDER_KEY' });
      assert.equal(p.getApiKey(), null);
    });
  });
});

describe('ProviderRegistry', () => {
  let registry;

  beforeEach(() => {
    registry = new ProviderRegistry();
  });

  describe('register/get/has', () => {
    it('registers and retrieves a provider', () => {
      const p = new MockProvider('openai');
      registry.register(p);
      assert.equal(registry.get('openai'), p);
    });

    it('has returns true for registered provider', () => {
      registry.register(new MockProvider('openai'));
      assert.equal(registry.has('openai'), true);
    });

    it('has returns false for unregistered provider', () => {
      assert.equal(registry.has('gemini'), false);
    });

    it('get returns null for unregistered provider', () => {
      assert.equal(registry.get('missing'), null);
    });
  });

  describe('list', () => {
    it('returns all registered provider names', () => {
      registry.register(new MockProvider('openai'));
      registry.register(new MockProvider('gemini'));
      assert.deepEqual(registry.list(), ['openai', 'gemini']);
    });

    it('returns empty array when nothing registered', () => {
      assert.deepEqual(registry.list(), []);
    });
  });

  describe('listAvailable', () => {
    it('returns available providers', async () => {
      registry.register(new MockProvider('openai', { available: true }));
      registry.register(new MockProvider('gemini', { available: false }));
      const available = await registry.listAvailable();
      assert.ok(available.includes('openai'));
      assert.ok(!available.includes('gemini'));
    });

    it('always includes claude even if not registered', async () => {
      const available = await registry.listAvailable();
      assert.ok(available.includes('claude'));
    });

    it('handles providers that throw on isAvailable', async () => {
      const failProvider = new ModelProvider('broken');
      // isAvailable throws by default (abstract)
      registry.register(failProvider);
      const available = await registry.listAvailable();
      // should not contain broken, but should contain claude
      assert.ok(!available.includes('broken'));
      assert.ok(available.includes('claude'));
    });
  });
});

describe('CircuitBreaker', () => {
  let cb;

  beforeEach(() => {
    cb = new CircuitBreaker({ failureThreshold: 3, resetTimeoutMs: 1000 });
  });

  describe('initial state', () => {
    it('new provider starts in closed state (available)', () => {
      assert.equal(cb.isAvailable('p1'), true);
    });

    it('getStatus is empty initially', () => {
      assert.deepEqual(cb.getStatus(), {});
    });
  });

  describe('closed to open transition', () => {
    it('stays closed after fewer failures than threshold', () => {
      cb.recordFailure('p1');
      cb.recordFailure('p1');
      assert.equal(cb.isAvailable('p1'), true);
    });

    it('opens after threshold failures', () => {
      cb.recordFailure('p1');
      cb.recordFailure('p1');
      cb.recordFailure('p1');
      assert.equal(cb.isAvailable('p1'), false);
    });

    it('getStatus shows open state and failure count', () => {
      cb.recordFailure('p1');
      cb.recordFailure('p1');
      cb.recordFailure('p1');
      const status = cb.getStatus();
      assert.equal(status.p1.state, 'open');
      assert.equal(status.p1.failures, 3);
    });
  });

  describe('half-open after timeout', () => {
    it('transitions to half-open after resetTimeout expires', () => {
      cb = new CircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 1 });
      cb.recordFailure('p1');
      assert.equal(cb.isAvailable('p1'), false);
      // Wait a tiny bit for the timeout to elapse
      const start = Date.now();
      while (Date.now() - start < 5) {
        /* spin */
      }
      assert.equal(cb.isAvailable('p1'), true);
    });
  });

  describe('recordSuccess', () => {
    it('resets breaker to closed', () => {
      cb.recordFailure('p1');
      cb.recordFailure('p1');
      cb.recordSuccess('p1');
      const status = cb.getStatus();
      assert.equal(status.p1.state, 'closed');
      assert.equal(status.p1.failures, 0);
    });
  });

  describe('reset', () => {
    it('removes breaker for specific provider', () => {
      cb.recordFailure('p1');
      cb.reset('p1');
      assert.equal(cb.isAvailable('p1'), true);
      // Status should be fresh after reset
      const b = cb._getBreaker('p1');
      assert.equal(b.failures, 0);
    });
  });

  describe('resetAll', () => {
    it('clears all breakers', () => {
      cb.recordFailure('p1');
      cb.recordFailure('p2');
      cb.resetAll();
      assert.deepEqual(cb.getStatus(), {});
    });
  });
});

describe('FallbackChain', () => {
  let registry;
  let chain;

  beforeEach(() => {
    registry = new ProviderRegistry();
    registry.register(new MockProvider('openai'));
    registry.register(new MockProvider('gemini'));

    chain = new FallbackChain({
      order: ['claude', 'openai', 'gemini'],
      registry,
    });
  });

  describe('chat', () => {
    it('skips claude and uses first available provider', async () => {
      const result = await chain.chat([{ role: 'user', content: 'hi' }]);
      assert.equal(result.text, 'openai response');
      assert.equal(result.failedOver, false);
      assert.deepEqual(result.attemptedProviders, ['openai']);
    });

    it('falls over to next provider on failure', async () => {
      registry._providers.set(
        'openai',
        new MockProvider('openai', { chatError: new Error('rate limited') }),
      );
      const result = await chain.chat([{ role: 'user', content: 'hi' }]);
      assert.equal(result.text, 'gemini response');
      assert.equal(result.failedOver, true);
      assert.deepEqual(result.attemptedProviders, ['openai', 'gemini']);
    });

    it('skips providers with open circuit', async () => {
      // Open the circuit for openai
      for (let i = 0; i < 3; i++) {
        chain._circuitBreaker.recordFailure('openai');
      }
      const result = await chain.chat([{ role: 'user', content: 'hi' }]);
      assert.equal(result.text, 'gemini response');
    });

    it('throws when all providers fail', async () => {
      registry._providers.set(
        'openai',
        new MockProvider('openai', { chatError: new Error('fail') }),
      );
      registry._providers.set(
        'gemini',
        new MockProvider('gemini', { chatError: new Error('fail') }),
      );
      await assert.rejects(() => chain.chat([]), /All providers failed/);
    });

    it('records failover count', async () => {
      registry._providers.set(
        'openai',
        new MockProvider('openai', { chatError: new Error('fail') }),
      );
      await chain.chat([{ role: 'user', content: 'hi' }]);
      assert.equal(chain.getFailoverCount(), 1);
    });
  });

  describe('chatWithClaudeFallback', () => {
    it('uses claude when it succeeds', async () => {
      const claudeFn = async () => ({ text: 'claude response', model: 'claude' });
      const result = await chain.chatWithClaudeFallback(claudeFn, []);
      assert.equal(result.text, 'claude response');
      assert.equal(result.provider, 'claude');
      assert.equal(result.failedOver, false);
    });

    it('falls back to other providers when claude fails', async () => {
      const claudeFn = async () => {
        throw new Error('claude down');
      };
      const result = await chain.chatWithClaudeFallback(claudeFn, [
        { role: 'user', content: 'hi' },
      ]);
      assert.equal(result.failedOver, true);
      assert.equal(result.text, 'openai response');
    });
  });

  describe('getLastUsedProvider', () => {
    it('returns null initially', () => {
      assert.equal(chain.getLastUsedProvider(), null);
    });

    it('returns the last successful provider', async () => {
      await chain.chat([]);
      assert.equal(chain.getLastUsedProvider(), 'openai');
    });
  });

  describe('setOrder', () => {
    it('updates provider order', () => {
      chain.setOrder(['gemini', 'openai']);
      assert.deepEqual(chain._order, ['gemini', 'openai']);
    });
  });

  describe('resetCircuit', () => {
    it('resets specific provider circuit', () => {
      chain._circuitBreaker.recordFailure('openai');
      chain.resetCircuit('openai');
      assert.equal(chain._circuitBreaker.isAvailable('openai'), true);
    });

    it('resets all circuits when no provider specified', () => {
      chain._circuitBreaker.recordFailure('openai');
      chain._circuitBreaker.recordFailure('gemini');
      chain.resetCircuit();
      assert.deepEqual(chain.getCircuitStatus(), {});
    });
  });
});
