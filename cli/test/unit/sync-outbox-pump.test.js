/**
 * The pump turns committed kernel_outbox rows into signed VES envelopes.
 *
 * The engine writes its fact in the same transaction as the mutation, so the
 * COMMIT — not the signature — is the point past which an event cannot be
 * lost. Everything below pins the properties that make signing-after-commit
 * safe:
 *
 *  - identity is derived deterministically from the committed row, so a
 *    crash between commit and sign re-derives a byte-identical envelope;
 *  - a failure leaves the row drainable, so no event is lost;
 *  - an unmapped event_type fails loudly rather than being invented;
 *  - a duplicate event_id (the replay case) is absorbed by the pump, while
 *    any OTHER error is still a failure.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';

import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { createOutboxPump, VES_OUTBOX_NAMESPACE } from '../../src/sync/outbox-pump.js';

const IDENTITY = {
  tenantId: '22222222-2222-2222-2222-222222222222',
  storeId: '33333333-3333-3333-3333-333333333333',
  agentId: '44444444-4444-4444-4444-444444444444',
};

const ROW_ID = '55555555-5555-5555-5555-555555555555';

/** Generate a raw Ed25519 key pair (32-byte seed + 32-byte public key) */
function generateEd25519Raw() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const pubKey32 = Buffer.from(publicKey.export({ type: 'spki', format: 'der' }).subarray(-32));
  const privKey32 = Buffer.from(privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32));
  return { pubKey32, privKey32 };
}

/**
 * Mock AgentKeyManager returning stable in-memory keys, mirroring
 * makeMockKeyManager() in cli/test/sync-outbox.test.js. Stability matters:
 * a replay must re-sign with the same key to produce the same bytes.
 */
function makeMockKeyManager() {
  const sk = generateEd25519Raw();
  const ek = (() => {
    const { publicKey, privateKey } = crypto.generateKeyPairSync('x25519');
    return {
      pubKey32: Buffer.from(publicKey.export({ type: 'spki', format: 'der' }).subarray(-32)),
      privKey32: Buffer.from(privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32)),
    };
  })();

  return {
    getCurrentSigningKey: async () => ({
      keyId: 1,
      publicKey: sk.pubKey32,
      privateKey: sk.privKey32,
      createdAt: '2026-09-12T00:00:00.000Z',
    }),
    getCurrentEncryptionKey: async () => ({
      keyId: 1,
      publicKey: ek.pubKey32,
      privateKey: ek.privKey32,
      createdAt: '2026-09-12T00:00:00.000Z',
    }),
  };
}

/** Independent RFC 4122 v5 implementation, so the test pins the derivation. */
function expectedUuidV5(namespace, name) {
  const ns = Buffer.from(namespace.replace(/-/g, ''), 'hex');
  const digest = crypto
    .createHash('sha1')
    .update(Buffer.concat([ns, Buffer.from(name, 'utf8')]))
    .digest();
  const b = Buffer.from(digest.subarray(0, 16));
  b[6] = (b[6] & 0x0f) | 0x50;
  b[8] = (b[8] & 0x3f) | 0x80;
  const hex = b.toString('hex');
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    hex.slice(12, 16),
    hex.slice(16, 20),
    hex.slice(20),
  ].join('-');
}

function createKernelOutbox(db) {
  db.exec(`
    CREATE TABLE IF NOT EXISTS kernel_outbox (
      id TEXT PRIMARY KEY, event_type TEXT NOT NULL, aggregate_type TEXT NOT NULL,
      aggregate_id TEXT NOT NULL, payload TEXT NOT NULL, command_id TEXT,
      idempotency_key TEXT, principal_type TEXT, principal_id TEXT,
      correlation_id TEXT, causation_id TEXT, created_at TEXT NOT NULL,
      published_at TEXT, attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT,
      lease_owner TEXT, lease_expires_at TEXT, next_attempt_at TEXT,
      dead_lettered_at TEXT, tier TEXT NOT NULL DEFAULT 'governed'
    )
  `);
}

