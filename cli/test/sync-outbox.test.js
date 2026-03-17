/**
 * Comprehensive tests for cli/src/sync/outbox.js — VES v1.0 SQLite Outbox
 *
 * Coverage:
 *  - Outbox / createOutbox construction and initialization
 *  - append: plaintext events (signing, hash binding, DB persistence)
 *  - append: idempotency key, baseVersion, custom eventId
 *  - append: missing signing key throws
 *  - append: encrypted path (payloadKind=1, payloadEncrypted, cipherHash)
 *  - appendBatch: atomic multi-event insert, ordering, mixed agents
 *  - appendBatch: encrypted batch path
 *  - Signature verification: Ed25519 signs correct signing hash
 *  - Hash binding: payloadPlainHash matches payload content
 *  - Hash binding: payloadCipherHash is ZERO_HASH for plaintext
 *  - getPending: ordering, limit, excludes synced/failed/rejected
 *  - getByEventId: found and not-found
 *  - getByEntityId: filters correctly
 *  - markSynced: status transition, remote_sequence, synced_at
 *  - markFailed: status + retry_count increment + last_error
 *  - markRejected: status + rejection_reason
 *  - retryFailed: resets all failed to pending
 *  - getStats: counts per status, oldestPending, lastSynced
 *  - getPendingCount: correct number
 *  - pruneOldEvents: only prunes synced; leaves others
 *  - getSyncState / updateSyncState: round-trip all fields
 *  - storePulledEvent / storePulledEvents: persists and upserts
 *  - getEntityVersion / updateEntityVersion: OCC round-trip
 *  - computePayloadHash (deprecated helper): hex output
 *  - computePayloadPlainHashBuffer: 32-byte Buffer
 *  - generateEventId: UUID v4 format
 *  - initialize: idempotent (called twice safely)
 *  - _rowToEvent: field mapping
 *  - Edge cases: empty payload {}, large payload, duplicate eventId, UUID fields
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';

import Database from 'better-sqlite3';

import { Outbox, createOutbox } from '../src/sync/outbox.js';
import {
  verifyEventSignature,
  computeEventSigningHash,
  computePayloadPlainHash,
  hexToBuffer,
  bufferToHex,
  ZERO_HASH,
} from '../src/sync/crypto.js';

// =============================================================================
// Helpers
// =============================================================================

/** Generate a raw Ed25519 key pair (32-byte seed + 32-byte public key) */
function generateEd25519Raw() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const pubKey32 = Buffer.from(publicKey.export({ type: 'spki', format: 'der' }).subarray(-32));
  const privKey32 = Buffer.from(privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32));
  return { pubKey32, privKey32 };
}

/** Generate a raw X25519 key pair (32-byte public + 32-byte private) */
function generateX25519Raw() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('x25519');
  const pubKey32 = Buffer.from(publicKey.export({ type: 'spki', format: 'der' }).subarray(-32));
  const privKey32 = Buffer.from(privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32));
  return { pubKey32, privKey32 };
}

/** Build a mock AgentKeyManager that returns deterministic in-memory keys */
function makeMockKeyManager({ signingKey = null, encryptionKey = null } = {}) {
  const sk = signingKey || generateEd25519Raw();
  const ek = encryptionKey || generateX25519Raw();

  return {
    _signingKey: sk,
    _encryptionKey: ek,
    getCurrentSigningKey: async (_agentId) => ({
      keyId: 1,
      publicKey: sk.pubKey32,
      privateKey: sk.privKey32,
      createdAt: new Date().toISOString(),
    }),
    getCurrentEncryptionKey: async (_agentId) => ({
      keyId: 1,
      publicKey: ek.pubKey32,
      privateKey: ek.privKey32,
      createdAt: new Date().toISOString(),
    }),
  };
}

/** Build a mock key manager that returns null for signing (simulates no key) */
function makeNoSigningKeyManager() {
  return {
    getCurrentSigningKey: async (_agentId) => null,
    getCurrentEncryptionKey: async (_agentId) => null,
  };
}

/** Fixed UUIDs used across tests */
const TENANT_ID = '550e8400-e29b-41d4-a716-446655440001';
const STORE_ID = '550e8400-e29b-41d4-a716-446655440002';
const AGENT_ID = '550e8400-e29b-41d4-a716-446655440003';
const AGENT_ID_2 = '550e8400-e29b-41d4-a716-446655440004';

/** Minimal valid event for append */
function makeEvent(overrides = {}) {
  return {
    tenantId: TENANT_ID,
    storeId: STORE_ID,
    entityType: 'order',
    entityId: 'ord-001',
    eventType: 'order.created',
    payload: { orderId: 'ord-001', total: 99.99 },
    sourceAgent: AGENT_ID,
    ...overrides,
  };
}

/** Create an in-memory DB + Outbox, pre-initialized */
function makeOutbox(keyManagerOverrides = {}) {
  const db = new Database(':memory:');
  const keyManager = makeMockKeyManager(keyManagerOverrides);
  const outbox = createOutbox(db, { keyManager });
  return { db, outbox, keyManager };
}

// =============================================================================
// 1. Construction & initialization
// =============================================================================

