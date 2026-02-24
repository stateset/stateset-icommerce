/**
 * Centralized configuration for StateSet iCommerce CLI
 *
 * This file contains all configurable settings in one place,
 * making it easy to update model versions, defaults, and feature flags.
 */

// Auto-load ~/.stateset/.env before anything checks for API keys
import './load-env.js';

// =============================================================================
// MODEL CONFIGURATION
// =============================================================================

/**
 * Available Claude models with their characteristics
 */
export const MODELS = {
  // Primary models
  SONNET: 'claude-sonnet-4-5-20250929',
  OPUS: 'claude-opus-4-5-20251101',
  HAIKU: 'claude-haiku-3-5-20241022',

  // Aliases for convenience
  DEFAULT: 'claude-sonnet-4-5-20250929',
  FAST: 'claude-haiku-3-5-20241022',
  POWERFUL: 'claude-opus-4-5-20251101',
};

/**
 * Default model for all agent operations
 * Change this single value to update the default model across all CLI tools
 */
export const DEFAULT_MODEL = MODELS.SONNET;

/**
 * Model recommendations by use case
 * Agents can use these to select appropriate models for their tasks
 */
export const MODEL_FOR_TASK = {
  // Simple routing and classification
  routing: MODELS.HAIKU,

  // Standard agent operations
  agent: MODELS.SONNET,

  // Complex analysis and reasoning
  analysis: MODELS.SONNET,

  // Creative and nuanced tasks
  creative: MODELS.OPUS,
};

/**
 * Per-agent model overrides (optional)
 * Set to null to use DEFAULT_MODEL
 */
export const AGENT_MODELS = {
  'customer-service': null, // Uses DEFAULT_MODEL
  checkout: null,
  orders: null,
  inventory: null,
  returns: null,
  analytics: null,
  storefront: null,
};

/**
 * Get the model for a specific agent
 * @param {string} agentName - The agent name
 * @returns {string} The model ID to use
 */
export function getModelForAgent(agentName) {
  return AGENT_MODELS[agentName] || DEFAULT_MODEL;
}

// =============================================================================
// DATABASE CONFIGURATION
// =============================================================================

export const DEFAULT_DB_PATH = './store.db';

// =============================================================================
// CLI CONFIGURATION
// =============================================================================

export const CLI_VERSION = '0.7.6';

export const CLI_DEFAULTS = {
  dbPath: DEFAULT_DB_PATH,
  model: DEFAULT_MODEL,
  apply: false,
  json: false,
  verbose: false,
};

// =============================================================================
// AGENT CONFIGURATION
// =============================================================================

export const AGENT_DEFAULTS = {
  maxTurns: 10,
  verbose: false,
};

// =============================================================================
// EXTENDED THINKING CONFIGURATION
// =============================================================================

/**
 * Extended thinking levels — maps user-facing names to maxThinkingTokens.
 * The Agent SDK passes `maxThinkingTokens` to `--max-thinking-tokens`.
 */
export const THINK_LEVELS = {
  off: 0,
  low: 10_000,
  medium: 50_000,
  high: 100_000,
};

// =============================================================================
// STREAMING CONFIGURATION
// =============================================================================

export const STREAMING_DEFAULTS = {
  enabled: false,
};

// =============================================================================
// BUDGET CONFIGURATION
// =============================================================================

export const BUDGET_DEFAULTS = {
  maxBudgetUsd: null,
};

// =============================================================================
// MULTI-MODEL PROVIDER CONFIGURATION
// =============================================================================

