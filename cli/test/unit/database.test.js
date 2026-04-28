/**
 * Unit tests for database.js — DatabaseManager
 *
 * DatabaseManager depends on @stateset/embedded (which may not be
 * installed).  We bypass this by directly instantiating the class and
 * stubbing the internal getCommerceCtor() path — only the methods that
 * actually call `getConnection` (and therefore require Commerce) need
 * the stub.  All pure-logic methods (resolvePath, formatSize, close,
 * closeAll, listConnections, evictOldest, isConnected-style checks)
 * are tested without any mocking.
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert';
import * as path from 'node:path';
import * as os from 'node:os';
import * as fs from 'node:fs';
import { createRequire } from 'node:module';

// We import only the class — the module-level require('@stateset/embedded')
// only fires inside getCommerceCtor(), which is lazy.
import { DatabaseManager, createDatabaseManager } from '../../src/database.js';

const require = createRequire(import.meta.url);
const BetterSqlite3 = require('better-sqlite3');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a fake Commerce instance with a .customers/.orders/.products/.returns mock */
function fakeCommerce() {
  return {
    customers: { count: () => 5 },
    orders: { count: () => 10 },
    products: { count: () => 20 },
    returns: { count: () => 2 },
  };
}

/**
 * Inject a fake connection into the manager's internal Map so we can test
 * cache / close / stats methods without needing a real Commerce constructor.
 */
function injectConnection(mgr, resolvedPath, commerce) {
  const now = Date.now();
  mgr.connections.set(resolvedPath, {
    commerce,
    path: resolvedPath,
    createdAt: now,
    lastUsed: now,
  });
}

// ===========================================================================
// constructor
// ===========================================================================

