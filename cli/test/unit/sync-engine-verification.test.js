/**
 * The pull path must verify every event against its author's key and
 * quarantine what it cannot verify.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { SyncConfig } from '../../src/sync/config.js';

const TENANT = '22222222-2222-2222-2222-222222222222';
const STORE = '33333333-3333-3333-3333-333333333333';
const AGENT = '44444444-4444-4444-4444-444444444444';
const SELF = '55555555-5555-5555-5555-555555555555';

function pulledEvent(overrides = {}) {
  return {
    sequenceNumber: 1,
    sequencedAt: '2026-09-12T00:00:01.000Z',
    envelope: {
      eventId: '11111111-1111-1111-1111-111111111111',
      tenantId: TENANT,
      storeId: STORE,
      entityType: 'order',
      entityId: 'ORD-1',
      eventType: 'order.created',
      vesVersion: 1,
      payload: { total: 99.99 },
      payloadKind: 0,
      payloadPlainHash: '0x00',
      payloadCipherHash: '0x00',
      agentKeyId: 1,
      agentSignature: '0xdead',
      baseVersion: 0,
      createdAt: '2026-09-12T00:00:00.000Z',
      sourceAgent: AGENT,
      ...overrides,
    },
  };
}

/**
 * The SyncEngine constructor eagerly builds its own outbox, client, resolver
 * and key directory from `options.db` / `options.config`, so the fakes are
 * installed after construction. The db is real: the point of these tests is
 * what does and does not reach `_ves_pulled_events`.
 */
function buildEngine(
  db,
  { verifies = true, resolves = true, events = [pulledEvent()], nextSequence = 2 } = {},
) {
  const config = new SyncConfig({
    sequencer: { url: 'https://sequencer.invalid' },
    identity: { tenantId: TENANT, storeId: STORE, agentId: SELF },
  });
  const engine = new SyncEngine({ db, config });

  const outbox = createOutbox(db, {});
  engine.outbox = outbox;
  engine.resolver = { detectConflicts: () => [] };

  const verifyCalls = [];
  engine.client = {
    async pull() {
      return { events, nextSequence, headSequence: nextSequence - 1 };
    },
    verifyEventSignature: (envelope, publicKey) => {
      verifyCalls.push({ envelope, publicKey });
      return typeof verifies === 'function' ? verifies(envelope) : verifies;
    },
  };
  engine.keyDirectory = {
    async resolve(agentId, keyId, atTime) {
      if (typeof resolves === 'function') return resolves(agentId, keyId, atTime);
      return resolves ? { publicKey: '0xaa', publicKeyBundle: null } : { error: 'key_unresolved' };
    },
  };

  return { engine, outbox, verifyCalls };
}

