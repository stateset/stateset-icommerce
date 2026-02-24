/**
 * Unit tests for session-persistence.js — SessionPersistence
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { SessionPersistence } from '../../src/session-persistence.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function tmpSessionDir() {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'sess-test-'));
}

// ===========================================================================
// SessionPersistence
// ===========================================================================

describe('SessionPersistence', () => {
  /** @type {SessionPersistence|null} */
  let sp = null;

  afterEach(async () => {
    sp = null;
  });

  it('initializes and creates directory', async () => {
    const dir = path.join(os.tmpdir(), `sess-init-${Date.now()}`);
    sp = new SessionPersistence({ sessionDir: dir });
    await sp.initialize();
    assert.ok(fs.existsSync(dir));
    assert.strictEqual(sp.initialized, true);
  });

  it('initialize is idempotent', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.initialize();
    await sp.initialize(); // Should not throw
    assert.strictEqual(sp.initialized, true);
  });

  it('saveSession persists and returns session data', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const saved = await sp.saveSession({ id: 'sess-1', operations: [] });
    assert.strictEqual(saved.id, 'sess-1');
    assert.ok(saved.lastAccessedAt);
    assert.ok(saved.persistedAt);
  });

  it('saveSession writes to disk', async () => {
    const dir = tmpSessionDir();
    sp = new SessionPersistence({ sessionDir: dir });
    await sp.saveSession({ id: 'sess-disk', operations: [] });
    const filePath = path.join(dir, 'sess-disk.json');
    assert.ok(fs.existsSync(filePath));
    const content = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
    assert.strictEqual(content.id, 'sess-disk');
  });

  it('saveSession rejects unsafe session ids', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await assert.rejects(
      () => sp.saveSession({ id: '../escape', operations: [] }),
      /Invalid session id/,
    );
  });

  it('getSession retrieves saved session', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.saveSession({ id: 'sess-get', operations: [{ tool: 'list_orders' }] });
    const session = await sp.getSession('sess-get');
    assert.ok(session);
    assert.strictEqual(session.id, 'sess-get');
    assert.strictEqual(session.operations.length, 1);
  });

  it('getSession returns null for unknown session', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.initialize();
    const session = await sp.getSession('nonexistent');
    assert.strictEqual(session, null);
  });

  it('getSession returns null for unsafe session ids', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.initialize();
    const session = await sp.getSession('../escape');
    assert.strictEqual(session, null);
  });

  it('getSession returns null for expired session', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir(), sessionTtl: 5000 });
    await sp.saveSession({ id: 'sess-expired', operations: [] });
    // Expire it by manually setting lastAccessedAt far in the past
    const session = sp.sessions.get('sess-expired');
    session.lastAccessedAt = Date.now() - 10000;
    const result = await sp.getSession('sess-expired');
    assert.strictEqual(result, null);
  });

  it('deleteSession removes from map and disk', async () => {
    const dir = tmpSessionDir();
    sp = new SessionPersistence({ sessionDir: dir });
    await sp.saveSession({ id: 'sess-del', operations: [] });
    await sp.deleteSession('sess-del');

    assert.strictEqual(sp.sessions.has('sess-del'), false);
    assert.strictEqual(fs.existsSync(path.join(dir, 'sess-del.json')), false);
  });

  it('deleteSession handles nonexistent session gracefully', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.initialize();
    await sp.deleteSession('nonexistent'); // Should not throw
  });

  it('listSessions returns sorted by lastAccessedAt', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.saveSession({ id: 'sess-a', operations: [], createdAt: 1000 });
    // Wait a tick so timestamps differ
    await new Promise((r) => setTimeout(r, 5));
    await sp.saveSession({ id: 'sess-b', operations: [], createdAt: 2000 });

    const list = await sp.listSessions();
    assert.strictEqual(list.length, 2);
    assert.strictEqual(list[0].id, 'sess-b'); // Most recent first
    assert.strictEqual(list[1].id, 'sess-a');
  });

  it('listSessions respects limit', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    for (let i = 0; i < 5; i++) {
      await sp.saveSession({ id: `sess-${i}`, operations: [] });
    }
    const list = await sp.listSessions({ limit: 2 });
    assert.strictEqual(list.length, 2);
  });

  it('listSessions includes operation count', async () => {
    sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.saveSession({
      id: 'sess-ops',
      operations: [{ tool: 'a' }, { tool: 'b' }, { tool: 'c' }],
    });
    const list = await sp.listSessions();
    assert.strictEqual(list[0].operationCount, 3);
  });

  it('persists across instances (loads from disk)', async () => {
    const dir = tmpSessionDir();
    sp = new SessionPersistence({ sessionDir: dir });
    await sp.saveSession({ id: 'persist-test', operations: [{ tool: 'get_order' }] });

    const sp2 = new SessionPersistence({ sessionDir: dir });
    await sp2.initialize();
    const session = await sp2.getSession('persist-test');
    assert.ok(session);
    assert.strictEqual(session.id, 'persist-test');
  });

  it('cleans up expired sessions on initialize', async () => {
    const dir = tmpSessionDir();
    // Write an expired session file directly
    const expiredSession = {
      id: 'old-session',
      lastAccessedAt: Date.now() - 48 * 60 * 60 * 1000,
      operations: [],
    };
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'old-session.json'), JSON.stringify(expiredSession));

    sp = new SessionPersistence({ sessionDir: dir, sessionTtl: 24 * 60 * 60 * 1000 });
    await sp.initialize();
    assert.strictEqual(sp.sessions.has('old-session'), false);
  });

  it('writes session files with restricted permissions', async () => {
    if (process.platform === 'win32') return;
    const dir = tmpSessionDir();
    sp = new SessionPersistence({ sessionDir: dir });
    await sp.saveSession({ id: 'sess-perm', operations: [] });
    const mode = fs.statSync(path.join(dir, 'sess-perm.json')).mode & 0o777;
    assert.strictEqual(mode, 0o600);
  });
});