describe('Outbox construction', () => {
  it('createOutbox returns an initialized Outbox', () => {
    const db = new Database(':memory:');
    const km = makeMockKeyManager();
    const outbox = createOutbox(db, { keyManager: km });
    assert.ok(outbox instanceof Outbox);
    assert.equal(outbox._initialized, true);
  });

  it('new Outbox + initialize() is idempotent (called twice)', () => {
    const db = new Database(':memory:');
    const km = makeMockKeyManager();
    const outbox = new Outbox(db, { keyManager: km });
    outbox.initialize();
    outbox.initialize(); // second call must not throw
    assert.equal(outbox._initialized, true);
  });

  it('default configDir is ".stateset"', () => {
    const db = new Database(':memory:');
    const km = makeMockKeyManager();
    const outbox = new Outbox(db, { keyManager: km });
    assert.equal(outbox.configDir, '.stateset');
  });

  it('custom configDir is stored', () => {
    const db = new Database(':memory:');
    const km = makeMockKeyManager();
    const outbox = new Outbox(db, { configDir: '/tmp/ves', keyManager: km });
    assert.equal(outbox.configDir, '/tmp/ves');
  });

  it('schema tables are created on initialize', () => {
    const db = new Database(':memory:');
    const km = makeMockKeyManager();
    const outbox = new Outbox(db, { keyManager: km });
    outbox.initialize();

    const tables = db
      .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
      .all()
      .map((r) => r.name);

    assert.ok(tables.includes('_ves_outbox'));
    assert.ok(tables.includes('_ves_sync_state'));
    assert.ok(tables.includes('_ves_entity_versions'));
    assert.ok(tables.includes('_ves_pulled_events'));
  });
});

// =============================================================================
// 2. Helper methods
// =============================================================================

describe('generateEventId', () => {
  it('returns a UUID v4 string', () => {
    const { outbox } = makeOutbox();
    const id = outbox.generateEventId();
    assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i);
  });

  it('each call returns a unique value', () => {
    const { outbox } = makeOutbox();
    const ids = new Set(Array.from({ length: 20 }, () => outbox.generateEventId()));
    assert.equal(ids.size, 20);
  });
});

describe('computePayloadPlainHashBuffer', () => {
  it('returns a 32-byte Buffer', () => {
    const { outbox } = makeOutbox();
    const hash = outbox.computePayloadPlainHashBuffer({ foo: 'bar' });
    assert.ok(Buffer.isBuffer(hash));
    assert.equal(hash.length, 32);
  });

  it('same payload produces same hash (deterministic)', () => {
    const { outbox } = makeOutbox();
    const h1 = outbox.computePayloadPlainHashBuffer({ x: 1, y: 2 });
    const h2 = outbox.computePayloadPlainHashBuffer({ y: 2, x: 1 }); // JCS sorts keys
    assert.deepEqual(h1, h2);
  });
});

describe('computePayloadHash (deprecated)', () => {
  it('returns a 0x-prefixed hex string', () => {
    const { outbox } = makeOutbox();
    const hex = outbox.computePayloadHash({ total: 100 });
    assert.match(hex, /^0x[0-9a-f]{64}$/);
  });

  it('matches computePayloadPlainHashBuffer result', () => {
    const { outbox } = makeOutbox();
    const payload = { order: 'test', amount: 42 };
    const bufHash = outbox.computePayloadPlainHashBuffer(payload);
    const hexHash = outbox.computePayloadHash(payload);
    assert.equal(hexHash, bufferToHex(bufHash));
  });
});

// =============================================================================
// 3. append — plaintext events
// =============================================================================

describe('append (plaintext)', () => {
  let outbox;
  let keyManager;

  beforeEach(() => {
    ({ outbox, keyManager } = makeOutbox());
  });

  it('returns a positive integer sequence number', async () => {
    const seq = await outbox.append(makeEvent());
    assert.ok(typeof seq === 'number' || typeof seq === 'bigint', `expected number or bigint, got ${typeof seq}`);
    assert.ok(Number(seq) >= 1);
  });

  it('inserts exactly one pending event', async () => {
    await outbox.append(makeEvent());
    const pending = outbox.getPending();
    assert.equal(pending.length, 1);
  });

  it('persists all required fields correctly', async () => {
    const event = makeEvent({
      entityId: 'ord-xyz',
      eventType: 'order.shipped',
      commandId: 'cmd-idempotency-001',
      baseVersion: 3,
    });
    await outbox.append(event);
    const [evt] = outbox.getPending();

    assert.equal(evt.tenantId, TENANT_ID);
    assert.equal(evt.storeId, STORE_ID);
    assert.equal(evt.entityType, 'order');
    assert.equal(evt.entityId, 'ord-xyz');
    assert.equal(evt.eventType, 'order.shipped');
    assert.equal(evt.commandId, 'cmd-idempotency-001');
    assert.equal(evt.baseVersion, 3);
    assert.equal(evt.sourceAgent, AGENT_ID);
    assert.equal(evt.syncStatus, 'pending');
    assert.equal(evt.vesVersion, 1);
    assert.equal(evt.payloadKind, 0);
    assert.equal(evt.retryCount, 0);
    assert.equal(evt.remoteSequence, null);
    assert.equal(evt.syncedAt, null);
    assert.equal(evt.rejectionReason, null);
    assert.equal(evt.payloadEncrypted, null);
  });

  it('uses provided eventId when supplied', async () => {
    const fixedId = '660e8400-e29b-41d4-a716-446655440099';
    await outbox.append(makeEvent({ eventId: fixedId }));
    const [evt] = outbox.getPending();
    assert.equal(evt.eventId, fixedId);
  });

  it('generates eventId when not supplied', async () => {
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.match(evt.eventId, /^[0-9a-f-]{36}$/);
  });

  it('stores payload correctly (round-trips JSON)', async () => {
    const payload = { orderId: 'ord-1', total: 42.5, items: [{ sku: 'X', qty: 2 }] };
    await outbox.append(makeEvent({ payload }));
    const [evt] = outbox.getPending();
    assert.deepEqual(evt.payload, payload);
  });

  it('handles empty payload object {}', async () => {
    await outbox.append(makeEvent({ payload: {} }));
    const [evt] = outbox.getPending();
    assert.deepEqual(evt.payload, {});
  });

  it('handles large payload (>10 KB)', async () => {
    const large = { data: 'x'.repeat(15_000), nested: { arr: Array.from({ length: 500 }, (_, i) => i) } };
    await outbox.append(makeEvent({ payload: large }));
    const [evt] = outbox.getPending();
    assert.equal(evt.payload.data.length, 15_000);
  });

  it('stores agentKeyId matching the mock key manager', async () => {
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.equal(evt.agentKeyId, 1);
  });

  it('throws when no signing key is found', async () => {
    const db = new Database(':memory:');
    const badKm = makeNoSigningKeyManager();
    const o = createOutbox(db, { keyManager: badKm });
    await assert.rejects(() => o.append(makeEvent()), /No signing key found/);
  });

  it('null commandId when not provided', async () => {
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.equal(evt.commandId, null);
  });

  it('null baseVersion when not provided', async () => {
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.equal(evt.baseVersion, null);
  });

  it('createdAt is a Date instance', async () => {
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.ok(evt.createdAt instanceof Date);
    assert.ok(!isNaN(evt.createdAt.getTime()));
  });

  it('sequence numbers are monotonically increasing', async () => {
    const s1 = await outbox.append(makeEvent({ entityId: 'e1' }));
    const s2 = await outbox.append(makeEvent({ entityId: 'e2' }));
    const s3 = await outbox.append(makeEvent({ entityId: 'e3' }));
    assert.ok(s1 < s2);
    assert.ok(s2 < s3);
  });

  it('duplicate eventId throws (UNIQUE constraint)', async () => {
    const fixedId = '770e8400-e29b-41d4-a716-446655440099';
    await outbox.append(makeEvent({ eventId: fixedId }));
    await assert.rejects(() => outbox.append(makeEvent({ eventId: fixedId })));
  });
});

