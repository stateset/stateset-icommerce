/**
 * Centralized configuration for StateSet iCommerce CLI
 *
 * This file contains all configurable settings in one place,
 * making it easy to update model versions, defaults, and feature flags.
 */

// =============================================================================
// MODEL CONFIGURATION
// =============================================================================

/**
 * Available Claude models with their characteristics
 */
export const MODELS = {
  // Primary models
  SONNET: 'claude-sonnet-4-20250514',
  OPUS: 'claude-opus-4-20250514',
  HAIKU: 'claude-haiku-3-5-20241022',

  // Aliases for convenience
  DEFAULT: 'claude-sonnet-4-20250514',
  FAST: 'claude-haiku-3-5-20241022',
  POWERFUL: 'claude-opus-4-20250514',
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
  'customer-service': null,  // Uses DEFAULT_MODEL
  'checkout': null,
  'orders': null,
  'inventory': null,
  'returns': null,
  'analytics': null,
  'storefront': null,
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

export const CLI_VERSION = '0.1.2';

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
    ...overrides,
  };
}