// ===========================================================================
// suggestNextSteps
// ===========================================================================

describe('suggestNextSteps', () => {
  it('suggests start_fresh for empty operations', () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const suggestions = sp.suggestNextSteps({ operations: [] });
    assert.strictEqual(suggestions.length, 1);
    assert.strictEqual(suggestions[0].action, 'start_fresh');
  });

  it('suggests retry for failed operation', () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const suggestions = sp.suggestNextSteps({
      operations: [{ tool: 'create_order', status: 'failed' }],
    });
    assert.ok(suggestions.some((s) => s.action === 'retry_last_operation'));
  });

  it('suggests reserve_inventory after create_order', () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const suggestions = sp.suggestNextSteps({
      operations: [{ tool: 'create_order', status: 'success', result: {} }],
    });
    assert.ok(suggestions.some((s) => s.action === 'reserve_inventory'));
  });

  it('suggests confirm_reservation after reserve_inventory', () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const suggestions = sp.suggestNextSteps({
      operations: [{ tool: 'reserve_inventory', status: 'success', result: {} }],
    });
    assert.ok(suggestions.some((s) => s.action === 'confirm_reservation'));
  });

  it('suggests rollback when pendingRollback exists', () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const suggestions = sp.suggestNextSteps({
      operations: [{ tool: 'some_tool', status: 'success' }],
      state: { pendingRollback: { reason: 'partial failure' } },
    });
    assert.ok(suggestions.some((s) => s.action === 'execute_rollback'));
  });

  it('always includes continue_new_operation', () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const suggestions = sp.suggestNextSteps({
      operations: [{ tool: 'create_order', status: 'success' }],
    });
    assert.ok(suggestions.some((s) => s.action === 'continue_new_operation'));
  });
});

// ===========================================================================
// restoreSession / exportSession / importSession / createAuditTrail
// ===========================================================================

describe('restoreSession', () => {
  it('restores a valid session', async () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.saveSession({
      id: 'restore-test',
      operations: [{ tool: 'list_orders', status: 'success' }],
      state: { cart: 'cart-1' },
      metadata: { agent: 'orders' },
    });
    const restored = await sp.restoreSession('restore-test');
    assert.strictEqual(restored.canResume, true);
    assert.strictEqual(restored.context.operations.length, 1);
    assert.ok(restored.nextSteps.length > 0);
  });

  it('throws for nonexistent session', async () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.initialize();
    await assert.rejects(() => sp.restoreSession('missing'), /not found or expired/);
  });
});

describe('exportSession', () => {
  it('exports session data', async () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.saveSession({ id: 'export-test', operations: [{ tool: 'a' }] });
    const exported = await sp.exportSession('export-test');
    assert.strictEqual(exported.id, 'export-test');
    assert.ok(exported.exportTimestamp);
    assert.strictEqual(exported.operations.length, 1);
  });

  it('throws for unknown session', async () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.initialize();
    await assert.rejects(() => sp.exportSession('nope'), /not found/);
  });
});

describe('importSession', () => {
  it('imports valid session data', async () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    const imported = await sp.importSession({
      id: 'import-test',
      operations: [{ tool: 'get_customer' }],
    });
    assert.strictEqual(imported.id, 'import-test');
    assert.strictEqual(imported.status, 'imported');
  });

  it('throws for invalid session data', async () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await assert.rejects(() => sp.importSession({}), /missing id or operations/);
  });
});

describe('createAuditTrail', () => {
  it('creates audit trail from session', async () => {
    const sp = new SessionPersistence({ sessionDir: tmpSessionDir() });
    await sp.saveSession({
      id: 'audit-test',
      createdAt: 1000,
      operations: [
        { tool: 'create_order', status: 'success', timestamp: 1001, params: {} },
        {
          tool: 'reserve_inventory',
          status: 'failed',
          timestamp: 1002,
          params: {},
          error: 'No stock',
        },
      ],
    });
    const audit = await sp.createAuditTrail('audit-test');
    assert.strictEqual(audit.sessionId, 'audit-test');
    assert.strictEqual(audit.totalOperations, 2);
    assert.strictEqual(audit.successfulOperations, 1);
    assert.strictEqual(audit.failedOperations, 1);
    assert.ok(audit.generatedAt);
  });
});
