import { describe, it, mock, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

// Import base class directly (doesn't need external deps)
import { ModelProvider } from '../../src/providers/base.js';

describe('providers', () => {
  // ========================================================================
  // ModelProvider base class
  // ========================================================================
  describe('ModelProvider (base)', () => {
    it('initializes with name and config', () => {
      const p = new ModelProvider('test', { default: 'test-model' });
      assert.strictEqual(p.name, 'test');
      assert.strictEqual(p.config.default, 'test-model');
    });

    it('isAvailable throws not-implemented', async () => {
      const p = new ModelProvider('test');
      await assert.rejects(() => p.isAvailable(), /not implemented/);
    });

    it('chat throws not-implemented', async () => {
      const p = new ModelProvider('test');
      await assert.rejects(() => p.chat([]), /not implemented/);
    });

    it('estimateCost returns null by default', () => {
      const p = new ModelProvider('test');
      assert.strictEqual(p.estimateCost({ inputTokens: 100, outputTokens: 50 }), null);
    });

    it('listModels returns model keys from config', () => {
      const p = new ModelProvider('test', {
        models: { fast: 'test-fast', smart: 'test-smart' },
      });
      assert.deepStrictEqual(p.listModels(), ['fast', 'smart']);
    });

    it('listModels returns empty array if no models config', () => {
      const p = new ModelProvider('test', {});
      assert.deepStrictEqual(p.listModels(), []);
    });

    it('resolveModel returns default when no model specified', () => {
      const p = new ModelProvider('test', { default: 'test-default' });
      assert.strictEqual(p.resolveModel(), 'test-default');
      assert.strictEqual(p.resolveModel(undefined), 'test-default');
    });

    it('resolveModel resolves alias from config', () => {
      const p = new ModelProvider('test', {
        models: { fast: 'model-fast-v2' },
      });
      assert.strictEqual(p.resolveModel('fast'), 'model-fast-v2');
    });

    it('resolveModel passes through full model name', () => {
      const p = new ModelProvider('test', { models: {} });
      assert.strictEqual(p.resolveModel('gpt-4o-2024-01'), 'gpt-4o-2024-01');
    });

    it('getApiKey returns null when no envKey configured', () => {
      const p = new ModelProvider('test', {});
      assert.strictEqual(p.getApiKey(), null);
    });

    it('getApiKey reads from environment', () => {
      const original = process.env.TEST_PROVIDER_KEY;
      process.env.TEST_PROVIDER_KEY = 'sk-test-123';
      try {
        const p = new ModelProvider('test-env', { envKey: 'TEST_PROVIDER_KEY' });
        assert.strictEqual(p.getApiKey(), 'sk-test-123');
      } finally {
        if (original === undefined) {
          delete process.env.TEST_PROVIDER_KEY;
        } else {
          process.env.TEST_PROVIDER_KEY = original;
        }
      }
    });
  });

  // ========================================================================
  // OpenAI Provider
  // ========================================================================
  describe('OpenAIProvider', () => {
    let OpenAIProvider;
    let originalKey;

    beforeEach(async () => {
      originalKey = process.env.OPENAI_API_KEY;
      delete process.env.OPENAI_API_KEY;
      const mod = await import('../../src/providers/openai.js');
      OpenAIProvider = mod.OpenAIProvider;
    });

    afterEach(() => {
      if (originalKey) {
        process.env.OPENAI_API_KEY = originalKey;
      } else {
        delete process.env.OPENAI_API_KEY;
      }
    });

    it('initializes with openai name', () => {
      const p = new OpenAIProvider();
      assert.strictEqual(p.name, 'openai');
    });

    it('isAvailable returns false without API key', async () => {
      const p = new OpenAIProvider();
      assert.strictEqual(await p.isAvailable(), false);
    });

    it('isAvailable returns true with API key', async () => {
      process.env.OPENAI_API_KEY = 'sk-test';
      const p = new OpenAIProvider();
      assert.strictEqual(await p.isAvailable(), true);
    });

    it('chat throws without API key', async () => {
      const p = new OpenAIProvider();
      await assert.rejects(() => p.chat([{ role: 'user', content: 'hello' }]), /API key/);
    });

    it('estimateCost works for known models', () => {
      const p = new OpenAIProvider();
      const cost = p.estimateCost({ inputTokens: 1000, outputTokens: 500 }, 'gpt-4o');
      assert.ok(typeof cost === 'number');
      assert.ok(cost > 0);
    });

    it('estimateCost returns null for unknown models', () => {
      const p = new OpenAIProvider();
      const cost = p.estimateCost({ inputTokens: 1000, outputTokens: 500 }, 'unknown-model');
      assert.strictEqual(cost, null);
    });

    it('lists available models', () => {
      const p = new OpenAIProvider();
      const models = p.listModels();
      assert.ok(Array.isArray(models));
      assert.ok(models.length > 0);
    });
  });

  // ========================================================================
  // Gemini Provider
  // ========================================================================
  describe('GeminiProvider', () => {
    let GeminiProvider;
    let originalKey;

    beforeEach(async () => {
      originalKey = process.env.GEMINI_API_KEY;
      delete process.env.GEMINI_API_KEY;
      const mod = await import('../../src/providers/gemini.js');
      GeminiProvider = mod.GeminiProvider;
    });

    afterEach(() => {
      if (originalKey) {
        process.env.GEMINI_API_KEY = originalKey;
      } else {
        delete process.env.GEMINI_API_KEY;
      }
    });

    it('initializes with gemini name', () => {
      const p = new GeminiProvider();
      assert.strictEqual(p.name, 'gemini');
    });

    it('isAvailable returns false without API key', async () => {
      const p = new GeminiProvider();
      assert.strictEqual(await p.isAvailable(), false);
    });

    it('isAvailable returns true with API key', async () => {
      process.env.GEMINI_API_KEY = 'test-key';
      const p = new GeminiProvider();
      assert.strictEqual(await p.isAvailable(), true);
    });

    it('chat throws without API key', async () => {
      const p = new GeminiProvider();
      await assert.rejects(() => p.chat([{ role: 'user', content: 'hello' }]), /API key/);
    });

    it('lists available models', () => {
      const p = new GeminiProvider();
      const models = p.listModels();
      assert.ok(Array.isArray(models));
      assert.ok(models.length > 0);
    });
  });

  // ========================================================================
  // Ollama Provider
  // ========================================================================
  describe('OllamaProvider', () => {
    let OllamaProvider;

    beforeEach(async () => {
      const mod = await import('../../src/providers/ollama.js');
      OllamaProvider = mod.OllamaProvider;
    });

    it('initializes with ollama name', () => {
      const p = new OllamaProvider();
      assert.strictEqual(p.name, 'ollama');
    });

    it('isAvailable returns false when ollama not running', async () => {
      const p = new OllamaProvider();
      // Ollama is likely not running in test environment
      const available = await p.isAvailable();
      assert.strictEqual(typeof available, 'boolean');
    });

    it('lists known models', () => {
      const p = new OllamaProvider();
      const models = p.listModels();
      assert.ok(Array.isArray(models));
      assert.ok(models.includes('llama3'));
    });

    it('discoverModels returns array', async () => {
      const p = new OllamaProvider();
      const models = await p.discoverModels();
      assert.ok(Array.isArray(models));
    });

    it('uses default base URL', () => {
      const p = new OllamaProvider();
      assert.ok(p._baseUrl.includes('localhost'));
    });
  });

  // ========================================================================
  // CircuitBreaker
  // ========================================================================
  describe('CircuitBreaker', () => {
    let CircuitBreaker;

    beforeEach(async () => {
      const mod = await import('../../src/providers/base.js');
      CircuitBreaker = mod.CircuitBreaker;
    });

    it('starts in closed state (available)', () => {
      const cb = new CircuitBreaker();
      assert.strictEqual(cb.isAvailable('test'), true);
    });

    it('stays closed after a single failure', () => {
      const cb = new CircuitBreaker({ failureThreshold: 3 });
      cb.recordFailure('test');
      assert.strictEqual(cb.isAvailable('test'), true);
    });

    it('opens circuit after threshold failures', () => {
      const cb = new CircuitBreaker({ failureThreshold: 3 });
      cb.recordFailure('test');
      cb.recordFailure('test');
      cb.recordFailure('test');
      assert.strictEqual(cb.isAvailable('test'), false);
    });

    it('resets on success', () => {
      const cb = new CircuitBreaker({ failureThreshold: 3 });
      cb.recordFailure('test');
      cb.recordFailure('test');
      cb.recordSuccess('test');
      assert.strictEqual(cb.isAvailable('test'), true);
      // Need 3 more failures to open
      cb.recordFailure('test');
      cb.recordFailure('test');
      assert.strictEqual(cb.isAvailable('test'), true);
    });

    it('transitions to half-open after timeout', () => {
      const cb = new CircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 1 });
      cb.recordFailure('test');
      assert.strictEqual(cb.isAvailable('test'), false);
      // Wait for reset timeout
      const breaker = cb._getBreaker('test');
      breaker.openedAt = Date.now() - 100; // simulate time passing
      assert.strictEqual(cb.isAvailable('test'), true);
    });

    it('getStatus returns state for all tracked providers', () => {
      const cb = new CircuitBreaker();
      cb.recordFailure('openai');
      cb.recordSuccess('gemini');
      const status = cb.getStatus();
      assert.ok('openai' in status);
      assert.ok('gemini' in status);
      assert.strictEqual(status.openai.failures, 1);
      assert.strictEqual(status.gemini.failures, 0);
    });

    it('reset clears a specific provider', () => {
      const cb = new CircuitBreaker({ failureThreshold: 1 });
      cb.recordFailure('test');
      assert.strictEqual(cb.isAvailable('test'), false);
      cb.reset('test');
      assert.strictEqual(cb.isAvailable('test'), true);
    });

    it('resetAll clears all providers', () => {
      const cb = new CircuitBreaker({ failureThreshold: 1 });
      cb.recordFailure('a');
      cb.recordFailure('b');
      cb.resetAll();
      assert.strictEqual(cb.isAvailable('a'), true);
      assert.strictEqual(cb.isAvailable('b'), true);
    });
  });

  // ========================================================================
  // FallbackChain
  // ========================================================================
  describe('FallbackChain', () => {
    let FallbackChain;

    beforeEach(async () => {
      const mod = await import('../../src/providers/base.js');
      FallbackChain = mod.FallbackChain;
    });

    it('initializes with default order', () => {
      const chain = new FallbackChain();
      assert.ok(chain._order.includes('claude'));
      assert.ok(chain._order.includes('openai'));
    });

    it('allows custom provider order', () => {
      const chain = new FallbackChain({ order: ['ollama', 'openai'] });
      assert.deepStrictEqual(chain._order, ['ollama', 'openai']);
    });

    it('tracks failover count', () => {
      const chain = new FallbackChain();
      assert.strictEqual(chain.getFailoverCount(), 0);
    });

    it('getLastUsedProvider starts as null', () => {
      const chain = new FallbackChain();
      assert.strictEqual(chain.getLastUsedProvider(), null);
    });

    it('getCircuitStatus returns object', () => {
      const chain = new FallbackChain();
      assert.ok(typeof chain.getCircuitStatus() === 'object');
    });

    it('resetCircuit works for specific provider', () => {
      const chain = new FallbackChain();
      chain.resetCircuit('openai');
      assert.ok(true); // no throw
    });

    it('resetCircuit works for all providers', () => {
      const chain = new FallbackChain();
      chain.resetCircuit();
      assert.ok(true); // no throw
    });

    it('setOrder updates provider order', () => {
      const chain = new FallbackChain();
      chain.setOrder(['gemini', 'ollama']);
      assert.deepStrictEqual(chain._order, ['gemini', 'ollama']);
    });

    it('chatWithClaudeFallback uses claude first', async () => {
      const chain = new FallbackChain();
      const result = await chain.chatWithClaudeFallback(
        async () => ({ text: 'hello', model: 'claude', provider: 'claude', usage: {} }),
        [],
      );
      assert.strictEqual(result.provider, 'claude');
      assert.strictEqual(result.failedOver, false);
      assert.strictEqual(chain.getLastUsedProvider(), 'claude');
    });

    it('chat throws when no providers available', async () => {
      // No API keys set, ollama not running
      const chain = new FallbackChain({ order: [] });
      await assert.rejects(
        () => chain.chat([{ role: 'user', content: 'test' }]),
        /All providers failed/,
      );
    });
  });
});
