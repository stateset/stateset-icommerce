/**
 * Tests for cli/src/channels/session-store.js
 *
 * Covers: ChannelSessionStore (SQLite-backed session persistence).
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';

import { ChannelSessionStore } from '../../src/channels/session-store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'css-test-'));
  return path.join(dir, 'test-sessions.db');
}

// ---------------------------------------------------------------------------
// ChannelSessionStore
// ---------------------------------------------------------------------------

describe('ChannelSessionStore', () => {
  let store;
  let dbPath;

  beforeEach(() => {
    dbPath = tmpDbPath();
    store = new ChannelSessionStore({ dbPath });
  });

  afterEach(() => {
    try {
      store.close();
    } catch {
      /* already closed */
    }
    try {
      fs.rmSync(path.dirname(dbPath), { recursive: true });
    } catch {
      /* ok */
    }
  });

  it('creates tables on construction', () => {
    // If constructor didn't throw, tables were created
    assert.ok(store.db);
  });

  it('get() returns null for unknown session', () => {
    const result = store.get('telegram', 'user123');
    assert.equal(result, null);
  });

  it('upsert() + get() round-trips a session', () => {
    store.upsert('telegram', 'user1', {
      sessionId: 'ses-001',
      agent: 'orders',
      lastActive: 1000,
      context: { topic: 'returns' },
    });

    const session = store.get('telegram', 'user1');
    assert.ok(session);
    assert.equal(session.sessionId, 'ses-001');
    assert.equal(session.agent, 'orders');
    assert.equal(session.lastActive, 1000);
    assert.deepStrictEqual(session.context, { topic: 'returns' });
  });

  it('upsert() updates existing session', () => {
    store.upsert('telegram', 'user1', {
      sessionId: 'ses-001',
      agent: 'orders',
      lastActive: 1000,
    });

    store.upsert('telegram', 'user1', {
      sessionId: 'ses-002',
      agent: 'returns',
      lastActive: 2000,
    });

    const session = store.get('telegram', 'user1');
    assert.equal(session.sessionId, 'ses-002');
    assert.equal(session.agent, 'returns');
  });

  it('isolates sessions by channel', () => {
    store.upsert('telegram', 'user1', { sessionId: 'ses-tg', agent: 'a', lastActive: 1000 });
    store.upsert('discord', 'user1', { sessionId: 'ses-dc', agent: 'b', lastActive: 1000 });

    assert.equal(store.get('telegram', 'user1').sessionId, 'ses-tg');
    assert.equal(store.get('discord', 'user1').sessionId, 'ses-dc');
  });

  it('handles null context gracefully', () => {
    store.upsert('slack', 'user1', { sessionId: 's1', agent: null, lastActive: 1000 });
    const session = store.get('slack', 'user1');
    assert.equal(session.context, null);
    assert.equal(session.agent, null);
  });

  it('handles invalid JSON context gracefully', () => {
    // Insert bad JSON directly
    store.db
      .prepare(
        `INSERT INTO channel_sessions (channel, sender_id, context, last_active)
       VALUES (?, ?, ?, ?)`,
      )
      .run('test', 'user1', '{invalid json}', 1000);

    const session = store.get('test', 'user1');
    assert.equal(session.context, null);
  });

  it('deleteExpired() removes old sessions', () => {
    const now = Date.now();
    store.upsert('ch', 'old-user', { sessionId: 'old', agent: null, lastActive: now - 100000 });
    store.upsert('ch', 'new-user', { sessionId: 'new', agent: null, lastActive: now });

    const deleted = store.deleteExpired(50000);
    assert.equal(deleted, 1);
    assert.equal(store.get('ch', 'old-user'), null);
    assert.ok(store.get('ch', 'new-user'));
  });

  it('close() closes the database', () => {
    store.close();
    // Second close should not throw
    store.close();
  });
});