// =============================================================================
// 4. Signature verification
// =============================================================================

describe('Ed25519 signature binding', () => {
  it('appended event has a valid Ed25519 signature over the event signing hash', async () => {
    const { outbox, keyManager } = makeOutbox();
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();

    const payloadPlainHash = hexToBuffer(evt.payloadPlainHash);
    const payloadCipherHash = hexToBuffer(evt.payloadCipherHash);

    const signingHash = computeEventSigningHash({
      vesVersion: evt.vesVersion,
      tenantId: evt.tenantId,
      storeId: evt.storeId,
      eventId: evt.eventId,
      sourceAgentId: evt.sourceAgent,
      agentKeyId: evt.agentKeyId,
      entityType: evt.entityType,
      entityId: evt.entityId,
      eventType: evt.eventType,
      createdAt: evt.createdAt.toISOString(),
      payloadKind: evt.payloadKind,
      payloadPlainHash,
      payloadCipherHash,
    });

    const sig = hexToBuffer(evt.agentSignature);
    const valid = verifyEventSignature(signingHash, sig, keyManager._signingKey.pubKey32);
    assert.equal(valid, true);
  });

  it('signature verification fails with a different public key', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();

    const payloadPlainHash = hexToBuffer(evt.payloadPlainHash);
    const payloadCipherHash = hexToBuffer(evt.payloadCipherHash);
    const signingHash = computeEventSigningHash({
      vesVersion: evt.vesVersion,
      tenantId: evt.tenantId,
      storeId: evt.storeId,
      eventId: evt.eventId,
      sourceAgentId: evt.sourceAgent,
      agentKeyId: evt.agentKeyId,
      entityType: evt.entityType,
      entityId: evt.entityId,
      eventType: evt.eventType,
      createdAt: evt.createdAt.toISOString(),
      payloadKind: evt.payloadKind,
      payloadPlainHash,
      payloadCipherHash,
    });

    const sig = hexToBuffer(evt.agentSignature);
    const { pubKey32: wrongPub } = generateEd25519Raw();
    const valid = verifyEventSignature(signingHash, sig, wrongPub);
    assert.equal(valid, false);
  });

  it('signature stored as 0x-prefixed 128-char hex (64 bytes)', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.match(evt.agentSignature, /^0x[0-9a-f]{128}$/);
  });
});

// =============================================================================
// 5. Hash binding
// =============================================================================

describe('Hash binding', () => {
  it('payloadPlainHash matches VES-prefixed SHA-256 of the payload', async () => {
    const { outbox } = makeOutbox();
    const payload = { orderId: 'ord-001', total: 99.99 };
    await outbox.append(makeEvent({ payload }));
    const [evt] = outbox.getPending();

    const expectedHash = bufferToHex(computePayloadPlainHash(payload));
    assert.equal(evt.payloadPlainHash, expectedHash);
  });

  it('payloadCipherHash equals ZERO_HASH for plaintext events', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.equal(evt.payloadCipherHash, bufferToHex(ZERO_HASH));
  });

  it('payloadPlainHash is 0x-prefixed 64-char hex (32 bytes)', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent());
    const [evt] = outbox.getPending();
    assert.match(evt.payloadPlainHash, /^0x[0-9a-f]{64}$/);
  });

  it('different payloads produce different payloadPlainHash values', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent({ payload: { x: 1 }, entityId: 'e1' }));
    await outbox.append(makeEvent({ payload: { x: 2 }, entityId: 'e2' }));
    const [e1, e2] = outbox.getPending();
    assert.notEqual(e1.payloadPlainHash, e2.payloadPlainHash);
  });

  it('same payload on two events produces same payloadPlainHash', async () => {
    const { outbox } = makeOutbox();
    const payload = { shared: true };
    await outbox.append(makeEvent({ payload, entityId: 'e1' }));
    await outbox.append(makeEvent({ payload, entityId: 'e2' }));
    const [e1, e2] = outbox.getPending();
    assert.equal(e1.payloadPlainHash, e2.payloadPlainHash);
  });
});

