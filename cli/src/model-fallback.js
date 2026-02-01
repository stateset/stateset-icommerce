/**
 * Model Fallback Chain for StateSet CLI
 *
 * Implements automatic fallback when primary models fail or rate-limit.
 * Tracks cooldown periods per model/key and gracefully degrades to
 * alternative models.
 *
 * Fallback chain (default):
 *   Claude Sonnet → Claude Haiku → OpenAI GPT-4o-mini
 *
 * Features:
 * - Automatic retry with exponential backoff
 * - Cooldown tracking per model/API key
 * - Rate limit detection and handling
 * - Graceful degradation with clear logging
 *
 * Usage:
 *   const fallback = new ModelFallback();
 *
 *   const result = await fallback.execute(async (model) => {
 *     return await callLLM(model, prompt);
 *   });
 */

// ============================================================================
// Constants
// ============================================================================

/**
 * Default model fallback chain
 */
export const DEFAULT_FALLBACK_CHAIN = [
  {
    id: 'claude-sonnet',
    provider: 'claude',
    model: 'claude-sonnet-4-5-20250929',
    envKey: 'ANTHROPIC_API_KEY',
    priority: 1,
    capabilities: ['tools', 'thinking', 'streaming']
  },
  {
    id: 'claude-haiku',
    provider: 'claude',
    model: 'claude-haiku-3-5-20241022',
    envKey: 'ANTHROPIC_API_KEY',
    priority: 2,
    capabilities: ['tools', 'streaming']
  },
  {
    id: 'openai-gpt4o-mini',
    provider: 'openai',
    model: 'gpt-4o-mini',
    envKey: 'OPENAI_API_KEY',
    priority: 3,
    capabilities: ['streaming']
  },
  {
    id: 'gemini-flash',
    provider: 'gemini',
    model: 'gemini-2.0-flash-exp',
    envKey: 'GOOGLE_API_KEY',
    priority: 4,
    capabilities: ['streaming']
  }
];

/**
 * Error patterns that indicate rate limiting or transient failures
 */
const RATE_LIMIT_PATTERNS = [
  /rate.?limit/i,
  /too.?many.?requests/i,
  /429/,
  /quota.?exceeded/i,
  /capacity/i,
  /overloaded/i,
  /temporarily.?unavailable/i,
  /503/,
  /504/,
  /timeout/i
];

/**
 * Error patterns that indicate permanent failures (don't retry)
 */
const PERMANENT_FAILURE_PATTERNS = [
  /invalid.?api.?key/i,
  /authentication/i,
  /unauthorized/i,
  /401/,
  /403/,
  /invalid.?model/i,
  /not.?found/i,
  /404/
];

// ============================================================================
// CooldownTracker
// ============================================================================

/**
 * Tracks cooldown periods for models/keys
 */
class CooldownTracker {
  constructor() {
    /** @type {Map<string, { until: number, reason: string, attempts: number }>} */
    this.cooldowns = new Map();
  }

  /**
   * Check if a model/key is in cooldown
   * @param {string} id - Model ID or "provider:key" identifier
   * @returns {boolean}
   */
  isInCooldown(id) {
    const entry = this.cooldowns.get(id);
    if (!entry) return false;

    if (Date.now() >= entry.until) {
      this.cooldowns.delete(id);
      return false;
    }

    return true;
  }

  /**
   * Get remaining cooldown time in ms
   * @param {string} id
   * @returns {number}
   */
  getCooldownRemaining(id) {
    const entry = this.cooldowns.get(id);
    if (!entry) return 0;
    return Math.max(0, entry.until - Date.now());
  }

  /**
   * Put a model/key in cooldown
   * @param {string} id
   * @param {number} durationMs
   * @param {string} reason
   */
  setCooldown(id, durationMs, reason) {
    const existing = this.cooldowns.get(id);
    const attempts = (existing?.attempts || 0) + 1;

    // Exponential backoff for repeated failures
    const adjustedDuration = durationMs * Math.pow(2, Math.min(attempts - 1, 5));

    this.cooldowns.set(id, {
      until: Date.now() + adjustedDuration,
      reason,
      attempts
    });
  }

  /**
   * Clear cooldown for a model/key
   * @param {string} id
   */
  clearCooldown(id) {
    this.cooldowns.delete(id);
  }

  /**
   * Get all active cooldowns
   * @returns {object[]}
   */
  getActiveCooldowns() {
    const now = Date.now();
    const active = [];

    for (const [id, entry] of this.cooldowns.entries()) {
      if (entry.until > now) {
        active.push({
          id,
          remainingMs: entry.until - now,
          reason: entry.reason,
          attempts: entry.attempts
        });
      }
    }

    return active;
  }
}

// ============================================================================
// ModelFallback
// ============================================================================

/**
 * Model Fallback Chain manager
 */
