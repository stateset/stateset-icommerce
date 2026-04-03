/**
 * Unit tests for memory/store.js — MemoryStore
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { MemoryStore, getMemoryStore, resetMemoryStore } from '../../src/memory/store.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpDbPath() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mem-test-'));
  return path.join(dir, 'memory.db');
}

// ===========================================================================
// MemoryStore
// ===========================================================================

describe('MemoryStore', () => {
  /** @type {MemoryStore|null} */
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
    store = new MemoryStore({ dbPath });
    assert.ok(fs.existsSync(dbPath));
  });

  it('save returns an id', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    const { id } = store.save({ summary: 'Test summary' });
    assert.ok(typeof id === 'number');
    assert.ok(id > 0);
  });

  it('count tracks entries', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    assert.strictEqual(store.count(), 0);
    store.save({ summary: 'First' });
    store.save({ summary: 'Second' });
    assert.strictEqual(store.count(), 2);
  });

  it('getRecent retrieves recent memories for a sender', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'Memory 1', channel: 'telegram', senderId: 'user-1' });
    store.save({ summary: 'Memory 2', channel: 'telegram', senderId: 'user-1' });
    store.save({ summary: 'Memory 3', channel: 'telegram', senderId: 'user-2' });

    const results = store.getRecent('telegram', 'user-1', 10);
    assert.strictEqual(results.length, 2);
    // Most recent first
    assert.strictEqual(results[0].summary, 'Memory 2');
    assert.strictEqual(results[1].summary, 'Memory 1');
  });

  it('getRecent respects limit', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    for (let i = 0; i < 10; i++) {
      store.save({ summary: `Memory ${i}` });
    }
    const results = store.getRecent('cli', 'local', 3);
    assert.strictEqual(results.length, 3);
  });

  it('getRecent uses default channel and senderId', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'Default sender' });
    const results = store.getRecent();
    assert.strictEqual(results.length, 1);
    assert.strictEqual(results[0].summary, 'Default sender');
  });

  it('save stores facts as JSON and deserializes them', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({
      summary: 'With facts',
      facts: ['fact1', 'fact2', 'fact3'],
    });
    const results = store.getRecent();
    assert.deepStrictEqual(results[0].facts, ['fact1', 'fact2', 'fact3']);
  });

  it('facts default to empty array', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'No facts' });
    const results = store.getRecent();
    assert.deepStrictEqual(results[0].facts, []);
  });

  it('save stores agent and sessionId', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({
      summary: 'Agent memory',
      agent: 'orders',
      sessionId: 'sess-123',
    });
    const results = store.getRecent();
    assert.strictEqual(results[0].agent, 'orders');
    assert.strictEqual(results[0].session_id, 'sess-123');
  });

  it('save stores tokenCount', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'Tokens tracked', tokenCount: 1500 });
    const results = store.getRecent();
    assert.strictEqual(results[0].token_count, 1500);
  });

  it('search finds memories by text', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'Ordered a blue widget' });
    store.save({ summary: 'Returned a red gadget' });
    store.save({ summary: 'Ordered three green items' });

    const results = store.search('cli', 'local', 'Ordered');
    assert.strictEqual(results.length, 2);
  });

  it('search is case-sensitive (LIKE behavior)', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'UPPERCASE test' });
    store.save({ summary: 'lowercase test' });

    const results = store.search('cli', 'local', 'UPPERCASE');
    assert.strictEqual(results.length, 1);
  });

  it('search returns empty for no match', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'Hello world' });
    const results = store.search('cli', 'local', 'nonexistent');
    assert.deepStrictEqual(results, []);
  });

  it('getAllRecent returns entries across all senders', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'From user A', senderId: 'a' });
    store.save({ summary: 'From user B', senderId: 'b' });
    store.save({ summary: 'From user C', channel: 'discord', senderId: 'c' });

    const results = store.getAllRecent(10);
    assert.strictEqual(results.length, 3);
  });

  it('getAllRecent respects limit', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    for (let i = 0; i < 10; i++) {
      store.save({ summary: `Entry ${i}`, senderId: `user-${i}` });
    }
    const results = store.getAllRecent(3);
    assert.strictEqual(results.length, 3);
  });

  it('delete removes specific memory', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    const { id: id1 } = store.save({ summary: 'Keep' });
    const { id: id2 } = store.save({ summary: 'Delete me' });

    assert.strictEqual(store.count(), 2);
    const deleted = store.delete(id2);
    assert.strictEqual(deleted, true);
    assert.strictEqual(store.count(), 1);

    const results = store.getRecent();
    assert.strictEqual(results[0].summary, 'Keep');
  });

  it('delete returns false for nonexistent id', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    const result = store.delete(9999);
    assert.strictEqual(result, false);
  });

  it('prune removes old entries', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    // Insert directly with old timestamp
    store._insertStmt.run('cli', 'local', null, 'Old memory', '[]', null, 1000, 0);
    store.save({ summary: 'Recent memory' });

    assert.strictEqual(store.count(), 2);
    const pruned = store.prune(1000); // 1 second max age — old entry is ancient
    assert.strictEqual(pruned, 1);
    assert.strictEqual(store.count(), 1);
  });

  it('prune returns 0 when nothing to delete', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.save({ summary: 'Fresh' });
    const pruned = store.prune();
    assert.strictEqual(pruned, 0);
  });

  it('persists data across instances', () => {
    const dbPath = tmpDbPath();
    store = new MemoryStore({ dbPath });
    store.save({ summary: 'Persistent memory', facts: ['remember this'] });
    store.close();

    const store2 = new MemoryStore({ dbPath });
    const results = store2.getRecent();
    assert.strictEqual(results[0].summary, 'Persistent memory');
    assert.deepStrictEqual(results[0].facts, ['remember this']);
    store2.close();
    store = null;
  });

  it('persists fallback memories across instances when SQLite is unavailable', () => {
    const dbPath = tmpDbPath();
    store = new MemoryStore({ dbPath, databaseCtor: null });
    assert.strictEqual(store.backend, 'json-fallback');
    store.save({ summary: 'Fallback memory', facts: ['persisted'] });
    store.close();

    assert.ok(fs.existsSync(`${dbPath}.fallback.json`));

    const reopened = new MemoryStore({ dbPath, databaseCtor: null });
    const results = reopened.getRecent();
    assert.strictEqual(results[0].summary, 'Fallback memory');
    assert.deepStrictEqual(results[0].facts, ['persisted']);
    reopened.close();
    store = null;
  });

  it('close is idempotent', () => {
    store = new MemoryStore({ dbPath: tmpDbPath() });
    store.close();
    store.close(); // Should not throw
    store = null;
  });
});

// ===========================================================================
// Singleton
// ===========================================================================

describe('getMemoryStore / resetMemoryStore', () => {
  afterEach(() => {
    resetMemoryStore();
  });

  it('getMemoryStore returns same instance', () => {
    const dbPath = tmpDbPath();
    const a = getMemoryStore({ dbPath });
    const b = getMemoryStore({ dbPath });
    assert.strictEqual(a, b);
  });

  it('resetMemoryStore clears instance', () => {
    const dbPath = tmpDbPath();
    const a = getMemoryStore({ dbPath });
    resetMemoryStore();
    const b = getMemoryStore({ dbPath });
    assert.notStrictEqual(a, b);
  });
});
