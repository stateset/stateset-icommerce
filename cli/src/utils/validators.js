/**
 * Input validators for CLI flags.
 *
 * Each validator returns { valid: boolean, error?: string, warning?: string }.
 */

export const VALID_FORMATS = ['table', 'json', 'csv', 'yaml'];
export const VALID_THINK_LEVELS = ['off', 'low', 'medium', 'high'];
export const VALID_PROVIDERS = ['claude', 'openai', 'gemini', 'ollama'];

/**
 * @param {string} fmt
 * @returns {{ valid: boolean, error?: string }}
 */
export function validateFormat(fmt) {
  if (VALID_FORMATS.includes(fmt)) return { valid: true };
  return {
    valid: false,
    error: `Invalid format '${fmt}'. Valid formats: ${VALID_FORMATS.join(', ')}`,
  };
}

/**
 * @param {string} val - raw string from parseArgs
 * @returns {{ valid: boolean, error?: string }}
 */
export function validateBudget(val) {
  const stripped = String(val).replace(/^\$/, '');
  const num = Number(stripped);
  if (!Number.isFinite(num) || num <= 0) {
    return {
      valid: false,
      error: `Invalid budget '${val}'. Must be a positive amount (e.g., --budget 1.00)`,
    };
  }
  return { valid: true };
}

/**
 * @param {string} provider
 * @returns {{ valid: boolean, error?: string }}
 */
export function validateProvider(provider) {
  if (VALID_PROVIDERS.includes(provider)) return { valid: true };
  return {
    valid: false,
    error: `Unknown provider '${provider}'. Valid providers: ${VALID_PROVIDERS.join(', ')}`,
  };
}

/**
 * Soft validation — warns but does not reject unknown models.
 * @param {string} model
 * @returns {{ valid: boolean, warning?: string }}
 */
export function validateModel(model) {
  const knownPrefixes = ['claude-', 'gpt-', 'gemini-', 'o1-', 'o3-', 'o4-'];
  if (knownPrefixes.some((p) => model.startsWith(p))) return { valid: true };
  // Ollama models can be anything — just warn
  return {
    valid: true,
    warning: `Model '${model}' is not a recognized default. This may work with ollama or a custom provider.`,
  };
}

/**
 * @param {string} level
 * @returns {{ valid: boolean, error?: string }}
 */
export function validateThinkLevel(level) {
  if (VALID_THINK_LEVELS.includes(level)) return { valid: true };
  return {
    valid: false,
    error: `Invalid think level '${level}'. Valid levels: ${VALID_THINK_LEVELS.join(', ')}`,
  };
}