// =============================================================================
// 6. append — encrypted events
// =============================================================================

describe('append (encrypted)', () => {
  it('throws when encrypt=true but no encryption key is available', async () => {
    const db = new Database(':memory:');
    const km = {
      getCurrentSigningKey: async () => {
        const sk = generateEd25519Raw();
        return { keyId: 1, publicKey: sk.pubKey32, privateKey: sk.privKey32, createdAt: new Date().toISOString() };
      },
      getCurrentEncryptionKey: async () => null,
    };
    const o = createOutbox(db, { keyManager: km });
    const { pubKey32 } = generateX25519Raw();
    await assert.rejects(
      () => o.append(makeEvent(), { encrypt: true, recipientPublicKey: pubKey32 }),
      /No encryption key found/,
    );
  });

  it('sets payloadKind=1 for encrypted events', async () => {
    const { outbox } = makeOutbox();
    const { pubKey32: recipientPub } = generateX25519Raw();
    await outbox.append(makeEvent(), { encrypt: true, recipientPublicKey: recipientPub });
    const [evt] = outbox.getPending();
    assert.equal(evt.payloadKind, 1);
  });

  it('stores a non-null payloadEncrypted structure for encrypted events', async () => {
    const { outbox } = makeOutbox();
    const { pubKey32: recipientPub } = generateX25519Raw();
    await outbox.append(makeEvent(), { encrypt: true, recipientPublicKey: recipientPub });
    const [evt] = outbox.getPending();
    assert.ok(evt.payloadEncrypted !== null);
    assert.equal(evt.payloadEncrypted.enc_version, 1);
    assert.equal(evt.payloadEncrypted.aead, 'AES-256-GCM');
    assert.ok(Array.isArray(evt.payloadEncrypted.recipients));
    assert.equal(evt.payloadEncrypted.recipients.length, 1);
  });

  it('stores a non-zero payloadCipherHash for encrypted events', async () => {
    const { outbox } = makeOutbox();
    const { pubKey32: recipientPub } = generateX25519Raw();
    await outbox.append(makeEvent(), { encrypt: true, recipientPublicKey: recipientPub });
    const [evt] = outbox.getPending();
    assert.notEqual(evt.payloadCipherHash, bufferToHex(ZERO_HASH));
    assert.match(evt.payloadCipherHash, /^0x[0-9a-f]{64}$/);
  });

  it('encrypted event has a valid Ed25519 signature', async () => {
    const { outbox, keyManager } = makeOutbox();
    const { pubKey32: recipientPub } = generateX25519Raw();
    await outbox.append(makeEvent(), { encrypt: true, recipientPublicKey: recipientPub });
    const [evt] = outbox.getPending();

    const payloadPlainHash = hexToBuffer(evt.payloadPlainHash);
    const payloadCipherHash = hexToBuffer(evt.payloadCipherHash);
    const signingHash = computeEventSigningHash({
      vesVersion: evt.vesVersion,
      tenantId: evt.tenantId,
      storeId: evt.storeId,
      eventId: evt.eventId,
      sourceAgentId: evt.sourceAgent,
      agentKeyId: evt.agentKeyId,
      entityType: evt.entityType,
      entityId: evt.entityId,
      eventType: evt.eventType,
      createdAt: evt.createdAt.toISOString(),
      payloadKind: evt.payloadKind,
      payloadPlainHash,
      payloadCipherHash,
    });
    const sig = hexToBuffer(evt.agentSignature);
    const valid = verifyEventSignature(signingHash, sig, keyManager._signingKey.pubKey32);
    assert.equal(valid, true);
  });
});

// =============================================================================
// 7. appendBatch
// =============================================================================

