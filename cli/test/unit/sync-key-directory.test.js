/**
 * Unit tests for sync/key-directory.js — peer key resolution, TTL, pinning.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { createPeerKeyDirectory } from '../../src/sync/key-directory.js';

const AGENT = '44444444-4444-4444-4444-444444444444';

function stubClient(keys) {
  return {
    calls: 0,
    async getAgentSigningKeys() {
      this.calls += 1;
      return { agentId: AGENT, keys, signedAt: new Date().toISOString() };
    },
  };
}

function key(overrides = {}) {
  return {
    keyId: 1,
    algorithm: 'ed25519',
    publicKey: '0xaa',
    publicKeyBundle: null,
    validFrom: null,
    validTo: null,
    revokedAt: null,
    ...overrides,
  };
}

describe('PeerKeyDirectory', () => {
  let db;
  let outbox;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('resolves a key and pins it on first use', async () => {
    const client = stubClient([key()]);
    const dir = createPeerKeyDirectory(outbox, client, {});

    const resolved = await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.publicKey, '0xaa');
    assert.equal(outbox.getPeerKeyPin(AGENT, 1).publicKey, '0xaa');
  });

  it('serves from cache within the TTL', async () => {
    const client = stubClient([key()]);
    const dir = createPeerKeyDirectory(outbox, client, { peerKeyTtlSeconds: 300 });

    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(client.calls, 1, 'a second resolve inside the TTL must not refetch');
  });

  it('refuses a changed key for an already-pinned key_id', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), {});
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');

    // Same key_id, different key material — the sequencer is lying or compromised.
    const attacker = createPeerKeyDirectory(outbox, stubClient([key({ publicKey: '0xbb' })]), {
      peerKeyTtlSeconds: 0,
    });
    const resolved = await attacker.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'peer_key_conflict');
  });

  it('accepts a new key_id as rotation', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), {});
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');

    const rotated = createPeerKeyDirectory(
      outbox,
      stubClient([key(), key({ keyId: 2, publicKey: '0xbb' })]),
      { peerKeyTtlSeconds: 0 },
    );
    const resolved = await rotated.resolve(AGENT, 2, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.publicKey, '0xbb');
    assert.equal(outbox.getPeerKeyPin(AGENT, 2).publicKey, '0xbb');
  });

  it('rejects an event created outside the key validity window', async () => {
    const dir = createPeerKeyDirectory(
      outbox,
      stubClient([key({ validFrom: '2026-09-01T00:00:00.000Z', validTo: '2026-09-10T00:00:00.000Z' })]),
      {},
    );
    const resolved = await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'key_outside_validity_window');
  });

  it('rejects an event signed after the key was revoked', async () => {
    const dir = createPeerKeyDirectory(
      outbox,
      stubClient([key({ revokedAt: '2026-09-11T00:00:00.000Z' })]),
      {},
    );
    const resolved = await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'key_revoked');
  });

  it('reports key_unresolved when the directory has no such key', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), {});
    const resolved = await dir.resolve(AGENT, 9, '2026-09-12T00:00:00.000Z');
    assert.equal(resolved.error, 'key_unresolved');
  });

  it('serves stale cache when refresh fails, then gives up past the stale limit', async () => {
    const dir = createPeerKeyDirectory(outbox, stubClient([key()]), { peerKeyTtlSeconds: 0 });
    await dir.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');

    const failing = createPeerKeyDirectory(
      outbox,
      { async getAgentSigningKeys() { throw new Error('unreachable'); } },
      { peerKeyTtlSeconds: 0, peerKeyMaxStaleSeconds: 86400 },
    );
    const stillOk = await failing.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z');
    assert.equal(stillOk.publicKey, '0xaa', 'a directory outage must not stop verification');

    const tooStale = createPeerKeyDirectory(
      outbox,
      { async getAgentSigningKeys() { throw new Error('unreachable'); } },
      { peerKeyTtlSeconds: 0, peerKeyMaxStaleSeconds: 0 },
    );
    assert.equal((await tooStale.resolve(AGENT, 1, '2026-09-12T00:00:00.000Z')).error, 'key_unresolved');
  });
});
