/**
 * The pull path must verify every event against its author's key and
 * quarantine what it cannot verify.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import crypto from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { createOutbox } from '../../src/sync/outbox.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { SyncConfig } from '../../src/sync/config.js';
import { SequencerClient } from '../../src/sync/client.js';
import {
  canonicalizeJson,
  computeEventSigningHash,
  computePayloadPlainHash,
  hexToBuffer,
} from '../../src/sync/crypto.js';

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
  {
    verifies = true,
    resolves = true,
    events = [pulledEvent()],
    nextSequence = 2,
    securityProfile = 'legacy',
  } = {},
) {
  const config = new SyncConfig({
    sequencer: { url: 'https://sequencer.invalid' },
    sync: { securityProfile },
    identity: { tenantId: TENANT, storeId: STORE, agentId: SELF },
  });
  const engine = new SyncEngine({ db, config });

  const outbox = createOutbox(db, {});
  engine.outbox = outbox;
  engine.resolver = { detectConflicts: () => [] };

  const verifyCalls = [];
  engine.client = {
    async pull(fromSequence) {
      return {
        events,
        nextSequence: events.length ? nextSequence : fromSequence,
        headSequence: nextSequence - 1,
      };
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

describe('receive health reflects durable outcomes', () => {
  let db;
  beforeEach(() => {
    db = new Database(':memory:');
  });
  afterEach(() => db.close());

  function enableStatus(engine, head) {
    engine.client.isConnected = () => true;
    engine.client.getHead = async () => ({ headSequence: head });
    engine.resolver.getConflictCount = () => 0;
  }

  it('reports the observed head separately from the next request cursor', async () => {
    const { engine } = buildEngine(db);
    enableStatus(engine, 1);
    await engine.pull();
    const status = await engine.getStatus();
    assert.equal(status.localHead, 1);
    assert.equal(status.nextPullCursor, 2);
    assert.equal(status.lag, 0);
    assert.equal(status.receive.verified, 1);
    assert.equal(status.receive.verifiedHead, 1);
    assert.ok(status.lastPull instanceof Date);
    assert.equal(status.health, 'healthy');
    assert.equal((await engine.getHealth()).healthy, true);
  });

  it('does not call a caught-up but quarantined receive healthy', async () => {
    const { engine } = buildEngine(db, { verifies: false });
    enableStatus(engine, 1);
    await engine.pull();
    const status = await engine.getStatus();
    assert.equal(status.lag, 0);
    assert.equal(status.receive.verified, 0);
    assert.equal(status.receive.verifiedHead, null);
    assert.equal(status.receive.quarantined, 1);
    assert.deepEqual(status.receive.quarantineReasons, [{ reason: 'signature_invalid', count: 1 }]);
    assert.equal(status.health, 'degraded');
    assert.deepEqual(status.healthReasons, ['quarantined_events']);
    assert.equal((await engine.getHealth()).healthy, false);
  });

  it('does not turn a general sync timestamp into evidence of a successful pull', async () => {
    const { engine, outbox } = buildEngine(db);
    enableStatus(engine, 0);
    outbox.updateSyncState({ lastSyncAt: new Date() });
    const status = await engine.getStatus();
    assert.equal(status.lastPull, null);
    assert.ok(status.localState.lastSyncAt instanceof Date);
  });

  it('reports retained schema failures without exposing their payloads', async () => {
    const { engine } = buildEngine(db, {
      events: [pulledEvent({ tenantId: null, payload: { secret: 'private-order-data' } })],
    });
    enableStatus(engine, 1);
    await engine.pull();
    const status = await engine.getStatus();
    assert.equal(status.lag, 0);
    assert.equal(status.receive.quarantined, 0);
    assert.deepEqual(status.receive.failures, { count: 1, oldestSequence: 1 });
    assert.deepEqual(status.healthReasons, ['retained_receive_failures']);
    assert.equal((await engine.getHealth()).healthy, false);
    assert.ok(!JSON.stringify(status).includes('private-order-data'));
  });

  it('reads local progress after the remote head request finishes', async () => {
    const { engine, outbox } = buildEngine(db);
    enableStatus(engine, 1);
    engine.client.getHead = async () => {
      await engine.pull();
      return { headSequence: 1 };
    };
    const status = await engine.getStatus();
    assert.equal(outbox.getSyncState().lastPulledSequence, 2);
    assert.equal(status.localHead, 1);
    assert.equal(status.receive.verified, 1);
    assert.equal(status.lag, 0);
  });

  it('retains offline diagnostics and never reports negative lag for a stale head', async () => {
    const { engine } = buildEngine(db, { verifies: false });
    enableStatus(engine, 0);
    await engine.pull();
    const stale = await engine.getStatus();
    assert.equal(stale.lag, 0);
    assert.equal(stale.health, 'degraded');
    assert.ok(stale.healthReasons.includes('remote_head_regressed'));
    engine.client.isConnected = () => false;
    const offline = await engine.getStatus();
    assert.equal(offline.health, 'offline');
    assert.equal(offline.remoteHead, 1);
    assert.equal(offline.receive.quarantined, 1);
    assert.ok(offline.healthReasons.includes('quarantined_events'));
  });

  for (const head of [-1, 1.5, '1', Number.MAX_SAFE_INTEGER + 1, undefined]) {
    it(`refuses healthy status for an invalid remote head: ${String(head)}`, async () => {
      const { engine } = buildEngine(db);
      enableStatus(engine, head);
      const status = await engine.getStatus();
      assert.equal(status.health, 'degraded');
      assert.ok(status.healthReasons.includes('invalid_remote_head'));
    });
  }
});

describe('pull verification', () => {
  let db;

  beforeEach(() => {
    db = new Database(':memory:');
  });

  afterEach(() => db.close());

  for (const [name, override, options] of [
    ['fractional sequence', { events: [{ ...pulledEvent(), sequenceNumber: 1.5 }] }],
    ['string sequence', { events: [{ ...pulledEvent(), sequenceNumber: '1' }] }],
    [
      'unsafe next cursor',
      { events: [{ ...pulledEvent(), sequenceNumber: Number.MAX_SAFE_INTEGER }] },
    ],
    ['duplicate sequence', { events: [pulledEvent(), pulledEvent()] }],
    ['head below event', { headSequence: 0 }],
    ['cursor skips beyond page', { nextSequence: 99 }],
    ['cursor moves backwards', { nextSequence: 0 }],
    ['empty page advances', { events: [], nextSequence: 2 }],
    ['empty continuation', { events: [], nextSequence: 0, hasMore: true }],
    ['stale page', {}, { fromSequence: 5 }],
    [
      'oversized page',
      {
        events: [pulledEvent(), { ...pulledEvent(), sequenceNumber: 2 }],
        nextSequence: 3,
        headSequence: 2,
      },
      { limit: 1 },
    ],
  ]) {
    it(`refuses ${name} before verification or persistence`, async () => {
      const { engine, outbox, verifyCalls } = buildEngine(db);
      engine.on('error', () => {});
      engine.client.pull = async () => ({
        events: [pulledEvent()],
        nextSequence: 2,
        headSequence: 1,
        ...override,
      });
      const result = await engine.pull(options);
      assert.equal(result.success, false);
      assert.match(result.error, /Invalid pull page/);
      assert.equal(verifyCalls.length, 0);
      assert.equal(outbox.getPulledEvents().length, 0);
      assert.equal(outbox.getQuarantinedEvents().length, 0);
      assert.equal(outbox.getSyncState().lastPulledSequence, 0);
    });
  }

  for (const options of [
    { fromSequence: -1 },
    { fromSequence: 0.5 },
    { limit: 0 },
    { limit: -1 },
    { limit: 1.5 },
  ]) {
    it(`rejects invalid pull request before contacting transport: ${JSON.stringify(options)}`, async () => {
      const { engine } = buildEngine(db);
      engine.on('error', () => {});
      let calls = 0;
      engine.client.pull = async () => {
        calls++;
        throw new Error('must not request');
      };
      const result = await engine.pull(options);
      assert.equal(result.success, false);
      assert.match(result.error, /Invalid pull request/);
      assert.equal(calls, 0);
    });
  }

  it('overlapping pulls and explicit replay never rewind committed cursor or head', async () => {
    const { engine, outbox } = buildEngine(db);
    const replies = [];
    engine.client.pull = () => new Promise((resolve) => replies.push(resolve));
    const first = engine.pull();
    const second = engine.pull();
    const one = pulledEvent();
    const two = {
      ...pulledEvent({ eventId: '88888888-1111-1111-1111-111111111111' }),
      sequenceNumber: 2,
    };
    replies[1]({ events: [one, two], nextSequence: 3, headSequence: 9 });
    assert.equal((await second).success, true);
    replies[0]({ events: [one], nextSequence: 2, headSequence: 4 });
    assert.equal((await first).success, true);
    assert.equal(outbox.getSyncState().lastPulledSequence, 3);
    assert.equal(outbox.getSyncState().headSequence, 9);
    const replay = engine.pull({ fromSequence: 0 });
    replies[2]({ events: [one], nextSequence: 2, headSequence: 1 });
    assert.equal((await replay).success, true);
    assert.equal(outbox.getSyncState().lastPulledSequence, 3);
    assert.equal(outbox.getSyncState().headSequence, 9);
    assert.equal(outbox.getPulledEvents().length, 2);
  });

  it('isolates a foreign-store event without blocking a valid event in the same batch', async () => {
    const foreign = pulledEvent({
      eventId: '88888888-1111-1111-1111-111111111111',
      storeId: 'another-store',
    });
    foreign.sequenceNumber = 2;
    const { engine, outbox, verifyCalls } = buildEngine(db, {
      events: [foreign, pulledEvent()],
      nextSequence: 3,
    });
    const result = await engine.pull();
    assert.equal(result.success, true);
    assert.equal(result.stored, 1);
    assert.equal(result.quarantined, 1);
    assert.equal(verifyCalls.length, 1);
    assert.equal(outbox.getQuarantineReason(foreign.envelope.eventId), 'scope_mismatch');
    assert.equal(outbox.getSyncState().lastPulledSequence, 3);
  });

  for (const field of ['tenantId', 'storeId']) {
    it(`retains an event missing ${field} without exposing it to reads`, async () => {
      const malformed = pulledEvent({ [field]: null });
      const { engine, outbox, verifyCalls } = buildEngine(db, { events: [malformed] });
      const result = await engine.pull();
      assert.equal(result.success, true);
      assert.equal(result.stored, 0);
      assert.equal(verifyCalls.length, 0);
      assert.equal(outbox.getReceiveFailures()[0].reason, 'scope_mismatch');
      assert.equal(outbox.getPulledEvents().length, 0);
    });
  }

  for (const securityProfile of ['hybrid', 'pqc-strict']) {
    it(`${securityProfile} quarantines legacy signatures before key lookup`, async () => {
      const { engine, outbox, verifyCalls } = buildEngine(db, {
        securityProfile,
        resolves: () => {
          throw new Error('must not resolve a disallowed event');
        },
      });
      const result = await engine.pull();
      assert.equal(result.success, true);
      assert.equal(result.stored, 0);
      assert.equal(result.quarantined, 1);
      assert.equal(verifyCalls.length, 0);
      assert.equal(
        outbox.getQuarantineReason(pulledEvent().envelope.eventId),
        'security_profile_mismatch',
      );
    });
  }

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

  /**
   * The same erasure the doctor path was fixed for, on the path that runs
   * unattended. A directory outage resolves EVERY event as `key_unresolved`,
   * and `pull()` re-quarantines through `INSERT OR REPLACE`, so without a guard
   * the background sync timer walked every finding down to the benign-outage
   * reason every `syncIntervalMs`.
   */
  describe('re-quarantine never erases a finding', () => {
    it('keeps signature_invalid when a later pull cannot resolve the key', async () => {
      const forged = buildEngine(db, { verifies: false });
      await forged.engine.pull({ fromSequence: 0 });
      assert.equal(forged.outbox.getQuarantinedEvents()[0].reason, 'signature_invalid');

      // Same event, pulled again while the sequencer is unreachable.
      const outage = buildEngine(db, { resolves: false });
      const result = await outage.engine.pull({ fromSequence: 0 });

      assert.equal(result.quarantined, 1);
      assert.equal(
        outage.outbox.getQuarantinedEvents()[0].reason,
        'signature_invalid',
        'an outage teaches nothing about the event and must not erase the forgery',
      );
      assert.equal(outage.outbox.getPulledEvents().length, 0);
    });

    it('keeps every finding, not just signature_invalid', async () => {
      for (const finding of [
        'directory_untrusted',
        'peer_key_conflict',
        'key_revoked',
        'key_outside_validity_window',
      ]) {
        const seeded = buildEngine(db, { resolves: () => ({ error: finding }) });
        await seeded.engine.pull({ fromSequence: 0 });
        assert.equal(seeded.outbox.getQuarantinedEvents()[0].reason, finding);

        const outage = buildEngine(db, { resolves: false });
        await outage.engine.pull({ fromSequence: 0 });
        assert.equal(
          outage.outbox.getQuarantinedEvents()[0].reason,
          finding,
          `${finding} must survive a pull during a directory outage`,
        );

        outage.outbox.deleteQuarantinedEvent(pulledEvent().envelope.eventId);
      }
    });

    it('still sharpens one acquisition failure into another', async () => {
      const outage = buildEngine(db, { resolves: false });
      await outage.engine.pull({ fromSequence: 0 });
      assert.equal(outage.outbox.getQuarantinedEvents()[0].reason, 'key_unresolved');

      const misconfigured = buildEngine(db, {
        resolves: () => ({ error: 'sequencer_key_not_configured' }),
      });
      await misconfigured.engine.pull({ fromSequence: 0 });
      assert.equal(
        misconfigured.outbox.getQuarantinedEvents()[0].reason,
        'sequencer_key_not_configured',
        'learning WHY a key cannot be obtained is still worth recording',
      );
    });

    it('still lets a finding replace an outage reason', async () => {
      const outage = buildEngine(db, { resolves: false });
      await outage.engine.pull({ fromSequence: 0 });
      assert.equal(outage.outbox.getQuarantinedEvents()[0].reason, 'key_unresolved');

      const forged = buildEngine(db, { verifies: false });
      await forged.engine.pull({ fromSequence: 0 });
      assert.equal(
        forged.outbox.getQuarantinedEvents()[0].reason,
        'signature_invalid',
        'the upgrade direction is the whole point of re-diagnosing',
      );
    });
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
    const stored = outbox.getPulledEvents();
    assert.equal(stored.length, 1);
    assert.equal(
      stored[0].eventId,
      '11111111-1111-1111-1111-111111111111',
      'the surviving row must be the one that verified, not merely one row',
    );
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

  it('promotes a previously quarantined event and clears its quarantine row', async () => {
    // The directory was unreachable, so the event quarantined unresolved.
    const first = buildEngine(db, { resolves: false });
    await first.engine.pull();
    assert.equal(first.outbox.getQuarantinedEvents().length, 1);

    // The user re-pulls the same sequence; the key now resolves and verifies.
    const second = buildEngine(db);
    await second.engine.pull({ fromSequence: 0 });

    assert.equal(second.outbox.getPulledEvents().length, 1);
    assert.equal(
      second.outbox.getQuarantinedEvents().length,
      0,
      'an event must never read as both readable and quarantined',
    );
  });

  it('reports honest zeroes when the sequencer returns no events', async () => {
    const { engine } = buildEngine(db, { events: [], nextSequence: 1 });
    const result = await engine.pull();

    assert.equal(result.success, true);
    assert.equal(result.pulled, 0);
    assert.equal(result.verified, 0);
    assert.equal(result.quarantined, 0);
    assert.equal(result.stored, 0);
    assert.equal(
      result.conflicts,
      null,
      'conflicts was not computed on this branch; 0 would be a claim it did not make',
    );
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
    assert.equal(result.conflicts, null);
    assert.equal(result.error, 'sequencer unreachable');
  });

  it('stores nothing on a dry run and reports no applied count', async () => {
    const { engine, outbox } = buildEngine(db);
    const result = await engine.pull({ dryRun: true });

    assert.equal(outbox.getPulledEvents().length, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 0);
    assert.equal(result.pulled, 1);
    assert.equal(result.stored, 0);
    assert.equal(result.conflicts, null);
    assert.equal('applied' in result, false);
  });

  it('a malformed envelope cannot wedge the cursor or take the batch down', async () => {
    // entityId is NOT NULL in both the pulled and quarantined tables, and the
    // whole envelope is attacker-controlled: a peer can sign this.
    const good = pulledEvent();
    const malformed = pulledEvent({
      eventId: '88888888-1111-1111-1111-111111111111',
      entityId: null,
    });
    malformed.sequenceNumber = 2;

    const { engine, outbox } = buildEngine(db, {
      events: [good, malformed],
      nextSequence: 3,
    });
    const storeFailures = [];
    engine.on('receive-store-failed', (payload) => storeFailures.push(payload));

    const result = await engine.pull();

    assert.equal(result.success, true, 'a malformed envelope must not fail the whole pull');
    assert.equal(result.verified, 2, 'both signatures were accepted by the stubbed verifier');
    assert.equal(result.stored, 1, 'stored counts what the local schema actually accepted');
    assert.equal(outbox.getPulledEvents().length, 1);
    assert.equal(storeFailures.length, 1);
    assert.equal(storeFailures[0].eventId, '88888888-1111-1111-1111-111111111111');
    const [retained] = outbox.getReceiveFailures();
    assert.equal(retained.stage, 'verified');
    assert.equal(retained.record.entityId, null);
    assert.equal(retained.record.agentSignature, malformed.envelope.agentSignature);
    assert.equal(
      outbox.getSyncState().lastPulledSequence,
      3,
      'one malformed event must not wedge this agent’s sync',
    );
  });

  it('a malformed envelope that fails verification cannot wedge the cursor either', async () => {
    const malformed = pulledEvent({
      eventId: '88888888-1111-1111-1111-111111111111',
      entityId: null,
    });

    const { engine, outbox } = buildEngine(db, {
      events: [malformed],
      nextSequence: 2,
      verifies: false,
    });
    const quarantineFailures = [];
    engine.on('receive-quarantine-failed', (payload) => quarantineFailures.push(payload));

    const result = await engine.pull();

    assert.equal(result.success, true);
    assert.equal(result.quarantined, 0, 'quarantined counts rows actually written');
    assert.equal(quarantineFailures.length, 1);
    assert.equal(quarantineFailures[0].reason, 'signature_invalid');
    const [retained] = outbox.getReceiveFailures();
    assert.equal(retained.stage, 'quarantine');
    assert.equal(retained.reason, 'signature_invalid');
    assert.equal(outbox.getSyncState().lastPulledSequence, 2);
  });
});

