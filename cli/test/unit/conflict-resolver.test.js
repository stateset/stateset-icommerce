/**
 * Tests for cli/src/sync/conflict.js
 *
 * Covers: ConflictResolver.detectConflicts, resolve strategies,
 * merge logic, conflict CRUD, resolveAll, skipConflict.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import Database from 'better-sqlite3';

import { ConflictResolver, createConflictResolver } from '../../src/sync/conflict.js';
import { createOutbox } from '../../src/sync/outbox.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { SyncConfig } from '../../src/sync/config.js';

// ---------------------------------------------------------------------------
// Mock Outbox
// ---------------------------------------------------------------------------

function createMockOutbox() {
  const events = [];
  const versions = new Map();
  let nextSeq = 1;

  const db = {
    exec: () => {},
    prepare: (sql) => ({
      run: (...params) => ({ changes: 1 }),
      get: (id) => null,
      all: () => [],
    }),
  };

  return {
    db,
    events,
    getEntityVersion: (tenantId, storeId, entityType, entityId) => {
      return versions.get(`${entityType}:${entityId}`) ?? null;
    },
    updateEntityVersion: (tenantId, storeId, entityType, entityId, version) => {
      versions.set(`${entityType}:${entityId}`, version);
    },
    setEntityVersion: (entityType, entityId, version) => {
      versions.set(`${entityType}:${entityId}`, version);
    },
    append: (event) => {
      const seq = nextSeq++;
      events.push({ ...event, localSeq: seq });
      return seq;
    },
    markRejected: (seq, reason) => {},
    _rowToEvent: (row) => row,
  };
}

function makeLocalEvent(overrides = {}) {
  return {
    localSeq: 1,
    eventId: 'evt-1',
    commandId: 'cmd-1',
    tenantId: 't1',
    storeId: 's1',
    entityType: 'order',
    entityId: 'ord-1',
    eventType: 'OrderCreated',
    payload: { total: 100 },
    sourceAgent: 'agent-aaa',
    baseVersion: 1,
    createdAt: new Date(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// ConflictResolver
// ---------------------------------------------------------------------------

describe('ConflictResolver', () => {
  let outbox;
  let resolver;

  beforeEach(() => {
    outbox = createMockOutbox();
    resolver = new ConflictResolver(outbox);
  });

  // -------------------------------------------------------------------------
  // Detection
  // -------------------------------------------------------------------------

  describe('detectConflicts', () => {
    it('detects version conflict when baseVersion < currentVersion', () => {
      outbox.setEntityVersion('order', 'ord-1', 5);
      const local = makeLocalEvent({ baseVersion: 3 });
      const conflicts = resolver.detectConflicts([local], []);
      assert.equal(conflicts.length, 1);
      assert.equal(conflicts[0].type, 'version');
    });

    it('no conflict when baseVersion matches', () => {
      outbox.setEntityVersion('order', 'ord-1', 3);
      const local = makeLocalEvent({ baseVersion: 3 });
      const conflicts = resolver.detectConflicts([local], []);
      assert.equal(conflicts.length, 0);
    });

    it('no conflict when baseVersion is null', () => {
      const local = makeLocalEvent({ baseVersion: null });
      const conflicts = resolver.detectConflicts([local], []);
      assert.equal(conflicts.length, 0);
    });

    it('no conflict when entity has no local version', () => {
      const local = makeLocalEvent({ baseVersion: 1 });
      const conflicts = resolver.detectConflicts([local], []);
      assert.equal(conflicts.length, 0);
    });

    it('detects concurrent modification conflict', () => {
      const now = new Date();
      const local = makeLocalEvent({
        sourceAgent: 'agent-aaa',
        createdAt: now,
        baseVersion: null,
      });
      const remote = {
        entityType: 'order',
        entityId: 'ord-1',
        sourceAgent: 'agent-bbb',
        sequencedAt: new Date(now.getTime() + 1000).toISOString(),
      };

      const conflicts = resolver.detectConflicts([local], [remote]);
      assert.equal(conflicts.length, 1);
      assert.equal(conflicts[0].type, 'concurrent');
    });

    it('no concurrent conflict for same agent', () => {
      const now = new Date();
      const local = makeLocalEvent({
        sourceAgent: 'agent-aaa',
        createdAt: now,
        baseVersion: null,
      });
      const remote = {
        entityType: 'order',
        entityId: 'ord-1',
        sourceAgent: 'agent-aaa',
        sequencedAt: now.toISOString(),
      };

      const conflicts = resolver.detectConflicts([local], [remote]);
      assert.equal(conflicts.length, 0);
    });
  });

  // -------------------------------------------------------------------------
  // Strategy suggestion
  // -------------------------------------------------------------------------

  describe('_suggestStrategy', () => {
    it('suggests merge for inventory', () => {
      const strategy = resolver._suggestStrategy('version', { entityType: 'inventory' });
      assert.equal(strategy, 'merge');
    });

    it('suggests remote-wins for order', () => {
      const strategy = resolver._suggestStrategy('version', { entityType: 'order' });
      assert.equal(strategy, 'remote-wins');
    });

    it('suggests local-wins for cart', () => {
      const strategy = resolver._suggestStrategy('version', { entityType: 'cart' });
      assert.equal(strategy, 'local-wins');
    });

    it('uses type default for unknown entity', () => {
      const strategy = resolver._suggestStrategy('invariant', { entityType: 'unknown' });
      assert.equal(strategy, 'manual');
    });

    it('falls back to default strategy', () => {
      resolver.defaultStrategy = 'local-wins';
      const strategy = resolver._suggestStrategy('unknown-type', { entityType: 'unknown' });
      assert.equal(strategy, 'local-wins');
    });
  });

  // -------------------------------------------------------------------------
  // Merge logic
  // -------------------------------------------------------------------------

  describe('_mergeInventory', () => {
    it('sums adjustments', () => {
      const result = resolver._mergeInventory({ adjustment: 10 }, { adjustment: -3 });
      assert.ok(result.canMerge);
      assert.equal(result.mergedPayload.adjustment, 7);
    });

    it('takes remote quantity for set operations', () => {
      const result = resolver._mergeInventory({ quantity: 10 }, { quantity: 20 });
      assert.ok(result.canMerge);
      assert.equal(result.mergedPayload.quantity, 20);
    });

    it('returns canMerge=false for incompatible changes', () => {
      const result = resolver._mergeInventory({ foo: 1 }, { bar: 2 });
      assert.ok(!result.canMerge);
    });
  });

  describe('_mergeCustomer', () => {
    it('merges non-conflicting fields', () => {
      const result = resolver._mergeCustomer(
        { name: 'Alice', phone: '555' },
        { name: 'Alice', email: 'a@b.com' },
      );
      assert.ok(result.canMerge);
      assert.equal(result.mergedPayload.phone, '555');
      assert.equal(result.mergedPayload.email, 'a@b.com');
    });

    it('fails when fields conflict', () => {
      const result = resolver._mergeCustomer({ name: 'Alice' }, { name: 'Bob' });
      assert.ok(!result.canMerge);
      assert.ok(result.conflictingFields.includes('name'));
    });
  });

  describe('_mergeGeneric', () => {
    it('merges identical payloads', () => {
      const payload = { x: 1, y: 2 };
      const result = resolver._mergeGeneric(payload, { ...payload });
      assert.ok(result.canMerge);
    });

    it('fails for different payloads', () => {
      const result = resolver._mergeGeneric({ x: 1 }, { x: 2 });
      assert.ok(!result.canMerge);
    });
  });

  // -------------------------------------------------------------------------
  // Resolution
  // -------------------------------------------------------------------------

  describe('resolve', () => {
    it('returns error for unknown conflict ID', async () => {
      const result = await resolver.resolve('nonexistent');
      assert.ok(!result.success);
      assert.equal(result.error, 'Conflict not found');
    });

    it('manual strategy returns requires-manual error', async () => {
      const conflict = {
        id: 'c1',
        type: 'invariant',
        localEvent: makeLocalEvent(),
        remoteEvent: null,
        entityType: 'order',
        entityId: 'ord-1',
        suggestedStrategy: 'manual',
      };

      const result = await resolver.resolve(conflict, 'manual');
      assert.ok(!result.success);
      assert.ok(result.error.includes('Manual resolution'));
    });

    it('throws on unknown strategy', async () => {
      const conflict = {
        id: 'c1',
        type: 'version',
        localEvent: makeLocalEvent(),
        remoteEvent: null,
        entityType: 'order',
        entityId: 'ord-1',
        suggestedStrategy: 'bogus',
      };

      const result = await resolver.resolve(conflict, 'bogus');
      assert.ok(!result.success);
      assert.ok(result.error.includes('Unknown resolution'));
    });
  });

  // -------------------------------------------------------------------------
  // Conflict count
  // -------------------------------------------------------------------------

  describe('getConflictCount', () => {
    it('returns 0 when no conflicts', () => {
      // Mock the db.prepare to return count: 0
      outbox.db.prepare = () => ({ get: () => ({ count: 0 }), run: () => ({}), all: () => [] });
      assert.equal(resolver.getConflictCount(), 0);
    });
  });

  // -------------------------------------------------------------------------
  // Skip conflict
  // -------------------------------------------------------------------------

  describe('skipConflict', () => {
    it('calls db update without throwing', () => {
      let called = false;
      outbox.db.prepare = () => ({
        run: () => {
          called = true;
        },
        get: () => null,
        all: () => [],
      });
      resolver.skipConflict('c1', 'not important');
      assert.ok(called);
    });
  });

  // -------------------------------------------------------------------------
  // Factory
  // -------------------------------------------------------------------------

  describe('createConflictResolver', () => {
    it('creates a ConflictResolver', () => {
      const cr = createConflictResolver(outbox);
      assert.ok(cr instanceof ConflictResolver);
    });

    it('accepts custom defaultStrategy', () => {
      const cr = createConflictResolver(outbox, { defaultStrategy: 'local-wins' });
      assert.equal(cr.defaultStrategy, 'local-wins');
    });
  });
});

/**
 * `detectConflicts()` writes, and `pull()` now calls it — which puts it on the
 * `syncIntervalMs` background loop. A conflict id that was a fresh UUID per
 * detection therefore grew `_ves_conflicts` without bound off a single
 * unresolved conflict, inflating the `sync conflicts` listing and the
 * `sync_conflicts` MCP count. These assertions run against real SQLite,
 * because the defect was in what `INSERT OR REPLACE` did to real rows.
 */
