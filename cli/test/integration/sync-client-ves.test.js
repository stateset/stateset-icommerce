/**
 * The REST SequencerClient and SyncEngine against the VES wire format.
 *
 * Runs against examples/agent-board/mock-sequencer.mjs: an in-memory HTTP
 * stand-in on an ephemeral 127.0.0.1 port whose routes and response shapes
 * are taken from stateset-sequencer src/api (master @ e97829b). No other
 * network access.
 *
 * Each test pins one of the mismatches the client had with the real API:
 * reads on the deprecated pre-VES routes, VES envelope fields read from the
 * wrong place, push settlement that could not read `event_id` rejections or
 * replayed receipts, legacy commitment fields, an API key sent as a JWT, and
 * an opaque failure from a sequencer without the key-directory route.
 */

import { after, before, describe, test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import Database from 'better-sqlite3';

import { startMockSequencer } from '../../../examples/agent-board/mock-sequencer.mjs';
import { SequencerClient } from '../../src/sync/client.js';
import { SyncConfig, normalizeSequencerPublicKey } from '../../src/sync/config.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { createPeerKeyDirectory } from '../../src/sync/key-directory.js';
import { computeEventSigningHash, computePayloadPlainHash } from '../../src/sync/crypto.js';
import { KEY_ALGORITHM_ED25519 } from '../../src/sync/pqc.js';

const ZERO_HASH = '0'.repeat(64);

/** A plain client config (as examples/agent-board/lib/config.mjs builds one). */
function clientConfig(mock, { apiKey = mock.writeApiKey, jwt = null, agentId = null } = {}) {
  return {
    sequencerUrl: mock.url,
    tenantId: mock.tenantId,
    storeId: mock.storeId,
    agentId,
    securityProfile: 'legacy',
    allowInsecureTransport: true,
    sequencerPublicKey: normalizeSequencerPublicKey(mock.sequencerPublicKey),
    peerKeyTtlSeconds: 300,
    peerKeyMaxStaleSeconds: 86400,
    requestTimeoutMs: 15000,
    retryPolicy: { maxRetries: 0, baseDelay: 1, maxDelay: 1 },
    getCredentials: () => ({ apiKey, jwt }),
  };
}

function rawPublicKey(pair) {
  return pair.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
}

/** An agent: an Ed25519 key registered with the mock under key id 1. */
async function registerAgent(mock) {
  const agentId = crypto.randomUUID();
  const pair = crypto.generateKeyPairSync('ed25519');
  const admin = new SequencerClient(clientConfig(mock, { apiKey: mock.adminApiKey }));
  const result = await admin.registerAgentKey({
    agentId,
    keyId: 1,
    keyAlgorithm: KEY_ALGORITHM_ED25519,
    publicKey: `0x${rawPublicKey(pair).toString('hex')}`,
  });
  assert.equal(result.success, true);
  return { agentId, pair, publicKey: rawPublicKey(pair) };
}

/** A VES v1 envelope signed by `agent`, in the client's push shape. */
function signedEnvelope(mock, agent, { entityType = 'order', entityId = 'ORD-1', payload } = {}) {
  const body = payload ?? { total: '19.99', nonce: crypto.randomUUID() };
  const payloadPlainHash = computePayloadPlainHash(body).toString('hex');
  const envelope = {
    eventId: crypto.randomUUID(),
    commandId: crypto.randomUUID(),
    tenantId: mock.tenantId,
    storeId: mock.storeId,
    entityType,
    entityId,
    eventType: `${entityType}.updated`,
    payload: body,
    vesVersion: 1,
    payloadKind: 0,
    payloadEncrypted: null,
    payloadPlainHash,
    payloadCipherHash: ZERO_HASH,
    agentKeyId: 1,
    agentSignatureScheme: 0,
    agentSignatureBundle: null,
    baseVersion: null,
    createdAt: new Date().toISOString(),
    sourceAgent: agent.agentId,
  };
  const hash = computeEventSigningHash({
    ...envelope,
    sourceAgentId: envelope.sourceAgent,
    payloadPlainHash: Buffer.from(payloadPlainHash, 'hex'),
    payloadCipherHash: Buffer.from(ZERO_HASH, 'hex'),
  });
  envelope.agentSignature = crypto.sign(null, hash, agent.pair.privateKey).toString('hex');
  return envelope;
}

const strip0x = (hex) => String(hex).replace(/^0x/, '').toLowerCase();

/**
 * The mock's request log, API routes only: on a shared machine other local
 * processes sometimes probe an ephemeral loopback port (`GET /`, no auth).
 */
const apiRequests = (requests) => requests.filter((r) => r.path.startsWith('/api/'));

describe('SequencerClient against the VES wire format', () => {
  let mock;
  let agent;
  let client;
  const pushed = [];

  before(async () => {
    mock = await startMockSequencer();
    agent = await registerAgent(mock);
    client = new SequencerClient(clientConfig(mock, { agentId: agent.agentId }));
    const events = [
      signedEnvelope(mock, agent, { entityId: 'ORD-1' }),
      signedEnvelope(mock, agent, { entityId: 'ORD-2' }),
      signedEnvelope(mock, agent, { entityId: 'ORD-1' }),
    ];
    const receipt = await client.push({ agentId: agent.agentId, events });
    assert.equal(receipt.eventsAccepted, 3);
    pushed.push(...events);
  });

  after(() => mock?.close());

  test('pull reads /api/v1/ves/events and returns the ingested events with their VES fields', async () => {
    const page = await client.pull(0, 100);
    assert.ok(mock.requests.some((r) => r.path === '/api/v1/ves/events'));
    assert.ok(!mock.requests.some((r) => r.path === '/api/v1/events'), 'no legacy route');
    assert.equal(page.events.length, 3);
    assert.equal(page.headSequence, 3);
    assert.equal(page.nextSequence, 4);
    assert.equal(page.hasMore, false);

    page.events.forEach((event, i) => {
      const sent = pushed[i];
      assert.equal(event.sequenceNumber, i + 1);
      assert.equal(event.envelope.eventId, sent.eventId);
      assert.equal(event.envelope.sourceAgent, agent.agentId, 'from source_agent_id');
      assert.equal(event.envelope.entityId, sent.entityId);
      assert.deepEqual(event.envelope.payload, sent.payload);
      assert.equal(strip0x(event.envelope.agentSignature), sent.agentSignature);
      assert.equal(strip0x(event.envelope.payloadPlainHash), sent.payloadPlainHash);
      assert.equal(event.envelope.createdAt, sent.createdAt);
      assert.ok(!Number.isNaN(Date.parse(event.sequencedAt)), 'sequenced_at from the envelope');
      assert.equal('receiptHash' in event, false);
    });
  });

  test('pulled events verify under the author key; an edited one does not', async () => {
    const { events } = await client.pull(1, 100);
    for (const event of events) {
      assert.equal(client.verifyEventSignature(event.envelope, agent.publicKey), true);
    }
    const forged = { ...events[0].envelope, entityId: 'ORD-999' };
    assert.equal(client.verifyEventSignature(forged, agent.publicKey), false);
  });

  test('pull pages with the server continuation', async () => {
    const first = await client.pull(1, 2);
    assert.deepEqual(
      first.events.map((e) => e.sequenceNumber),
      [1, 2],
    );
    assert.equal(first.hasMore, true);
    const second = await client.pull(first.nextSequence, 2);
    assert.deepEqual(
      second.events.map((e) => e.sequenceNumber),
      [3],
    );
    assert.equal(second.hasMore, false);
  });

  test('getHead reads /api/v1/ves/head', async () => {
    assert.deepEqual(await client.getHead(), {
      tenantId: mock.tenantId,
      storeId: mock.storeId,
      headSequence: 3,
    });
    assert.ok(mock.requests.some((r) => r.path === '/api/v1/ves/head'));
  });

  test('entity history follows has_more across 100-event pages, URL-encoding the id', async () => {
    const entityId = 'cart 7/ü?x';
    const events = Array.from({ length: 205 }, () =>
      signedEnvelope(mock, agent, { entityType: 'cart', entityId }),
    );
    for (let i = 0; i < events.length; i += 100) {
      const receipt = await client.push({
        agentId: agent.agentId,
        events: events.slice(i, i + 100),
      });
      assert.equal(receipt.eventsRejected, 0);
    }
    const before = mock.requests.length;
    const history = await client.getEntityHistory('cart', entityId);
    const pages = apiRequests(mock.requests.slice(before));
    assert.equal(pages.length, 3, '100 + 100 + 5');
    assert.ok(
      pages.every((r) => r.path === `/api/v1/ves/entities/cart/${encodeURIComponent(entityId)}`),
    );
    assert.equal(history.length, 205);
    assert.deepEqual(
      history.map((e) => e.envelope.eventId),
      events.map((e) => e.eventId),
    );
    for (const event of history.slice(0, 3)) {
      assert.equal(client.verifyEventSignature(event.envelope, agent.publicKey), true);
    }
  });

  test('a VES commitment and its proofs verify with the leaf count bound from the commitment', async () => {
    const head = (await client.getHead()).headSequence;
    const last = (await client.listCommitments()).reduce((m, c) => Math.max(m, c.sequenceEnd), 0);
    // Commit an odd-sized range so the tree is padded.
    const created = await client.createCommitment(last + 1, last + 3);
    assert.ok(head >= last + 3);
    assert.equal(created.leafCount, 3);
    assert.equal(created.paddedLeafCount, 4);
    assert.match(created.merkleRoot, /^[0-9a-f]{64}$/);

    const fetched = await client.getCommitment(created.batchId);
    assert.deepEqual(fetched, created);
    const listed = await client.listCommitments();
    assert.ok(listed.some((c) => c.batchId === created.batchId));
    assert.equal(await client.getCommitment(crypto.randomUUID()), null, '404 -> null');

    const { events } = await client.pull(created.sequenceStart, 3);
    for (const event of events) {
      const verdict = await client.verifyEventInclusion(event, fetched);
      assert.equal(verdict.valid, true, JSON.stringify(verdict));
      assert.equal(verdict.pathBound, true);
      assert.equal(verdict.leafCountSource, 'caller');
      assert.equal(verdict.leafCount, 3);
    }

    // The proof states no tree size, so on its own it binds nothing.
    const proof = await client.getInclusionProof(events[1].sequenceNumber, created.batchId);
    assert.equal(proof.leafCount, undefined);
    assert.equal(proof.leafIndex, 1);
    const unbound = client.verifyInclusionDetailed(events[1].envelope, proof, created.merkleRoot, {
      sequenceNumber: events[1].sequenceNumber,
    });
    assert.equal(unbound.valid, false);
    assert.match(unbound.reason, /leaf count/);

    // A size other than the committed one, or an event outside the range, fails.
    const wrongSize = await client.verifyEventInclusion(events[0], { ...fetched, leafCount: 4 });
    assert.equal(wrongSize.valid, false);
    assert.match(wrongSize.reason, /leaf_count disagrees/);
    const outside = await client.verifyEventInclusion(
      { ...events[0], sequenceNumber: created.sequenceEnd + 1 },
      fetched,
    );
    assert.equal(outside.valid, false);
    assert.match(outside.reason, /outside the commitment range/);
    const edited = await client.verifyEventInclusion(
      { ...events[0], envelope: { ...events[0].envelope, eventType: 'order.voided' } },
      fetched,
    );
    assert.equal(edited.valid, false, 'an edited envelope is not the committed leaf');
  });

  test('every request sends the API key as Authorization: ApiKey', () => {
    const authed = apiRequests(mock.requests);
    assert.ok(authed.length > 0);
    assert.deepEqual([...new Set(authed.map((r) => r.auth))], ['ApiKey']);
  });

  test('a JWT is sent as Bearer, which the sequencer validates as a JWT', async () => {
    const jwtClient = new SequencerClient(clientConfig(mock, { apiKey: null, jwt: 'eyJ.e30.sig' }));
    const before = mock.requests.length;
    await assert.rejects(jwtClient.getHead(), /401 .*invalid_jwt/);
    assert.deepEqual(
      apiRequests(mock.requests.slice(before)).map((r) => r.auth),
      ['Bearer'],
    );
  });

  test('a sequencer without the key directory route yields a clear, coded failure', async () => {
    const reader = new SequencerClient(clientConfig(mock));
    const keys = await reader.getAgentSigningKeys(agent.agentId);
    assert.deepEqual(
      keys.keys.map((k) => k.keyId),
      [1],
      'the PR #6 directory verifies when served',
    );

    mock.setKeyDirectory(false);
    try {
      await assert.rejects(reader.getAgentSigningKeys(agent.agentId), {
        code: 'key_directory_unsupported',
        message: /does not expose signed agent key lists.*Supply peer public keys another way/s,
      });

      const db = new Database(':memory:');
      try {
        const engine = new SyncEngine({ db, config: engineConfig(mock, crypto.randomUUID()) });
        const directory = createPeerKeyDirectory(engine.outbox, reader, reader.config);
        const resolution = await directory.resolve(agent.agentId, 1, new Date().toISOString());
        assert.equal(resolution.error, 'key_unresolved', 'an outage, not a hard refusal');
        assert.equal(resolution.code, 'key_directory_unsupported');
        assert.match(resolution.detail, /does not expose signed agent key lists/);
      } finally {
        db.close();
      }
    } finally {
      mock.setKeyDirectory(true);
    }
  });
});

// ---------------------------------------------------------------------------
// SyncEngine: push settlement and the verified receive path
// ---------------------------------------------------------------------------

function engineConfig(mock, agentId) {
  return new SyncConfig({
    sequencer: { url: mock.url, insecure: true },
    sync: { securityProfile: 'legacy' },
    identity: { tenantId: mock.tenantId, storeId: mock.storeId, agentId },
    sequencerPublicKey: normalizeSequencerPublicKey(mock.sequencerPublicKey),
  });
}

/** A SyncEngine whose REST transport points at the mock with a plain-object config. */
async function engineOn(t, mock, count) {
  const agent = await registerAgent(mock);
  const db = new Database(':memory:');
  t.after(() => db.close());
  const engine = new SyncEngine({
    db,
    config: engineConfig(mock, agent.agentId),
    keyManager: {
      getCurrentSigningKey: async () => ({
        keyId: 1,
        publicKey: agent.publicKey,
        privateKey: agent.pair.privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32),
      }),
    },
  });
  const errors = [];
  engine.on('error', (error) => errors.push(error));
  // The one substitution (as in sync-engine-verification.test.js): the
  // engine's own UnifiedSequencerClient gets a REST client whose credentials
  // come from this test, not from STATESET_API_KEY in the environment.
  const rest = new SequencerClient(clientConfig(mock, { agentId: agent.agentId }));
  engine.client._client = rest;
  engine.client._transport = 'rest';
  for (let i = 0; i < count; i++) {
    await engine.outbox.append({
      eventId: crypto.randomUUID(),
      commandId: crypto.randomUUID(),
      tenantId: mock.tenantId,
      storeId: mock.storeId,
      sourceAgent: agent.agentId,
      entityType: 'order',
      entityId: `order-${i}`,
      eventType: 'order.created',
      payload: { amount: '19.99', line: i },
    });
  }
  return { agent, db, engine, rest, errors, events: engine.outbox.getPending() };
}

