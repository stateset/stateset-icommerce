import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import fs from 'node:fs';
import os from 'node:os';

import { AuditStore, resetAuditStore } from '../../src/audit-store.js';

const TEST_DB_DIR = path.join(os.tmpdir(), 'stateset-audit-test-' + process.pid);
const TEST_DB_PATH = path.join(TEST_DB_DIR, 'audit.db');

describe('AuditStore', () => {
  /** @type {AuditStore} */
  let store;

  before(() => {
    resetAuditStore();
    store = new AuditStore({ dbPath: TEST_DB_PATH });
  });

  after(() => {
    store.close();
    resetAuditStore();
    // Clean up test database
    try {
      fs.rmSync(TEST_DB_DIR, { recursive: true });
    } catch {
      /* ignore */
    }
  });

  describe('log()', () => {
    it('inserts an audit entry', () => {
      store.log({
        tool: 'create_order',
        params: { customerId: 'c1', totalAmount: 100 },
        result: 'allowed',
        level: 'write',
      });

      const entries = store.query({ tool: 'create_order' });
      assert.equal(entries.length, 1);
      assert.equal(entries[0].tool, 'create_order');
      assert.equal(entries[0].result, 'allowed');
      assert.equal(entries[0].level, 'write');
      assert.deepEqual(entries[0].params, { customerId: 'c1', totalAmount: 100 });
    });

    it('stores denied entries with reasons', () => {
      store.log({
        tool: 'cancel_order',
        result: 'denied',
        reason: 'Insufficient permission level',
        level: 'preview',
      });

      const denied = store.query({ result: 'denied' });
      assert.ok(denied.length >= 1);
      const entry = denied.find((e) => e.tool === 'cancel_order');
      assert.ok(entry);
      assert.equal(entry.reason, 'Insufficient permission level');
    });

    it('stores session_id and agent metadata', () => {
      store.log({
        tool: 'list_orders',
        result: 'allowed',
        level: 'read',
        sessionId: 'sess-123',
        agent: 'orders-agent',
      });

      const entries = store.query({ tool: 'list_orders' });
      const entry = entries.find((e) => e.session_id === 'sess-123');
      assert.ok(entry);
      assert.equal(entry.agent, 'orders-agent');
    });
  });

  describe('query()', () => {
    it('filters by tool name', () => {
      const results = store.query({ tool: 'create_order' });
      assert.ok(results.every((e) => e.tool === 'create_order'));
    });

    it('filters by result', () => {
      const results = store.query({ result: 'denied' });
      assert.ok(results.every((e) => e.result === 'denied'));
    });

    it('filters by since timestamp', () => {
      const future = new Date(Date.now() + 86400000).toISOString();
      const results = store.query({ since: future });
      assert.equal(results.length, 0);
    });

    it('respects limit', () => {
      const results = store.query({ limit: 1 });
      assert.equal(results.length, 1);
    });

    it('returns entries in descending timestamp order', () => {
      const results = store.query({});
      for (let i = 1; i < results.length; i++) {
        assert.ok(results[i - 1].timestamp >= results[i].timestamp);
      }
    });
  });

  describe('count()', () => {
    it('returns total entry count', () => {
      const count = store.count();
      assert.ok(count >= 3, `Expected at least 3, got ${count}`);
    });
  });

  describe('durable fallback', () => {
    it('persists audit entries when SQLite is unavailable', () => {
      const fallbackPath = path.join(TEST_DB_DIR, 'audit-fallback.db');
      const fallbackStore = new AuditStore({ dbPath: fallbackPath, databaseCtor: null });

      fallbackStore.log({
        tool: 'create_order',
        params: { customerId: 'c-fallback' },
        result: 'allowed',
        level: 'write',
      });
      fallbackStore.close();

      const reopened = new AuditStore({ dbPath: fallbackPath, databaseCtor: null });
      const entries = reopened.query({ tool: 'create_order' });
      assert.equal(reopened.backend, 'json-fallback');
      assert.equal(entries.length, 1);
      assert.deepEqual(entries[0].params, { customerId: 'c-fallback' });
      assert.ok(fs.existsSync(`${fallbackPath}.fallback.json`));

      reopened.close();
    });
  });

  describe('export()', () => {
    it('returns exportedAt, totalEntries, and entries', () => {
      const exported = store.export();
      assert.ok(exported.exportedAt);
      assert.equal(typeof exported.totalEntries, 'number');
      assert.ok(Array.isArray(exported.entries));
      assert.ok(exported.entries.length > 0);
    });

    it('respects since filter in export', () => {
      const future = new Date(Date.now() + 86400000).toISOString();
      const exported = store.export({ since: future });
      assert.equal(exported.entries.length, 0);
    });
  });

  describe('cleanup()', () => {
    it('removes entries older than retention period', () => {
      // Create store with very short retention
      const shortRetention = new AuditStore({
        dbPath: path.join(TEST_DB_DIR, 'audit-cleanup.db'),
        retentionDays: 0, // 0 days = delete everything older than now
      });

      shortRetention.log({ tool: 'test_tool', result: 'allowed', level: 'read' });
      assert.equal(shortRetention.count(), 1);

      // Cleanup with 0-day retention deletes entries timestamped before now
      // Since the entry was just inserted, it might survive — use negative to force
      shortRetention.retentionDays = -1;
      shortRetention.cleanup();
      assert.equal(shortRetention.count(), 0);

      shortRetention.close();
      try {
        fs.unlinkSync(path.join(TEST_DB_DIR, 'audit-cleanup.db'));
      } catch {
        /* ignore */
      }
    });
  });
});
