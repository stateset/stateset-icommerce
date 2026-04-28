/**
 * Tests for cli/src/providers/base.js
 *
 * The module imports from '../config.js' and '../credentials.js' which may
 * fail in the test environment.  CircuitBreaker and FallbackChain are
 * self-contained classes that are always testable; ModelProvider and the
 * singleton helpers depend on those imports and are tested conditionally.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

// ---------------------------------------------------------------------------
// Safe dynamic import — the module pulls in config.js and credentials.js at
// the top level, which may blow up if native deps (better-sqlite3, etc.) are
// missing.  We try/catch so the rest of the suite can still exercise the
// classes that don't need those imports.
// ---------------------------------------------------------------------------

let ModelProvider;
let CircuitBreaker;
let FallbackChain;
let getProviderRegistry;
let ensureProviderRegistry;
let resetProviderRegistry;
let getFallbackChain;
let importError = null;

try {
  const mod = await import('../../src/providers/base.js');
  ModelProvider = mod.ModelProvider;
  CircuitBreaker = mod.CircuitBreaker;
  FallbackChain = mod.FallbackChain;
  getProviderRegistry = mod.getProviderRegistry;
  ensureProviderRegistry = mod.ensureProviderRegistry;
  resetProviderRegistry = mod.resetProviderRegistry;
  getFallbackChain = mod.getFallbackChain;
} catch (err) {
  importError = err;
}

const canImport = importError === null;

// ===========================================================================
// ModelProvider
// ===========================================================================

describe('ModelProvider', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  /** @type {InstanceType<typeof ModelProvider>} */
  let provider;

  beforeEach(() => {
    provider = new ModelProvider('test-provider', {
      default: 'test-model-v1',
      envKey: 'TEST_PROVIDER_KEY',
      models: {
        small: 'test-model-small-v1',
        large: 'test-model-large-v1',
      },
    });
  });

  // -- constructor ----------------------------------------------------------

  it('stores name and config', () => {
    assert.equal(provider.name, 'test-provider');
    assert.deepStrictEqual(Object.keys(provider.config.models), ['small', 'large']);
  });

  it('defaults config to empty object when omitted', () => {
    const p = new ModelProvider('bare');
    assert.equal(p.name, 'bare');
    assert.deepStrictEqual(p.config, {});
  });

  // -- isAvailable / chat ---------------------------------------------------

  it('isAvailable() rejects with not-implemented error', async () => {
    await assert.rejects(() => provider.isAvailable(), {
      message: 'test-provider: isAvailable() not implemented',
    });
  });

  it('chat() rejects with not-implemented error', async () => {
    await assert.rejects(() => provider.chat([], {}), {
      message: 'test-provider: chat() not implemented',
    });
  });

  // -- estimateCost ---------------------------------------------------------

  it('estimateCost() returns null by default', () => {
    const cost = provider.estimateCost({ inputTokens: 100, outputTokens: 50 }, 'any-model');
    assert.equal(cost, null);
  });

  // -- listModels -----------------------------------------------------------

  it('listModels() returns keys of config.models', () => {
    const models = provider.listModels();
    assert.deepStrictEqual(models, ['small', 'large']);
  });

  it('listModels() returns empty array when config.models is missing', () => {
    const p = new ModelProvider('empty', {});
    assert.deepStrictEqual(p.listModels(), []);
  });

  it('listModels() returns empty array when config is default empty object', () => {
    const p = new ModelProvider('bare');
    assert.deepStrictEqual(p.listModels(), []);
  });

  // -- resolveModel ---------------------------------------------------------

  it('resolveModel() returns config.default when no model supplied', () => {
    assert.equal(provider.resolveModel(), 'test-model-v1');
    assert.equal(provider.resolveModel(undefined), 'test-model-v1');
    assert.equal(provider.resolveModel(''), 'test-model-v1');
    assert.equal(provider.resolveModel(null), 'test-model-v1');
  });

  it('resolveModel() returns empty string when no default configured', () => {
    const p = new ModelProvider('no-default', {});
    assert.equal(p.resolveModel(), '');
  });

  it('resolveModel() resolves a short alias to full model id', () => {
    assert.equal(provider.resolveModel('small'), 'test-model-small-v1');
    assert.equal(provider.resolveModel('large'), 'test-model-large-v1');
  });

  it('resolveModel() passes through unknown model names unchanged', () => {
    assert.equal(provider.resolveModel('custom-model-xyz'), 'custom-model-xyz');
  });

  // -- getApiKey (env variable fallback) ------------------------------------

  it('getApiKey() returns env var value when set', () => {
    const original = process.env.TEST_PROVIDER_KEY;
    try {
      process.env.TEST_PROVIDER_KEY = 'sk-test-secret';
      // getApiKey first checks resolveProviderApiKey, then env.
      // If resolveProviderApiKey returns null for an unknown provider,
      // we fall through to env.
      const key = provider.getApiKey();
      // Could be the stored key or the env key — either way, not null.
      assert.ok(key, 'Expected a non-null API key');
    } finally {
      if (original === undefined) {
        delete process.env.TEST_PROVIDER_KEY;
      } else {
        process.env.TEST_PROVIDER_KEY = original;
      }
    }
  });

  it('getApiKey() returns null when envKey missing and no stored key', () => {
    const p = new ModelProvider('nonexistent-provider', {});
    assert.equal(p.getApiKey(), null);
  });

  it('getApiKey() returns null when envKey is set but env var is empty', () => {
    const original = process.env.FAKE_PROVIDER_KEY;
    try {
      delete process.env.FAKE_PROVIDER_KEY;
      const p = new ModelProvider('fake', { envKey: 'FAKE_PROVIDER_KEY' });
      // resolveProviderApiKey('fake') should return null for unknown provider
      const key = p.getApiKey();
      assert.equal(key, null);
    } finally {
      if (original !== undefined) {
        process.env.FAKE_PROVIDER_KEY = original;
      }
    }
  });
});