describe('appendBatch', () => {
  let outbox;

  beforeEach(() => {
    ({ outbox } = makeOutbox());
  });

  it('returns an array of sequence numbers (one per event)', async () => {
    const events = [
      makeEvent({ entityId: 'e1' }),
      makeEvent({ entityId: 'e2' }),
      makeEvent({ entityId: 'e3' }),
    ];
    const seqs = await outbox.appendBatch(events);
    assert.equal(seqs.length, 3);
    for (const s of seqs) {
      assert.ok(typeof s === 'number' || typeof s === 'bigint', `expected number or bigint, got ${typeof s}`);
      assert.ok(Number(s) >= 1);
    }
  });

  it('inserts all events as pending', async () => {
    const events = Array.from({ length: 5 }, (_, i) =>
      makeEvent({ entityId: `e${i}`, eventType: `order.event${i}` }),
    );
    await outbox.appendBatch(events);
    assert.equal(outbox.getPendingCount(), 5);
  });

  it('preserves insertion order (seq numbers are ascending)', async () => {
    const events = [
      makeEvent({ entityId: 'e1' }),
      makeEvent({ entityId: 'e2' }),
      makeEvent({ entityId: 'e3' }),
    ];
    const seqs = await outbox.appendBatch(events);
    assert.ok(seqs[0] < seqs[1]);
    assert.ok(seqs[1] < seqs[2]);
  });

  it('supports events from multiple agents in one batch', async () => {
    const sk2 = generateEd25519Raw();
    const km = {
      getCurrentSigningKey: async (agentId) => {
        const key = agentId === AGENT_ID
          ? generateEd25519Raw()
          : sk2;
        return { keyId: 1, publicKey: key.pubKey32, privateKey: key.privKey32, createdAt: new Date().toISOString() };
      },
      getCurrentEncryptionKey: async () => null,
    };
    const db = new Database(':memory:');
    const o = createOutbox(db, { keyManager: km });

    const events = [
      makeEvent({ sourceAgent: AGENT_ID, entityId: 'e1' }),
      makeEvent({ sourceAgent: AGENT_ID_2, entityId: 'e2' }),
    ];
    const seqs = await o.appendBatch(events);
    assert.equal(seqs.length, 2);
    assert.equal(o.getPendingCount(), 2);
  });

  it('throws when an agent has no signing key', async () => {
    const db = new Database(':memory:');
    const o = createOutbox(db, { keyManager: makeNoSigningKeyManager() });
    await assert.rejects(() => o.appendBatch([makeEvent()]), /No signing key found/);
  });

  it('empty batch returns empty array', async () => {
    const seqs = await outbox.appendBatch([]);
    assert.deepEqual(seqs, []);
    assert.equal(outbox.getPendingCount(), 0);
  });

  it('each event in batch gets a distinct eventId', async () => {
    const events = Array.from({ length: 4 }, (_, i) => makeEvent({ entityId: `e${i}` }));
    await outbox.appendBatch(events);
    const pending = outbox.getPending();
    const ids = new Set(pending.map((e) => e.eventId));
    assert.equal(ids.size, 4);
  });

  it('batch is atomic — failure of one rolls back the transaction', async () => {
    // Force an error mid-batch by supplying an event that will hit the JSON
    // CHECK constraint (payload must be valid JSON — but we can hit the UNIQUE
    // constraint by providing the same eventId twice).
    const fixedId = 'aa0e8400-e29b-41d4-a716-446655440099';
    await outbox.append(makeEvent({ eventId: fixedId }));

    const events = [
      makeEvent({ entityId: 'ok1' }),
      makeEvent({ entityId: 'dup', eventId: fixedId }), // will violate UNIQUE
    ];
    await assert.rejects(() => outbox.appendBatch(events));
    // Only the pre-existing event should remain
    assert.equal(outbox.getPendingCount(), 1);
  });

  it('encrypted batch path sets payloadKind=1 and stores payloadEncrypted for all events', async () => {
    const { pubKey32: recipientPub } = generateX25519Raw();
    const events = [
      makeEvent({ entityId: 'eb1' }),
      makeEvent({ entityId: 'eb2' }),
    ];
    await outbox.appendBatch(events, { encrypt: true, recipientPublicKey: recipientPub });
    const pending = outbox.getPending();
    assert.equal(pending.length, 2);
    for (const evt of pending) {
      assert.equal(evt.payloadKind, 1);
      assert.ok(evt.payloadEncrypted !== null);
      assert.equal(evt.payloadEncrypted.enc_version, 1);
      assert.notEqual(evt.payloadCipherHash, bufferToHex(ZERO_HASH));
    }
  });
});

// =============================================================================
// 8. getPending
// =============================================================================

describe('getPending', () => {
  let outbox;

  beforeEach(() => {
    ({ outbox } = makeOutbox());
  });

  it('returns empty array when outbox is empty', () => {
    assert.deepEqual(outbox.getPending(), []);
  });

  it('default limit is 100', async () => {
    // Insert 105 events
    for (let i = 0; i < 105; i++) {
      await outbox.append(makeEvent({ entityId: `e${i}` }));
    }
    const pending = outbox.getPending();
    assert.equal(pending.length, 100);
  });

  it('respects explicit limit', async () => {
    for (let i = 0; i < 10; i++) {
      await outbox.append(makeEvent({ entityId: `e${i}` }));
    }
    const pending = outbox.getPending(5);
    assert.equal(pending.length, 5);
  });

  it('returns events in ascending local_seq order', async () => {
    for (let i = 0; i < 5; i++) {
      await outbox.append(makeEvent({ entityId: `e${i}` }));
    }
    const pending = outbox.getPending();
    for (let i = 1; i < pending.length; i++) {
      assert.ok(pending[i].localSeq > pending[i - 1].localSeq);
    }
  });

  it('excludes synced events', async () => {
    const seq = await outbox.append(makeEvent());
    outbox.markSynced([{ localSeq: Number(seq), remoteSeq: 42 }]);
    assert.equal(outbox.getPending().length, 0);
  });

  it('excludes failed events', async () => {
    const seq = await outbox.append(makeEvent());
    outbox.markFailed(Number(seq), 'timeout');
    assert.equal(outbox.getPending().length, 0);
  });

  it('excludes rejected events', async () => {
    const seq = await outbox.append(makeEvent());
    outbox.markRejected(Number(seq), 'invalid payload');
    assert.equal(outbox.getPending().length, 0);
  });
});

// =============================================================================
// 9. getByEventId
// =============================================================================

describe('getByEventId', () => {
  it('returns the event when found', async () => {
    const { outbox } = makeOutbox();
    const fixedId = 'bb0e8400-e29b-41d4-a716-446655440099';
    await outbox.append(makeEvent({ eventId: fixedId }));
    const evt = outbox.getByEventId(fixedId);
    assert.ok(evt !== null);
    assert.equal(evt.eventId, fixedId);
  });

  it('returns null when event ID does not exist', () => {
    const { outbox } = makeOutbox();
    const result = outbox.getByEventId('00000000-0000-0000-0000-000000000000');
    assert.equal(result, null);
  });

  it('returned event has the correct payload', async () => {
    const { outbox } = makeOutbox();
    const fixedId = 'cc0e8400-e29b-41d4-a716-446655440099';
    const payload = { special: 'value', count: 99 };
    await outbox.append(makeEvent({ eventId: fixedId, payload }));
    const evt = outbox.getByEventId(fixedId);
    assert.deepEqual(evt.payload, payload);
  });
});

// =============================================================================
// 10. getByEntityId
// =============================================================================

