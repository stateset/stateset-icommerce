/**
 * Tests for cli/src/sync/conflict.js
 *
 * Covers: ConflictResolver.detectConflicts, resolve strategies,
 * merge logic, conflict CRUD, resolveAll, skipConflict.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { ConflictResolver, createConflictResolver } from '../../src/sync/conflict.js';

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
