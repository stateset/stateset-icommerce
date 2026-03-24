/**
 * Reusable doctor checks for harness diagnostics.
 */

import fs from 'node:fs';
import {
  AgentSessionStore,
  DEFAULT_DB_PATH as DEFAULT_SESSION_DB_PATH,
} from './agent-session-store.js';
import { loadAgentSettings } from './settings.js';

export async function checkHarnessSettings({ settingsOverrides = {}, reload = true } = {}) {
  const settings = loadAgentSettings(settingsOverrides, { reload });
  const issues = [];
  const warnings = [];

  if (!settings.provider?.default) {
    issues.push('provider.default is not configured');
  }
  if (!settings.model?.default) {
    issues.push('model.default is not configured');
  }

  if (settings.contextGuard?.enabled !== false) {
    const warningThreshold = Number(settings.contextGuard?.warningThreshold);
    const compactThreshold = Number(settings.contextGuard?.compactThreshold);
    const abortThreshold = Number(settings.contextGuard?.abortThreshold);
    if (
      !Number.isFinite(warningThreshold) ||
      !Number.isFinite(compactThreshold) ||
      !Number.isFinite(abortThreshold) ||
      !(warningThreshold < compactThreshold && compactThreshold < abortThreshold)
    ) {
      warnings.push('context guard thresholds are invalid');
    }
  }

  if (settings.watchdog?.enabled !== false) {
    const fresh = Number(settings.watchdog?.freshInactivityMs);
    const resumed = Number(settings.watchdog?.resumeInactivityMs);
    if (!Number.isFinite(fresh) || fresh <= 0) {
      warnings.push('watchdog.freshInactivityMs must be a positive number');
    }
    if (!Number.isFinite(resumed) || resumed <= 0) {
      warnings.push('watchdog.resumeInactivityMs must be a positive number');
    }
    if (Number.isFinite(fresh) && Number.isFinite(resumed) && resumed < fresh) {
      warnings.push('watchdog.resumeInactivityMs is lower than freshInactivityMs');
    }
  }

  if (settings.retry?.enabled !== false) {
    const maxRetries = Number(settings.retry?.maxRetries);
    if (!Number.isFinite(maxRetries) || maxRetries < 0) {
      warnings.push('retry.maxRetries must be zero or greater');
    }
  }

  const queueSettings = settings.queue || {};
  const queuePositiveFields = [
    ['queue.maxLanes', queueSettings.maxLanes],
    ['queue.laneTimeoutMs', queueSettings.laneTimeoutMs ?? queueSettings.laneTimeout],
    ['queue.maxQueueSize', queueSettings.maxQueueSize],
    ['queue.idleCleanupMs', queueSettings.idleCleanupMs],
    ['queue.parallelConcurrency', queueSettings.parallelConcurrency],
    ['queue.warningThrottleMs', queueSettings.warningThrottleMs],
    ['queue.monitorIntervalMs', queueSettings.monitorIntervalMs],
  ];
  for (const [name, value] of queuePositiveFields) {
    const numeric = Number(value);
    if (!Number.isFinite(numeric) || numeric <= 0) {
      warnings.push(`${name} must be a positive number`);
    }
  }

  const waitWarningMs = Number(queueSettings.waitWarningMs);
  const runningWarningMs = Number(queueSettings.runningWarningMs);
  if (!Number.isFinite(waitWarningMs) || waitWarningMs < 0) {
    warnings.push('queue.waitWarningMs must be zero or greater');
  }
  if (!Number.isFinite(runningWarningMs) || runningWarningMs < 0) {
    warnings.push('queue.runningWarningMs must be zero or greater');
  }
  if (
    Number.isFinite(waitWarningMs) &&
    Number.isFinite(runningWarningMs) &&
    waitWarningMs > 0 &&
    runningWarningMs > 0 &&
    runningWarningMs < waitWarningMs
  ) {
    warnings.push('queue.runningWarningMs is lower than waitWarningMs');
  }

  const stats = buildHarnessStats(settings);
  if (issues.length > 0) {
    return {
      status: 'error',
      message: `Harness settings invalid: ${issues.join('; ')}`,
      hint: 'Fix the settings in ~/.stateset/settings.json or the workspace .stateset/settings.json.',
      stats: { ...stats, issues, warnings },
    };
  }

  if (warnings.length > 0) {
    return {
      status: 'warning',
      message: `Harness settings need attention: ${warnings.join('; ')}`,
      hint: 'Review the harness settings before relying on long-running sessions.',
      stats: { ...stats, issues, warnings },
    };
  }

  return {
    status: 'ok',
    message: 'Harness settings look valid',
    stats: { ...stats, issues, warnings },
  };
}