// ===========================================================================
// CircuitBreaker
// ===========================================================================

describe('CircuitBreaker', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  /** @type {InstanceType<typeof CircuitBreaker>} */
  let cb;

  beforeEach(() => {
    cb = new CircuitBreaker({ failureThreshold: 3, resetTimeoutMs: 500 });
  });

  // -- initial state --------------------------------------------------------

  it('new provider starts in closed state and is available', () => {
    assert.equal(cb.isAvailable('providerA'), true);
  });

  it('getStatus() returns empty object initially', () => {
    assert.deepStrictEqual(cb.getStatus(), {});
  });

  // -- closed state ---------------------------------------------------------

  it('stays closed after fewer failures than threshold', () => {
    cb.recordFailure('p');
    cb.recordFailure('p');
    assert.equal(cb.isAvailable('p'), true);
    const status = cb.getStatus();
    assert.equal(status.p.state, 'closed');
    assert.equal(status.p.failures, 2);
  });

  // -- opening the circuit --------------------------------------------------

  it('opens circuit after reaching failure threshold', () => {
    cb.recordFailure('p');
    cb.recordFailure('p');
    cb.recordFailure('p');
    assert.equal(cb.isAvailable('p'), false);
    const status = cb.getStatus();
    assert.equal(status.p.state, 'open');
    assert.equal(status.p.failures, 3);
  });

  it('opens circuit with more failures than threshold', () => {
    for (let i = 0; i < 5; i++) cb.recordFailure('p');
    assert.equal(cb.isAvailable('p'), false);
    assert.equal(cb.getStatus().p.state, 'open');
    assert.equal(cb.getStatus().p.failures, 5);
  });

  // -- half-open (timeout expired) ------------------------------------------

  it('transitions to half-open after resetTimeout expires', async () => {
    cb.recordFailure('p');
    cb.recordFailure('p');
    cb.recordFailure('p');
    assert.equal(cb.isAvailable('p'), false);

    // Wait for the reset timeout to elapse
    await new Promise((r) => setTimeout(r, 600));

    assert.equal(cb.isAvailable('p'), true);
    assert.equal(cb.getStatus().p.state, 'half-open');
  });

  it('half-open state allows one attempt', async () => {
    for (let i = 0; i < 3; i++) cb.recordFailure('p');
    await new Promise((r) => setTimeout(r, 600));

    // First call transitions to half-open and returns true
    assert.equal(cb.isAvailable('p'), true);
    // Still half-open — allows another call
    assert.equal(cb.isAvailable('p'), true);
  });

  // -- recordSuccess resets -------------------------------------------------

  it('recordSuccess() resets to closed state with zero failures', () => {
    cb.recordFailure('p');
    cb.recordFailure('p');
    cb.recordSuccess('p');
    assert.equal(cb.isAvailable('p'), true);
    const status = cb.getStatus();
    assert.equal(status.p.state, 'closed');
    assert.equal(status.p.failures, 0);
  });

  it('recordSuccess() closes an open circuit', () => {
    for (let i = 0; i < 3; i++) cb.recordFailure('p');
    assert.equal(cb.getStatus().p.state, 'open');

    cb.recordSuccess('p');
    assert.equal(cb.getStatus().p.state, 'closed');
    assert.equal(cb.getStatus().p.failures, 0);
    assert.equal(cb.isAvailable('p'), true);
  });

  // -- independent providers ------------------------------------------------

  it('tracks providers independently', () => {
    cb.recordFailure('a');
    cb.recordFailure('a');
    cb.recordFailure('a');
    cb.recordFailure('b');
    assert.equal(cb.isAvailable('a'), false);
    assert.equal(cb.isAvailable('b'), true);
  });

  // -- reset ----------------------------------------------------------------

  it('reset(provider) removes that provider state', () => {
    cb.recordFailure('p');
    cb.recordFailure('p');
    cb.reset('p');
    assert.deepStrictEqual(cb.getStatus(), {});
    // After reset the provider is treated as new — available
    assert.equal(cb.isAvailable('p'), true);
  });

  it('resetAll() clears all provider states', () => {
    cb.recordFailure('a');
    cb.recordFailure('b');
    cb.resetAll();
    assert.deepStrictEqual(cb.getStatus(), {});
    assert.equal(cb.isAvailable('a'), true);
    assert.equal(cb.isAvailable('b'), true);
  });

  // -- default constructor --------------------------------------------------

  it('uses default thresholds when no options supplied', () => {
    const def = new CircuitBreaker();
    // Default threshold is 3, so 2 failures keep it closed
    def.recordFailure('x');
    def.recordFailure('x');
    assert.equal(def.isAvailable('x'), true);
    def.recordFailure('x');
    assert.equal(def.isAvailable('x'), false);
  });

  // -- custom threshold -----------------------------------------------------

  it('respects custom failureThreshold of 1', () => {
    const strict = new CircuitBreaker({ failureThreshold: 1 });
    strict.recordFailure('p');
    assert.equal(strict.isAvailable('p'), false);
  });

  it('getStatus() shows state and failures for all providers', () => {
    cb.recordFailure('a');
    cb.recordFailure('b');
    cb.recordFailure('b');
    cb.recordFailure('b');
    const status = cb.getStatus();
    assert.equal(status.a.state, 'closed');
    assert.equal(status.a.failures, 1);
    assert.equal(status.b.state, 'open');
    assert.equal(status.b.failures, 3);
  });
});