describe('conflict persistence is idempotent', () => {
  let db;
  let resolver;

  /** Minimal outbox: a real database plus the one accessor detection reads. */
  function realishOutbox(database, currentVersion = 5) {
    return { db: database, getEntityVersion: () => currentVersion };
  }

  const local = () => ({
    localSeq: 7,
    eventId: 'evt-1',
    tenantId: 't1',
    storeId: 's1',
    entityType: 'order',
    entityId: 'ord-1',
    eventType: 'OrderUpdated',
    sourceAgent: 'agent-aaa',
    baseVersion: 1,
    createdAt: new Date('2026-09-12T00:00:00.000Z'),
  });

  const remote = () => ({
    sequence_number: 42,
    entity_type: 'order',
    entity_id: 'ord-1',
    source_agent: 'agent-bbb',
    sequenced_at: '2026-09-12T00:00:00.000Z',
  });

  function rowCount() {
    return db.prepare('SELECT COUNT(*) AS count FROM _ves_conflicts').get().count;
  }

  beforeEach(() => {
    db = new Database(':memory:');
    resolver = new ConflictResolver(realishOutbox(db));
  });

  afterEach(() => db.close());

  it('detecting the same conflict repeatedly yields exactly one row', () => {
    for (let i = 0; i < 3; i++) {
      const conflicts = resolver.detectConflicts([local()], [remote()]);
      assert.equal(conflicts.length, 1, 'the same conflict is still reported each time');
    }

    assert.equal(rowCount(), 1, 'three detections of one conflict must not be three rows');
    assert.equal(resolver.getConflictCount(), 1);
  });

  it('gives the same conflict the same id every time', () => {
    const first = resolver.detectConflicts([local()], [remote()])[0];
    const second = resolver.detectConflicts([local()], [remote()])[0];
    assert.equal(first.id, second.id);
    assert.equal(first.id, 'version:7:order:ord-1');
  });

  it('still distinguishes conflicts on different entities and local events', () => {
    resolver.detectConflicts([local()], [remote()]);
    resolver.detectConflicts([{ ...local(), localSeq: 8 }], [remote()]);
    resolver.detectConflicts(
      [{ ...local(), entityId: 'ord-2' }],
      [{ ...remote(), entity_id: 'ord-2' }],
    );
    assert.equal(rowCount(), 3);
  });

  it('does not resurrect a conflict an operator already resolved', () => {
    const conflict = resolver.detectConflicts([local()], [remote()])[0];
    db.prepare("UPDATE _ves_conflicts SET status = 'resolved' WHERE id = ?").run(conflict.id);

    resolver.detectConflicts([local()], [remote()]);

    assert.equal(rowCount(), 1);
    assert.equal(
      db.prepare('SELECT status FROM _ves_conflicts WHERE id = ?').get(conflict.id).status,
      'resolved',
    );
    assert.equal(resolver.getConflictCount(), 0);
  });
});