describe('getByEntityId', () => {
  it('returns events matching entityType + entityId', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent({ entityType: 'order', entityId: 'ord-100', eventType: 'order.created' }));
    await outbox.append(makeEvent({ entityType: 'order', entityId: 'ord-100', eventType: 'order.shipped' }));
    await outbox.append(makeEvent({ entityType: 'customer', entityId: 'cust-1' }));

    const results = outbox.getByEntityId('order', 'ord-100');
    assert.equal(results.length, 2);
    assert.ok(results.every((e) => e.entityType === 'order' && e.entityId === 'ord-100'));
  });

  it('returns events in ascending local_seq order', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent({ entityId: 'ord-200', eventType: 'order.created' }));
    await outbox.append(makeEvent({ entityId: 'ord-200', eventType: 'order.shipped' }));
    const results = outbox.getByEntityId('order', 'ord-200');
    assert.ok(results[0].localSeq < results[1].localSeq);
  });

  it('returns empty array when no events match', () => {
    const { outbox } = makeOutbox();
    assert.deepEqual(outbox.getByEntityId('order', 'nonexistent'), []);
  });
});

// =============================================================================
// 11. markSynced
// =============================================================================

describe('markSynced', () => {
  it('transitions event to synced status', async () => {
    const { outbox } = makeOutbox();
    const seq = await outbox.append(makeEvent());
    outbox.markSynced([{ localSeq: Number(seq), remoteSeq: 7 }]);
    assert.equal(outbox.getStats().synced, 1);
    assert.equal(outbox.getPendingCount(), 0);
  });

  it('stores the remote_sequence', async () => {
    const { outbox } = makeOutbox();
    const seq = await outbox.append(makeEvent());
    const localSeq = Number(seq);
    outbox.markSynced([{ localSeq, remoteSeq: 999 }]);
    const row = outbox.db.prepare('SELECT remote_sequence FROM _ves_outbox WHERE local_seq = ?').get(localSeq);
    assert.equal(row.remote_sequence, 999);
  });

  it('sets synced_at timestamp', async () => {
    const { outbox } = makeOutbox();
    const seq = await outbox.append(makeEvent());
    const localSeq = Number(seq);
    outbox.markSynced([{ localSeq, remoteSeq: 1 }]);
    const row = outbox.db.prepare('SELECT synced_at FROM _ves_outbox WHERE local_seq = ?').get(localSeq);
    assert.ok(row.synced_at != null);
  });

  it('handles empty acks array without error', () => {
    const { outbox } = makeOutbox();
    assert.doesNotThrow(() => outbox.markSynced([]));
  });

  it('syncs multiple events in one call', async () => {
    const { outbox } = makeOutbox();
    const s1 = Number(await outbox.append(makeEvent({ entityId: 'e1' })));
    const s2 = Number(await outbox.append(makeEvent({ entityId: 'e2' })));
    outbox.markSynced([{ localSeq: s1, remoteSeq: 10 }, { localSeq: s2, remoteSeq: 11 }]);
    assert.equal(outbox.getStats().synced, 2);
    assert.equal(outbox.getPendingCount(), 0);
  });
});

// =============================================================================
// 12. markFailed
// =============================================================================

describe('markFailed', () => {
  it('transitions event to failed status', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markFailed(seq, 'network error');
    assert.equal(outbox.getStats().failed, 1);
  });

  it('increments retry_count each call', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markFailed(seq, 'err1');
    outbox.markFailed(seq, 'err2');
    const row = outbox.db.prepare('SELECT retry_count FROM _ves_outbox WHERE local_seq = ?').get(seq);
    assert.equal(row.retry_count, 2);
  });

  it('stores last_error message', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markFailed(seq, 'upstream unavailable');
    const row = outbox.db.prepare('SELECT last_error FROM _ves_outbox WHERE local_seq = ?').get(seq);
    assert.equal(row.last_error, 'upstream unavailable');
  });
});

// =============================================================================
// 13. markRejected
// =============================================================================

describe('markRejected', () => {
  it('transitions event to rejected status', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markRejected(seq, 'duplicate event');
    assert.equal(outbox.getStats().rejected, 1);
  });

  it('stores rejection_reason', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markRejected(seq, 'out of order sequence');
    const row = outbox.db
      .prepare('SELECT rejection_reason FROM _ves_outbox WHERE local_seq = ?')
      .get(seq);
    assert.equal(row.rejection_reason, 'out of order sequence');
  });
});

// =============================================================================
// 14. retryFailed
// =============================================================================

describe('retryFailed', () => {
  it('returns the number of events reset to pending', async () => {
    const { outbox } = makeOutbox();
    const s1 = Number(await outbox.append(makeEvent({ entityId: 'e1' })));
    const s2 = Number(await outbox.append(makeEvent({ entityId: 'e2' })));
    outbox.markFailed(s1, 'err');
    outbox.markFailed(s2, 'err');
    const count = outbox.retryFailed();
    assert.equal(count, 2);
  });

  it('resets failed events back to pending', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markFailed(seq, 'timeout');
    assert.equal(outbox.getPendingCount(), 0);
    outbox.retryFailed();
    assert.equal(outbox.getPendingCount(), 1);
  });

  it('does not reset rejected events', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markRejected(seq, 'invalid');
    outbox.retryFailed();
    assert.equal(outbox.getStats().rejected, 1);
    assert.equal(outbox.getPendingCount(), 0);
  });

  it('returns 0 when nothing is failed', () => {
    const { outbox } = makeOutbox();
    assert.equal(outbox.retryFailed(), 0);
  });
});

// =============================================================================
// 15. getStats
// =============================================================================

