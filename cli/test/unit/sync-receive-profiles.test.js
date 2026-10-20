import { test } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import { SyncEngine } from '../../src/sync/engine.js';
import { SyncConfig } from '../../src/sync/config.js';
import { SequencerClient } from '../../src/sync/client.js';
import { syncDoctor } from '../../src/commands/sync.js';
import {
  generateHybridSigningKeypair,
  generateStrictSigningKeypair,
  signEventHashHybrid,
  signEventHashStrict,
  computeEventSigningHash,
  computePayloadPlainHash,
} from '../../src/sync/crypto.js';

const identity = {
  tenantId: '22222222-2222-2222-2222-222222222222',
  storeId: '33333333-3333-3333-3333-333333333333',
  agentId: '44444444-4444-4444-4444-444444444444',
};

// Native signing is mandatory here; missing native capabilities fail, not skip.
function signed(profile, overrides = {}) {
  const keys =
    profile === 'hybrid' ? generateHybridSigningKeypair() : generateStrictSigningKeypair();
  const payload = { total: '19.99' };
  const envelope = {
    ...identity,
    eventId: '11111111-1111-1111-1111-111111111111',
    entityType: 'order',
    entityId: 'order-1',
    eventType: 'order.created',
    sourceAgent: identity.agentId,
    agentKeyId: 1,
    vesVersion: 1,
    payloadKind: 0,
    payload,
    payloadPlainHash: computePayloadPlainHash(payload).toString('hex'),
    payloadCipherHash: Buffer.alloc(32).toString('hex'),
    createdAt: '2026-09-12T00:00:00.123456+00:00',
    agentSignatureScheme: profile === 'hybrid' ? 3 : 2,
    ...overrides,
  };
  const hash = computeEventSigningHash({
    ...envelope,
    sourceAgentId: identity.agentId,
    payloadPlainHash: Buffer.from(envelope.payloadPlainHash, 'hex'),
    payloadCipherHash: Buffer.from(envelope.payloadCipherHash, 'hex'),
  });
  const bundle =
    profile === 'hybrid'
      ? signEventHashHybrid(hash, keys)
      : { mlDsa65Signature: signEventHashStrict(hash, keys) };
  envelope.agentSignatureBundle = Object.fromEntries(
    Object.entries(bundle).map(([key, value]) => [key, value.toString('hex')]),
  );
  envelope.agentSignature = envelope.agentSignatureBundle.ed25519Signature ?? '';
  const publicKeys = { mlDsa65PublicKey: keys.mlDsa65PublicKey.toString('hex') };
  if (keys.ed25519PublicKey) publicKeys.ed25519PublicKey = keys.ed25519PublicKey.toString('hex');
  return { envelope, publicKeys };
}

for (const profile of ['hybrid', 'pqc-strict']) {
  for (const field of ['tenantId', 'storeId']) {
    test(`${profile} refuses an authentically signed event from another ${field}`, async () => {
      const db = new Database(':memory:');
      try {
        const { envelope, publicKeys } = signed(profile, {
          [field]: '99999999-9999-9999-9999-999999999999',
        });
        const config = new SyncConfig({
          sequencer: { url: 'https://sequencer.invalid' },
          identity,
          sync: { securityProfile: profile },
        });
        const client = new SequencerClient(config);
        assert.equal(
          client.verifyEventSignature(envelope, publicKeys),
          true,
          'signature is valid; routing policy must still refuse it',
        );
        const engine = new SyncEngine({ db, config });
        client.pull = async () => ({
          events: [{ envelope, sequenceNumber: 1, sequencedAt: envelope.createdAt }],
          nextSequence: 2,
          headSequence: 1,
        });
        engine.client = client;
        let keyLookups = 0;
        engine.keyDirectory = {
          resolve: async () => {
            keyLookups++;
            return { publicKeyBundle: publicKeys };
          },
        };
        const result = await engine.pull();
        assert.equal(result.success, true);
        assert.equal(result.stored, 0);
        assert.equal(result.quarantined, 1);
        assert.equal(engine.outbox.getQuarantineReason(envelope.eventId), 'scope_mismatch');
        const report = await syncDoctor({
          outbox: engine.outbox,
          client,
          keyDirectory: engine.keyDirectory,
          promote: true,
        });
        assert.equal(report.promoted, 0);
        assert.equal(engine.outbox.getPulledEvents().length, 0);
        assert.equal(keyLookups, 0);
      } finally {
        db.close();
      }
    });
  }
  for (const promotion of [false, true]) {
    test(`${profile} verifies real native signatures during ${promotion ? 'promotion' : 'pull'}`, async () => {
      const db = new Database(':memory:');
      try {
        const { envelope, publicKeys } = signed(profile);
        const config = new SyncConfig({
          sequencer: { url: 'https://sequencer.invalid' },
          identity,
          sync: { securityProfile: profile },
        });
        const engine = new SyncEngine({ db, config });
        const client = new SequencerClient(config);
        const sequencedAt = '2026-09-12T00:00:01.000Z';
        client.pull = async () => ({
          events: [{ envelope, sequenceNumber: 1, sequencedAt }],
          nextSequence: 2,
          headSequence: 1,
        });
        engine.client = client;
        // Isolate the receive policy and cryptography from key-directory trust.
        engine.keyDirectory = { resolve: async () => ({ publicKeyBundle: publicKeys }) };
        if (promotion) {
          engine.outbox.storeQuarantinedEvents(
            [{ ...envelope, sequenceNumber: 1, sequencedAt }],
            'key_unresolved',
          );
          const report = await syncDoctor({
            outbox: engine.outbox,
            client,
            keyDirectory: engine.keyDirectory,
            promote: true,
          });
          assert.equal(report.promoted, 1);
        } else {
          const result = await engine.pull();
          assert.equal(result.success, true);
          assert.equal(result.verified, 1);
          assert.equal(result.stored, 1);
        }
        assert.equal(engine.outbox.getPulledEvents().length, 1);
        assert.equal(engine.outbox.getQuarantinedEvents().length, 0);
      } finally {
        db.close();
      }
    });
  }
  test(`${profile} rejects invalid PQ signatures and unknown schemes without classical fallback`, () => {
    const { envelope, publicKeys } = signed(profile);
    const client = new SequencerClient(
      new SyncConfig({ sequencer: { url: 'https://sequencer.invalid' }, identity }),
    );
    assert.equal(client.verifyEventSignature(envelope, publicKeys), true);
    assert.equal(
      client.verifyEventSignature({ ...envelope, payload: { total: '0.01' } }, publicKeys),
      false,
    );
    const tampered = structuredClone(envelope);
    tampered.agentSignatureBundle.mlDsa65Signature = 'ff';
    assert.equal(client.verifyEventSignature(tampered, publicKeys), false);
    const missingKey = { ed25519PublicKey: publicKeys.ed25519PublicKey };
    assert.equal(client.verifyEventSignature(envelope, missingKey), false);
    assert.equal(
      client.verifyEventSignature({ ...envelope, agentSignatureScheme: 99 }, publicKeys),
      false,
    );
    assert.equal(
      client.verifyEventSignature({ ...envelope, agentSignatureBundle: null }, publicKeys),
      false,
    );
  });
}
