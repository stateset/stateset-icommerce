/**
 * Shared run-setup helpers for the Claude harness entry points
 * (runAgentLoop, runAgentStream, createAgentStreamSession).
 *
 * Extracted from claude-harness.js. All collaborators are dependency-injected;
 * there is no module-scope state.
 */

import path from 'path';
import { getAgentSessionStore } from '../agent-session-store.js';
import { redactSensitive, redactObject } from '../privacy.js';
import { normalizeAbortController } from '../harness-utils.js';

/**
 * Resolve the on-disk policy store directory for a run.
 */
export function resolvePolicyStorePath(dbPath, override = null) {
  if (override) return override;
  if (process.env.STATESET_POLICY_DIR) return process.env.STATESET_POLICY_DIR;
  const resolvedDbPath = dbPath ? path.resolve(dbPath) : path.resolve('./store.db');
  return path.join(path.dirname(resolvedDbPath), '.stateset');
}

/**
 * Build event redaction helpers from privacy settings.
 */
export function createEventRedactors(privacySettings) {
  const eventRedact = privacySettings.redactLogs;
  return {
    redactEventText: (text) => (eventRedact ? redactSensitive(text, privacySettings) : text),
    redactEventValue: (value) => (eventRedact ? redactObject(value, privacySettings) : value),
  };
}

/**
 * Resolve the agent session store instance (explicit override first, then
 * the shared store unless disabled). Warns and degrades to null on failure.
 */
export function initSessionStore({ sessionStore, resolvedSettings, fallbackMaxSummaries }) {
  const useSessionStore = resolvedSettings.sessionStore?.enabled !== false;
  let sessionStoreInstance = sessionStore || null;
  if (!sessionStoreInstance && useSessionStore) {
    try {
      sessionStoreInstance = getAgentSessionStore({
        dbPath: resolvedSettings.sessionStore?.dbPath || undefined,
        maxSummaries: resolvedSettings.sessionStore?.maxSummaries || fallbackMaxSummaries || 5,
      });
    } catch (err) {
      console.warn('[Harness] Session store unavailable:', err.message);
      sessionStoreInstance = null;
    }
  }
  return sessionStoreInstance;
}

/**
 * Load stored session metadata for a resumed session (when enabled).
 */
export function loadSessionMeta({ resumeSessionId, sessionStoreInstance, resolvedSettings }) {
  let sessionMeta = null;
  if (resumeSessionId && sessionStoreInstance && resolvedSettings.model?.preferSession !== false) {
    try {
      sessionMeta = sessionStoreInstance.get(resumeSessionId);
    } catch (err) {
      console.warn('[Harness] Session store read failed:', err.message);
      sessionMeta = null;
    }
  }
  return sessionMeta;
}

/**
 * Apply restored session metadata over the effective run parameters.
 * Explicit caller-provided values always win over stored metadata.
 */
export function applySessionMeta({
  sessionMeta,
  provider,
  model,
  thinkLevel,
  agent,
  effectiveProvider,
  effectiveModel,
  effectiveThinkLevel,
  effectiveSlaLevel,
}) {
  const resolved = {
    effectiveProvider,
    effectiveModel,
    effectiveThinkLevel,
    effectiveSlaLevel,
    agent,
  };
  if (sessionMeta) {
    if (!provider && sessionMeta.provider) resolved.effectiveProvider = sessionMeta.provider;
    if (!model && sessionMeta.model) resolved.effectiveModel = sessionMeta.model;
    if ((thinkLevel === null || thinkLevel === undefined) && sessionMeta.thinkLevel) {
      resolved.effectiveThinkLevel = sessionMeta.thinkLevel;
    }
    if (
      (resolved.effectiveSlaLevel === null || resolved.effectiveSlaLevel === undefined) &&
      sessionMeta.slaLevel
    ) {
      resolved.effectiveSlaLevel = sessionMeta.slaLevel;
    }
    if (!agent && sessionMeta.agent) resolved.agent = sessionMeta.agent;
  }
  return resolved;
}

/**
 * Resolve the inactivity-watchdog timeout for this run. Only active for the
 * claude provider when the watchdog is enabled and configured with a
 * positive finite timeout.
 */
export function resolveWatchdogTimeoutMs({ watchdogSettings, resumeSessionId, effectiveProvider }) {
  const configuredWatchdogTimeoutMs = resumeSessionId
    ? Number(watchdogSettings.resumeInactivityMs)
    : Number(watchdogSettings.freshInactivityMs);
  return effectiveProvider === 'claude' &&
    watchdogSettings.enabled !== false &&
    Number.isFinite(configuredWatchdogTimeoutMs) &&
    configuredWatchdogTimeoutMs > 0
    ? configuredWatchdogTimeoutMs
    : null;
}

/**
 * Resolve the abort controller/signal for the run. When a watchdog is
 * active and no controller was supplied, a dedicated controller is created
 * so the watchdog can abort the SDK query.
 */
export function resolveAbortState({ abortController, signal, watchdogTimeoutMs }) {
  const resolvedAbortController = normalizeAbortController({ abortController, signal });
  const effectiveAbortController =
    resolvedAbortController || (watchdogTimeoutMs ? new AbortController() : null);
  const effectiveSignal = effectiveAbortController?.signal || signal || null;
  return { effectiveAbortController, effectiveSignal };
}