describe('durable receive failure recovery', () => {
  it('preserves an existing record and conflict evidence across later good replays', async () => {
    const db = new Database(':memory:');
    try {
      const first = buildEngine(db);
      assert.equal((await first.engine.pull()).stored, 1);
      const before = db.prepare('SELECT * FROM _ves_pulled_events').get();
      const conflict = pulledEvent({ entityId: 'conflicting-order' });
      const second = buildEngine(db, { events: [conflict] });
      const result = await second.engine.pull({ fromSequence: 0 });
      assert.equal(result.success, true);
      assert.equal(result.stored, 0);
      assert.deepEqual(db.prepare('SELECT * FROM _ves_pulled_events').get(), before);
      const [retained] = second.outbox.getReceiveFailures();
      assert.equal(retained.reason, 'event_identity_conflict');
      assert.equal(retained.record.entityId, 'conflicting-order');
      assert.equal((await first.engine.pull({ fromSequence: 0 })).stored, 1);
      assert.deepEqual(
        first.outbox.getReceiveFailures()[0],
        retained,
        'good replay must not erase conflicting evidence',
      );
      first.outbox.storeReceiveFailure(
        { ...retained.record, entityId: null },
        'verified',
        null,
        'later storage issue',
      );
      assert.deepEqual(
        first.outbox.getReceiveFailures()[0],
        retained,
        'later generic failure must not erase conflict evidence',
      );
    } finally {
      db.close();
    }
  });
  it('retains refused records across restart and clears them after identical replay', async (t) => {
    const dir = mkdtempSync(join(tmpdir(), 'stateset-receive-'));
    let db;
    t.after(() => {
      if (db?.open) db.close();
      rmSync(dir, { recursive: true, force: true });
    });
    const path = join(dir, 'store.db');
    db = new Database(path);
    const original = pulledEvent();
    const first = buildEngine(db, { events: [original] });
    db.exec(
      "CREATE TRIGGER refuse_receive BEFORE INSERT ON _ves_pulled_events BEGIN SELECT RAISE(ABORT, 'temporary store failure'); END",
    );
    assert.equal((await first.engine.pull()).success, true);
    db.close();
    db = new Database(path);
    const replay = buildEngine(db, { events: [original] });
    const [retained] = replay.outbox.getReceiveFailures();
    assert.equal(retained.record.eventId, original.envelope.eventId);
    assert.deepEqual(retained.record.payload, original.envelope.payload);
    assert.equal(retained.record.agentSignature, original.envelope.agentSignature);
    assert.match(retained.error, /temporary store failure/);
    assert.equal(replay.outbox.getSyncState().lastPulledSequence, 2);
    db.exec('DROP TRIGGER refuse_receive');
    assert.equal((await replay.engine.pull({ fromSequence: 0 })).stored, 1);
    assert.equal(replay.outbox.getReceiveFailures().length, 0);
    assert.equal(replay.outbox.getPulledEvents().length, 1);
  });

  for (const verifies of [true, false]) {
    it(`rolls back all receive writes when the failure journal is unavailable (verifies=${verifies})`, async () => {
      const db = new Database(':memory:');
      try {
        const malformed = pulledEvent({
          entityId: null,
          eventId: '88888888-1111-1111-1111-111111111111',
        });
        malformed.sequenceNumber = 2;
        const { engine, outbox } = buildEngine(db, {
          events: [pulledEvent(), malformed],
          nextSequence: 3,
          verifies,
        });
        engine.on('error', () => {});
        db.exec(
          "CREATE TRIGGER refuse_failure_journal BEFORE INSERT ON _ves_receive_failures BEGIN SELECT RAISE(ABORT, 'journal unavailable'); END",
        );
        const result = await engine.pull();
        assert.equal(result.success, false);
        assert.match(result.error, /journal unavailable/);
        assert.equal(outbox.getSyncState().lastPulledSequence, 0);
        assert.equal(outbox.getPulledEvents().length, 0);
        assert.equal(outbox.getQuarantinedEvents().length, 0);
        assert.equal(outbox.getReceiveFailures().length, 0);
      } finally {
        db.close();
      }
    });
  }

  it('commits cursor, stored events, quarantine, and receive failures atomically', async () => {
    const db = new Database(':memory:');
    try {
      const bad = pulledEvent({ eventId: '88888888-1111-1111-1111-111111111111' });
      bad.sequenceNumber = 2;
      const malformed = pulledEvent({
        eventId: '99999999-1111-1111-1111-111111111111',
        entityId: null,
      });
      malformed.sequenceNumber = 3;
      const { engine, outbox } = buildEngine(db, {
        events: [pulledEvent(), bad, malformed],
        nextSequence: 4,
        verifies: (envelope) => envelope.eventId !== bad.envelope.eventId,
      });
      engine.on('error', () => {});
      db.exec(
        "CREATE TRIGGER refuse_cursor BEFORE UPDATE ON _ves_sync_state WHEN NEW.key = 'last_pulled_sequence' BEGIN SELECT RAISE(ABORT, 'cursor unavailable'); END",
      );
      assert.equal((await engine.pull()).success, false);
      assert.equal(outbox.getSyncState().lastPulledSequence, 0);
      assert.equal(outbox.getSyncState().lastPullAt, null);
      assert.equal(outbox.getPulledEvents().length, 0);
      assert.equal(outbox.getQuarantinedEvents().length, 0);
      assert.equal(outbox.getReceiveFailures().length, 0);
      db.exec('DROP TRIGGER refuse_cursor');
      assert.equal((await engine.pull()).success, true);
      assert.equal(outbox.getSyncState().lastPulledSequence, 4);
      assert.ok(outbox.getSyncState().lastPullAt instanceof Date);
      assert.equal(outbox.getPulledEvents().length, 1);
      assert.equal(outbox.getQuarantinedEvents().length, 1);
      assert.equal(outbox.getReceiveFailures().length, 1);
    } finally {
      db.close();
    }
  });
});