describe('getStats', () => {
  it('returns zero counts on empty outbox', () => {
    const { outbox } = makeOutbox();
    const stats = outbox.getStats();
    assert.equal(stats.total, 0);
    // SQLite SUM() returns NULL for empty sets; the outbox returns that as-is
    assert.ok(stats.pending == null || stats.pending === 0);
    assert.ok(stats.synced == null || stats.synced === 0);
    assert.ok(stats.failed == null || stats.failed === 0);
    assert.ok(stats.rejected == null || stats.rejected === 0);
    assert.equal(stats.oldestPending, null);
    assert.equal(stats.lastSynced, null);
  });

  it('counts each status bucket correctly', async () => {
    const { outbox } = makeOutbox();
    const s1 = Number(await outbox.append(makeEvent({ entityId: 'e1' })));
    const s2 = Number(await outbox.append(makeEvent({ entityId: 'e2' })));
    const s3 = Number(await outbox.append(makeEvent({ entityId: 'e3' })));
    const s4 = Number(await outbox.append(makeEvent({ entityId: 'e4' })));

    outbox.markSynced([{ localSeq: s1, remoteSeq: 1 }]);
    outbox.markFailed(s2, 'err');
    outbox.markRejected(s3, 'bad');

    const stats = outbox.getStats();
    assert.equal(stats.total, 4);
    assert.equal(stats.pending, 1);
    assert.equal(stats.synced, 1);
    assert.equal(stats.failed, 1);
    assert.equal(stats.rejected, 1);
  });

  it('oldestPending is a Date when pending events exist', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent());
    const stats = outbox.getStats();
    assert.ok(stats.oldestPending instanceof Date);
  });

  it('lastSynced is a Date when synced events exist', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markSynced([{ localSeq: seq, remoteSeq: 5 }]);
    const stats = outbox.getStats();
    assert.ok(stats.lastSynced instanceof Date);
  });
});

// =============================================================================
// 16. getPendingCount
// =============================================================================

describe('getPendingCount', () => {
  it('returns 0 for empty outbox', () => {
    const { outbox } = makeOutbox();
    assert.equal(outbox.getPendingCount(), 0);
  });

  it('returns correct count after appends', async () => {
    const { outbox } = makeOutbox();
    for (let i = 0; i < 7; i++) {
      await outbox.append(makeEvent({ entityId: `e${i}` }));
    }
    assert.equal(outbox.getPendingCount(), 7);
  });

  it('decrements after markSynced', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markSynced([{ localSeq: seq, remoteSeq: 1 }]);
    assert.equal(outbox.getPendingCount(), 0);
  });
});

// =============================================================================
// 17. pruneOldEvents
// =============================================================================

describe('pruneOldEvents', () => {
  it('returns 0 when no synced events match the age threshold', async () => {
    const { outbox } = makeOutbox();
    const seq = Number(await outbox.append(makeEvent()));
    outbox.markSynced([{ localSeq: seq, remoteSeq: 1 }]);
    // Synced just now — 30 days threshold should not prune it
    const pruned = outbox.pruneOldEvents(30);
    assert.equal(pruned, 0);
  });

  it('does not prune pending events regardless of age threshold', async () => {
    const { outbox } = makeOutbox();
    await outbox.append(makeEvent());
    const pruned = outbox.pruneOldEvents(0);
    assert.equal(pruned, 0);
    assert.equal(outbox.getPendingCount(), 1);
  });

  it('does not prune failed or rejected events', async () => {
    const { outbox } = makeOutbox();
    const s1 = Number(await outbox.append(makeEvent({ entityId: 'e1' })));
    const s2 = Number(await outbox.append(makeEvent({ entityId: 'e2' })));
    outbox.markFailed(s1, 'err');
    outbox.markRejected(s2, 'bad');
    const pruned = outbox.pruneOldEvents(0);
    assert.equal(pruned, 0);
  });
});

// =============================================================================
// 18. getSyncState / updateSyncState
// =============================================================================

describe('getSyncState / updateSyncState', () => {
  it('getSyncState returns default numeric zeros on empty store', () => {
    const { outbox } = makeOutbox();
    const state = outbox.getSyncState();
    assert.equal(state.lastPushedSequence, 0);
    assert.equal(state.lastPulledSequence, 0);
    assert.equal(state.headSequence, 0);
  });

  it('updateSyncState persists lastPushedSequence', () => {
    const { outbox } = makeOutbox();
    outbox.updateSyncState({ lastPushedSequence: 42 });
    const state = outbox.getSyncState();
    assert.equal(state.lastPushedSequence, 42);
  });

  it('updateSyncState persists lastPulledSequence', () => {
    const { outbox } = makeOutbox();
    outbox.updateSyncState({ lastPulledSequence: 17 });
    assert.equal(outbox.getSyncState().lastPulledSequence, 17);
  });

  it('updateSyncState persists headSequence', () => {
    const { outbox } = makeOutbox();
    outbox.updateSyncState({ headSequence: 100 });
    assert.equal(outbox.getSyncState().headSequence, 100);
  });

  it('updateSyncState persists tenantId and storeId', () => {
    const { outbox } = makeOutbox();
    outbox.updateSyncState({ tenantId: TENANT_ID, storeId: STORE_ID });
    const state = outbox.getSyncState();
    assert.equal(state.tenantId, TENANT_ID);
    assert.equal(state.storeId, STORE_ID);
  });

  it('updateSyncState persists agentId', () => {
    const { outbox } = makeOutbox();
    outbox.updateSyncState({ agentId: AGENT_ID });
    assert.equal(outbox.getSyncState().agentId, AGENT_ID);
  });

  it('updateSyncState persists lastSyncAt', () => {
    const { outbox } = makeOutbox();
    const ts = new Date('2026-01-15T12:00:00Z');
    outbox.updateSyncState({ lastSyncAt: ts });
    const state = outbox.getSyncState();
    assert.ok(state.lastSyncAt instanceof Date);
    assert.equal(state.lastSyncAt.toISOString(), ts.toISOString());
  });

  it('multiple updates upsert correctly (no duplicate key error)', () => {
    const { outbox } = makeOutbox();
    outbox.updateSyncState({ lastPushedSequence: 1 });
    outbox.updateSyncState({ lastPushedSequence: 2 });
    outbox.updateSyncState({ lastPushedSequence: 3 });
    assert.equal(outbox.getSyncState().lastPushedSequence, 3);
  });

  it('partial update does not clobber other fields', () => {
    const { outbox } = makeOutbox();
    outbox.updateSyncState({ lastPushedSequence: 10, lastPulledSequence: 5 });
    outbox.updateSyncState({ headSequence: 50 });
    const state = outbox.getSyncState();
    assert.equal(state.lastPushedSequence, 10);
    assert.equal(state.lastPulledSequence, 5);
    assert.equal(state.headSequence, 50);
  });
});

