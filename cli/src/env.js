/**
 * Centralized environment variable validation.
 *
 * Provides Zod-validated, typed access to all environment configuration.
 * Import `env` for validated values; call `validateEnv()` at startup
 * to surface configuration problems early.
 */

import { z } from 'zod';

// ============================================================================
// Parsing helpers
// ============================================================================

/**
 * Parse an env string as a boolean.
 * Accepts: 'true', '1', 'yes', 'on' → true (case-insensitive).
 * Everything else (including undefined) → false.
 */
function envBool(value) {
  if (!value) return false;
  return ['true', '1', 'yes', 'on'].includes(value.trim().toLowerCase());
}

/**
 * Parse an env string as an integer with bounds.
 * Returns `fallback` on missing/invalid/out-of-range values.
 */
function envInt(value, fallback, min = 0, max = Number.MAX_SAFE_INTEGER) {
  if (!value) return fallback;
  const n = parseInt(value, 10);
  if (!Number.isFinite(n)) return fallback;
  return Math.max(min, Math.min(max, n));
}

/**
 * Parse an env string as a float with bounds.
 */
function envFloat(value, fallback, min = 0, max = Number.MAX_SAFE_INTEGER) {
  if (!value) return fallback;
  const n = parseFloat(value);
  if (!Number.isFinite(n)) return fallback;
  return Math.max(min, Math.min(max, n));
}

// ============================================================================
// Schema
// ============================================================================

/**
 * Zod schema for the full environment configuration.
 *
 * Only secrets that are genuinely required at startup are `.min(1)`;
 * optional API keys default to undefined so features degrade gracefully.
 */
const EnvSchema = z.object({
  // -- Logging & output -------------------------------------------------------
  LOG_LEVEL: z.enum(['trace', 'debug', 'info', 'warn', 'error', 'fatal', 'silent']).default('info'),
  LOG_FORMAT: z.enum(['json', 'text']).default('text'),
  DEBUG: z.boolean().default(false),
  NO_COLOR: z.boolean().default(false),
  FORCE_COLOR: z.boolean().default(false),

  // -- API keys (optional — features degrade without them) --------------------
  ANTHROPIC_API_KEY: z.string().optional(),
  OPENAI_API_KEY: z.string().optional(),
  ELEVENLABS_API_KEY: z.string().optional(),
  ELEVENLABS_VOICE_ID: z.string().optional(),

  // -- Permission limits -----------------------------------------------------
  STATESET_MAX_MUTATIONS: z.number().int().min(1).max(10_000).default(50),
  STATESET_MAX_MONETARY: z.number().min(0).max(1_000_000_000).default(10_000),

  // -- Concurrency limits -----------------------------------------------------
  STATESET_INVENTORY_STOCK_CONCURRENCY: z.number().int().min(1).max(32).default(8),

  // -- Paths ------------------------------------------------------------------
  DATABASE_PATH: z.string().default('./store.db'),
  STATESET_SETTINGS: z.string().optional(),
  STATESET_POLICY_DIR: z.string().optional(),
  STATESET_CONFIG_DIR: z.string().default('.stateset'),

  // -- Auth -------------------------------------------------------------------
  STATESET_API_KEY: z.string().optional(),
  STATESET_JWT: z.string().optional(),
  STATESET_CREDENTIALS_ENCRYPTION_KEY: z.string().optional(),

  // -- Audit signing ----------------------------------------------------------
  STATESET_AGENTIC_AUDIT_SIGNING_KEY: z.string().optional(),
  STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID: z.string().default('stateset-default'),

  // -- Treasury ---------------------------------------------------------------
  TREASURY_AGENT: z.string().default('default'),
  TREASURY_DB: z.string().optional(),
  TREASURY_BILLING: z.boolean().default(false),
  TREASURY_CHAIN: z.string().optional(),
  TREASURY_TOKEN: z.string().optional(),
  TREASURY_LLM_BILLING: z.boolean().default(false),
  TREASURY_ERC8004_REGISTRY: z.string().optional(),
  TREASURY_ERC8004_DB: z.string().optional(),

  // -- x402 Protocol ----------------------------------------------------------
  X402_ENABLE: z.boolean().default(false),
  X402_SEQUENCER_URL: z.string().url().optional(),

  // -- Feature flags ----------------------------------------------------------
  STATESET_ALLOW_PRIVATE_BROWSER_URLS: z.boolean().default(false),
  STATESET_MCP_STRUCTURED_TOOL_RESULTS: z.boolean().default(false),
  VES_DEBUG: z.boolean().default(false),
});

// ============================================================================
// Build the validated env object
// ============================================================================