describe('DatabaseManager', () => {
  describe('constructor', () => {
    it('uses sensible defaults', () => {
      const mgr = new DatabaseManager();
      assert.strictEqual(mgr.defaultPath, './store.db');
      assert.strictEqual(mgr.maxConnections, 10);
      assert.strictEqual(mgr.connectionTimeout, 30000);
      assert.ok(mgr.connections instanceof Map);
      assert.strictEqual(mgr.connections.size, 0);
      assert.strictEqual(mgr.activeConnection, null);
    });

    it('accepts custom options', () => {
      const mgr = new DatabaseManager({
        defaultPath: '/tmp/custom.db',
        maxConnections: 5,
        connectionTimeout: 60000,
      });
      assert.strictEqual(mgr.defaultPath, '/tmp/custom.db');
      assert.strictEqual(mgr.maxConnections, 5);
      assert.strictEqual(mgr.connectionTimeout, 60000);
    });

    it('starts with empty connections map', () => {
      const mgr = new DatabaseManager();
      assert.strictEqual(mgr.connections.size, 0);
    });

    it('starts with null activeConnection', () => {
      const mgr = new DatabaseManager();
      assert.strictEqual(mgr.activeConnection, null);
    });
  });

  // =========================================================================
  // resolvePath
  // =========================================================================

  describe('resolvePath', () => {
    let mgr;

    beforeEach(() => {
      mgr = new DatabaseManager();
    });

    it('passes through :memory: unchanged', () => {
      assert.strictEqual(mgr.resolvePath(':memory:'), ':memory:');
    });

    it('resolves relative paths to absolute', () => {
      const result = mgr.resolvePath('./store.db');
      assert.ok(path.isAbsolute(result));
      assert.ok(result.endsWith('store.db'));
    });

    it('expands ~ to the home directory', () => {
      const result = mgr.resolvePath('~/data/commerce.db');
      const expected = path.join(os.homedir(), 'data/commerce.db');
      // path.resolve will normalise so compare resolved forms
      assert.strictEqual(result, path.resolve(expected));
    });

    it('resolves already-absolute path unchanged (after normalisation)', () => {
      const abs = '/tmp/stateset/my.db';
      assert.strictEqual(mgr.resolvePath(abs), path.resolve(abs));
    });

    it('resolves bare filename relative to cwd', () => {
      const result = mgr.resolvePath('local.db');
      assert.strictEqual(result, path.resolve('local.db'));
    });
  });

  // =========================================================================
  // formatSize
  // =========================================================================

  describe('formatSize', () => {
    let mgr;

    beforeEach(() => {
      mgr = new DatabaseManager();
    });

    it('formats bytes', () => {
      assert.strictEqual(mgr.formatSize(500), '500.00 B');
    });

    it('formats kilobytes', () => {
      assert.strictEqual(mgr.formatSize(1024), '1.00 KB');
    });

    it('formats megabytes', () => {
      assert.strictEqual(mgr.formatSize(1024 * 1024), '1.00 MB');
    });

    it('formats gigabytes', () => {
      assert.strictEqual(mgr.formatSize(1024 * 1024 * 1024), '1.00 GB');
    });

    it('formats fractional kilobytes', () => {
      assert.strictEqual(mgr.formatSize(1536), '1.50 KB');
    });

    it('formats zero bytes', () => {
      assert.strictEqual(mgr.formatSize(0), '0.00 B');
    });
  });

  // =========================================================================
  // connection cache management (injected connections, no real Commerce)
  // =========================================================================

  describe('connection cache management', () => {
    let mgr;

    beforeEach(() => {
      mgr = new DatabaseManager();
    });

    it('connections map tracks injected entries', () => {
      const fc = fakeCommerce();
      injectConnection(mgr, '/tmp/a.db', fc);
      assert.strictEqual(mgr.connections.size, 1);
      assert.strictEqual(mgr.connections.get('/tmp/a.db').commerce, fc);
    });

    it('close removes a connection and returns true', () => {
      injectConnection(mgr, '/tmp/a.db', fakeCommerce());
      const result = mgr.close('/tmp/a.db');
      assert.strictEqual(result, true);
      assert.strictEqual(mgr.connections.size, 0);
    });

    it('close returns false for unknown path', () => {
      const result = mgr.close('/tmp/nope.db');
      assert.strictEqual(result, false);
    });

    it('close clears activeConnection if it matches', () => {
      injectConnection(mgr, '/tmp/a.db', fakeCommerce());
      mgr.activeConnection = '/tmp/a.db';
      mgr.close('/tmp/a.db');
      assert.strictEqual(mgr.activeConnection, null);
    });

    it('close does not clear activeConnection if it does not match', () => {
      injectConnection(mgr, '/tmp/a.db', fakeCommerce());
      injectConnection(mgr, '/tmp/b.db', fakeCommerce());
      mgr.activeConnection = '/tmp/b.db';
      mgr.close('/tmp/a.db');
      assert.strictEqual(mgr.activeConnection, '/tmp/b.db');
    });

    it('closeAll clears all connections', () => {
      injectConnection(mgr, '/tmp/a.db', fakeCommerce());
      injectConnection(mgr, '/tmp/b.db', fakeCommerce());
      mgr.activeConnection = '/tmp/a.db';
      mgr.closeAll();
      assert.strictEqual(mgr.connections.size, 0);
      assert.strictEqual(mgr.activeConnection, null);
    });

    it('closeAll is safe when already empty', () => {
      mgr.closeAll();
      assert.strictEqual(mgr.connections.size, 0);
      assert.strictEqual(mgr.activeConnection, null);
    });
  });

  // =========================================================================
  // listConnections
  // =========================================================================

  describe('listConnections', () => {
    let mgr;

    beforeEach(() => {
      mgr = new DatabaseManager();
    });

    it('returns empty array when no connections', () => {
      assert.deepStrictEqual(mgr.listConnections(), []);
    });

    it('returns info for each connection', () => {
      injectConnection(mgr, '/tmp/a.db', fakeCommerce());
      injectConnection(mgr, '/tmp/b.db', fakeCommerce());
      mgr.activeConnection = '/tmp/a.db';

      const list = mgr.listConnections();
      assert.strictEqual(list.length, 2);

      const a = list.find((c) => c.path === '/tmp/a.db');
      const b = list.find((c) => c.path === '/tmp/b.db');
      assert.ok(a);
      assert.ok(b);
      assert.strictEqual(a.active, true);
      assert.strictEqual(b.active, false);
      assert.ok(typeof a.createdAt === 'number');
      assert.ok(typeof a.lastUsed === 'number');
    });
  });

  // =========================================================================
  // evictOldest
  // =========================================================================

  describe('evictOldest', () => {
    let mgr;

    beforeEach(() => {
      mgr = new DatabaseManager({ maxConnections: 2 });
    });

    it('evicts the connection with the oldest lastUsed timestamp', () => {
      injectConnection(mgr, '/tmp/old.db', fakeCommerce());
      // Make "old" genuinely older
      mgr.connections.get('/tmp/old.db').lastUsed = 1000;

      injectConnection(mgr, '/tmp/new.db', fakeCommerce());
      mgr.connections.get('/tmp/new.db').lastUsed = 2000;

      mgr.evictOldest();
      assert.strictEqual(mgr.connections.size, 1);
      assert.ok(mgr.connections.has('/tmp/new.db'));
      assert.ok(!mgr.connections.has('/tmp/old.db'));
    });

    it('does not evict the active connection', () => {
      injectConnection(mgr, '/tmp/active.db', fakeCommerce());
      mgr.connections.get('/tmp/active.db').lastUsed = 1000;
      mgr.activeConnection = '/tmp/active.db';

      injectConnection(mgr, '/tmp/other.db', fakeCommerce());
      mgr.connections.get('/tmp/other.db').lastUsed = 2000;

      mgr.evictOldest();
      // The active connection should survive even though it's oldest
      assert.ok(mgr.connections.has('/tmp/active.db'));
      assert.ok(!mgr.connections.has('/tmp/other.db'));
    });

    it('does nothing when only active connection exists', () => {
      injectConnection(mgr, '/tmp/solo.db', fakeCommerce());
      mgr.activeConnection = '/tmp/solo.db';

      mgr.evictOldest();
      assert.strictEqual(mgr.connections.size, 1);
    });

    it('does nothing when map is empty', () => {
      mgr.evictOldest();
      assert.strictEqual(mgr.connections.size, 0);
    });
  });

  // =========================================================================
  // exists
  // =========================================================================

  describe('exists', () => {
    let mgr;

    beforeEach(() => {
      mgr = new DatabaseManager();
    });

    it('returns false for :memory:', () => {
      assert.strictEqual(mgr.exists(':memory:'), false);
    });

    it('returns false for a path that does not exist on disk', () => {
      assert.strictEqual(mgr.exists('/tmp/__does_not_exist_test__.db'), false);
    });
  });

  // =========================================================================
  // backup / restore
  // =========================================================================

  describe('backup and restore', () => {
    let mgr;
    let tempDir;

    beforeEach(() => {
      mgr = new DatabaseManager();
      tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'stateset-db-test-'));
    });

    afterEach(() => {
      fs.rmSync(tempDir, { recursive: true, force: true });
    });

    it('copies SQLite sidecar files during backup', () => {
      const sourceDb = path.join(tempDir, 'store.db');
      const db = new BetterSqlite3(sourceDb);
      db.pragma('journal_mode = WAL');
      db.exec('CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)');
      db.exec(`INSERT INTO test (value) VALUES ('hello')`);

      const sourceWal = `${sourceDb}-wal`;
      const sourceShm = `${sourceDb}-shm`;
      assert.ok(fs.existsSync(sourceWal));
      assert.ok(fs.existsSync(sourceShm));

      const result = mgr.backup(sourceDb, tempDir);
      db.close();

      assert.ok(fs.existsSync(result.backup));
      assert.ok(fs.existsSync(`${result.backup}-wal`));
      assert.ok(fs.existsSync(`${result.backup}-shm`));
      assert.ok(result.size > 0);
    });

    it('restores SQLite sidecar files and removes stale target sidecars', () => {
      const backupDb = path.join(tempDir, 'snapshot.db');
      const targetDb = path.join(tempDir, 'restored.db');

      fs.writeFileSync(backupDb, 'backup-main');
      fs.writeFileSync(`${backupDb}-wal`, 'backup-wal');
      fs.writeFileSync(targetDb, 'old-main');
      fs.writeFileSync(`${targetDb}-wal`, 'old-wal');
      fs.writeFileSync(`${targetDb}-shm`, 'stale-shm');

      const result = mgr.restore(backupDb, targetDb);

      assert.strictEqual(fs.readFileSync(targetDb, 'utf8'), 'backup-main');
      assert.strictEqual(fs.readFileSync(`${targetDb}-wal`, 'utf8'), 'backup-wal');
      assert.ok(!fs.existsSync(`${targetDb}-shm`));
      assert.ok(result.size >= 'backup-main'.length + 'backup-wal'.length);
    });
  });

  // =========================================================================
  // createDatabaseManager factory
  // =========================================================================

  describe('createDatabaseManager', () => {
    it('returns a DatabaseManager instance', () => {
      const mgr = createDatabaseManager();
      assert.ok(mgr instanceof DatabaseManager);
    });

    it('passes options through', () => {
      const mgr = createDatabaseManager({ maxConnections: 3 });
      assert.strictEqual(mgr.maxConnections, 3);
    });
  });
});