export class ModelFallback {
  /**
   * @param {object} [options]
   * @param {object[]} [options.chain] - Custom fallback chain
   * @param {number} [options.maxRetries=3] - Max retries per model
   * @param {number} [options.baseCooldownMs=60000] - Base cooldown duration (1 min)
   * @param {number} [options.retryDelayMs=1000] - Initial retry delay
   * @param {string[]} [options.requiredCapabilities] - Required model capabilities
   * @param {Function} [options.onFallback] - Callback when falling back to next model
   * @param {Function} [options.onCooldown] - Callback when model enters cooldown
   */
  constructor(options = {}) {
    this.chain = options.chain || DEFAULT_FALLBACK_CHAIN;
    this.maxRetries = options.maxRetries ?? 3;
    this.baseCooldownMs = options.baseCooldownMs ?? 60000;
    this.retryDelayMs = options.retryDelayMs ?? 1000;
    this.requiredCapabilities = options.requiredCapabilities || [];
    this.onFallback = options.onFallback;
    this.onCooldown = options.onCooldown;

    this.cooldownTracker = new CooldownTracker();

    // Filter chain based on required capabilities
    if (this.requiredCapabilities.length > 0) {
      this.chain = this.chain.filter(model =>
        this.requiredCapabilities.every(cap =>
          model.capabilities?.includes(cap)
        )
      );
    }
  }

  /**
   * Get available models (not in cooldown, has valid API key)
   * @returns {object[]}
   */
  getAvailableModels() {
    return this.chain.filter(model => {
      // Check cooldown
      if (this.cooldownTracker.isInCooldown(model.id)) {
        return false;
      }

      // Check API key availability
      if (model.envKey && !process.env[model.envKey]) {
        return false;
      }

      return true;
    });
  }

  /**
   * Execute an operation with automatic fallback
   *
   * @param {Function} operation - Async function that takes (model) and returns result
   * @param {object} [options]
   * @param {string} [options.preferredModel] - Preferred model ID to start with
   * @returns {Promise<{ result: any, model: object, attempts: object[] }>}
   */
  async execute(operation, options = {}) {
    const { preferredModel } = options;
    const attempts = [];

    // Build ordered list of models to try
    let modelsToTry = this.getAvailableModels();

    if (modelsToTry.length === 0) {
      // Provide helpful error message based on why no models are available
      const missingKeys = this.chain.filter(m => m.envKey && !process.env[m.envKey]);
      const inCooldown = this.chain.filter(m => this.cooldownTracker.isInCooldown(m.id));

      let errorMsg = 'No models available.';
      let hint = '';

      if (missingKeys.length > 0 && inCooldown.length === 0) {
        // All models missing API keys
        errorMsg = 'No API keys configured.';
        hint = `\n\nTo fix this, set up your API key:\n` +
               `  1. Run: stateset-config set-key anthropic\n` +
               `  2. Or set: export ANTHROPIC_API_KEY="sk-ant-..."\n` +
               `  3. Get key from: https://console.anthropic.com/\n\n` +
               `Run 'stateset-config show-keys' to check your configuration.`;
      } else if (inCooldown.length > 0) {
        errorMsg = 'All models are in cooldown due to rate limiting.';
        hint = '\n\nTry again in a few minutes, or use a different provider with --provider.';
      } else {
        hint = '\n\nRun stateset-doctor to diagnose the issue.';
      }

      throw new Error(errorMsg + hint);
    }

    // If preferred model specified, move it to front
    if (preferredModel) {
      const preferredIndex = modelsToTry.findIndex(m => m.id === preferredModel || m.model === preferredModel);
      if (preferredIndex > 0) {
        const [preferred] = modelsToTry.splice(preferredIndex, 1);
        modelsToTry.unshift(preferred);
      }
    }

    // Try each model in order
    for (const model of modelsToTry) {
      const modelAttempts = [];
      let lastError = null;

      // Retry loop for this model
      for (let retry = 0; retry < this.maxRetries; retry++) {
        try {
          // Execute the operation
          const startTime = Date.now();
          const result = await operation(model);
          const duration = Date.now() - startTime;

          // Success - clear any cooldown
          this.cooldownTracker.clearCooldown(model.id);

          modelAttempts.push({
            retry,
            success: true,
            duration
          });

          attempts.push({
            model: model.id,
            provider: model.provider,
            attempts: modelAttempts,
            success: true
          });

          return { result, model, attempts };
        } catch (error) {
          lastError = error;
          const errorMsg = error.message || String(error);

          modelAttempts.push({
            retry,
            success: false,
            error: errorMsg
          });

          // Check if this is a permanent failure
          if (this._isPermanentFailure(errorMsg)) {
            // Put in long cooldown and move to next model
            this.cooldownTracker.setCooldown(model.id, this.baseCooldownMs * 10, errorMsg);
            if (this.onCooldown) {
              this.onCooldown({ model, reason: errorMsg, permanent: true });
            }
            break;
          }

          // Check if this is a rate limit
          if (this._isRateLimitError(errorMsg)) {
            // Put in cooldown
            this.cooldownTracker.setCooldown(model.id, this.baseCooldownMs, errorMsg);
            if (this.onCooldown) {
              this.onCooldown({ model, reason: errorMsg, permanent: false });
            }
            break;
          }

          // For other errors, retry with backoff
          if (retry < this.maxRetries - 1) {
            const delay = this.retryDelayMs * Math.pow(2, retry);
            await this._sleep(delay);
          }
        }
      }

      // Log this model's attempts
      attempts.push({
        model: model.id,
        provider: model.provider,
        attempts: modelAttempts,
        success: false,
        error: lastError?.message
      });

      // Notify fallback
      const nextModel = modelsToTry[modelsToTry.indexOf(model) + 1];
      if (nextModel && this.onFallback) {
        this.onFallback({
          from: model,
          to: nextModel,
          reason: lastError?.message
        });
      }
    }

    // All models exhausted
    const lastAttempt = attempts[attempts.length - 1];
    throw new Error(
      `All models failed. Last error: ${lastAttempt?.error || 'Unknown error'}. ` +
      `Tried ${attempts.length} model(s): ${attempts.map(a => a.model).join(', ')}`
    );
  }

