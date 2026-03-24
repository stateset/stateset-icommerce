/**
 * Unit tests for doctor-checks.js
 */

import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { AgentSessionStore } from '../../src/agent-session-store.js';
import { checkHarnessSettings, checkSessionStoreHealth } from '../../src/doctor-checks.js';
import { resetAgentSettingsCache } from '../../src/settings.js';

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'doctor-checks-'));
  return path.join(dir, 'agent-sessions.db');
}

afterEach(() => {
  resetAgentSettingsCache();
});

describe('checkHarnessSettings', () => {
  it('returns ok for valid harness settings', async () => {
    const result = await checkHarnessSettings({
      settingsOverrides: {
        provider: { default: 'claude' },
        model: { default: 'claude-sonnet-4-5-20250929' },
        watchdog: {
          enabled: true,
          freshInactivityMs: 1000,
          resumeInactivityMs: 2000,
        },
        contextGuard: {
          enabled: true,
          warningThreshold: 0.7,
          compactThreshold: 0.8,
          abortThreshold: 0.9,
        },
        retry: {
          enabled: true,
          maxRetries: 2,
        },
      },
    });
    assert.strictEqual(result.status, 'ok');
    assert.strictEqual(result.stats.watchdogEnabled, true);
    assert.ok(result.stats.watchdogResumeInactivityMs >= result.stats.watchdogFreshInactivityMs);
    assert.ok(result.stats.queueRunningWarningMs >= result.stats.queueWaitWarningMs);
  });

  it('returns warning for invalid watchdog, context, and queue settings', async () => {
    const result = await checkHarnessSettings({
      settingsOverrides: {
        watchdog: {
          enabled: true,
          freshInactivityMs: -1,
          resumeInactivityMs: 0,
        },
        contextGuard: {
          enabled: true,
          warningThreshold: 0.9,
          compactThreshold: 0.8,
          abortThreshold: 0.7,
        },
        queue: {
          maxLanes: 0,
          laneTimeoutMs: -1,
          maxQueueSize: -1,
          idleCleanupMs: -1,
          parallelConcurrency: 0,
          waitWarningMs: -1,
          runningWarningMs: 1,
          warningThrottleMs: 0,
          monitorIntervalMs: 0,
        },
      },
    });

    assert.strictEqual(result.status, 'warning');
    assert.ok(result.message.includes('watchdog'));
    assert.ok(result.message.includes('queue'));
    assert.ok(Array.isArray(result.stats.warnings));
    assert.ok(result.stats.warnings.some((warning) => warning.includes('queue.maxLanes')));
    assert.ok(result.stats.warnings.some((warning) => warning.includes('queue.waitWarningMs')));
    assert.ok(result.stats.warnings.length >= 4);
  });
});

describe('checkSessionStoreHealth', () => {
  it('returns info when session store is disabled', async () => {
    const result = await checkSessionStoreHealth({
      settingsOverrides: {
        sessionStore: {
          enabled: false,
        },
      },
    });

    assert.strictEqual(result.status, 'info');
    assert.strictEqual(result.stats.enabled, false);
  });

  it('returns info when the session store has not been created yet', async () => {
    const dbPath = tmpDbPath();
    fs.rmSync(path.dirname(dbPath), { recursive: true, force: true });

    const result = await checkSessionStoreHealth({
      settingsOverrides: {
        sessionStore: {
          enabled: true,
          dbPath,
        },
      },
    });

    assert.strictEqual(result.status, 'info');
    assert.strictEqual(result.stats.exists, false);
  });

  it('reports recent failures from the session store', async () => {
    const dbPath = tmpDbPath();
    const store = new AgentSessionStore({ dbPath });
    store.upsert('sess-ok', { provider: 'claude', model: 'sonnet', agent: 'orders' });
    store.recordRun('sess-ok', { lastCostUsd: 0.2, lastRunMs: 1200 });
    store.upsert('sess-fail', {
      provider: 'claude',
      model: 'sonnet',
      agent: 'orders',
      promptReport: {
        historySource: 'conversation_history',
        historyInjected: true,
        historyMessagesInjected: 2,
        totalInputTokens: 144,
        systemPromptTokens: 44,
        userPromptTokens: 100,
        compactionApplied: true,
        estimatedContextTokensSaved: 50,
      },
      sessionRefresh: {
        reason: 'treasury_budget_refresh',
        previousSessionId: 'sess-old',
        sessionId: 'sess-fail',
        replayedMessages: 2,
        recordedAt: '2026-03-23T12:00:00.000Z',
      },
      lastError: 'No Claude SDK activity received after 1000ms',
      lastErrorCode: 'WATCHDOG_TIMEOUT',
      abortedLastRun: true,
      lastRunMs: 1000,
    });
    store.close();

    const result = await checkSessionStoreHealth({
      settingsOverrides: {
        sessionStore: {
          enabled: true,
          dbPath,
        },
      },
      now: Date.now(),
    });

    assert.strictEqual(result.status, 'warning');
    assert.ok(result.message.includes('recent failure'));
    assert.ok(result.hint.includes('watchdog'));
    assert.strictEqual(result.stats.count, 2);
    assert.strictEqual(result.stats.recentFailures[0].lastErrorCode, 'WATCHDOG_TIMEOUT');
    assert.deepStrictEqual(result.stats.recentFailures[0].promptReport, {
      historySource: 'conversation_history',
      resumeSession: false,
      historyInjected: true,
      historyMessagesInjected: 2,
      totalInputTokens: 144,
      systemPromptTokens: 44,
      userPromptTokens: 100,
      compactionApplied: true,
      estimatedContextTokensSaved: 50,
    });
    assert.deepStrictEqual(result.stats.recentFailures[0].sessionRefresh, {
      reason: 'treasury_budget_refresh',
      previousSessionId: 'sess-old',
      sessionId: 'sess-fail',
      replayedMessages: 2,
      recordedAt: '2026-03-23T12:00:00.000Z',
    });
  });
});