export async function checkSessionStoreHealth({
  settingsOverrides = {},
  reload = true,
  recentLimit = 5,
  now = Date.now(),
} = {}) {
  const settings = loadAgentSettings(settingsOverrides, { reload });
  const enabled = settings.sessionStore?.enabled !== false;
  const dbPath = settings.sessionStore?.dbPath || DEFAULT_SESSION_DB_PATH;

  if (!enabled) {
    return {
      status: 'info',
      message: 'Session store disabled',
      hint: 'Enable sessionStore in settings to persist resumable harness state.',
      stats: { enabled: false, path: dbPath },
    };
  }

  if (!fs.existsSync(dbPath)) {
    return {
      status: 'info',
      message: 'Session store enabled but no session database exists yet',
      hint: 'Run a harness command first to create and populate the session store.',
      stats: { enabled: true, path: dbPath, exists: false },
    };
  }

  let store = null;
  try {
    store = new AgentSessionStore({
      dbPath,
      maxSummaries: settings.sessionStore?.maxSummaries || settings.memory?.maxSummaries || 5,
    });
    const count = store.count();
    const recentSessions = store
      .listRecent(recentLimit)
      .map((session) => summarizeSession(session, now));
    const recentFailures = store
      .listRecentFailures(recentLimit)
      .map((session) => summarizeSession(session, now));
    const warningMessage = buildSessionWarningMessage(recentFailures);

    return {
      status: recentFailures.length > 0 ? 'warning' : 'ok',
      message:
        recentFailures.length > 0
          ? `Session store readable (${count} session(s), ${recentFailures.length} recent failure(s))`
          : `Session store readable (${count} session(s))`,
      hint: warningMessage,
      stats: {
        enabled: true,
        path: dbPath,
        exists: true,
        count,
        recentSessions,
        recentFailures,
      },
    };
  } catch (error) {
    return {
      status: 'error',
      message: `Session store error: ${error.message}`,
      hint: 'Check the session store path and SQLite file permissions.',
      stats: {
        enabled: true,
        path: dbPath,
        exists: true,
      },
    };
  } finally {
    try {
      store?.close();
    } catch {
      // ignore close errors in diagnostics
    }
  }
}

