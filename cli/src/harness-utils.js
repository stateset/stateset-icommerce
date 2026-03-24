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
 * Error thrown when a harness run stops producing observable activity.
 */
export class InactivityWatchdogError extends Error {
  constructor({ timeoutMs, elapsedMs = timeoutMs, message = null } = {}) {
    super(message || `No harness activity received for ${timeoutMs}ms`);
    this.name = 'InactivityWatchdogError';
    this.code = 'WATCHDOG_TIMEOUT';
    this.timeoutMs = timeoutMs;
    this.elapsedMs = elapsedMs;
  }
}

/**
 * Detect abort-like errors emitted by AbortController-aware APIs.
 */
export function isAbortLikeError(error) {
  if (!error) return false;
  if (error.cause && error.cause !== error && isAbortLikeError(error.cause)) {
    return true;
  }
  const name = String(error.name || '');
  const code = String(error.code || '');
  const message = String(error.message || '').toLowerCase();
  return (
    name === 'AbortError' ||
    code === 'ABORT_ERR' ||
    code === 'ERR_ABORTED' ||
    message.includes('aborted') ||
    message.includes('aborterror')
  );
}

/**
 * Create an inactivity watchdog that aborts the run if no activity is observed.
 */
export function createInactivityWatchdog({
  timeoutMs = null,
  abortController = null,
  onTimeout = null,
  message = null,
} = {}) {
  let timer = null;
  let stopped = false;
  let timedOut = false;
  let timeoutError = null;
  let lastActivityAt = Date.now();

  const clearTimer = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  };

  const schedule = () => {
    clearTimer();
    if (stopped || !Number.isFinite(timeoutMs) || timeoutMs <= 0) {
      return;
    }
    timer = setTimeout(() => {
      if (stopped || timedOut) return;
      timedOut = true;
      timeoutError = new InactivityWatchdogError({
        timeoutMs,
        elapsedMs: Date.now() - lastActivityAt,
        message,
      });

      if (abortController && !abortController.signal?.aborted) {
        try {
          abortController.abort(timeoutError);
        } catch {
          // Ignore abort propagation failures.
        }
      }

      if (typeof onTimeout === 'function') {
        try {
          onTimeout(timeoutError);
        } catch (err) {
          console.error('[Harness] watchdog onTimeout error:', err?.message || err);
        }
      }
    }, timeoutMs);

    if (typeof timer.unref === 'function') {
      timer.unref();
    }
  };

  if (Number.isFinite(timeoutMs) && timeoutMs > 0) {
    schedule();
  }

  return {
    touch() {
      if (stopped || timedOut) return;
      lastActivityAt = Date.now();
      schedule();
    },
    stop() {
      stopped = true;
      clearTimer();
    },
    getElapsedMs() {
      return Date.now() - lastActivityAt;
    },
    get timedOut() {
      return timedOut;
    },
    get error() {
      return timeoutError;
    },
  };
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
