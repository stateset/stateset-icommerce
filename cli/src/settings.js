/**
 * Agent Settings Manager for StateSet iCommerce
 *
 * Loads settings from:
 * 1) Defaults (hardcoded)
 * 2) Global settings (~/.stateset/settings.json)
 * 3) Workspace settings (.stateset/settings.json)
 * 4) Explicit overrides (function argument)
 */

import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { DEFAULT_MODEL } from './config.js';
import { DEFAULT_GUARDRAILS } from './permissions.js';

// ============================================================================
// Defaults
// ============================================================================

export const DEFAULT_AGENT_SETTINGS = {
  agent: {
    default: 'customer-service',
  },
  model: {
    default: DEFAULT_MODEL,
    preferSession: true,
  },
  thinkLevel: {
    default: 'off',
  },
  provider: {
    default: 'claude',
  },
  guardrails: { ...DEFAULT_GUARDRAILS },
  contextGuard: {
    enabled: true,
    warningThreshold: 0.7,
    compactThreshold: 0.8,
    abortThreshold: 0.95,
    reserveTokens: 4096,
  },
  retry: {
    enabled: true,
    maxRetries: 2,
    baseDelayMs: 500,
    maxDelayMs: 8_000,
    jitter: 0.2,
    retryableErrors: [
      'overloaded',
      'rate limit',
      'too many requests',
      '429',
      '500',
      '502',
      '503',
      '504',
      'service unavailable',
      'server error',
      'internal error',
      'connection error',
      'connection refused',
      'fetch failed',
      'upstream connect',
      'reset before headers',
      'terminated',
      'timeout',
    ],
  },
  watchdog: {
    enabled: true,
    freshInactivityMs: 180_000,
    resumeInactivityMs: 300_000,
  },
  memory: {
    enabled: false,
    useMarkdown: true,
    maxSummaries: 5,
  },
  plugins: {
    enabled: false,
    verbose: false,
  },
  sessionStore: {
    enabled: true,
    dbPath: null,
    maxSummaries: 5,
  },
  queue: {
    maxLanes: 1000,
    laneTimeoutMs: 300_000,
    maxQueueSize: 100,
    idleCleanupMs: 3_600_000,
    parallelConcurrency: 5,
    waitWarningMs: 30_000,
    runningWarningMs: 120_000,
    warningThrottleMs: 30_000,
    monitorIntervalMs: 5_000,
    emitWarnings: true,
  },
  privacy: {
    redactLogs: true,
    redactMemory: true,
    redactHistory: false,
    redactResponse: false,
  },
};

// ============================================================================
// Helpers
// ============================================================================

function isObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value);
}

const DANGEROUS_KEYS = new Set(['__proto__', 'constructor', 'prototype']);

function mergeDeep(target, source) {
  if (!isObject(source)) return target;
  const output = { ...target };
  for (const [key, value] of Object.entries(source)) {
    if (DANGEROUS_KEYS.has(key)) continue;
    if (isObject(value) && isObject(output[key])) {
      output[key] = mergeDeep(output[key], value);
    } else {
      output[key] = value;
    }
  }
  return output;
}

function loadJson(filePath) {
  if (!filePath || !fs.existsSync(filePath)) return null;
  try {
    const raw = fs.readFileSync(filePath, 'utf-8');
    return JSON.parse(raw);
  } catch (err) {
    console.warn(`[settings] Failed to parse ${filePath}: ${err.message}`);
    return null;
  }
}

function getDefaultSettingsPaths() {
  return [
    path.join(os.homedir(), '.stateset', 'settings.json'),
    path.join(process.cwd(), '.stateset', 'settings.json'),
  ];
}

// ============================================================================
// Public API
// ============================================================================

let _cachedSettings = null;

/**
 * Load agent settings with optional overrides.
 *
 * @param {Object} [overrides]
 * @param {Object} [opts]
 * @param {boolean} [opts.reload=false]
 * @returns {Object}
 */
export function loadAgentSettings(overrides = {}, opts = {}) {
  if (_cachedSettings && !opts.reload) {
    return mergeDeep(_cachedSettings, overrides);
  }

  const settingsPath = process.env.STATESET_SETTINGS;
  const paths = settingsPath
    ? [settingsPath, ...getDefaultSettingsPaths()]
    : getDefaultSettingsPaths();

  let merged = { ...DEFAULT_AGENT_SETTINGS };

  for (const p of paths) {
    const data = loadJson(p);
    if (data) {
      merged = mergeDeep(merged, data);
    }
  }

  _cachedSettings = merged;
  return mergeDeep(merged, overrides);
}

/**
 * Clear cached settings (useful for tests).
 */
export function resetAgentSettingsCache() {
  _cachedSettings = null;
}