function buildEnv() {
  const raw = process.env;
  return EnvSchema.parse({
    // Logging
    LOG_LEVEL: raw.LOG_LEVEL || 'info',
    LOG_FORMAT: raw.LOG_FORMAT === 'json' ? 'json' : 'text',
    DEBUG: envBool(raw.DEBUG),
    NO_COLOR: raw.NO_COLOR !== undefined,
    FORCE_COLOR: envBool(raw.FORCE_COLOR),

    // API keys
    ANTHROPIC_API_KEY: raw.ANTHROPIC_API_KEY || undefined,
    OPENAI_API_KEY: raw.OPENAI_API_KEY || undefined,
    ELEVENLABS_API_KEY: raw.ELEVENLABS_API_KEY || undefined,
    ELEVENLABS_VOICE_ID: raw.ELEVENLABS_VOICE_ID || undefined,

    // Limits
    STATESET_MAX_MUTATIONS: envInt(raw.STATESET_MAX_MUTATIONS, 50, 1, 10_000),
    STATESET_MAX_MONETARY: envFloat(raw.STATESET_MAX_MONETARY, 10_000, 0, 1_000_000_000),
    STATESET_INVENTORY_STOCK_CONCURRENCY: envInt(
      raw.STATESET_INVENTORY_STOCK_CONCURRENCY,
      8,
      1,
      32,
    ),

    // Paths
    DATABASE_PATH: raw.DATABASE_PATH || './store.db',
    STATESET_SETTINGS: raw.STATESET_SETTINGS || undefined,
    STATESET_POLICY_DIR: raw.STATESET_POLICY_DIR || undefined,
    STATESET_CONFIG_DIR: raw.STATESET_CONFIG_DIR || '.stateset',

    // Auth
    STATESET_API_KEY: raw.STATESET_API_KEY || undefined,
    STATESET_JWT: raw.STATESET_JWT || undefined,
    STATESET_CREDENTIALS_ENCRYPTION_KEY: raw.STATESET_CREDENTIALS_ENCRYPTION_KEY || undefined,

    // Audit
    STATESET_AGENTIC_AUDIT_SIGNING_KEY:
      raw.STATESET_AGENTIC_AUDIT_SIGNING_KEY || raw.STATESET_AUDIT_SIGNING_KEY || undefined,
    STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID:
      raw.STATESET_AGENTIC_AUDIT_SIGNING_KEY_ID || 'stateset-default',

    // Treasury
    TREASURY_AGENT: raw.TREASURY_AGENT || 'default',
    TREASURY_DB: raw.TREASURY_DB || undefined,
    TREASURY_BILLING: envBool(raw.TREASURY_BILLING),
    TREASURY_CHAIN: raw.TREASURY_CHAIN || undefined,
    TREASURY_TOKEN: raw.TREASURY_TOKEN || undefined,
    TREASURY_LLM_BILLING: envBool(raw.TREASURY_LLM_BILLING),
    TREASURY_ERC8004_REGISTRY: raw.TREASURY_ERC8004_REGISTRY || undefined,
    TREASURY_ERC8004_DB: raw.TREASURY_ERC8004_DB || undefined,

    // x402
    X402_ENABLE: envBool(raw.X402_ENABLE),
    X402_SEQUENCER_URL: raw.X402_SEQUENCER_URL || undefined,

    // Feature flags
    STATESET_ALLOW_PRIVATE_BROWSER_URLS: envBool(raw.STATESET_ALLOW_PRIVATE_BROWSER_URLS),
    // Support legacy typo (STATESSET_*) for backwards compatibility
    STATESET_MCP_STRUCTURED_TOOL_RESULTS: envBool(
      raw.STATESET_MCP_STRUCTURED_TOOL_RESULTS || raw.STATESSET_MCP_STRUCTURED_TOOL_RESULTS,
    ),
    VES_DEBUG: envBool(raw.VES_DEBUG),
  });
}

/** Validated environment configuration — lazy-initialized on first access. */
let _cached;

export function getEnv() {
  if (!_cached) {
    _cached = buildEnv();
  }
  return _cached;
}

/**
 * Validate environment variables at startup and warn about issues.
 *
 * Returns `{ valid: boolean, warnings: string[] }`.
 * Does NOT throw — callers decide whether to abort.
 */
export function validateEnv() {
  const warnings = [];

  try {
    const env = getEnv();

    // Warn if no AI provider key is set
    if (!env.ANTHROPIC_API_KEY && !env.OPENAI_API_KEY) {
      warnings.push(
        'No AI provider API key set (ANTHROPIC_API_KEY or OPENAI_API_KEY). AI features will be unavailable.',
      );
    }

    // Warn about high permission limits
    if (env.STATESET_MAX_MONETARY > 100_000) {
      warnings.push(
        `STATESET_MAX_MONETARY is set to ${env.STATESET_MAX_MONETARY}. Consider a lower limit for safety.`,
      );
    }

    return { valid: true, warnings };
  } catch (err) {
    if (err instanceof z.ZodError) {
      for (const issue of err.issues) {
        warnings.push(`env.${issue.path.join('.')}: ${issue.message}`);
      }
    } else {
      warnings.push(`Environment validation failed: ${err.message}`);
    }
    return { valid: false, warnings };
  }
}

// Re-export helpers for use in other modules that need ad-hoc parsing
export { envBool, envInt, envFloat };