  /**
   * Execute with a specific model, no fallback
   *
   * @param {string} modelId - Model ID or model name
   * @param {Function} operation - Async function
   * @returns {Promise<any>}
   */
  async executeWithModel(modelId, operation) {
    const model = this.chain.find(m => m.id === modelId || m.model === modelId);
    if (!model) {
      throw new Error(`Unknown model: ${modelId}`);
    }

    if (this.cooldownTracker.isInCooldown(model.id)) {
      const remaining = this.cooldownTracker.getCooldownRemaining(model.id);
      throw new Error(`Model ${model.id} is in cooldown for ${Math.ceil(remaining / 1000)}s`);
    }

    return operation(model);
  }

  /**
   * Check if error indicates rate limiting
   * @private
   */
  _isRateLimitError(errorMsg) {
    return RATE_LIMIT_PATTERNS.some(pattern => pattern.test(errorMsg));
  }

  /**
   * Check if error indicates permanent failure
   * @private
   */
  _isPermanentFailure(errorMsg) {
    return PERMANENT_FAILURE_PATTERNS.some(pattern => pattern.test(errorMsg));
  }

  /**
   * Sleep helper
   * @private
   */
  _sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
  }

  /**
   * Get current status of all models
   * @returns {object[]}
   */
  getStatus() {
    return this.chain.map(model => {
      const inCooldown = this.cooldownTracker.isInCooldown(model.id);
      const cooldownRemaining = this.cooldownTracker.getCooldownRemaining(model.id);
      const hasApiKey = !model.envKey || !!process.env[model.envKey];

      return {
        id: model.id,
        provider: model.provider,
        model: model.model,
        available: !inCooldown && hasApiKey,
        inCooldown,
        cooldownRemainingMs: cooldownRemaining,
        hasApiKey,
        capabilities: model.capabilities
      };
    });
  }

  /**
   * Manually clear cooldown for a model
   * @param {string} modelId
   */
  clearModelCooldown(modelId) {
    this.cooldownTracker.clearCooldown(modelId);
  }

  /**
   * Manually set cooldown for a model
   * @param {string} modelId
   * @param {number} durationMs
   * @param {string} reason
   */
  setModelCooldown(modelId, durationMs, reason) {
    this.cooldownTracker.setCooldown(modelId, durationMs, reason);
  }
}

// ============================================================================
// Integration Helper
// ============================================================================

/**
 * Create a fallback-enabled LLM caller
 *
 * @param {object} options
 * @param {Function} options.claudeCall - Function to call Claude: (model, prompt) => result
 * @param {Function} options.openaiCall - Function to call OpenAI: (model, prompt) => result
 * @param {Function} options.geminiCall - Function to call Gemini: (model, prompt) => result
 * @param {string[]} [options.requiredCapabilities] - Required capabilities
 * @param {Function} [options.onFallback] - Fallback callback
 * @returns {ModelFallback}
 */
export function createFallbackCaller({
  claudeCall,
  openaiCall,
  geminiCall,
  requiredCapabilities = [],
  onFallback
}) {
  const fallback = new ModelFallback({
    requiredCapabilities,
    onFallback
  });

  /**
   * Call LLM with fallback
   * @param {string} prompt
   * @param {object} [options]
   * @returns {Promise<{ result: any, model: object }>}
   */
  async function call(prompt, options = {}) {
    return fallback.execute(async (model) => {
      switch (model.provider) {
        case 'claude':
          if (!claudeCall) throw new Error('Claude provider not configured');
          return claudeCall(model.model, prompt, options);

        case 'openai':
          if (!openaiCall) throw new Error('OpenAI provider not configured');
          return openaiCall(model.model, prompt, options);

        case 'gemini':
          if (!geminiCall) throw new Error('Gemini provider not configured');
          return geminiCall(model.model, prompt, options);

        default:
          throw new Error(`Unknown provider: ${model.provider}`);
      }
    }, { preferredModel: options.preferredModel });
  }

  return {
    call,
    fallback,
    getStatus: () => fallback.getStatus()
  };
}

// ============================================================================
// Exports
// ============================================================================

export default {
  ModelFallback,
  CooldownTracker,
  DEFAULT_FALLBACK_CHAIN,
  createFallbackCaller
};
