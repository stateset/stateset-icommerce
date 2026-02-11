/**
 * Shared runtime helpers for harness execution.
 */

import { resolveProviderApiKey } from './credentials.js';

/**
 * Build environment variables for Claude SDK calls.
 *
 * Explicit API key override takes precedence, followed by existing environment
 * variables, then locally configured provider credentials.
 */
export function buildClaudeEnv({ env: envOverrides = null, apiKey = null } = {}) {
  const env = { ...process.env, ...(envOverrides || {}) };
  if (apiKey) {
    env.ANTHROPIC_API_KEY = apiKey;
    return env;
  }
  if (!env.ANTHROPIC_API_KEY) {
    const storedKey = resolveProviderApiKey('claude');
    if (storedKey) env.ANTHROPIC_API_KEY = storedKey;
  }
  return env;
}

/**
 * Normalize an abort controller/signal pair into a single controller reference.
 */
export function normalizeAbortController({ abortController = null, signal = null } = {}) {
  if (abortController) return abortController;
  if (!signal) return null;
  const controller = new AbortController();
  if (signal.aborted) {
    controller.abort(signal.reason);
    return controller;
  }
  signal.addEventListener('abort', () => controller.abort(signal.reason), { once: true });
  return controller;
}

/**
 * Emit lifecycle event safely without failing the harness run.
 */
export function emitEvent(onEvent, event) {
  if (typeof onEvent !== 'function') return;
  try {
    const result = onEvent(event);
    if (result && typeof result.catch === 'function') {
      result.catch((err) => {
        console.error('[Harness] onEvent error:', err?.message || err);
      });
    }
  } catch (err) {
    console.error('[Harness] onEvent error:', err?.message || err);
  }
}