export const PROVIDERS = {
  claude: {
    name: 'Claude',
    models: {
      'claude-sonnet-4-5': 'claude-sonnet-4-5-20250929',
      'claude-opus-4-5': 'claude-opus-4-5-20251101',
      'claude-haiku-3-5': 'claude-haiku-3-5-20241022',
    },
    default: 'claude-sonnet-4-5-20250929',
    envKey: 'ANTHROPIC_API_KEY',
  },
  openai: {
    name: 'OpenAI',
    models: {
      'gpt-4o': 'gpt-4o',
      'gpt-4': 'gpt-4',
      o1: 'o1',
      'o1-mini': 'o1-mini',
    },
    default: 'gpt-4o',
    envKey: 'OPENAI_API_KEY',
  },
  gemini: {
    name: 'Gemini',
    models: {
      'gemini-2.0-flash': 'gemini-2.0-flash',
      'gemini-2.0-pro': 'gemini-2.0-pro',
    },
    default: 'gemini-2.0-flash',
    envKey: 'GEMINI_API_KEY',
  },
  ollama: {
    name: 'Ollama',
    models: {},
    default: 'llama3',
    envKey: null,
    baseUrl: 'http://localhost:11434',
  },
};

// =============================================================================
// MEMORY CONFIGURATION
// =============================================================================

export const MEMORY_DEFAULTS = {
  enabled: false,
  maxSummaries: 5,
  summaryModel: MODELS.HAIKU,
  dbPath: null,
};

// =============================================================================
// HEARTBEAT CONFIGURATION
// =============================================================================

export const HEARTBEAT_DEFAULTS = {
  enabled: false,
  verbose: false,
  checks: null, // null = use built-in defaults
};

// =============================================================================
// HTTP GATEWAY CONFIGURATION
// =============================================================================

export const HTTP_GATEWAY_DEFAULTS = {
  enabled: true,
  port: 8080,
  host: '127.0.0.1',
  apiKeys: [], // when empty, protected routes return 401 (health/ready still public)
  allowAnonymous: false, // insecure: allow requests without keys (see http-auth.js)
  anonymousIdentity: { name: 'anonymous', level: 'admin' },
  allowQueryParamAuth: false, // insecure: allow ?api_key=... auth
  corsOrigins: ['loopback'], // allow browser requests from localhost/127.0.0.1 origins
  allowRemoteAdminEndpoints: false, // restrict /daemon and /remote-access to loopback by default
  sandbox: null, // null = no restrictions
};

// =============================================================================
// FEATURE FLAGS
// =============================================================================

export const FEATURES = {
  // Enable experimental semantic routing (requires embeddings)
  semanticRouting: false,

  // Enable retry logic for transient failures
  retryOnFailure: true,

  // Maximum retries for transient failures
  maxRetries: 3,

  // Enable telemetry by default
  telemetryEnabled: true,

  // v0.2.8 features
  extendedThinking: true,
  streaming: true,
  multiModel: true,
  memory: true,
  budgetControls: true,
  webchat: true,
};

// =============================================================================
// HELP TEXT GENERATION
// =============================================================================

/**
 * Generate the model option help text
 * @returns {string} Help text for --model option
 */
export function getModelHelpText() {
  return `Claude model to use (default: ${DEFAULT_MODEL})`;
}

/**
 * Generate CLI options for parseArgs with centralized defaults
 * @param {Object} overrides - Override specific options
 * @returns {Object} Options object for parseArgs
 */
export function getParseArgsOptions(overrides = {}) {
  return {
    db: { type: 'string', default: CLI_DEFAULTS.dbPath },
    apply: { type: 'boolean', default: CLI_DEFAULTS.apply },
    model: { type: 'string', default: CLI_DEFAULTS.model },
    resume: { type: 'string' },
    json: { type: 'boolean', default: CLI_DEFAULTS.json },
    help: { type: 'boolean', short: 'h', default: false },
    version: { type: 'boolean', short: 'v', default: false },
    // v0.2.8: Extended thinking, streaming, multi-model, budget, memory
    think: { type: 'string', default: 'off' },
    stream: { type: 'boolean', default: false },
    provider: { type: 'string', default: 'claude' },
    budget: { type: 'string' },
    memory: { type: 'boolean', default: false },
    ...overrides,
  };
}