describe('SyncEngine against the VES wire format', () => {
  let mock;

  before(async () => {
    mock = await startMockSequencer();
  });

  after(() => mock?.close());

  test('a push with one rejected event settles the others and marks that one rejected', async (t) => {
    const { db, engine, events, errors } = await engineOn(t, mock, 3);
    // Signed under a key id the sequencer has never seen: agent_key_invalid.
    db.prepare('UPDATE _ves_outbox SET agent_key_id = 99 WHERE event_id = ?').run(
      events[1].eventId,
    );

    const result = await engine.push();
    assert.equal(result.success, true, errors[0]?.message);
    assert.equal(result.pushed, 2);
    assert.equal(result.rejected, 1);
    // The rejection arrived as { event_id, reason, message }.
    assert.equal(result.receipt.rejections[0].event_id, events[1].eventId);

    const [first, rejected, third] = events.map((e) => engine.outbox.getByEventId(e.eventId));
    assert.equal(first.syncStatus, 'synced');
    assert.equal(third.syncStatus, 'synced');
    assert.equal(third.remoteSequence, first.remoteSequence + 1, 'consecutive, in order');
    assert.equal(rejected.syncStatus, 'rejected');
    assert.match(rejected.rejectionReason, /^agent_key_invalid/);
    assert.equal(engine.outbox.getPendingCount(), 0, 'nothing left pending forever');
    assert.equal(engine.outbox.getSyncState().lastPushedSequence, third.remoteSequence);
  });

  test('a retry after a lost response settles replayed receipts beside new events', async (t) => {
    const { engine, rest, events, errors } = await engineOn(t, mock, 3);
    const realPush = rest.pushWithRetry.bind(rest);
    let lose = true;
    engine.client.pushWithRetry = async (batch) => {
      if (lose) {
        lose = false;
        // The sequencer sequences the first two, then the response is lost.
        await realPush({ ...batch, events: batch.events.slice(0, 2) });
        throw new Error('socket hang up');
      }
      return realPush(batch);
    };

    const lost = await engine.push();
    assert.equal(lost.success, false);
    assert.equal(engine.outbox.getPendingCount(), 3);
    const sequencedBefore = mock
      .events()
      .filter((e) => events.some((p) => p.eventId === e.event_id));
    assert.equal(sequencedBefore.length, 2);

    const retried = await engine.push();
    assert.equal(retried.success, true, errors.at(-1)?.message);
    // Two exact replays (stored receipts, counted in neither total) + one new.
    assert.equal(retried.receipt.eventsAccepted, 1);
    assert.equal(retried.receipt.eventsRejected, 0);
    assert.equal(retried.receipt.receipts.length, 3);
    assert.equal(retried.pushed, 3);
    assert.equal(retried.rejected, 0);

    const bySeq = new Map(sequencedBefore.map((e) => [e.event_id, e.sequence_number]));
    for (const event of events.slice(0, 2)) {
      const row = engine.outbox.getByEventId(event.eventId);
      assert.equal(row.syncStatus, 'synced');
      assert.equal(row.remoteSequence, bySeq.get(event.eventId), 'the replayed receipt sequence');
    }
    const third = engine.outbox.getByEventId(events[2].eventId);
    assert.equal(third.syncStatus, 'synced');
    assert.equal(third.remoteSequence, retried.receipt.sequenceStart);
    assert.equal(engine.outbox.getPendingCount(), 0);
  });

  test('a full replay (accepted 0, rejected 0) settles every event', async (t) => {
    const { engine, rest, errors } = await engineOn(t, mock, 2);
    const realPush = rest.pushWithRetry.bind(rest);
    let lose = true;
    engine.client.pushWithRetry = async (batch) => {
      const receipt = await realPush(batch);
      if (lose) {
        lose = false;
        throw new Error('response lost');
      }
      return receipt;
    };
    assert.equal((await engine.push()).success, false);
    const retried = await engine.push();
    assert.equal(retried.success, true, errors.at(-1)?.message);
    assert.equal(retried.receipt.eventsAccepted, 0);
    assert.equal(retried.pushed, 2);
    assert.equal(engine.outbox.getPendingCount(), 0);
  });

  test('pull stores events whose signatures verify through the signed key directory', async (t) => {
    const writer = await engineOn(t, mock, 2);
    assert.equal((await writer.engine.push()).success, true);

    const reader = await engineOn(t, mock, 0);
    const result = await reader.engine.pull();
    assert.equal(result.success, true, result.error);
    assert.ok(result.pulled >= 2);
    assert.equal(result.quarantined, 0, 'every pulled event verifies');
    assert.equal(result.verified, result.pulled);
    assert.equal(result.stored, result.pulled);
    const state = reader.engine.outbox.getSyncState();
    assert.equal(state.lastPulledSequence, mock.events().length + 1);
  });
});