describe('streamed events are not a second writer', () => {
  let db;
  let originalWarn;

  beforeEach(() => {
    db = new Database(':memory:');
    // The refusal warns once per engine; keep it out of the test output.
    originalWarn = console.warn;
    console.warn = () => {};
  });

  afterEach(() => {
    console.warn = originalWarn;
    db.close();
  });

  /** A gRPC streamed event: flat, no envelope, no usable signature material. */
  function streamedEvent() {
    return {
      sequenceNumber: 7,
      eventId: '77777777-1111-1111-1111-111111111111',
      tenantId: TENANT,
      storeId: STORE,
      entityType: 'order',
      entityId: 'ORD-7',
      eventType: 'order.created',
      payload: { total: 1 },
      createdAt: '2026-09-12T00:00:00.000Z',
      sequencedAt: '2026-09-12T00:00:01.000Z',
      sourceAgent: AGENT,
    };
  }

  it('refuses to store a streamed event and does not advance the cursor', async () => {
    const { engine, outbox } = buildEngine(db);
    outbox.initialize();
    const before = outbox.getSyncState().lastPulledSequence;

    const refusals = [];
    engine.on('stream-store-refused', (payload) => refusals.push(payload));

    engine._handleStreamedEvent(streamedEvent());

    assert.equal(
      outbox.getPulledEvents().length,
      0,
      'the stream must never write into _ves_pulled_events: it verifies nothing',
    );
    assert.equal(outbox.getQuarantinedEvents().length, 0);
    assert.equal(
      outbox.getSyncState().lastPulledSequence,
      before,
      'advancing the cursor from the stream would skip these events on the verified path',
    );

    assert.equal(refusals.length, 1, 'the refusal must be loud, not a silent drop');
    assert.equal(refusals[0].reason, 'stream_unverified');
    assert.equal(refusals[0].sequenceNumber, 7);
    assert.ok(refusals[0].error instanceof Error);
  });

  it('still surfaces streamed events to real-time consumers', async () => {
    const { engine } = buildEngine(db);
    const seen = [];
    engine.on('event', (event) => seen.push(event));
    engine.on('stream-store-refused', () => {});

    engine._handleStreamedEvent(streamedEvent());

    assert.equal(seen.length, 1, 'the stream stays a notification channel');
    assert.equal(engine.getRecentEvents().length, 1);
  });
});

