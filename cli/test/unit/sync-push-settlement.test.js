import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import Database from 'better-sqlite3';
import { SyncConfig } from '../../src/sync/config.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { SequencerClient } from '../../src/sync/client.js';

async function setup(t, count = 3) {
  const db = new Database(':memory:');
  t.after(() => db.close());
  const pair = crypto.generateKeyPairSync('ed25519');
  const config = new SyncConfig({
    sequencer: { url: 'https://sequencer.invalid' },
    sync: { securityProfile: 'legacy' },
    identity: {
      tenantId: crypto.randomUUID(),
      storeId: crypto.randomUUID(),
      agentId: crypto.randomUUID(),
    },
  });
  const engine = new SyncEngine({
    db,
    config,
    keyManager: {
      getCurrentSigningKey: async () => ({
        keyId: 1,
        publicKey: pair.publicKey.export({ type: 'spki', format: 'der' }).subarray(-32),
        privateKey: pair.privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32),
      }),
    },
  });
  engine.on('error', () => {});
  for (let i = 0; i < count; i++)
    await engine.outbox.append({
      eventId: crypto.randomUUID(),
      tenantId: config.tenantId,
      storeId: config.storeId,
      sourceAgent: config.agentId,
      entityType: 'order',
      entityId: `order-${i}`,
      eventType: 'order.created',
      payload: { amount: '19.99' },
    });
  const events = engine.outbox.getPending();
  return { db, engine, events };
}

function accepted(count = 3, start = 1) {
  return {
    eventsAccepted: count,
    eventsRejected: 0,
    sequenceStart: start,
    sequenceEnd: start + count - 1,
    headSequence: start + count - 1,
    rejections: [],
  };
}

for (const field of ['tenantId', 'storeId']) {
  for (const dryRun of [false, true]) {
    test(`engine refuses an old outbox after ${field} configuration changes (dryRun=${dryRun})`, async (t) => {
      const { db, engine } = await setup(t);
      const before = db.prepare('SELECT * FROM _ves_outbox').all();
      engine.config.identity[field] = crypto.randomUUID();
      let requests = 0;
      engine.client = {
        pushWithRetry: async () => {
          requests++;
          return accepted();
        },
      };
      const result = await engine.push({ dryRun });
      assert.equal(result.success, false);
      assert.match(result.error, /configured destination/);
      assert.equal(requests, 0);
      assert.deepEqual(db.prepare('SELECT * FROM _ves_outbox').all(), before);
    });
  }
  test(`REST rejects a mixed-scope batch before issuing any request: ${field}`, async (t) => {
    const { engine, events } = await setup(t);
    const client = new SequencerClient(engine.config);
    let requests = 0;
    client._request = async () => {
      requests++;
      return accepted();
    };
    for (const value of [crypto.randomUUID(), null, undefined]) {
      await assert.rejects(
        client.push({
          agentId: engine.config.agentId,
          events: [events[0], { ...events[1], [field]: value }],
        }),
        /configured destination/,
      );
    }
    assert.equal(requests, 0);
    const normalPush = client.push;
    client.push = async () => {
      throw new Error('retry loop must not start for a scope mismatch');
    };
    await assert.rejects(
      client.pushWithRetry({
        agentId: engine.config.agentId,
        events: [{ ...events[0], [field]: 'other-scope' }],
      }),
      /configured destination/,
    );
    client.push = normalPush;
    await client.push({ agentId: engine.config.agentId, events });
    assert.equal(requests, 1, 'matching-scope batches still send normally');
  });
}

for (const [name, bad] of [
  ['incomplete counts', () => ({ ...accepted(), eventsAccepted: 2 })],
  ['missing rejection identities', () => ({ ...accepted(2), eventsRejected: 1 })],
  [
    'foreign rejection',
    () => ({
      ...accepted(2),
      eventsRejected: 1,
      rejections: [{ eventId: 'foreign', reason: 'invalid' }],
    }),
  ],
  [
    'duplicate rejections',
    (events) => ({
      ...accepted(1),
      eventsRejected: 2,
      rejections: [1, 2].map(() => ({ eventId: events[0].eventId, reason: 'invalid' })),
    }),
  ],
  ['invalid range length', () => ({ ...accepted(), sequenceEnd: 4, headSequence: 4 })],
  ['range beyond head', () => ({ ...accepted(), headSequence: 2 })],
  ['unsafe sequence', () => ({ ...accepted(), sequenceStart: Number.MAX_SAFE_INTEGER + 1 })],
  ['fractional head', () => ({ ...accepted(), headSequence: 3.5 })],
  ['negative count', () => ({ ...accepted(), eventsRejected: -1, eventsAccepted: 4 })],
  ['coerced count', () => ({ ...accepted(), eventsAccepted: '3' })],
]) {
  test(`invalid push receipt leaves the full batch pending: ${name}`, async (t) => {
    const { db, engine, events } = await setup(t);
    const before = db.prepare('SELECT * FROM _ves_outbox').all();
    engine.client = { pushWithRetry: async () => bad(events) };
    const result = await engine.push();
    assert.equal(result.success, false);
    assert.match(result.error, /Invalid push receipt/);
    assert.deepEqual(db.prepare('SELECT * FROM _ves_outbox').all(), before);
    assert.equal(engine.outbox.getSyncState().lastPushedSequence, 0);
  });
}

