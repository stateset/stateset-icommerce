/**
 * Unit tests for the quarantine and peer-key tables in sync/outbox.js.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';

function sampleEvent(overrides = {}) {
  return {
    sequenceNumber: 1,
    eventId: '11111111-1111-1111-1111-111111111111',
    tenantId: '22222222-2222-2222-2222-222222222222',
    storeId: '33333333-3333-3333-3333-333333333333',
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
    sequencedAt: '2026-09-12T00:00:01.000Z',
    sourceAgent: '44444444-4444-4444-4444-444444444444',
    ...overrides,
  };
}

describe('quarantine storage', () => {
  let db;
  let outbox;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('stores quarantined events separately from pulled events', () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'signature_invalid');

    assert.equal(outbox.getPulledEvents().length, 0, 'quarantined events must never reach application reads');
    const quarantined = outbox.getQuarantinedEvents();
    assert.equal(quarantined.length, 1);
    assert.equal(quarantined[0].reason, 'signature_invalid');
    assert.equal(quarantined[0].eventId, '11111111-1111-1111-1111-111111111111');
  });

  it('removes a quarantined event once it has been promoted', () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');
    outbox.deleteQuarantinedEvent('11111111-1111-1111-1111-111111111111');
    assert.equal(outbox.getQuarantinedEvents().length, 0);
  });
});

describe('peer key storage', () => {
  let db;
  let outbox;
  const agentId = '44444444-4444-4444-4444-444444444444';

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('round-trips cached peer keys', () => {
    outbox.upsertPeerKeys(
      agentId,
      [{ keyId: 1, algorithm: 'ed25519', publicKey: '0xaa', publicKeyBundle: null, validFrom: null, validTo: null, revokedAt: null }],
      '2026-09-12T00:00:00.000Z',
    );

    const keys = outbox.getPeerKeys(agentId);
    assert.equal(keys.length, 1);
    assert.equal(keys[0].keyId, 1);
    assert.equal(keys[0].publicKey, '0xaa');
    assert.equal(keys[0].fetchedAt, '2026-09-12T00:00:00.000Z');
  });

  it('records a pin and reads it back', () => {
    outbox.pinPeerKey(agentId, 1, '0xaa');
    assert.equal(outbox.getPeerKeyPin(agentId, 1).publicKey, '0xaa');
    assert.equal(outbox.getPeerKeyPin(agentId, 2), null);
  });
});