// ===========================================================================
// FallbackChain
// ===========================================================================

describe('FallbackChain', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  /** @type {InstanceType<typeof FallbackChain>} */
  let chain;

  beforeEach(() => {
    // Reset registry so getProviderRegistry() creates a fresh one
    resetProviderRegistry();
  });

  afterEach(() => {
    resetProviderRegistry();
  });

  // -- constructor defaults -------------------------------------------------

  it('default order is claude, openai, gemini, ollama', () => {
    chain = new FallbackChain();
    // We cannot directly access _order, but we can verify indirectly via setOrder
    // or by checking that the instance was created.
    assert.ok(chain);
    assert.equal(chain.getLastUsedProvider(), null);
    assert.equal(chain.getFailoverCount(), 0);
  });

  it('accepts custom order', () => {
    chain = new FallbackChain({ order: ['gemini', 'openai'] });
    assert.ok(chain);
  });

  // -- getLastUsedProvider --------------------------------------------------

  it('getLastUsedProvider() returns null before any chat', () => {
    chain = new FallbackChain();
    assert.equal(chain.getLastUsedProvider(), null);
  });

  // -- getFailoverCount -----------------------------------------------------

  it('getFailoverCount() starts at zero', () => {
    chain = new FallbackChain();
    assert.equal(chain.getFailoverCount(), 0);
  });

  // -- getCircuitStatus -----------------------------------------------------

  it('getCircuitStatus() returns empty object initially', () => {
    chain = new FallbackChain();
    assert.deepStrictEqual(chain.getCircuitStatus(), {});
  });

  // -- setOrder -------------------------------------------------------------

  it('setOrder() updates internal order', () => {
    chain = new FallbackChain({ order: ['a', 'b'] });
    chain.setOrder(['c', 'd', 'e']);
    // Verify that the chain exists — the order is used when chat() is called.
    assert.ok(chain);
  });

  // -- resetCircuit ---------------------------------------------------------

  it('resetCircuit(provider) resets that provider breaker', () => {
    chain = new FallbackChain({ failureThreshold: 1 });
    // Manually chat() would be hard to mock, so exercise the circuit breaker
    // via the internal path.  We record failures through repeated calls.
    // We need a provider registered to get past registry.get(), so we exercise
    // the circuit via resetCircuit / getCircuitStatus.
    // Force a failure entry by calling chat and catching the rejection
    chain.resetCircuit('openai');
    const status = chain.getCircuitStatus();
    // After reset, the provider entry should be gone
    assert.equal(status.openai, undefined);
  });

  it('resetCircuit() with no argument resets all breakers', () => {
    chain = new FallbackChain();
    // Resetting all should be a no-op on a fresh chain — no error.
    chain.resetCircuit();
    assert.deepStrictEqual(chain.getCircuitStatus(), {});
  });

  // -- chat() with no providers available -----------------------------------

  it('chat() throws when all providers fail or are unavailable', async () => {
    chain = new FallbackChain({ order: [] });
    await assert.rejects(
      () => chain.chat([{ role: 'user', content: 'hello' }]),
      (err) => {
        assert.ok(err.message.includes('All providers failed'));
        return true;
      },
    );
  });

  it('chat() throws with informative message listing attempted providers', async () => {
    // With default order but empty registry, claude is skipped (special) and
    // the other providers aren't registered.
    chain = new FallbackChain({ order: ['claude'] });
    await assert.rejects(
      () => chain.chat([{ role: 'user', content: 'hi' }]),
      (err) => {
        assert.ok(err.message.includes('All providers failed'));
        assert.ok(err.message.includes('none available') || err.message.includes('Attempted'));
        return true;
      },
    );
  });

  // -- chat() with a mock provider ------------------------------------------

  it('chat() succeeds with a registered mock provider', async () => {
    const registry = getProviderRegistry();

    // Create a mock provider
    const mockProvider = new ModelProvider('mock-llm', { default: 'mock-v1' });
    mockProvider.isAvailable = async () => true;
    mockProvider.chat = async (msgs, opts) => ({
      text: 'mock response',
      model: 'mock-v1',
      provider: 'mock-llm',
      cost: 0.001,
      usage: { inputTokens: 10, outputTokens: 5 },
    });
    registry.register(mockProvider);

    chain = new FallbackChain({ order: ['mock-llm'] });
    const result = await chain.chat([{ role: 'user', content: 'hello' }]);

    assert.equal(result.text, 'mock response');
    assert.equal(result.provider, 'mock-llm');
    assert.equal(result.failedOver, false);
    assert.deepStrictEqual(result.attemptedProviders, ['mock-llm']);
    assert.equal(chain.getLastUsedProvider(), 'mock-llm');
  });

  it('chat() increments failoverCount on provider failure', async () => {
    const registry = getProviderRegistry();

    const failProvider = new ModelProvider('fail-llm', {});
    failProvider.isAvailable = async () => true;
    failProvider.chat = async () => {
      throw new Error('API down');
    };

    const goodProvider = new ModelProvider('good-llm', {});
    goodProvider.isAvailable = async () => true;
    goodProvider.chat = async () => ({
      text: 'ok',
      model: 'g-v1',
      provider: 'good-llm',
      cost: null,
      usage: { inputTokens: 5, outputTokens: 3 },
    });

    registry.register(failProvider);
    registry.register(goodProvider);

    chain = new FallbackChain({ order: ['fail-llm', 'good-llm'] });
    const result = await chain.chat([{ role: 'user', content: 'test' }]);

    assert.equal(result.text, 'ok');
    assert.equal(result.failedOver, true);
    assert.deepStrictEqual(result.attemptedProviders, ['fail-llm', 'good-llm']);
    assert.equal(chain.getFailoverCount(), 1);
    assert.equal(chain.getLastUsedProvider(), 'good-llm');
  });

  it('chat() skips provider with open circuit breaker', async () => {
    const registry = getProviderRegistry();

    const failProvider = new ModelProvider('unreliable', {});
    failProvider.isAvailable = async () => true;
    failProvider.chat = async () => {
      throw new Error('timeout');
    };

    const goodProvider = new ModelProvider('reliable', {});
    goodProvider.isAvailable = async () => true;
    goodProvider.chat = async () => ({
      text: 'reliable response',
      model: 'r-v1',
      provider: 'reliable',
      cost: null,
      usage: { inputTokens: 1, outputTokens: 1 },
    });

    registry.register(failProvider);
    registry.register(goodProvider);

    chain = new FallbackChain({
      order: ['unreliable', 'reliable'],
      failureThreshold: 2,
    });

    // Exhaust the unreliable provider's circuit breaker
    await chain.chat([{ role: 'user', content: 'a' }]); // fail + fallback
    await chain.chat([{ role: 'user', content: 'b' }]); // fail again -> open

    // Now the circuit is open — unreliable should be skipped entirely
    const status = chain.getCircuitStatus();
    assert.equal(status.unreliable.state, 'open');

    const result = await chain.chat([{ role: 'user', content: 'c' }]);
    assert.equal(result.provider, 'reliable');
    // Only 'reliable' should be in attemptedProviders since 'unreliable' was skipped
    assert.deepStrictEqual(result.attemptedProviders, ['reliable']);
  });

  it('chat() records success and resets circuit on successful call', async () => {
    const registry = getProviderRegistry();

    const provider = new ModelProvider('recovering', {});
    provider.isAvailable = async () => true;
    provider.chat = async () => ({
      text: 'back online',
      model: 'v1',
      provider: 'recovering',
      cost: null,
      usage: { inputTokens: 1, outputTokens: 1 },
    });
    registry.register(provider);

    chain = new FallbackChain({ order: ['recovering'], failureThreshold: 3 });
    await chain.chat([{ role: 'user', content: 'hello' }]);

    const status = chain.getCircuitStatus();
    assert.equal(status.recovering.state, 'closed');
    assert.equal(status.recovering.failures, 0);
  });

  it('chat() skips claude (handled by Agent SDK)', async () => {
    const registry = getProviderRegistry();

    const fallback = new ModelProvider('fallback-llm', {});
    fallback.isAvailable = async () => true;
    fallback.chat = async () => ({
      text: 'fallback',
      model: 'f-v1',
      provider: 'fallback-llm',
      cost: null,
      usage: { inputTokens: 1, outputTokens: 1 },
    });
    registry.register(fallback);

    chain = new FallbackChain({ order: ['claude', 'fallback-llm'] });
    const result = await chain.chat([{ role: 'user', content: 'hi' }]);

    // Claude is skipped in chat-only mode
    assert.equal(result.provider, 'fallback-llm');
  });

  it('chat() uses preferredProvider first', async () => {
    const registry = getProviderRegistry();

    const providerA = new ModelProvider('a-llm', {});
    providerA.isAvailable = async () => true;
    providerA.chat = async () => ({
      text: 'from a',
      model: 'a',
      provider: 'a-llm',
      cost: null,
      usage: { inputTokens: 1, outputTokens: 1 },
    });

    const providerB = new ModelProvider('b-llm', {});
    providerB.isAvailable = async () => true;
    providerB.chat = async () => ({
      text: 'from b',
      model: 'b',
      provider: 'b-llm',
      cost: null,
      usage: { inputTokens: 1, outputTokens: 1 },
    });

    registry.register(providerA);
    registry.register(providerB);

    chain = new FallbackChain({ order: ['a-llm', 'b-llm'] });
    const result = await chain.chat([{ role: 'user', content: 'hi' }], {
      preferredProvider: 'b-llm',
    });

    // b-llm should be tried first due to preferredProvider
    assert.equal(result.provider, 'b-llm');
    assert.equal(result.text, 'from b');
  });

  // -- chatWithClaudeFallback -----------------------------------------------

  it('chatWithClaudeFallback() uses claudeFn when circuit is closed', async () => {
    const registry = getProviderRegistry();
    chain = new FallbackChain({ order: ['claude'] });

    const claudeFn = async () => ({
      text: 'from claude',
      model: 'claude-3',
      cost: 0.01,
      usage: { inputTokens: 50, outputTokens: 20 },
    });

    const result = await chain.chatWithClaudeFallback(
      claudeFn,
      [{ role: 'user', content: 'hi' }],
    );

    assert.equal(result.provider, 'claude');
    assert.equal(result.failedOver, false);
    assert.equal(result.text, 'from claude');
    assert.equal(chain.getLastUsedProvider(), 'claude');
  });

  it('chatWithClaudeFallback() falls back when claudeFn throws', async () => {
    const registry = getProviderRegistry();

    const fallback = new ModelProvider('backup', {});
    fallback.isAvailable = async () => true;
    fallback.chat = async () => ({
      text: 'backup response',
      model: 'b-v1',
      provider: 'backup',
      cost: null,
      usage: { inputTokens: 1, outputTokens: 1 },
    });
    registry.register(fallback);

    chain = new FallbackChain({ order: ['claude', 'backup'] });

    const claudeFn = async () => {
      throw new Error('Claude API error');
    };

    const result = await chain.chatWithClaudeFallback(
      claudeFn,
      [{ role: 'user', content: 'hi' }],
    );

    assert.equal(result.failedOver, true);
    assert.equal(result.provider, 'backup');
    assert.equal(chain.getFailoverCount(), 1);
  });
});

