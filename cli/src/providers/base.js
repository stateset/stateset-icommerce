/**
 * Multi-Model Provider Abstraction for StateSet iCommerce
 *
 * Defines the ModelProvider interface and a ProviderRegistry singleton.
 * Non-Claude providers operate in chat-only mode (no MCP tools).
 *
 * Providers auto-register when imported. The Claude provider path uses
 * the Agent SDK directly (in claude-harness.js), so it is not registered here.
 */

import { PROVIDERS } from '../config.js';
import { resolveProviderApiKey } from '../credentials.js';

// ============================================================================
// Constants
// ============================================================================

/** Default maximum output tokens for chat completions */
export const DEFAULT_MAX_TOKENS = 4096;

// ============================================================================
// ModelProvider Base Class
// ============================================================================

/**
 * @typedef {Object} ChatMessage
 * @property {'system'|'user'|'assistant'} role
 * @property {string} content
 */

/**
 * @typedef {Object} ChatOptions
 * @property {string} [model] - Model ID override
 * @property {boolean} [stream=false] - Enable streaming
 * @property {Function} [onPartialMessage] - Streaming callback
 * @property {number} [maxTokens=4096] - Max output tokens
 * @property {number} [temperature=0.7] - Temperature
 * @property {string} [apiKey] - Override API key for this call
 * @property {AbortSignal} [signal] - Abort signal for request cancellation
 */

/**
 * @typedef {Object} ChatResult
 * @property {string} text - Response text
 * @property {string} model - Model used
 * @property {string} provider - Provider name
 * @property {number|null} cost - Estimated cost in USD (null if unknown)
 * @property {{ inputTokens: number, outputTokens: number }} usage - Token usage
 */

export class ModelProvider {
  /**
   * @param {string} name - Provider name (e.g., 'openai')
   * @param {Object} [config] - Provider config from PROVIDERS
   */
  constructor(name, config = {}) {
    this.name = name;
    this.config = config;
  }

  /**
   * Check if this provider is available (API key set, service reachable).
   * @returns {Promise<boolean>}
   */
  async isAvailable() {
    throw new Error(`${this.name}: isAvailable() not implemented`);
  }

  /**
   * Send a chat request and get a response.
   * @param {ChatMessage[]} messages
   * @param {ChatOptions} [options]
   * @returns {Promise<ChatResult>}
   */
  async chat(_messages, _options = {}) {
    throw new Error(`${this.name}: chat() not implemented`);
  }

  /**
   * Estimate cost given token usage (if pricing is known).
   * @param {{ inputTokens: number, outputTokens: number }} usage
   * @param {string} [model]
   * @returns {number|null}
   */
  estimateCost(_usage, _model = null) {
    return null;
  }

  /**
   * List available models for this provider.
   * @returns {string[]}
   */
  listModels() {
    return Object.keys(this.config.models || {});
  }

  /**
   * Resolve a model name to a full model ID.
   * @param {string} [model] - Short or full model name
   * @returns {string}
   */
  resolveModel(model) {
    if (!model) return this.config.default || '';
    // Check if it's a short alias
    if (this.config.models && this.config.models[model]) {
      return this.config.models[model];
    }
    return model;
  }

  /**
   * Get the API key from environment.
   * @returns {string|null}
   */
  getApiKey() {
    const stored = resolveProviderApiKey(this.name);
    if (stored) return stored;
    if (!this.config.envKey) return null;
    return process.env[this.config.envKey] || null;
  }
}

// ============================================================================
// ProviderRegistry
// ============================================================================

class ProviderRegistry {
  constructor() {
    /** @type {Map<string, ModelProvider>} */
    this._providers = new Map();
  }

  /**
   * Register a provider.
   * @param {ModelProvider} provider
   */
  register(provider) {
    this._providers.set(provider.name, provider);
  }

  /**
   * Get a provider by name.
   * @param {string} name
   * @returns {ModelProvider|null}
   */
  get(name) {
    return this._providers.get(name) || null;
  }

  /**
   * Check if a provider is registered.
   * @param {string} name
   * @returns {boolean}
   */
  has(name) {
    return this._providers.has(name);
  }

  /**
   * List all registered provider names.
   * @returns {string[]}
   */
  list() {
    return [...this._providers.keys()];
  }

  /**
   * List all available providers (API key set or no key required).
   * @returns {Promise<string[]>}
   */
  async listAvailable() {
    const available = [];
    for (const [name, provider] of this._providers) {
      try {
        if (await provider.isAvailable()) {
          available.push(name);
        }
      } catch (err) {
        console.debug(
          '[providers] Provider',
          name,
          'availability check failed:',
          err.message || err,
        );
      }
    }
    // Claude is always in the list (handled by Agent SDK)
    if (!available.includes('claude')) {
      available.unshift('claude');
    }
    return available;
  }