// =============================================================================
// 19. storePulledEvent / storePulledEvents
// =============================================================================

describe('storePulledEvent', () => {
  function makePulledEvent(overrides = {}) {
    return {
      sequenceNumber: 1,
      eventId: crypto.randomUUID(),
      commandId: null,
      tenantId: TENANT_ID,
      storeId: STORE_ID,
      entityType: 'order',
      entityId: 'ord-remote-1',
      eventType: 'order.created',
      vesVersion: 1,
      payload: { orderId: 'ord-remote-1' },
      payloadKind: 0,
      payloadEncrypted: null,
      payloadPlainHash: bufferToHex(computePayloadPlainHash({ orderId: 'ord-remote-1' })),
      payloadCipherHash: bufferToHex(ZERO_HASH),
      agentKeyId: 1,
      agentSignature: '0x' + '0a'.repeat(64),
      baseVersion: null,
      createdAt: new Date().toISOString(),
      sequencedAt: new Date().toISOString(),
      sourceAgent: AGENT_ID,
      ...overrides,
    };
  }

  it('stores a pulled event without throwing', () => {
    const { outbox } = makeOutbox();
    assert.doesNotThrow(() => outbox.storePulledEvent(makePulledEvent()));
  });

  it('can upsert (INSERT OR REPLACE) the same sequence_number', () => {
    const { outbox } = makeOutbox();
    const evt = makePulledEvent({ sequenceNumber: 5 });
    outbox.storePulledEvent(evt);
    outbox.storePulledEvent({ ...evt, entityId: 'updated-id' });
    const row = outbox.db
      .prepare('SELECT entity_id FROM _ves_pulled_events WHERE sequence_number = 5')
      .get();
    assert.equal(row.entity_id, 'updated-id');
  });

  it('stores multiple pulled events via storePulledEvents', () => {
    const { outbox } = makeOutbox();
    const events = [
      makePulledEvent({ sequenceNumber: 10, eventId: crypto.randomUUID() }),
      makePulledEvent({ sequenceNumber: 11, eventId: crypto.randomUUID() }),
      makePulledEvent({ sequenceNumber: 12, eventId: crypto.randomUUID() }),
    ];
    outbox.storePulledEvents(events);
    const count = outbox.db
      .prepare('SELECT COUNT(*) as n FROM _ves_pulled_events')
      .get();
    assert.equal(count.n, 3);
  });

  it('storePulledEvents with INSERT OR REPLACE replaces duplicate event_id rows', () => {
    // INSERT OR REPLACE silently replaces on UNIQUE conflicts rather than throwing.
    // This test documents the actual behavior: the last row with a given event_id wins.
    const { outbox } = makeOutbox();
    const dupId = crypto.randomUUID();
    const events = [
      makePulledEvent({ sequenceNumber: 21, eventId: dupId, entityId: 'first' }),
      makePulledEvent({ sequenceNumber: 22, eventId: dupId, entityId: 'second' }), // replaces first
    ];
    // Should not throw — INSERT OR REPLACE handles the conflict
    assert.doesNotThrow(() => outbox.storePulledEvents(events));
    const row = outbox.db
      .prepare('SELECT entity_id FROM _ves_pulled_events WHERE event_id = ?')
      .get(dupId);
    // The second insert replaces the first (sequence_number 22 wins)
    assert.equal(row.entity_id, 'second');
  });
});

// =============================================================================
// 20. getEntityVersion / updateEntityVersion
// =============================================================================

describe('getEntityVersion / updateEntityVersion', () => {
  it('returns null when entity has no version', () => {
    const { outbox } = makeOutbox();
    const v = outbox.getEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-new');
    assert.equal(v, null);
  });

  it('stores and retrieves a version', () => {
    const { outbox } = makeOutbox();
    outbox.updateEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-v1', 5);
    assert.equal(outbox.getEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-v1'), 5);
  });

  it('upserts — subsequent update overwrites the version', () => {
    const { outbox } = makeOutbox();
    outbox.updateEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-v2', 1);
    outbox.updateEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-v2', 2);
    outbox.updateEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-v2', 3);
    assert.equal(outbox.getEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-v2'), 3);
  });

  it('different entity IDs are independent', () => {
    const { outbox } = makeOutbox();
    outbox.updateEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-a', 10);
    outbox.updateEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-b', 20);
    assert.equal(outbox.getEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-a'), 10);
    assert.equal(outbox.getEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-b'), 20);
  });

  it('different tenants are independent', () => {
    const { outbox } = makeOutbox();
    const t2 = '550e8400-e29b-41d4-a716-446655440099';
    outbox.updateEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-1', 1);
    outbox.updateEntityVersion(t2, STORE_ID, 'order', 'ord-1', 99);
    assert.equal(outbox.getEntityVersion(TENANT_ID, STORE_ID, 'order', 'ord-1'), 1);
    assert.equal(outbox.getEntityVersion(t2, STORE_ID, 'order', 'ord-1'), 99);
  });
});
