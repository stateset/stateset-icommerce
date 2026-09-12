/**
 * `sync doctor` — inspect quarantine, inspect pins, promote events that now verify.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { syncDoctor } from '../../src/commands/sync.js';

const AGENT = '44444444-4444-4444-4444-444444444444';

function sampleEvent() {
  return {
    sequenceNumber: 1,
    eventId: '11111111-1111-1111-1111-111111111111',
    tenantId: '22222222-2222-2222-2222-222222222222',
    storeId: '33333333-3333-3333-3333-333333333333',
    entityType: 'order',
    entityId: 'ORD-1',
    eventType: 'order.created',
    vesVersion: 1,
    payload: { total: 1 },
    payloadKind: 0,
    payloadPlainHash: '0x00',
    payloadCipherHash: '0x00',
    agentKeyId: 1,
    agentSignature: '0xdead',
    baseVersion: 0,
    createdAt: '2026-09-12T00:00:00.000Z',
    sequencedAt: '2026-09-12T00:00:01.000Z',
    sourceAgent: AGENT,
  };
}

describe('syncDoctor', () => {
  let db;
  let outbox;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('summarizes quarantined events by reason', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({ outbox, client: {}, keyDirectory: {} });
    assert.deepEqual(report.quarantined, [{ reason: 'key_unresolved', count: 1 }]);
    assert.equal(report.promoted, 0);
  });

  it('promotes events that verify once the key arrives', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => true },
      keyDirectory: {
        async resolve() {
          return { publicKey: '0xaa', publicKeyBundle: null };
        },
      },
      promote: true,
    });

    assert.equal(report.promoted, 1);
    assert.equal(outbox.getPulledEvents().length, 1);
    assert.equal(outbox.getQuarantinedEvents().length, 0);
  });

  it('leaves events that still do not verify in quarantine', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'signature_invalid');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => false },
      keyDirectory: {
        async resolve() {
          return { publicKey: '0xaa', publicKeyBundle: null };
        },
      },
      promote: true,
    });

    assert.equal(report.promoted, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 1);
  });

  it('lists current pins', async () => {
    outbox.pinPeerKey(AGENT, 1, '0xaa');
    const report = await syncDoctor({ outbox, client: {}, keyDirectory: {} });
    assert.equal(report.pins.length, 1);
    assert.equal(report.pins[0].agentId, AGENT);
    assert.equal(report.pins[0].keyId, 1);
  });

  it('is read-only unless promote is true', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => true },
      keyDirectory: {
        async resolve() {
          return { publicKey: '0xaa', publicKeyBundle: null };
        },
      },
    });

    assert.equal(report.promoted, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 1);
    assert.equal(outbox.getPulledEvents().length, 0);
  });

  it('does not promote when key resolution still fails', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => true },
      keyDirectory: {
        async resolve() {
          return { error: 'key_unresolved' };
        },
      },
      promote: true,
    });

    assert.equal(report.promoted, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 1);
  });
});
