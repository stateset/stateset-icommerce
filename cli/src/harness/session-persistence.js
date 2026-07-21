/**
 * Session-store persistence helpers for the Claude harness.
 *
 * Extracted from claude-harness.js. All collaborators are dependency-injected;
 * there is no module-scope state. Payload shapes are built by the callers so
 * each entry point stores exactly the fields it stored before extraction.
 */

import { redactSensitive } from '../privacy.js';
import { InactivityWatchdogError, isAbortLikeError } from '../harness-utils.js';

/** Apply memory redaction to a stored request/response string when enabled. */
export function redactStoredText(text, privacySettings) {
  return privacySettings.redactMemory && text ? redactSensitive(text, privacySettings) : text;
}

/** Normalize a response value to a string ('' stays '', null stays null). */
export function normalizeResponseText(responseText, { allowNull = false } = {}) {
  if (allowNull && (responseText === null || responseText === undefined)) return null;
  return typeof responseText === 'string' ? responseText : String(responseText || '');
}

/** Error metadata fields shared by all session-store payloads. */
export function buildErrorFields(error) {
  return {
    lastError: error ? error?.message || String(error) : null,
    lastErrorCode: error?.code || null,
    lastErrorAt: error ? Date.now() : null,
    abortedLastRun: error
      ? error instanceof InactivityWatchdogError || isAbortLikeError(error)
      : false,
  };
}

/**
 * Write a run record to the session store, preferring recordRun() and
 * falling back to upsert(). Optionally appends the compaction summary.
 * Failures are logged and swallowed, matching the original behavior.
 */
export function writeSessionRecord({
  sessionStoreInstance,
  sessionId,
  payload,
  compactionSummary = null,
  appendCompactionSummary = false,
  preferRecordRun = true,
}) {
  if (!sessionStoreInstance || !sessionId) return;
  try {
    if (preferRecordRun && typeof sessionStoreInstance.recordRun === 'function') {
      sessionStoreInstance.recordRun(sessionId, payload);
    } else if (typeof sessionStoreInstance.upsert === 'function') {
      sessionStoreInstance.upsert(sessionId, payload);
    }
    if (
      appendCompactionSummary &&
      compactionSummary &&
      typeof sessionStoreInstance.appendSummary === 'function'
    ) {
      sessionStoreInstance.appendSummary(sessionId, compactionSummary);
    }
  } catch (err) {
    console.warn('[Harness] Session store write failed:', err.message);
  }
}
