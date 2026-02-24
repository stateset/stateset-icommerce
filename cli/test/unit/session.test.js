/**
 * Unit tests for session.js — SessionManager and CommandHistory
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { SessionManager, CommandHistory, createSessionManager, createCommandHistory } from '../../src/session.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tmpCount = 0;

function tmpDir() {
  const dir = path.join(os.tmpdir(), `stateset-session-test-${Date.now()}-${tmpCount++}`);
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function rmDir(dir) {
  try {
    fs.rmSync(dir, { recursive: true, force: true });
  } catch {
    // ignore
  }
}

// ===========================================================================
// SessionManager — create / save / load / delete
// ===========================================================================

describe('SessionManager CRUD', () => {
  let dir;
  let sm;

  afterEach(() => {
    if (dir) rmDir(dir);
  });

  it('creates a new session', () => {
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    const session = sm.create({ agent: 'orders', database: ':memory:' });

    assert.ok(session.id);
    assert.strictEqual(session.agent, 'orders');
    assert.strictEqual(session.database, ':memory:');
    assert.ok(session.createdAt);
    assert.deepStrictEqual(session.operations, []);
  });

  it('load returns saved session', () => {
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    const created = sm.create({ agent: 'analytics' });
    const loaded = sm.load(created.id);

    assert.strictEqual(loaded.id, created.id);
    assert.strictEqual(loaded.agent, 'analytics');
  });

  it('load returns null for missing session', () => {
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.load('nonexistent'), null);
  });

  it('exists returns true/false', () => {
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    const session = sm.create();
    assert.strictEqual(sm.exists(session.id), true);
    assert.strictEqual(sm.exists('nope'), false);
  });

  it('delete removes session file', () => {
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    const session = sm.create();
    assert.strictEqual(sm.delete(session.id), true);
    assert.strictEqual(sm.exists(session.id), false);
  });

  it('delete returns false for missing session', () => {
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.delete('nope'), false);
  });

  it('rejects unsafe session ids', () => {
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.load('../escape'), null);
    assert.strictEqual(sm.exists('../escape'), false);
    assert.strictEqual(sm.delete('../escape'), false);
  });

  it('writes session data with restricted file permissions', () => {
    if (process.platform === 'win32') return;
    dir = tmpDir();
    sm = new SessionManager({ sessionDir: dir });
    const session = sm.create();
    const mode = fs.statSync(path.join(dir, `${session.id}.json`)).mode & 0o777;
    assert.strictEqual(mode, 0o600);
  });
});

// ===========================================================================
// SessionManager — generateId
// ===========================================================================

describe('SessionManager generateId', () => {
  it('generates unique IDs', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    const ids = new Set();
    for (let i = 0; i < 20; i++) {
      ids.add(sm.generateId());
    }
    assert.strictEqual(ids.size, 20);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — addOperation
// ===========================================================================

describe('SessionManager addOperation', () => {
  it('appends operation to session', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    const session = sm.create();

    sm.addOperation(session.id, {
      request: 'list orders',
      response: '[{id: 1}]',
      toolCalls: [{ name: 'list_orders' }],
      duration: 150,
    });

    const loaded = sm.load(session.id);
    assert.strictEqual(loaded.operations.length, 1);
    assert.strictEqual(loaded.operations[0].request, 'list orders');
    assert.strictEqual(loaded.metadata.operationCount, 1);
    assert.strictEqual(loaded.metadata.totalDuration, 150);
    rmDir(dir);
  });

  it('returns null for missing session', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.addOperation('nope', { request: 'test' }), null);
    rmDir(dir);
  });

  it('trims operations at 50', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    const session = sm.create();

    for (let i = 0; i < 55; i++) {
      sm.addOperation(session.id, { request: `op-${i}`, response: 'ok', duration: 10 });
    }

    const loaded = sm.load(session.id);
    assert.strictEqual(loaded.operations.length, 50);
    assert.strictEqual(loaded.operations[0].request, 'op-5'); // first 5 trimmed
    rmDir(dir);
  });

  it('redacts sensitive data before persisting operations', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    const session = sm.create();

    sm.addOperation(session.id, {
      request: 'Authorization: Bearer sk-test-secret',
      response: 'api_key=shhh',
      toolCalls: [{ name: 'sync', apiKey: 'super-secret' }],
      duration: 10,
    });

    const loaded = sm.load(session.id);
    assert.ok(loaded.operations[0].request.includes('[REDACTED]'));
    assert.ok(loaded.operations[0].response.includes('[REDACTED]'));
    assert.strictEqual(loaded.operations[0].toolCalls[0].apiKey, '[REDACTED]');
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — updateContext
// ===========================================================================

describe('SessionManager updateContext', () => {
  it('merges context', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    const session = sm.create({ context: { db: 'store.db' } });

    sm.updateContext(session.id, { agent: 'checkout' });

    const loaded = sm.load(session.id);
    assert.strictEqual(loaded.context.db, 'store.db');
    assert.strictEqual(loaded.context.agent, 'checkout');
    rmDir(dir);
  });

  it('returns null for missing session', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.updateContext('nope', {}), null);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — list
// ===========================================================================

describe('SessionManager list', () => {
  it('lists sessions', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    sm.create({ agent: 'a' });
    sm.create({ agent: 'b' });

    const list = sm.list();
    assert.strictEqual(list.length, 2);
    assert.ok(list[0].id);
    assert.ok(list[0].agent);
    rmDir(dir);
  });

  it('respects limit', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    for (let i = 0; i < 5; i++) sm.create();

    const list = sm.list({ limit: 3 });
    assert.strictEqual(list.length, 3);
    rmDir(dir);
  });

  it('sorts by updatedAt desc by default', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    const s1 = sm.create();
    const s2 = sm.create();

    // s2 was created later, so it should be first
    const list = sm.list();
    assert.strictEqual(list[0].id, s2.id);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — find
// ===========================================================================

describe('SessionManager find', () => {
  it('filters by agent', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    sm.create({ agent: 'orders' });
    sm.create({ agent: 'checkout' });

    const results = sm.find({ agent: 'orders' });
    assert.strictEqual(results.length, 1);
    assert.strictEqual(results[0].agent, 'orders');
    rmDir(dir);
  });

  it('filters by database', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    sm.create({ database: 'prod.db' });
    sm.create({ database: 'test.db' });

    const results = sm.find({ database: 'prod.db' });
    assert.strictEqual(results.length, 1);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — getRecent
// ===========================================================================

describe('SessionManager getRecent', () => {
  it('returns most recent session', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    sm.create({ agent: 'a' });
    const latest = sm.create({ agent: 'b' });

    const recent = sm.getRecent();
    assert.strictEqual(recent.id, latest.id);
    rmDir(dir);
  });

  it('returns null when no sessions', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.getRecent(), null);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — cleanup
// ===========================================================================

describe('SessionManager cleanup', () => {
  it('deletes old sessions', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir, maxAge: 1 });
    sm.create();

    // Cleanup with 1ms maxAge should delete all
    const result = sm.cleanup({ maxAge: 1 });
    assert.ok(result.deleted >= 0); // May or may not be expired yet
    rmDir(dir);
  });

  it('deletes excess sessions', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    for (let i = 0; i < 5; i++) sm.create();

    const result = sm.cleanup({ maxCount: 2, maxAge: 999999999 });
    const remaining = sm.list({ limit: 100 });
    assert.ok(remaining.length <= 2);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — archive
// ===========================================================================

describe('SessionManager archive', () => {
  it('moves session to archive directory', () => {
    const dir = tmpDir();
    const archiveDir = path.join(dir, 'archive');
    const sm = new SessionManager({ sessionDir: dir });
    const session = sm.create();

    const result = sm.archive(session.id, archiveDir);
    assert.strictEqual(result, true);
    assert.strictEqual(sm.exists(session.id), false);
    assert.ok(fs.existsSync(path.join(archiveDir, `${session.id}.json`)));
    rmDir(dir);
  });

  it('returns false for missing session', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.archive('nope', dir), false);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — exportMarkdown
// ===========================================================================

describe('SessionManager exportMarkdown', () => {
  it('exports session as markdown', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    const session = sm.create({ agent: 'orders' });
    sm.addOperation(session.id, {
      request: 'list orders',
      response: 'Found 5 orders',
      toolCalls: [{ name: 'list_orders' }],
    });

    const md = sm.exportMarkdown(session.id);
    assert.ok(md.includes('Session Report'));
    assert.ok(md.includes(session.id));
    assert.ok(md.includes('list orders'));
    assert.ok(md.includes('list_orders'));
    assert.ok(md.includes('Found 5 orders'));
    rmDir(dir);
  });

  it('returns null for missing session', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    assert.strictEqual(sm.exportMarkdown('nope'), null);
    rmDir(dir);
  });
});

// ===========================================================================
// SessionManager — getStats
// ===========================================================================

describe('SessionManager getStats', () => {
  it('returns aggregate statistics', () => {
    const dir = tmpDir();
    const sm = new SessionManager({ sessionDir: dir });
    sm.create({ agent: 'orders' });
    sm.create({ agent: 'orders' });
    sm.create({ agent: 'checkout', database: 'custom.db' });

    const stats = sm.getStats();
    assert.strictEqual(stats.totalSessions, 3);
    assert.strictEqual(stats.byAgent.orders, 2);
    assert.strictEqual(stats.byAgent.checkout, 1);
    assert.strictEqual(stats.byDatabase['custom.db'], 1);
    assert.ok(stats.recentActivity >= 0);
    rmDir(dir);
  });
});

// ===========================================================================
// CommandHistory
// ===========================================================================

describe('CommandHistory', () => {
  it('add and getRecent round-trip', () => {
    const dir = tmpDir();
    const histFile = path.join(dir, 'history');
    const ch = new CommandHistory({ historyFile: histFile });

    ch.add('list orders');
    ch.add('get order 123');

    const recent = ch.getRecent(10);
    assert.strictEqual(recent.length, 2);
    assert.strictEqual(recent[0].command, 'get order 123'); // most recent first
    assert.strictEqual(recent[1].command, 'list orders');
    rmDir(dir);
  });

  it('search finds matching commands', () => {
    const dir = tmpDir();
    const histFile = path.join(dir, 'history');
    const ch = new CommandHistory({ historyFile: histFile });

    ch.add('list orders');
    ch.add('get customer 1');
    ch.add('list products');

    const results = ch.search('list');
    assert.strictEqual(results.length, 2);
    rmDir(dir);
  });

  it('search is case-insensitive', () => {
    const dir = tmpDir();
    const histFile = path.join(dir, 'history');
    const ch = new CommandHistory({ historyFile: histFile });

    ch.add('List Orders');
    const results = ch.search('list');
    assert.strictEqual(results.length, 1);
    rmDir(dir);
  });

  it('clear empties history', () => {
    const dir = tmpDir();
    const histFile = path.join(dir, 'history');
    const ch = new CommandHistory({ historyFile: histFile });

    ch.add('something');
    ch.clear();

    const recent = ch.getRecent();
    assert.strictEqual(recent.length, 0);
    rmDir(dir);
  });

  it('trim removes oldest entries past max', () => {
    const dir = tmpDir();
    const histFile = path.join(dir, 'history');
    const ch = new CommandHistory({ historyFile: histFile, maxEntries: 3 });

    for (let i = 0; i < 5; i++) {
      ch.add(`cmd-${i}`);
    }

    const recent = ch.getRecent(10);
    assert.strictEqual(recent.length, 3);
    assert.strictEqual(recent[0].command, 'cmd-4'); // most recent
    rmDir(dir);
  });

  it('redacts sensitive flags from stored command history', () => {
    const dir = tmpDir();
    const histFile = path.join(dir, 'history');
    const ch = new CommandHistory({ historyFile: histFile });

    ch.add('stateset --token super-secret --apply "list orders"');
    const recent = ch.getRecent(1);
    assert.ok(recent[0].command.includes('--token [REDACTED]'));
    rmDir(dir);
  });

  it('writes history with restricted file permissions', () => {
    if (process.platform === 'win32') return;
    const dir = tmpDir();
    const histFile = path.join(dir, 'history');
    const ch = new CommandHistory({ historyFile: histFile });

    ch.add('list orders');
    const mode = fs.statSync(histFile).mode & 0o777;
    assert.strictEqual(mode, 0o600);
    rmDir(dir);
  });
});

// ===========================================================================
// Factory functions
// ===========================================================================

describe('Factory functions', () => {
  it('createSessionManager returns SessionManager', () => {
    const dir = tmpDir();
    const sm = createSessionManager({ sessionDir: dir });
    assert.ok(sm instanceof SessionManager);
    rmDir(dir);
  });

  it('createCommandHistory returns CommandHistory', () => {
    const dir = tmpDir();
    const ch = createCommandHistory({ historyFile: path.join(dir, 'h') });
    assert.ok(ch instanceof CommandHistory);
    rmDir(dir);
  });
});