// =============================================================================
// Seam test — real client, real directory, real signature, only _request stubbed
// =============================================================================

/**
 * The tests above stub `engine.client`, `engine.keyDirectory` and
 * `engine.resolver`, so the fakes define their own contracts. That style proves
 * the orchestration but cannot catch a broken seam: a method the real client
 * does not have, argument-order or return-shape drift in
 * `PeerKeyDirectory.resolve`, the REST envelope mapping, or the signing-hash
 * plumbing.
 *
 * This test stubs exactly one thing — `SequencerClient._request`, the HTTP
 * boundary — and drives everything else for real: a genuinely signed event, a
 * genuinely signed key directory, the real `UnifiedSequencerClient`, the real
 * `PeerKeyDirectory`, the real outbox, and the real conflict resolver.
 */
/**
 * Before this branch a gRPC deployment stored UNVERIFIED streamed events. It
 * now stores nothing at all — streamed events are refused, the gRPC envelope
 * mapping cannot satisfy `verifyEventSignature`, and the gRPC key directory is
 * never attested. That regression is deliberate, and must be loud rather than
 * presenting as an endlessly empty pull.
 */
describe('the gRPC receive path is refused, not silently empty', () => {
  let db;

  beforeEach(() => {
    db = new Database(':memory:');
  });

  afterEach(() => db.close());

  function grpcEngine() {
    const config = new SyncConfig({
      sequencer: { url: 'grpcs://sequencer.example.com' },
      identity: { tenantId: TENANT, storeId: STORE, agentId: SELF },
    });
    const engine = new SyncEngine({ db, config });
    engine.outbox = createOutbox(db, {});

    let pullCalls = 0;
    engine.client = {
      transport: 'grpc',
      async pull(fromSequence) {
        pullCalls += 1;
        return { events: [], nextSequence: fromSequence, headSequence: 0 };
      },
    };
    return { engine, pullCalls: () => pullCalls };
  }

  it('fails the pull with an explanation instead of reporting an empty success', async () => {
    const { engine, pullCalls } = grpcEngine();

    const errors = [];
    const originalError = console.error;
    console.error = (message) => errors.push(String(message));
    let result;
    try {
      result = await engine.pull();
    } finally {
      console.error = originalError;
    }

    assert.equal(result.success, false);
    assert.match(result.error, /gRPC receive path is unsupported/);
    assert.match(result.error, /https:\/\/ sequencer URL/);
    assert.equal(result.stored, 0);
    assert.equal(result.conflicts, null, 'a refused pull claims nothing about conflicts');
    assert.equal(pullCalls(), 0, 'it must not even ask the gRPC transport for events');
    assert.ok(
      errors.some((line) => /gRPC receive path is unsupported/.test(line)),
      'the refusal must not depend on anyone reading the return value',
    );
  });

  it('still pulls normally over REST', async () => {
    const { engine } = grpcEngine();
    engine.client.transport = 'rest';

    const result = await engine.pull();
    assert.equal(result.success, true);
  });

  it('says the same thing on the streaming path', () => {
    const { engine } = grpcEngine();
    const refusals = [];
    engine.on('stream-store-refused', (event) => refusals.push(event));

    const originalWarn = console.warn;
    const warnings = [];
    console.warn = (message) => warnings.push(String(message));
    try {
      engine._handleStreamedEvent({ sequenceNumber: 1, eventId: 'evt-1' });
    } finally {
      console.warn = originalWarn;
    }

    assert.equal(refusals.length, 1);
    assert.equal(refusals[0].reason, 'stream_unverified');
    assert.match(refusals[0].error.message, /gRPC receive path is unsupported/);
    assert.equal(engine.outbox.getPulledEvents().length, 0);
    assert.ok(warnings.some((line) => /gRPC receive path is unsupported/.test(line)));
  });
});

