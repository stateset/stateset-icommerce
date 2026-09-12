/**
 * The pull path must verify every event against its author's key and
 * quarantine what it cannot verify.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import crypto from 'node:crypto';

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
    assert.equal(outbox.getSyncState().lastPulledSequence, 2);
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
    configureSequencerKey = true,
    directoryAgentId = AGENT,
    directoryTenantId = TENANT,
  } = {}) {
    const agentKey = crypto.generateKeyPairSync('ed25519');
    const sequencerKey = crypto.generateKeyPairSync('ed25519');

    const config = new SyncConfig({
      sequencer: { url: SEQ_URL },
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
        return { events: [signedWireEvent(agentKey, { tamper })], head_sequence: 1 };
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
