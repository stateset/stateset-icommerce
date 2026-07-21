/**
 * Unit tests for agent-session-store.js
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import { createRequire } from 'node:module';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { AgentSessionStore, resetAgentSessionStore } from '../../src/agent-session-store.js';

const require = createRequire(import.meta.url);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'sess-test-'));
  return path.join(dir, 'agent-sessions.db');
}

function loadNativeDatabaseCtor() {
  try {
    const mod = require('better-sqlite3');
    const Database = mod.default || mod;
    const db = new Database(':memory:');
    db.close();
    return Database;
  } catch (error) {
    if (error?.code === 'ERR_DLOPEN_FAILED' || error?.code === 'MODULE_NOT_FOUND') {
      return null;
    }
    throw error;
  }
}

// ===========================================================================
// AgentSessionStore
// ===========================================================================

describe('AgentSessionStore', () => {
  /** @type {AgentSessionStore|null} */
  let store = null;

  afterEach(() => {
    if (store) {
      try {
        store.close();
      } catch {}
      store = null;
    }
  });

  it('creates database on construction', () => {
    const dbPath = tmpDbPath();
    store = new AgentSessionStore({ dbPath });
    assert.ok(fs.existsSync(dbPath));
  });

  it('get returns null for unknown session', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.get('nonexistent'), null);
  });

  it('get returns null for null/empty session id', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.get(null), null);
    assert.strictEqual(store.get(''), null);
  });

  it('upsert creates a new session', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    const result = store.upsert('sess-1', {
      provider: 'claude',
      model: 'claude-sonnet-4-5-20250929',
      agent: 'orders',
    });
    assert.ok(result);
    assert.strictEqual(result.sessionId, 'sess-1');
    assert.strictEqual(result.provider, 'claude');
    assert.strictEqual(result.model, 'claude-sonnet-4-5-20250929');
    assert.strictEqual(result.agent, 'orders');
  });

  it('upsert returns null for null session id', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.upsert(null, {}), null);
  });

  it('upsert merges with existing data', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    store.upsert('sess-1', { provider: 'claude', model: 'old-model' });
    const updated = store.upsert('sess-1', { model: 'new-model' });
    assert.strictEqual(updated.provider, 'claude');
    assert.strictEqual(updated.model, 'new-model');
  });

  it('upsert preserves createdAt on update', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    const created = store.upsert('sess-1', { provider: 'claude' });
    const createdAt = created.createdAt;

    const updated = store.upsert('sess-1', { model: 'new-model' });
    assert.strictEqual(updated.createdAt, createdAt);
    assert.ok(updated.updatedAt >= createdAt);
  });

  it('get returns stored session with all fields', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    store.upsert('sess-1', {
      provider: 'openai',
      model: 'gpt-4o',
      thinkLevel: 'high',
      slaLevel: 'critical',
      agent: 'analytics',
      lastRequest: 'show me revenue',
      lastResponse: '$50,000 this month',
      promptReport: {
        historySource: 'conversation_history',
        historyInjected: true,
        totalInputTokens: 123,
      },
      sessionRefresh: {
        reason: 'treasury_budget_refresh',
        previousSessionId: 'sess-0',
        sessionId: 'sess-1',
        replayedMessages: 2,
        recordedAt: '2026-03-23T10:20:30.123Z',
      },
      lastError: 'previous failure',
      lastErrorCode: 'WATCHDOG_TIMEOUT',
      abortedLastRun: true,
      lastRunMs: 2400,
      inputTokens: 12,
      outputTokens: 5,
      cacheReadTokens: 2,
      cacheWriteTokens: 1,
    });

    const session = store.get('sess-1');
    assert.strictEqual(session.provider, 'openai');
    assert.strictEqual(session.model, 'gpt-4o');
    assert.strictEqual(session.thinkLevel, 'high');
    assert.strictEqual(session.slaLevel, 'critical');
    assert.strictEqual(session.agent, 'analytics');
    assert.strictEqual(session.lastRequest, 'show me revenue');
    assert.strictEqual(session.lastResponse, '$50,000 this month');
    assert.deepStrictEqual(session.promptReport, {
      historySource: 'conversation_history',
      historyInjected: true,
      totalInputTokens: 123,
    });
    assert.deepStrictEqual(session.sessionRefresh, {
      reason: 'treasury_budget_refresh',
      previousSessionId: 'sess-0',
      sessionId: 'sess-1',
      replayedMessages: 2,
      recordedAt: '2026-03-23T10:20:30.123Z',
    });
    assert.strictEqual(session.lastError, 'previous failure');
    assert.strictEqual(session.lastErrorCode, 'WATCHDOG_TIMEOUT');
    assert.strictEqual(session.abortedLastRun, true);
    assert.strictEqual(session.lastRunMs, 2400);
    assert.strictEqual(session.totalTokens, 17);
    assert.strictEqual(session.cacheReadTokens, 2);
    assert.strictEqual(session.cacheWriteTokens, 1);
    assert.ok(Array.isArray(session.summaries));
  });

  it('recordRun accumulates total cost and compaction counts', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    store.upsert('sess-1', { provider: 'claude', model: 'sonnet' });

    store.recordRun('sess-1', {
      lastCostUsd: 0.25,
      compactionCount: 1,
      lastError: 'timeout',
      lastErrorCode: 'WATCHDOG_TIMEOUT',
      lastErrorAt: 100,
      abortedLastRun: true,
    });
    store.recordRun('sess-1', {
      lastCostUsd: 0.75,
      compactionCount: 2,
      promptReport: {
        historySource: 'session_summary',
        totalInputTokens: 88,
      },
      sessionRefresh: {
        reason: 'treasury_budget_refresh',
        previousSessionId: 'sess-1',
        sessionId: 'sess-2',
        replayedMessages: 4,
        recordedAt: '2026-03-23T11:00:00.000Z',
      },
      lastError: null,
      lastErrorCode: null,
      lastErrorAt: null,
      abortedLastRun: false,
    });

    const session = store.get('sess-1');
    assert.strictEqual(session.lastCostUsd, 0.75);
    assert.strictEqual(session.totalCostUsd, 1);
    assert.strictEqual(session.compactionCount, 3);
    assert.deepStrictEqual(session.promptReport, {
      historySource: 'session_summary',
      totalInputTokens: 88,
    });
    assert.deepStrictEqual(session.sessionRefresh, {
      reason: 'treasury_budget_refresh',
      previousSessionId: 'sess-1',
      sessionId: 'sess-2',
      replayedMessages: 4,
      recordedAt: '2026-03-23T11:00:00.000Z',
    });
    assert.strictEqual(session.lastError, null);
    assert.strictEqual(session.lastErrorCode, null);
    assert.strictEqual(session.lastErrorAt, null);
    assert.strictEqual(session.abortedLastRun, false);
  });

  it('lists recent sessions and failures in reverse update order', async () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    store.upsert('sess-ok', { provider: 'claude', model: 'sonnet', lastResponse: 'ok' });
    await new Promise((resolve) => setTimeout(resolve, 2));
    store.upsert('sess-fail', {
      provider: 'claude',
      model: 'sonnet',
      lastError: 'stalled',
      lastErrorCode: 'WATCHDOG_TIMEOUT',
      abortedLastRun: true,
    });
    await new Promise((resolve) => setTimeout(resolve, 2));
    store.upsert('sess-newest', { provider: 'openai', model: 'gpt-4o' });

    const recent = store.listRecent(2);
    const failures = store.listRecentFailures(5);

    assert.strictEqual(store.count(), 3);
    assert.strictEqual(recent.length, 2);
    assert.strictEqual(recent[0].sessionId, 'sess-newest');
    assert.strictEqual(recent[1].sessionId, 'sess-fail');
    assert.strictEqual(failures.length, 1);
    assert.strictEqual(failures[0].sessionId, 'sess-fail');
    assert.strictEqual(failures[0].lastErrorCode, 'WATCHDOG_TIMEOUT');
  });

  it('appendSummary adds summary to front of list', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    store.upsert('sess-1', { provider: 'claude' });

    store.appendSummary('sess-1', 'First summary');
    store.appendSummary('sess-1', 'Second summary');

    const session = store.get('sess-1');
    assert.strictEqual(session.summaries.length, 2);
    assert.strictEqual(session.summaries[0], 'Second summary');
    assert.strictEqual(session.summaries[1], 'First summary');
  });

  it('appendSummary respects maxSummaries', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath(), maxSummaries: 2 });
    store.upsert('sess-1', {});

    store.appendSummary('sess-1', 'Summary 1');
    store.appendSummary('sess-1', 'Summary 2');
    store.appendSummary('sess-1', 'Summary 3');

    const session = store.get('sess-1');
    assert.strictEqual(session.summaries.length, 2);
    assert.strictEqual(session.summaries[0], 'Summary 3');
    assert.strictEqual(session.summaries[1], 'Summary 2');
  });

  it('appendSummary returns null for null session id', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.appendSummary(null, 'test'), null);
  });

  it('appendSummary returns null for null summary', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.appendSummary('sess-1', null), null);
  });

  it('delete removes existing session', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    store.upsert('sess-1', { provider: 'claude' });
    const deleted = store.delete('sess-1');
    assert.strictEqual(deleted, true);
    assert.strictEqual(store.get('sess-1'), null);
  });

  it('delete returns false for non-existent session', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.delete('nonexistent'), false);
  });

  it('persists data across instances', () => {
    const dbPath = tmpDbPath();
    store = new AgentSessionStore({ dbPath });
    store.upsert('sess-1', { provider: 'claude', model: 'opus', slaLevel: 'standard' });
    store.close();

    const store2 = new AgentSessionStore({ dbPath });
    const session = store2.get('sess-1');
    assert.strictEqual(session.provider, 'claude');
    assert.strictEqual(session.model, 'opus');
    assert.strictEqual(session.slaLevel, 'standard');
    store2.close();
    store = null;
  });

  it('persists fallback sessions across instances when SQLite is unavailable', () => {
    const dbPath = tmpDbPath();
    store = new AgentSessionStore({ dbPath, databaseCtor: null });
    assert.strictEqual(store.backend, 'json-fallback');
    store.upsert('sess-fallback', {
      provider: 'openai',
      model: 'gpt-4o',
      slaLevel: 'standard',
      summaries: ['replay this'],
    });
    store.close();

    assert.ok(fs.existsSync(`${dbPath}.fallback.json`));

    const reopened = new AgentSessionStore({ dbPath, databaseCtor: null });
    const session = reopened.get('sess-fallback');
    assert.strictEqual(session.provider, 'openai');
    assert.strictEqual(session.model, 'gpt-4o');
    assert.strictEqual(session.slaLevel, 'standard');
    assert.deepStrictEqual(session.summaries, ['replay this']);
    reopened.close();
    store = null;
  });

  it('migrates legacy stores to the richer schema', () => {
    const dbPath = tmpDbPath();
    const Database = loadNativeDatabaseCtor();

    if (Database) {
      const legacyDb = new Database(dbPath);
      legacyDb.exec(`
        CREATE TABLE agent_sessions (
          session_id TEXT PRIMARY KEY,
          provider TEXT,
          model TEXT,
          think_level TEXT,
          agent TEXT,
          summaries TEXT,
          last_request TEXT,
          last_response TEXT,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );
      `);
      legacyDb
        .prepare(
          `INSERT INTO agent_sessions
          (session_id, provider, model, think_level, agent, summaries, last_request, last_response, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .run(
          'legacy-1',
          'claude',
          'legacy-model',
          'low',
          'orders',
          '[]',
          'old request',
          'old response',
          1,
          1,
        );
      legacyDb.close();
    } else {
      const legacyStore = new AgentSessionStore({ dbPath });
      legacyStore.upsert('legacy-1', {
        provider: 'claude',
        model: 'legacy-model',
        thinkLevel: 'low',
        agent: 'orders',
        summaries: [],
        lastRequest: 'old request',
        lastResponse: 'old response',
      });
      legacyStore.close();
    }

    store = new AgentSessionStore({ dbPath });
    const migrated = store.upsert('legacy-1', {
      slaLevel: 'critical',
      lastError: 'recovered',
      abortedLastRun: true,
    });

    assert.strictEqual(migrated.model, 'legacy-model');
    assert.strictEqual(migrated.slaLevel, 'critical');
    assert.strictEqual(migrated.lastError, 'recovered');
    assert.strictEqual(migrated.abortedLastRun, true);
  });
});

// ===========================================================================
// resetAgentSessionStore
// ===========================================================================

describe('resetAgentSessionStore', () => {
  it('can be called safely even when no store is initialized', () => {
    assert.doesNotThrow(() => resetAgentSessionStore());
  });
});