describe('pull verification — end to end through the real client and directory', () => {
  let db;

  beforeEach(() => {
    db = new Database(':memory:');
  });

  afterEach(() => db.close());

  const SEQ_URL = 'https://sequencer.example.com';

  function rawEd25519PublicKey(keyPair) {
    return keyPair.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
  }

  function signKeyDirectory(body, sequencerKey) {
    const preimage = Buffer.concat([
      Buffer.from('VES_KEYDIR_V1'),
      Buffer.from(canonicalizeJson(body)),
    ]);
    const hash = crypto.createHash('sha256').update(preimage).digest();
    return crypto.sign(null, hash, sequencerKey.privateKey);
  }

  /**
   * Build one genuinely signed event plus the snake_case wire row the sequencer
   * would return for it.
   */
  function signedWireEvent(agentKey, { tamper = false } = {}) {
    const payload = { total: 99.99 };
    const payloadPlainHash = computePayloadPlainHash(payload).toString('hex');
    const payloadCipherHash = '0'.repeat(64);

    const envelope = {
      vesVersion: 1,
      tenantId: TENANT,
      storeId: STORE,
      eventId: '11111111-1111-1111-1111-111111111111',
      sourceAgentId: AGENT,
      agentKeyId: 1,
      entityType: 'order',
      entityId: 'ORD-1',
      eventType: 'order.created',
      payloadKind: 0,
      createdAt: '2026-09-12T00:00:00.000Z',
      payloadPlainHash: hexToBuffer(payloadPlainHash),
      payloadCipherHash: hexToBuffer(payloadCipherHash),
    };

    const signingHash = computeEventSigningHash(envelope);
    const signature = crypto.sign(null, signingHash, agentKey.privateKey);

    return {
      envelope: {
        sequence_number: 1,
        event_id: envelope.eventId,
        command_id: null,
        tenant_id: TENANT,
        store_id: STORE,
        // Tampering changes a field the signing hash binds, so the signature
        // that was genuinely produced above no longer covers this envelope.
        entity_type: tamper ? 'invoice' : 'order',
        entity_id: 'ORD-1',
        event_type: 'order.created',
        payload,
        ves_version: 1,
        payload_kind: 0,
        payload_plain_hash: payloadPlainHash,
        payload_cipher_hash: payloadCipherHash,
        agent_key_id: 1,
        agent_signature: signature.toString('hex'),
        agent_signature_scheme: 0,
        base_version: null,
        created_at: envelope.createdAt,
        source_agent: AGENT,
      },
      sequenced_at: '2026-09-12T00:00:01.000Z',
    };
  }

  /** Wire the engine's own client to a real SequencerClient over a stubbed _request. */
  function buildRealEngine({
    tamper = false,
    tamperPayload = false,
    configureSequencerKey = true,
    directoryAgentId = AGENT,
    directoryTenantId = TENANT,
  } = {}) {
    const agentKey = crypto.generateKeyPairSync('ed25519');
    const sequencerKey = crypto.generateKeyPairSync('ed25519');

    const config = new SyncConfig({
      sequencer: { url: SEQ_URL },
      sync: { securityProfile: 'legacy' },
      identity: { tenantId: TENANT, storeId: STORE, agentId: SELF },
      sequencerPublicKey: configureSequencerKey
        ? rawEd25519PublicKey(sequencerKey).toString('hex')
        : null,
    });

    const directoryBody = {
      agentId: directoryAgentId,
      tenantId: directoryTenantId,
      keys: [
        {
          keyId: 1,
          algorithm: 'ed25519',
          publicKey: `0x${rawEd25519PublicKey(agentKey).toString('hex')}`,
          publicKeyBundle: null,
          validFrom: null,
          validTo: null,
          revokedAt: null,
        },
      ],
      signedAt: new Date().toISOString(),
    };
    const directorySignature = signKeyDirectory(directoryBody, sequencerKey);

    const requests = [];
    const rest = new SequencerClient(config);
    rest._request = async (method, path) => {
      requests.push(`${method} ${path.split('?')[0]}`);
      if (path.startsWith('/api/v1/events')) {
        const event = signedWireEvent(agentKey, { tamper });
        if (tamperPayload) event.envelope.payload = { total: '0.01' };
        return { events: [event], head_sequence: 1 };
      }
      if (path.startsWith('/api/v1/agents/')) {
        return { ...directoryBody, directorySignature: `0x${directorySignature.toString('hex')}` };
      }
      throw new Error(`unexpected request: ${method} ${path}`);
    };

    const engine = new SyncEngine({ db, config });
    // The ONLY substitution: hand the engine's own UnifiedSequencerClient a
    // connected REST client whose HTTP boundary is stubbed. Its outbox,
    // key directory, conflict resolver and every client method stay real.
    engine.client._client = rest;
    engine.client._transport = 'rest';

    return { engine, requests };
  }

  it('stores a genuinely signed event pulled through the real client stack', async () => {
    const { engine, requests } = buildRealEngine();

    const result = await engine.pull();

    assert.equal(result.success, true);
    assert.equal(result.pulled, 1);
    assert.equal(result.verified, 1, 'a real signature must verify through the real client');
    assert.equal(result.quarantined, 0);
    assert.equal(result.stored, 1);

    const stored = engine.outbox.getPulledEvents();
    assert.equal(stored.length, 1);
    assert.equal(stored[0].eventId, '11111111-1111-1111-1111-111111111111');
    assert.equal(engine.outbox.getQuarantinedEvents().length, 0);

    // The directory really was fetched and pinned through PeerKeyDirectory.
    assert.ok(
      requests.includes(`GET /api/v1/agents/${AGENT}/signing-keys`),
      'the key directory must be fetched over the real client',
    );
    assert.ok(engine.outbox.getPeerKeyPin(AGENT, 1), 'the resolved key must be pinned');
  });

  it('quarantines a tampered event pulled through the real client stack', async () => {
    const { engine } = buildRealEngine({ tamper: true });

    const result = await engine.pull();

    assert.equal(result.verified, 0);
    assert.equal(result.quarantined, 1);
    assert.equal(engine.outbox.getPulledEvents().length, 0);
    assert.equal(engine.outbox.getQuarantinedEvents()[0].reason, 'signature_invalid');
  });

  it('quarantines altered payloads even when the signed hashes and signature are untouched', async () => {
    const { engine } = buildRealEngine({ tamperPayload: true });
    const result = await engine.pull();
    assert.equal(result.success, true);
    assert.equal(result.stored, 0);
    assert.equal(result.quarantined, 1);
    assert.equal(engine.outbox.getPulledEvents().length, 0);
    assert.equal(engine.outbox.getQuarantinedEvents()[0].reason, 'signature_invalid');
  });

  /**
   * The path EVERY deployment took before `sequencerPublicKey` was
   * configurable: a correct sequencer, a correct peer, a genuinely signed
   * event — and nothing stored, because the local config has no key to verify
   * the directory with. This is not an edge case, so it is pinned by name: the
   * reason must point the operator at their own config, not at the peer's key
   * registry.
   */
  it('names the missing sequencer public key instead of blaming the key registry', async () => {
    const warnings = [];
    const originalWarn = console.warn;
    console.warn = (message) => warnings.push(String(message));

    let result;
    let engine;
    try {
      ({ engine } = buildRealEngine({ configureSequencerKey: false }));
      result = await engine.pull();
    } finally {
      console.warn = originalWarn;
    }

    assert.equal(result.success, true);
    assert.equal(result.pulled, 1);
    assert.equal(result.verified, 0);
    assert.equal(result.quarantined, 1);
    assert.equal(result.stored, 0);
    assert.equal(engine.outbox.getPulledEvents().length, 0);

    const quarantined = engine.outbox.getQuarantinedEvents();
    assert.equal(quarantined.length, 1);
    assert.equal(
      quarantined[0].reason,
      'sequencer_key_not_configured',
      'a missing local config line must not read as key_unresolved',
    );

    // And it is loud without anyone attaching a listener.
    assert.ok(
      warnings.some((line) => line.includes('sequencer_key_not_configured')),
      `expected a warning naming the cause, got ${JSON.stringify(warnings)}`,
    );
  });

  it('emits the underlying cause as `detail` on receive-verification-failed', async () => {
    const { engine } = buildRealEngine({ configureSequencerKey: false });
    const failures = [];
    engine.on('receive-verification-failed', (event) => failures.push(event));

    const originalWarn = console.warn;
    console.warn = () => {};
    try {
      await engine.pull();
    } finally {
      console.warn = originalWarn;
    }

    assert.equal(failures.length, 1);
    assert.equal(failures[0].reason, 'sequencer_key_not_configured');
    assert.match(failures[0].detail, /sequencerPublicKey is not configured/);
  });

  it('refuses a validly signed directory issued for a different agent', async () => {
    const { engine } = buildRealEngine({
      directoryAgentId: '99999999-9999-9999-9999-999999999999',
    });

    const originalWarn = console.warn;
    console.warn = () => {};
    let result;
    try {
      result = await engine.pull();
    } finally {
      console.warn = originalWarn;
    }

    assert.equal(result.stored, 0);
    assert.equal(engine.outbox.getQuarantinedEvents()[0].reason, 'directory_untrusted');
    assert.equal(engine.outbox.getPeerKeys(AGENT).length, 0, 'no foreign key may be cached');
  });

  it('refuses a validly signed directory issued for a different tenant', async () => {
    const { engine } = buildRealEngine({
      directoryTenantId: '99999999-9999-9999-9999-999999999999',
    });

    const originalWarn = console.warn;
    console.warn = () => {};
    let result;
    try {
      result = await engine.pull();
    } finally {
      console.warn = originalWarn;
    }

    assert.equal(result.stored, 0);
    assert.equal(engine.outbox.getQuarantinedEvents()[0].reason, 'directory_untrusted');
    assert.equal(engine.outbox.getPeerKeys(AGENT).length, 0, 'no foreign key may be cached');
  });
});