function seedKernelOutbox(db, row = {}) {
  createKernelOutbox(db);
  db.prepare(
    `INSERT INTO kernel_outbox (id, event_type, aggregate_type, aggregate_id, payload, created_at, tier)
     VALUES (?, ?, ?, ?, ?, ?, ?)`,
  ).run(
    row.id ?? ROW_ID,
    row.eventType ?? 'order.created',
    row.aggregateType ?? 'order',
    row.aggregateId ?? 'ORD-1',
    JSON.stringify(row.payload ?? { total: 1 }),
    row.createdAt ?? '2026-09-12T00:00:00.000Z',
    row.tier ?? 'recorded',
  );
}

describe('OutboxPump', () => {
  let db;
  let outbox;
  let pump;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, { keyManager: makeMockKeyManager() });
    pump = createOutboxPump(db, outbox, { identity: IDENTITY });
    seedKernelOutbox(db);
  });

  afterEach(() => db.close());

  it('appends a signed VES event per committed row', async () => {
    const result = await pump.drain();
    assert.equal(result.appended, 1);

    const [event] = outbox.getPending(10);
    assert.equal(event.entityType, 'order');
    assert.equal(event.entityId, 'ORD-1');
    assert.equal(event.eventType, 'order.created');
    assert.ok(event.agentSignature, 'the envelope must be signed');
    assert.deepEqual(event.payload, { total: 1 });
    assert.equal(event.tenantId, IDENTITY.tenantId);
    assert.equal(event.storeId, IDENTITY.storeId);
    assert.equal(event.sourceAgent, IDENTITY.agentId);
  });

  it('carries the committed row timestamp verbatim into the envelope', async () => {
    await pump.drain();
    const stored = db.prepare('SELECT created_at FROM _ves_outbox').get();
    assert.equal(
      stored.created_at,
      '2026-09-12T00:00:00.000Z',
      'the signing preimage must use the committed timestamp, not the wall clock',
    );
  });

  it('derives event_id as uuidv5(row id, VES_OUTBOX_NAMESPACE)', async () => {
    await pump.drain();
    const [event] = outbox.getPending(10);
    assert.equal(event.eventId, expectedUuidV5(VES_OUTBOX_NAMESPACE, ROW_ID));
  });

  it('derives event_id deterministically from the kernel_outbox row', async () => {
    await pump.drain();
    const [first] = outbox.getPending(10);

    // Simulate a crash after signing but before marking published.
    db.prepare('UPDATE kernel_outbox SET published_at = NULL').run();
    const replay = await pump.drain();

    const all = outbox.getPending(10);
    assert.equal(all.length, 1, 'a replay must not produce a second event');
    assert.equal(all[0].eventId, first.eventId);
    assert.equal(
      all[0].agentSignature,
      first.agentSignature,
      'same row must re-derive a byte-identical envelope',
    );
    assert.equal(replay.duplicates, 1, 'the replay must be reported as an absorbed duplicate');
    assert.equal(replay.failed, 0, 'an absorbed duplicate is not a failure');
    assert.notEqual(
      db.prepare('SELECT published_at FROM kernel_outbox').get().published_at,
      null,
      'an absorbed duplicate must leave the row published',
    );
  });

  it('marks rows published so they drain once', async () => {
    await pump.drain();
    const published = db
      .prepare('SELECT COUNT(*) AS n FROM kernel_outbox WHERE published_at IS NOT NULL')
      .get().n;
    assert.equal(published, 1);

    const second = await pump.drain();
    assert.equal(second.appended, 0);
    assert.equal(second.leased, 0);
  });

  it('records the error and increments attempts when append fails', async () => {
    outbox.append = async () => {
      throw new Error('signing key missing');
    };
    const result = await pump.drain();

    assert.equal(result.failed, 1);
    const row = db.prepare('SELECT attempts, last_error, published_at FROM kernel_outbox').get();
    assert.equal(row.attempts, 1);
    assert.match(row.last_error, /signing key missing/);
    assert.equal(row.published_at, null, 'a failed row must remain drainable');
  });

  it('releases the lease on failure so the row is drainable again', async () => {
    outbox.append = async () => {
      throw new Error('transient');
    };
    await pump.drain();

    const row = db.prepare('SELECT lease_owner, lease_expires_at FROM kernel_outbox').get();
    assert.equal(row.lease_owner, null);
    assert.equal(row.lease_expires_at, null);

    const second = await pump.drain();
    assert.equal(second.leased, 1, 'a failed row must be re-leasable');
  });

  it('dead-letters a row that exhausts its attempts', async () => {
    pump = createOutboxPump(db, outbox, { identity: IDENTITY, maxAttempts: 2 });
    outbox.append = async () => {
      throw new Error('permanently broken');
    };

    await pump.drain();
    assert.equal(
      db.prepare('SELECT dead_lettered_at FROM kernel_outbox').get().dead_lettered_at,
      null,
      'one failure must not dead-letter',
    );

    await pump.drain();
    assert.notEqual(
      db.prepare('SELECT dead_lettered_at FROM kernel_outbox').get().dead_lettered_at,
      null,
      'an exhausted row must not block the queue behind it forever',
    );

    const third = await pump.drain();
    assert.equal(third.leased, 0, 'a dead-lettered row is no longer drained');
  });

  it('skips event types it cannot map rather than inventing one', async () => {
    db.prepare('UPDATE kernel_outbox SET event_type = ?').run('unknown.thing');
    const result = await pump.drain();

    assert.equal(result.appended, 0);
    assert.equal(result.failed, 1);
    const row = db.prepare('SELECT last_error FROM kernel_outbox').get();
    assert.match(row.last_error, /unmapped/i);
    assert.equal(outbox.getPending(10).length, 0, 'no event may be invented for an unmapped type');
  });

  it('does not block the queue behind an unmapped row', async () => {
    db.prepare('UPDATE kernel_outbox SET event_type = ?').run('unknown.thing');
    seedKernelOutbox(db, {
      id: '66666666-6666-6666-6666-666666666666',
      aggregateId: 'ORD-2',
      createdAt: '2026-09-12T00:00:01.000Z',
    });

    const result = await pump.drain();
    assert.equal(result.failed, 1);
    assert.equal(result.appended, 1);
    assert.equal(outbox.getPending(10)[0].entityId, 'ORD-2');
  });

  // ---------------------------------------------------------------------
  // Duplicate absorption: the pump — not Outbox.append — owns the knowledge
  // that a deterministically derived event_id may already exist. append()
  // must keep failing loudly on a duplicate for every other producer.
  // ---------------------------------------------------------------------

  it('absorbs a UNIQUE violation only when its own derived event_id is present', async () => {
    // Pre-insert the envelope this row would produce, exactly as a crash
    // between append() and the published_at update would leave things.
    await outbox.append({
      tenantId: IDENTITY.tenantId,
      storeId: IDENTITY.storeId,
      entityType: 'order',
      entityId: 'ORD-1',
      eventType: 'order.created',
      payload: { total: 1 },
      sourceAgent: IDENTITY.agentId,
      eventId: expectedUuidV5(VES_OUTBOX_NAMESPACE, ROW_ID),
      createdAt: '2026-09-12T00:00:00.000Z',
    });

    const result = await pump.drain();
    assert.equal(result.duplicates, 1);
    assert.equal(result.failed, 0);
    assert.equal(result.appended, 0);
    assert.equal(outbox.getPending(10).length, 1, 'no second envelope may be written');
    assert.notEqual(db.prepare('SELECT published_at FROM kernel_outbox').get().published_at, null);
  });

  it('does not swallow a UNIQUE violation from a different row as "already published"', async () => {
    // A real SqliteError shape, but the pump's derived event_id is NOT in the
    // outbox — so this is somebody else's constraint failure, not a replay.
    outbox.append = async () => {
      const error = new Error('UNIQUE constraint failed: _ves_outbox.event_id');
      error.code = 'SQLITE_CONSTRAINT_UNIQUE';
      throw error;
    };

    const result = await pump.drain();
    assert.equal(result.duplicates, 0, 'a foreign UNIQUE failure is not a duplicate');
    assert.equal(result.failed, 1);
    const row = db.prepare('SELECT attempts, published_at, last_error FROM kernel_outbox').get();
    assert.equal(row.attempts, 1);
    assert.equal(row.published_at, null, 'a lost event is exactly what this phase prevents');
    assert.match(row.last_error, /UNIQUE constraint failed/);
  });

  it('does not treat a non-constraint error as a duplicate', async () => {
    outbox.append = async () => {
      const error = new Error('disk I/O error');
      error.code = 'SQLITE_IOERR';
      throw error;
    };

    const result = await pump.drain();
    assert.equal(result.duplicates, 0);
    assert.equal(result.failed, 1);
    assert.equal(db.prepare('SELECT published_at FROM kernel_outbox').get().published_at, null);
  });

  it('fails loudly when the stored envelope does not match the committed row', async () => {
    // Same derived event_id, different content: a genuine collision, never a
    // replay. Publishing it would leave the wrong event in the log.
    await outbox.append({
      tenantId: IDENTITY.tenantId,
      storeId: IDENTITY.storeId,
      entityType: 'order',
      entityId: 'ORD-OTHER',
      eventType: 'order.created',
      payload: { total: 999 },
      sourceAgent: IDENTITY.agentId,
      eventId: expectedUuidV5(VES_OUTBOX_NAMESPACE, ROW_ID),
      createdAt: '2026-09-12T00:00:00.000Z',
    });

    const result = await pump.drain();
    assert.equal(result.duplicates, 0);
    assert.equal(result.failed, 1);
    assert.match(db.prepare('SELECT last_error FROM kernel_outbox').get().last_error, /collision/i);
    assert.equal(db.prepare('SELECT published_at FROM kernel_outbox').get().published_at, null);
  });

  // ---------------------------------------------------------------------
  // Leasing & ordering
  // ---------------------------------------------------------------------

  it('drains in committed order and honours the limit', async () => {
    seedKernelOutbox(db, {
      id: '66666666-6666-6666-6666-666666666666',
      aggregateId: 'ORD-2',
      createdAt: '2026-09-12T00:00:01.000Z',
    });
    seedKernelOutbox(db, {
      id: '77777777-7777-7777-7777-777777777777',
      aggregateId: 'ORD-3',
      createdAt: '2026-09-12T00:00:02.000Z',
    });

    const first = await pump.drain(2);
    assert.equal(first.appended, 2);
    assert.deepEqual(
      outbox.getPending(10).map((e) => e.entityId),
      ['ORD-1', 'ORD-2'],
    );

    await pump.drain(2);
    assert.deepEqual(
      outbox.getPending(10).map((e) => e.entityId),
      ['ORD-1', 'ORD-2', 'ORD-3'],
    );
  });

  it('does not lease a row another worker holds an unexpired lease on', async () => {
    db.prepare('UPDATE kernel_outbox SET lease_owner = ?, lease_expires_at = ?').run(
      'other-worker',
      new Date(Date.now() + 60_000).toISOString(),
    );
    const result = await pump.drain();
    assert.equal(result.leased, 0);
    assert.equal(result.appended, 0);
  });

  it('reclaims a row whose lease has expired', async () => {
    db.prepare('UPDATE kernel_outbox SET lease_owner = ?, lease_expires_at = ?').run(
      'dead-worker',
      new Date(Date.now() - 60_000).toISOString(),
    );
    const result = await pump.drain();
    assert.equal(result.leased, 1);
    assert.equal(result.appended, 1);
  });

  it('drains governed rows as well as recorded ones', async () => {
    db.prepare('DELETE FROM kernel_outbox').run();
    seedKernelOutbox(db, { tier: 'governed' });
    const result = await pump.drain();
    assert.equal(result.appended, 1);
  });

  it('returns zeros when there is nothing to drain', async () => {
    db.prepare('DELETE FROM kernel_outbox').run();
    assert.deepEqual(await pump.drain(), { leased: 0, appended: 0, failed: 0, duplicates: 0 });
  });
});