  /**
   * Get provider info for display.
   * @returns {Object[]}
   */
  getInfo() {
    const info = [
      {
        name: 'claude',
        displayName: 'Claude',
        available: !!resolveProviderApiKey('claude') || !!process.env.ANTHROPIC_API_KEY,
        models: Object.keys(PROVIDERS.claude.models),
        default: PROVIDERS.claude.default,
      },
    ];

    for (const [name, provider] of this._providers) {
      const storedKey = resolveProviderApiKey(name);
      info.push({
        name,
        displayName: provider.config.name || name,
        available: !!storedKey || !!provider.getApiKey() || !provider.config.envKey,
        models: provider.listModels(),
        default: provider.config.default,
      });
    }

    return info;
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _registry = null;
let _registryInitPromise = null;

/**
 * Get the global ProviderRegistry singleton.
 * On first call, auto-registers available providers.
 * @returns {ProviderRegistry}
 */
export function getProviderRegistry() {
  if (!_registry) {
    _registry = new ProviderRegistry();
    // Auto-register providers on first access
    _registryInitPromise = _autoRegister();
  }
  return _registry;
}

/**
 * Ensure async auto-registration has completed before provider selection.
 */
async function ensureProviderRegistryReady() {
  if (_registryInitPromise) {
    await _registryInitPromise;
  }
}

/**
 * Reset the registry (for testing).
 */
export function resetProviderRegistry() {
  _registry = null;
  _registryInitPromise = null;
  _fallbackChain = null;
}

/**
 * Auto-register available providers.
 * @private
 */
async function _autoRegister() {
  try {
    const { OpenAIProvider } = await import('./openai.js');
    _registry.register(new OpenAIProvider());
  } catch (err) {
    console.debug('[providers] OpenAI provider not available:', err.message || err);
  }

  try {
    const { GeminiProvider } = await import('./gemini.js');
    _registry.register(new GeminiProvider());
  } catch (err) {
    console.debug('[providers] Gemini provider not available:', err.message || err);
  }

  try {
    const { OllamaProvider } = await import('./ollama.js');
    _registry.register(new OllamaProvider());
  } catch (err) {
    console.debug('[providers] Ollama provider not available:', err.message || err);
  }
}

// ============================================================================
// FallbackChain — Automatic Provider Failover
// ============================================================================

/**
 * Circuit breaker states for each provider.
 * Tracks failures and prevents cascading retries.
 */
class CircuitBreaker {
  constructor({ failureThreshold = 3, resetTimeoutMs = 60_000 } = {}) {
    this._failureThreshold = failureThreshold;
    this._resetTimeoutMs = resetTimeoutMs;
    /** @type {Map<string, { failures: number, state: 'closed'|'open'|'half-open', openedAt: number }>} */
    this._breakers = new Map();
  }

  _getBreaker(provider) {
    if (!this._breakers.has(provider)) {
      this._breakers.set(provider, { failures: 0, state: 'closed', openedAt: 0 });
    }
    return this._breakers.get(provider);
  }

  /** Check if provider is available (circuit not open). */
  isAvailable(provider) {
    const b = this._getBreaker(provider);
    if (b.state === 'closed') return true;
    if (b.state === 'open') {
      // Check if enough time has passed to try again
      if (Date.now() - b.openedAt >= this._resetTimeoutMs) {
        b.state = 'half-open';
        return true;
      }
      return false;
    }
    return true; // half-open allows one attempt
  }

  /** Record a successful call — reset breaker. */
  recordSuccess(provider) {
    const b = this._getBreaker(provider);
    b.failures = 0;
    b.state = 'closed';
  }

  /** Record a failed call — increment failures, possibly open circuit. */
  recordFailure(provider) {
    const b = this._getBreaker(provider);
    b.failures++;
    if (b.failures >= this._failureThreshold) {
      b.state = 'open';
      b.openedAt = Date.now();
    }
  }

  /** Get status for all tracked providers. */
  getStatus() {
    const status = {};
    for (const [name, b] of this._breakers) {
      status[name] = { state: b.state, failures: b.failures };
    }
    return status;
  }

  /** Reset a specific provider's breaker. */
  reset(provider) {
    this._breakers.delete(provider);
  }

  /** Reset all breakers. */
  resetAll() {
    this._breakers.clear();
  }
}

/**
 * FallbackChain provides automatic failover across providers.
 *
 * Usage:
 *   const chain = getFallbackChain();
 *   const result = await chain.chat(messages, { preferredProvider: 'claude' });
 *
 * The chain tries providers in order: preferred → fallback list.
 * Uses circuit breakers to skip providers that are consistently failing.
 */
class FallbackChain {
  /**
   * @param {Object} opts
   * @param {string[]} [opts.order] - Provider priority order
   * @param {number} [opts.failureThreshold] - Failures before circuit opens
   * @param {number} [opts.resetTimeoutMs] - Time before retrying open circuit
   * @param {boolean} [opts.verbose] - Log failover events
   */
  constructor(opts = {}) {
    this._order = opts.order || ['claude', 'openai', 'gemini', 'ollama'];
    this._circuitBreaker = new CircuitBreaker({
      failureThreshold: opts.failureThreshold || 3,
      resetTimeoutMs: opts.resetTimeoutMs || 60_000,
    });
    this._verbose = opts.verbose || false;
    this._lastUsedProvider = null;
    this._failoverCount = 0;
  }

  /**
   * Send a chat request with automatic failover.
   * @param {ChatMessage[]} messages
   * @param {ChatOptions & { preferredProvider?: string }} options
   * @returns {Promise<ChatResult & { failedOver: boolean, attemptedProviders: string[] }>}
   */
  async chat(messages, options = {}) {
    const preferred = options.preferredProvider || this._order[0];
    const registry = getProviderRegistry();
    await ensureProviderRegistryReady();
    const attempted = [];

    // Build provider order: preferred first, then fallback chain
    const order = [preferred, ...this._order.filter((p) => p !== preferred)];

    for (const providerName of order) {
      // Skip if circuit breaker is open
      if (!this._circuitBreaker.isAvailable(providerName)) {
        if (this._verbose) {
          console.debug(`[FallbackChain] Skipping ${providerName} (circuit open)`);
        }
        continue;
      }

      // Skip claude in chat-only mode (claude uses Agent SDK, not chat API)
      if (providerName === 'claude') {
        // Claude is handled specially — if it fails, we fall through
        // This entry is for ordering purposes only
        continue;
      }

      const provider = registry.get(providerName);
      if (!provider) continue;

      try {
        const available = await provider.isAvailable();
        if (!available) continue;

        attempted.push(providerName);
        if (this._verbose && attempted.length > 1) {
          console.debug(`[FallbackChain] Failing over to ${providerName}`);
        }

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
        if (this._verbose) {
          console.error(`[FallbackChain] ${providerName} failed: ${err.message}`);
        }
      }
    }

    throw new Error(
      `All providers failed. Attempted: ${attempted.join(', ') || 'none available'}. ` +
        `Check API keys and provider availability.`,
    );
  }

  /**
   * Chat with Claude first (via Agent SDK), falling back to other providers.
   * This is the primary method used by the channel gateway.
   * @param {Function} claudeFn - Async function that calls Claude Agent SDK
   * @param {ChatMessage[]} fallbackMessages - Messages for fallback providers
   * @param {ChatOptions} options
   * @returns {Promise<ChatResult & { provider: string, failedOver: boolean }>}
   */
  async chatWithClaudeFallback(claudeFn, fallbackMessages, options = {}) {
    // Try Claude first (primary provider with full MCP tools)
    if (this._circuitBreaker.isAvailable('claude')) {
      try {
        const result = await claudeFn();
        this._circuitBreaker.recordSuccess('claude');
        this._lastUsedProvider = 'claude';
        return { ...result, provider: 'claude', failedOver: false };
      } catch (err) {
        this._circuitBreaker.recordFailure('claude');
        this._failoverCount++;
        if (this._verbose) {
          console.error(`[FallbackChain] Claude failed: ${err.message}, trying fallback...`);
        }
      }
    }

    // Fall back to other providers (chat-only mode, no MCP tools)
    const result = await this.chat(fallbackMessages, options);
    return { ...result, failedOver: true };
  }

  /** Get the last provider that successfully handled a request. */
  getLastUsedProvider() {
    return this._lastUsedProvider;
  }

  /** Get total number of failover events. */
  getFailoverCount() {
    return this._failoverCount;
  }

  /** Get circuit breaker status for all providers. */
  getCircuitStatus() {
    return this._circuitBreaker.getStatus();
  }

  /** Reset circuit breaker for a provider. */
  resetCircuit(provider) {
    if (provider) {
      this._circuitBreaker.reset(provider);
    } else {
      this._circuitBreaker.resetAll();
    }
  }

  /** Update the provider order. */
  setOrder(order) {
    this._order = order;
  }
}

// ============================================================================
// FallbackChain Singleton
// ============================================================================

let _fallbackChain = null;

/**
 * Get the global FallbackChain singleton.
 * @param {Object} [opts] - Options (only used on first call)
 * @returns {FallbackChain}
 */
export function getFallbackChain(opts) {
  if (!_fallbackChain) {
    _fallbackChain = new FallbackChain(opts);
  }
  return _fallbackChain;
}

export { CircuitBreaker, FallbackChain };
