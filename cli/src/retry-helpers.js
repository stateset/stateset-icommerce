/**
 * Retry helpers for the Claude Agent harness.
 *
 * Provides exponential back-off with jitter for transient errors
 * encountered during agent loop execution.
 */

import crypto from 'crypto';

// ============================================================================
// Retry Helpers
// ============================================================================

/**
 * Determine whether an error is retryable based on configured patterns.
 *
 * The error message is compared (case-insensitively) against each pattern
 * in `retrySettings.retryableErrors`.
 *
 * @param {Error|string} error - The error to check
 * @param {Object} [retrySettings]
 * @param {string[]} [retrySettings.retryableErrors] - Substring patterns that indicate a retryable error
 * @returns {boolean} True if the error matches a retryable pattern
 */
export function isRetryableError(error, retrySettings) {
  if (!error) return false;
  const message = typeof error === 'string' ? error : error.message || '';
  const patterns = retrySettings?.retryableErrors || [];
  const lower = message.toLowerCase();
  return patterns.some((p) => lower.includes(String(p).toLowerCase()));
}

/**
 * Compute the retry delay for a given attempt using exponential back-off
 * with optional jitter.
 *
 * delay = min(baseDelayMs * 2^(attempt-1), maxDelayMs) * (1 + random jitter)
 *
 * @param {number} attempt - The current retry attempt (1-based)
 * @param {Object} retrySettings
 * @param {number} [retrySettings.baseDelayMs=500]  - Base delay in ms
 * @param {number} [retrySettings.maxDelayMs=8000]   - Maximum delay cap in ms
 * @param {number} [retrySettings.jitter=0]           - Jitter factor (0-1)
 * @returns {number} Delay in milliseconds
 */
export function computeRetryDelay(attempt, retrySettings) {
  const base = retrySettings.baseDelayMs || 500;
  const max = retrySettings.maxDelayMs || 8000;
  const jitter = retrySettings.jitter || 0;
  let delay = base * 2 ** Math.max(0, attempt - 1);
  delay = Math.min(delay, max);
  if (jitter > 0) {
    const rand = ((crypto.getRandomValues(new Uint32Array(1))[0] / 0xffffffff) * 2 - 1) * jitter;
    delay = Math.max(0, Math.floor(delay * (1 + rand)));
  }
  return delay;
}

/**
 * Promise-based delay.
 *
 * @param {number} ms - Milliseconds to sleep
 * @returns {Promise<void>}
 */
export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