/**
 * `_ves_conflicts.status` is unresolved -> resolved | skipped, and resolving
 * supersedes the local event, which must still be unsettled. Before these
 * guards, resolving an already-resolved conflict three times appended three
 * rebased events and called markRejected on an event that had synced; the
 * append (async, unawaited) and the markRejected were not atomic; and
 * skipConflict flipped a resolved conflict to skipped (and resolve flipped a
 * skipped one back to resolved). These run against a real outbox and SQLite.
 */
describe('conflict resolution is guarded and atomic', () => {
  const TENANT = '550e8400-e29b-41d4-a716-446655440001';
  const STORE = '550e8400-e29b-41d4-a716-446655440002';
  const AGENT = '550e8400-e29b-41d4-a716-446655440003';
  let db;
  let configDir;
  let outbox;
  let resolver;

  function keyManager() {
    const pair = crypto.generateKeyPairSync('ed25519');
    const key = {
      keyId: 1,
      publicKey: pair.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32),
      privateKey: pair.privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32),
    };
    return { getCurrentSigningKey: async () => key, getCurrentEncryptionKey: async () => null };
  }

  /** Append a local event and record a conflict over it. */
  async function seed({ conflictStatus = 'unresolved', payload = { total: '10.00' } } = {}) {
    const seq = Number(
      await outbox.append({
        tenantId: TENANT,
        storeId: STORE,
        entityType: 'cart',
        entityId: 'cart-1',
        eventType: 'cart.updated',
        payload,
        sourceAgent: AGENT,
        baseVersion: 1,
      }),
    );
    const id = `version:${seq}:cart:cart-1`;
    db.prepare(
      `INSERT INTO _ves_conflicts (id, conflict_type, local_event_seq, entity_type, entity_id, description, suggested_strategy, status)
       VALUES (?, 'version', ?, 'cart', 'cart-1', 'behind', 'local-wins', ?)`,
    ).run(id, seq, conflictStatus);
    return { id, seq };
  }

  const outboxCount = () => db.prepare('SELECT COUNT(*) AS n FROM _ves_outbox').get().n;
  const syncStatus = (seq) =>
    db.prepare('SELECT sync_status FROM _ves_outbox WHERE local_seq = ?').get(seq).sync_status;
  const conflictRow = (id) => db.prepare('SELECT * FROM _ves_conflicts WHERE id = ?').get(id);

  beforeEach(() => {
    db = new Database(':memory:');
    configDir = mkdtempSync(join(tmpdir(), 'conflict-resolver-'));
    outbox = createOutbox(db, { keyManager: keyManager(), configDir });
    outbox.initialize();
    resolver = new ConflictResolver(outbox);
  });

  afterEach(() => {
    db.close();
    rmSync(configDir, { recursive: true, force: true });
  });

  it('refuses to re-resolve a resolved conflict over a synced event (no appends, no rejection)', async () => {
    const { id, seq } = await seed({ conflictStatus: 'resolved' });
    outbox.markSynced([{ localSeq: seq, remoteSeq: 5 }]);
    const before = outboxCount();

    const results = [];
    for (let i = 0; i < 3; i++) results.push(await resolver.resolve(id, 'local-wins'));

    assert.deepEqual(
      results.map((r) => r.success),
      [false, false, false],
    );
    assert.match(results[0].error, /already resolved/);
    assert.equal(outboxCount(), before, 'no rebased events may be appended');
    assert.equal(syncStatus(seq), 'synced', 'a synced event must not be marked rejected');
    assert.equal(conflictRow(id).status, 'resolved');
  });

  it('refuses to resolve an unresolved conflict whose local event already synced', async () => {
    const { id, seq } = await seed();
    outbox.markSynced([{ localSeq: seq, remoteSeq: 5 }]);
    const before = outboxCount();

    for (const strategy of ['local-wins', 'remote-wins', 'merge']) {
      const result = await resolver.resolve(id, strategy);
      assert.equal(result.success, false, strategy);
      assert.match(result.error, /already synced/);
    }
    assert.equal(outboxCount(), before);
    assert.equal(syncStatus(seq), 'synced');
    assert.equal(conflictRow(id).status, 'unresolved');
  });

  it('skipConflict only skips an unresolved conflict, and a skipped one cannot be resolved', async () => {
    const resolved = await seed({ conflictStatus: 'resolved' });
    assert.equal(resolver.skipConflict(resolved.id, 'late'), false);
    assert.equal(conflictRow(resolved.id).status, 'resolved');

    const open = await seed();
    assert.equal(resolver.skipConflict(open.id, 'not now'), true);
    assert.equal(conflictRow(open.id).status, 'skipped');
    assert.equal(resolver.skipConflict(open.id, 'again'), false);

    const result = await resolver.resolve(open.id, 'local-wins');
    assert.equal(result.success, false);
    assert.match(result.error, /already skipped/);
    assert.equal(conflictRow(open.id).status, 'skipped');
    assert.equal(syncStatus(open.seq), 'pending');
  });

  it('local-wins supersedes a pending event exactly once, with a real local sequence', async () => {
    const { id, seq } = await seed();
    const before = outboxCount();

    const result = await resolver.resolve(id, 'local-wins');
    assert.equal(result.success, true, result.error);
    assert.equal(typeof result.result.newSeq, 'number');
    assert.equal(outboxCount(), before + 1);
    assert.equal(syncStatus(seq), 'rejected');
    assert.equal(syncStatus(result.result.newSeq), 'pending');
    const stored = conflictRow(id);
    assert.equal(stored.status, 'resolved');
    assert.equal(stored.resolution_strategy, 'local-wins');
    assert.equal(JSON.parse(stored.resolution_data).newSeq, result.result.newSeq);

    const again = await resolver.resolve(id, 'local-wins');
    assert.equal(again.success, false);
    assert.equal(outboxCount(), before + 1);
  });

  it('remote-wins rejects the pending event and resolves the conflict', async () => {
    const { id, seq } = await seed();
    const result = await resolver.resolve(id, 'remote-wins');
    assert.equal(result.success, true, result.error);
    assert.equal(syncStatus(seq), 'rejected');
    assert.equal(conflictRow(id).status, 'resolved');
  });

  it('concurrent resolutions of one conflict apply exactly one', async () => {
    const { id } = await seed();
    const before = outboxCount();

    const results = await Promise.all([
      resolver.resolve(id, 'local-wins'),
      resolver.resolve(id, 'local-wins'),
    ]);

    assert.equal(results.filter((r) => r.success).length, 1);
    assert.equal(outboxCount(), before + 1);
  });

  it('a failed append leaves the local event pending and the conflict unresolved', async () => {
    const { id, seq } = await seed();
    db.exec(
      "CREATE TRIGGER refuse_rebase BEFORE INSERT ON _ves_outbox BEGIN SELECT RAISE(ABORT, 'disk full'); END",
    );

    const result = await resolver.resolve(id, 'local-wins');

    assert.equal(result.success, false);
    assert.match(result.error, /disk full/);
    assert.equal(syncStatus(seq), 'pending');
    assert.equal(conflictRow(id).status, 'unresolved');
  });

  it('the engine refuses to skip a conflict that is not unresolved', async () => {
    const { id } = await seed({ conflictStatus: 'resolved' });
    const engine = new SyncEngine({
      db,
      keyManager: keyManager(),
      preferGrpc: false,
      config: new SyncConfig({
        sequencer: { url: 'https://sequencer.invalid' },
        identity: { tenantId: TENANT, storeId: STORE, agentId: AGENT },
      }),
    });
    let emitted = 0;
    engine.on('conflictSkipped', () => emitted++);

    assert.throws(() => engine.skipConflict(id, 'late'), /already resolved/);
    assert.throws(() => engine.skipConflict('no-such-conflict'), /not found/);
    assert.equal(emitted, 0);
    assert.equal(conflictRow(id).status, 'resolved');
  });
});