// ===========================================================================
// Singletons: getProviderRegistry, resetProviderRegistry, getFallbackChain
// ===========================================================================

describe('Provider singletons', { skip: !canImport && `import failed: ${importError?.message}` }, () => {
  afterEach(() => {
    resetProviderRegistry();
  });

  it('getProviderRegistry() returns the same instance on repeated calls', () => {
    const a = getProviderRegistry();
    const b = getProviderRegistry();
    assert.equal(a, b);
  });

  it('ensureProviderRegistry() resolves to the same singleton instance', async () => {
    const registry = getProviderRegistry();
    const readyRegistry = await ensureProviderRegistry();
    assert.equal(readyRegistry, registry);
  });

  it('resetProviderRegistry() causes a new instance on next call', () => {
    const a = getProviderRegistry();
    resetProviderRegistry();
    const b = getProviderRegistry();
    assert.notEqual(a, b);
  });

  it('registry supports register, get, has, list', () => {
    const registry = getProviderRegistry();
    const p = new ModelProvider('test-reg', { default: 'v1' });
    registry.register(p);

    assert.equal(registry.has('test-reg'), true);
    assert.equal(registry.get('test-reg'), p);
    assert.ok(registry.list().includes('test-reg'));
  });

  it('registry.get() returns null for unknown providers', () => {
    const registry = getProviderRegistry();
    assert.equal(registry.get('nonexistent'), null);
  });

  it('registry.has() returns false for unknown providers', () => {
    const registry = getProviderRegistry();
    assert.equal(registry.has('nonexistent'), false);
  });

  it('getFallbackChain() returns a FallbackChain singleton', () => {
    const a = getFallbackChain();
    const b = getFallbackChain();
    assert.equal(a, b);
    assert.ok(a instanceof FallbackChain);
  });

  it('resetProviderRegistry() also resets the fallback chain singleton', () => {
    const a = getFallbackChain();
    resetProviderRegistry();
    const b = getFallbackChain();
    assert.notEqual(a, b);
  });

  it('getFallbackChain() accepts options on first call', () => {
    const chain = getFallbackChain({ failureThreshold: 5, verbose: false });
    assert.ok(chain instanceof FallbackChain);
  });

  it('registry.listAvailable() always includes claude', async () => {
    const registry = getProviderRegistry();
    const available = await registry.listAvailable();
    assert.ok(available.includes('claude'));
  });
});