test('all-rejected batches leave pending state and retain rejection reasons', async (t) => {
  const { engine, events } = await setup(t);
  engine.client = {
    pushWithRetry: async () => ({
      eventsAccepted: 0,
      eventsRejected: 3,
      sequenceStart: 0,
      sequenceEnd: 0,
      headSequence: 10,
      rejections: events.map((event) => ({ eventId: event.eventId, reason: 'invalid signature' })),
    }),
  };
  assert.equal((await engine.push()).rejected, 3);
  for (const event of events) {
    const stored = engine.outbox.getByEventId(event.eventId);
    assert.equal(stored.syncStatus, 'rejected');
    assert.equal(stored.rejectionReason, 'invalid signature');
  }
  assert.equal(engine.outbox.getPendingCount(), 0);
  assert.equal(engine.outbox.getSyncState().lastPushedSequence, 0);
});

test('partial acceptance maps accepted events in order and keeps own cursor separate from head', async (t) => {
  const { engine, events } = await setup(t);
  engine.client = {
    pushWithRetry: async () => ({
      ...accepted(2, 40),
      eventsRejected: 1,
      headSequence: 60,
      rejections: [{ eventId: events[1].eventId, reason: 'rejected' }],
    }),
  };
  assert.equal((await engine.push()).success, true);
  assert.equal(engine.outbox.getByEventId(events[0].eventId).remoteSequence, 40);
  assert.equal(engine.outbox.getByEventId(events[1].eventId).syncStatus, 'rejected');
  assert.equal(engine.outbox.getByEventId(events[2].eventId).remoteSequence, 41);
  assert.equal(engine.outbox.getSyncState().lastPushedSequence, 41);
  assert.equal(engine.outbox.getSyncState().headSequence, 60);
});

test('cursor write failure rolls accepted and rejected state back together', async (t) => {
  const { db, engine, events } = await setup(t);
  const receipt = {
    ...accepted(2),
    eventsRejected: 1,
    rejections: [{ eventId: events[1].eventId, reason: 'rejected' }],
  };
  engine.client = { pushWithRetry: async () => receipt };
  db.exec(
    "CREATE TRIGGER refuse_push_cursor BEFORE UPDATE ON _ves_sync_state WHEN NEW.key = 'last_pushed_sequence' BEGIN SELECT RAISE(ABORT, 'cursor failure'); END",
  );
  assert.equal((await engine.push()).success, false);
  assert.equal(engine.outbox.getPendingCount(), 3);
  assert.equal(engine.outbox.getSyncState().lastPushedSequence, 0);
  db.exec('DROP TRIGGER refuse_push_cursor');
  assert.equal((await engine.push()).success, true);
  assert.equal(engine.outbox.getPendingCount(), 0);
});

for (const late of ['rejected', 'same', 'conflicting']) {
  test(`overlapping push keeps durable acknowledgement when late response is ${late}`, async (t) => {
    const { engine, events } = await setup(t, 1);
    const responses = [];
    engine.client = { pushWithRetry: () => new Promise((resolve) => responses.push(resolve)) };
    const first = engine.push();
    const second = engine.push();
    assert.equal(responses.length, 2);
    responses[1]({ ...accepted(1, 10), headSequence: 20 });
    assert.equal((await second).success, true);
    responses[0](
      late === 'rejected'
        ? {
            eventsAccepted: 0,
            eventsRejected: 1,
            headSequence: 10,
            rejections: [{ eventId: events[0].eventId, reason: 'duplicate' }],
          }
        : accepted(1, late === 'same' ? 10 : 11),
    );
    assert.equal((await first).success, late !== 'conflicting');
    const stored = engine.outbox.getByEventId(events[0].eventId);
    assert.equal(stored.syncStatus, 'synced');
    assert.equal(stored.remoteSequence, 10);
    assert.equal(engine.outbox.getSyncState().headSequence, 20);
    assert.equal(engine.outbox.getSyncState().lastPushedSequence, 10);
  });
}

test('late acceptance supersedes an earlier rejection without leaving stale failure metadata', async (t) => {
  const { engine, events } = await setup(t, 1);
  const responses = [];
  engine.client = { pushWithRetry: () => new Promise((resolve) => responses.push(resolve)) };
  const first = engine.push();
  const second = engine.push();
  responses[1]({
    eventsAccepted: 0,
    eventsRejected: 1,
    headSequence: 10,
    rejections: [{ eventId: events[0].eventId, reason: 'duplicate' }],
  });
  assert.equal((await second).success, true);
  responses[0](accepted(1, 10));
  assert.equal((await first).success, true);
  const stored = engine.outbox.getByEventId(events[0].eventId);
  assert.equal(stored.syncStatus, 'synced');
  assert.equal(stored.rejectionReason, null);
  assert.equal(stored.lastError, null);
});