describe('pull verification', () => {
  let db;

  beforeEach(() => {
    db = new Database(':memory:');
  });

  afterEach(() => db.close());

  it('stores an event whose signature verifies', async () => {
    const { engine, outbox } = buildEngine(db);
    const result = await engine.pull();

    assert.equal(outbox.getPulledEvents().length, 1);
    assert.equal(outbox.getQuarantinedEvents().length, 0);
    assert.equal(result.verified, 1);
    assert.equal(result.quarantined, 0);
    assert.equal(result.stored, 1);
  });

  it('quarantines an event whose signature does not verify', async () => {
    const { engine, outbox } = buildEngine(db, { verifies: false });
    const result = await engine.pull();

    assert.equal(
      outbox.getPulledEvents().length,
      0,
      'a forged event must never reach application reads',
    );
    const quarantined = outbox.getQuarantinedEvents();
    assert.equal(quarantined.length, 1);
    assert.equal(quarantined[0].reason, 'signature_invalid');
    assert.equal(result.quarantined, 1);
  });

  it('quarantines an event whose key cannot be resolved', async () => {
    const { engine, outbox } = buildEngine(db, { resolves: false });
    await engine.pull();

    assert.equal(outbox.getQuarantinedEvents()[0].reason, 'key_unresolved');
  });

  it('advances the cursor even when every event quarantines', async () => {
    const { engine, outbox } = buildEngine(db, { verifies: false });
    await engine.pull();

    assert.equal(
      outbox.getSyncState().lastPulledSequence,
      2,
      'one bad peer must not wedge this agent’s sync',
    );
  });

  it('never reports an applied count', async () => {
    const { engine } = buildEngine(db);
    const result = await engine.pull();

    assert.equal(
      'applied' in result,
      false,
      'nothing is applied to local state until Phase D; a field claiming otherwise is the bug',
    );
  });

  it('reports a real conflict count instead of a literal zero', async () => {
    const { engine } = buildEngine(db);
    engine.resolver = { detectConflicts: () => [{ eventId: 'a' }, { eventId: 'b' }] };

    const result = await engine.pull();
    assert.equal(result.conflicts, 2);
  });

  it('verifies self-authored echoes too', async () => {
    const { engine, verifyCalls } = buildEngine(db, {
      events: [pulledEvent({ sourceAgent: SELF })],
    });
    await engine.pull();

    assert.equal(verifyCalls.length, 1, 'our own events are a free check on our signing path');
    assert.equal(verifyCalls[0].envelope.sourceAgent, SELF);
  });

  it('verifies against the resolved key bundle when the peer has one', async () => {
    const bundle = { ed25519PublicKey: '0xaa', mlDsa65PublicKey: '0xbb' };
    const { engine, verifyCalls } = buildEngine(db, {
      resolves: () => ({ publicKey: '0xaa', publicKeyBundle: bundle }),
    });
    await engine.pull();

    assert.deepEqual(verifyCalls[0].publicKey, bundle);
  });

  it('groups a mixed batch by reason and stores only what verified', async () => {
    // keyId 1 resolves and verifies, 2 is revoked, 3 is unknown, 4 resolves but
    // the signature is bad — four outcomes, three distinct quarantine reasons.
    const events = [
      { ...pulledEvent({ eventId: '11111111-1111-1111-1111-111111111111', agentKeyId: 1 }) },
      { ...pulledEvent({ eventId: '22222222-1111-1111-1111-111111111111', agentKeyId: 2 }) },
      { ...pulledEvent({ eventId: '33333333-1111-1111-1111-111111111111', agentKeyId: 3 }) },
      { ...pulledEvent({ eventId: '44444444-1111-1111-1111-111111111111', agentKeyId: 4 }) },
    ];
    events.forEach((event, index) => {
      event.sequenceNumber = index + 1;
    });

    const { engine, outbox } = buildEngine(db, {
      events,
      nextSequence: 5,
      resolves: (_agentId, keyId) => {
        if (keyId === 2) return { error: 'key_revoked' };
        if (keyId === 3) return { error: 'key_unresolved' };
        return { publicKey: '0xaa', publicKeyBundle: null };
      },
      verifies: (envelope) => envelope.agentKeyId === 1,
    });

    const emitted = [];
    engine.on('receive-verification-failed', (payload) => emitted.push(payload));

    const result = await engine.pull();

    assert.equal(result.pulled, 4);
    assert.equal(result.verified, 1);
    assert.equal(result.stored, 1);
    assert.equal(result.quarantined, 3);
    assert.equal(outbox.getPulledEvents().length, 1);
    assert.deepEqual(
      outbox.getQuarantinedEvents().map((event) => event.reason),
      ['key_revoked', 'key_unresolved', 'signature_invalid'],
    );
    assert.deepEqual(emitted.map((payload) => `${payload.reason}:${payload.count}`).sort(), [
      'key_revoked:1',
      'key_unresolved:1',
      'signature_invalid:1',
    ]);
  });

  it('only reports sequence numbers for events that were actually stored', async () => {
    const good = pulledEvent();
    const bad = { ...pulledEvent({ eventId: '99999999-1111-1111-1111-111111111111' }) };
    bad.sequenceNumber = 2;

    const { engine } = buildEngine(db, {
      events: [good, bad],
      nextSequence: 3,
      verifies: (envelope) => envelope.eventId.startsWith('1111'),
    });

    const result = await engine.pull({ includeEvents: true });
    assert.deepEqual(result.sequenceNumbers, [1]);
  });

  it('emits receive-verification-failed once per reason', async () => {
    const { engine } = buildEngine(db, { verifies: false });
    const emitted = [];
    engine.on('receive-verification-failed', (payload) => emitted.push(payload));

    await engine.pull();

    assert.equal(emitted.length, 1);
    assert.equal(emitted[0].reason, 'signature_invalid');
    assert.equal(emitted[0].count, 1);
  });

  it('re-quarantining the same event records the latest reason', async () => {
    // First pull: the directory is unreachable, so the key cannot be resolved.
    const first = buildEngine(db, { resolves: false });
    await first.engine.pull();
    assert.equal(first.outbox.getQuarantinedEvents()[0].reason, 'key_unresolved');

    // Second pull of the same sequence: the key now resolves and the signature
    // is provably bad. The operator must see the forgery, not the stale outage.
    const second = buildEngine(db, { verifies: false });
    await second.engine.pull({ fromSequence: 0 });

    const quarantined = second.outbox.getQuarantinedEvents();
    assert.equal(quarantined.length, 1);
    assert.equal(quarantined[0].reason, 'signature_invalid');
  });

  it('reports honest zeroes when the sequencer returns no events', async () => {
    const { engine } = buildEngine(db, { events: [], nextSequence: 1 });
    const result = await engine.pull();

    assert.equal(result.success, true);
    assert.equal(result.pulled, 0);
    assert.equal(result.verified, 0);
    assert.equal(result.quarantined, 0);
    assert.equal(result.stored, 0);
    assert.equal(result.conflicts, 0);
    assert.equal('applied' in result, false);
  });

  it('reports honest zeroes and no applied count when the pull fails', async () => {
    const { engine } = buildEngine(db);
    engine.client.pull = async () => {
      throw new Error('sequencer unreachable');
    };
    engine.on('error', () => {});

    const result = await engine.pull();

    assert.equal(result.success, false);
    assert.equal('applied' in result, false);
    assert.equal(result.pulled, 0);
    assert.equal(result.verified, 0);
    assert.equal(result.quarantined, 0);
    assert.equal(result.stored, 0);
    assert.equal(result.error, 'sequencer unreachable');
  });

  it('stores nothing on a dry run and reports no applied count', async () => {
    const { engine, outbox } = buildEngine(db);
    const result = await engine.pull({ dryRun: true });

    assert.equal(outbox.getPulledEvents().length, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 0);
    assert.equal(result.pulled, 1);
    assert.equal(result.stored, 0);
    assert.equal('applied' in result, false);
  });
});