function buildHarnessStats(settings) {
  const sessionStorePath = settings.sessionStore?.dbPath || DEFAULT_SESSION_DB_PATH;
  return {
    provider: settings.provider?.default || null,
    model: settings.model?.default || null,
    thinkLevel: settings.thinkLevel?.default || null,
    watchdogEnabled: settings.watchdog?.enabled !== false,
    watchdogFreshInactivityMs: settings.watchdog?.freshInactivityMs ?? null,
    watchdogResumeInactivityMs: settings.watchdog?.resumeInactivityMs ?? null,
    contextGuardEnabled: settings.contextGuard?.enabled !== false,
    contextWarningThreshold: settings.contextGuard?.warningThreshold ?? null,
    contextCompactThreshold: settings.contextGuard?.compactThreshold ?? null,
    contextAbortThreshold: settings.contextGuard?.abortThreshold ?? null,
    sessionStoreEnabled: settings.sessionStore?.enabled !== false,
    sessionStorePath,
    queueMaxLanes: settings.queue?.maxLanes ?? null,
    queueLaneTimeoutMs: settings.queue?.laneTimeoutMs ?? settings.queue?.laneTimeout ?? null,
    queueMaxQueueSize: settings.queue?.maxQueueSize ?? null,
    queueParallelConcurrency: settings.queue?.parallelConcurrency ?? null,
    queueWaitWarningMs: settings.queue?.waitWarningMs ?? null,
    queueRunningWarningMs: settings.queue?.runningWarningMs ?? null,
    queueWarningThrottleMs: settings.queue?.warningThrottleMs ?? null,
    queueMonitorIntervalMs: settings.queue?.monitorIntervalMs ?? null,
    queueEmitWarnings: settings.queue?.emitWarnings !== false,
    memoryEnabled: settings.memory?.enabled === true,
    pluginsEnabled: settings.plugins?.enabled === true,
    redactLogs: settings.privacy?.redactLogs !== false,
    blockedTools: Array.isArray(settings.guardrails?.blockedTools)
      ? settings.guardrails.blockedTools.length
      : 0,
    approvalRules: Array.isArray(settings.guardrails?.requireApprovalFor)
      ? settings.guardrails.requireApprovalFor.length
      : 0,
    maxToolCallsPerMinute: settings.guardrails?.maxToolCallsPerMinute ?? null,
    maxWriteOpsPerMinute: settings.guardrails?.maxWriteOpsPerMinute ?? null,
  };
}

function summarizeSession(session, now) {
  return {
    sessionId: session.sessionId,
    provider: session.provider,
    model: session.model,
    agent: session.agent,
    slaLevel: session.slaLevel,
    updatedAt: session.updatedAt,
    updatedAgoMs: Number.isFinite(now - session.updatedAt) ? now - session.updatedAt : null,
    lastRunMs: session.lastRunMs,
    lastCostUsd: session.lastCostUsd,
    lastError: session.lastError,
    lastErrorCode: session.lastErrorCode,
    abortedLastRun: session.abortedLastRun,
    compactionCount: session.compactionCount,
    promptReport: summarizePromptReport(session.promptReport),
    sessionRefresh: summarizeSessionRefresh(session.sessionRefresh),
  };
}

function summarizePromptReport(promptReport) {
  if (!promptReport || typeof promptReport !== 'object') return null;
  return {
    historySource: promptReport.historySource || null,
    resumeSession: promptReport.resumeSession === true,
    historyInjected: promptReport.historyInjected === true,
    historyMessagesInjected: promptReport.historyMessagesInjected ?? 0,
    totalInputTokens: promptReport.totalInputTokens ?? null,
    systemPromptTokens: promptReport.systemPromptTokens ?? null,
    userPromptTokens: promptReport.userPromptTokens ?? null,
    compactionApplied: promptReport.compactionApplied === true,
    estimatedContextTokensSaved: promptReport.estimatedContextTokensSaved ?? null,
  };
}

function summarizeSessionRefresh(sessionRefresh) {
  if (!sessionRefresh || typeof sessionRefresh !== 'object') return null;
  return {
    reason: sessionRefresh.reason || 'session_refresh',
    previousSessionId: sessionRefresh.previousSessionId || null,
    sessionId: sessionRefresh.sessionId || null,
    replayedMessages: sessionRefresh.replayedMessages ?? 0,
    recordedAt: sessionRefresh.recordedAt || null,
  };
}

function buildSessionWarningMessage(recentFailures) {
  if (!Array.isArray(recentFailures) || recentFailures.length === 0) return null;
  if (recentFailures.some((session) => session.lastErrorCode === 'WATCHDOG_TIMEOUT')) {
    return 'Recent watchdog timeouts detected. Inspect stalled tool calls or increase watchdog thresholds.';
  }
  if (recentFailures.some((session) => session.abortedLastRun)) {
    return 'Recent interrupted runs detected. Verify approvals, cancellations, or stalled tool execution.';
  }
  return 'Recent failed runs were found in the session store. Review the latest session metadata.';
}
