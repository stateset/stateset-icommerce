/**
 * Unit tests for agent-session-store.js
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { AgentSessionStore, resetAgentSessionStore } from '../../src/agent-session-store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'sess-test-'));
  return path.join(dir, 'agent-sessions.db');
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
    assert.strictEqual(updated.provider, 'claude'); // preserved
    assert.strictEqual(updated.model, 'new-model'); // updated
  });

  it('upsert preserves createdAt on update', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    const created = store.upsert('sess-1', { provider: 'claude' });
    const createdAt = created.createdAt;

    // Small delay to ensure timestamps differ
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
      agent: 'analytics',
      lastRequest: 'show me revenue',
      lastResponse: '$50,000 this month',
    });

    const session = store.get('sess-1');
    assert.strictEqual(session.provider, 'openai');
    assert.strictEqual(session.model, 'gpt-4o');
    assert.strictEqual(session.thinkLevel, 'high');
    assert.strictEqual(session.agent, 'analytics');
    assert.strictEqual(session.lastRequest, 'show me revenue');
    assert.strictEqual(session.lastResponse, '$50,000 this month');
    assert.ok(Array.isArray(session.summaries));
  });

  it('appendSummary adds summary to front of list', () => {
    store = new AgentSessionStore({ dbPath: tmpDbPath() });
    store.upsert('sess-1', { provider: 'claude' });

    store.appendSummary('sess-1', 'First summary');
    store.appendSummary('sess-1', 'Second summary');

    const session = store.get('sess-1');
    assert.strictEqual(session.summaries.length, 2);
    assert.strictEqual(session.summaries[0], 'Second summary'); // most recent first
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
    store.upsert('sess-1', { provider: 'claude', model: 'opus' });
    store.close();

    const store2 = new AgentSessionStore({ dbPath });
    const session = store2.get('sess-1');
    assert.strictEqual(session.provider, 'claude');
    assert.strictEqual(session.model, 'opus');
    store2.close();
    store = null;
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
