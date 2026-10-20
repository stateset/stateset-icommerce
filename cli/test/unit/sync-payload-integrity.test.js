import { test } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import { verifyPayloadIntegrity } from '../../src/sync/payload-integrity.js';
import {
  encryptPayload,
  encryptPayloadHybrid,
  encryptPayloadStrict,
  generateHybridRecipientKeypair,
  generateStrictRecipientKeypair,
  computePayloadPlainHash,
  computeEventSigningHash,
} from '../../src/sync/crypto.js';
import { SequencerClient } from '../../src/sync/client.js';
import { SyncConfig } from '../../src/sync/config.js';

const base = {
  vesVersion: 1,
  tenantId: '22222222-2222-2222-2222-222222222222',
  storeId: '33333333-3333-3333-3333-333333333333',
  eventId: '11111111-1111-1111-1111-111111111111',
  sourceAgent: '44444444-4444-4444-4444-444444444444',
  agentKeyId: 1,
  entityType: 'order',
  entityId: 'order-1',
  eventType: 'order.created',
  createdAt: '2026-09-12T00:00:00.123456+00:00',
};
const client = new SequencerClient(
  new SyncConfig({
    sequencer: { url: 'https://sequencer.invalid' },
    sync: { securityProfile: 'legacy' },
  }),
);

test('plaintext integrity binds exact payload and requires the zero cipher hash', () => {
  const payload = { amount: '19.99', nested: { quantity: 2 } };
  const envelope = {
    ...base,
    payload,
    payloadKind: 0,
    payloadPlainHash: computePayloadPlainHash(payload).toString('hex'),
    payloadCipherHash: '00'.repeat(32),
  };
  assert.equal(verifyPayloadIntegrity(envelope), true);
  assert.equal(
    verifyPayloadIntegrity({ ...envelope, payload: { nested: { quantity: 2 }, amount: '19.99' } }),
    true,
  );
  for (const patch of [
    { payload: { ...payload, amount: '0.01' } },
    { payload: { ...payload, amount: 19.99 } },
    { payload: undefined },
    { payloadPlainHash: '00' },
    { payloadCipherHash: 'ff'.repeat(32) },
    { payloadKind: 3 },
    { payloadEncrypted: {} },
  ])
    assert.equal(verifyPayloadIntegrity({ ...envelope, ...patch }), false);
});

for (const profile of ['legacy', 'hybrid', 'pqc-strict']) {
  test(`${profile} encrypted integrity matches real encryption and rejects wire tampering`, async () => {
    const aad = { ...base, sourceAgentId: base.sourceAgent };
    let encrypted;
    if (profile === 'legacy') {
      const { publicKey } = crypto.generateKeyPairSync('x25519');
      encrypted = encryptPayload({ amount: '19.99' }, aad, [
        { kid: 1, publicKey: publicKey.export({ type: 'spki', format: 'der' }).subarray(-32) },
      ]);
    } else if (profile === 'hybrid') {
      encrypted = encryptPayloadHybrid({ amount: '19.99' }, aad, [
        generateHybridRecipientKeypair(1),
      ]);
    } else {
      encrypted = encryptPayloadStrict({ amount: '19.99' }, aad, [
        generateStrictRecipientKeypair(1),
      ]);
    }
    const envelope = {
      ...base,
      payloadKind: 1,
      payload: {},
      payloadEncrypted: encrypted.payloadEncrypted,
      payloadPlainHash: encrypted.payloadPlainHash.toString('hex'),
      payloadCipherHash: encrypted.payloadCipherHash.toString('hex'),
    };
    const { privateKey, publicKey } = crypto.generateKeyPairSync('ed25519');
    const hash = computeEventSigningHash({
      ...aad,
      payloadKind: 1,
      payloadPlainHash: encrypted.payloadPlainHash,
      payloadCipherHash: encrypted.payloadCipherHash,
    });
    envelope.agentSignature = crypto.sign(null, hash, privateKey).toString('hex');
    const rawKey = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
    assert.equal(verifyPayloadIntegrity(envelope), true);
    assert.equal(client.verifyEventSignature(envelope, rawKey), true);
    const transport = new SequencerClient(
      new SyncConfig({
        sequencer: { url: 'https://sequencer.invalid' },
        identity: { tenantId: base.tenantId, storeId: base.storeId, agentId: base.sourceAgent },
        sync: { securityProfile: 'legacy' },
      }),
    );
    let wire;
    transport._request = async (method, _path, body) => {
      if (method === 'POST') {
        [wire] = JSON.parse(JSON.stringify(body)).events;
        return { eventsAccepted: 1, sequenceStart: 1, sequenceEnd: 1, headSequence: 1 };
      }
      return {
        events: [
          {
            envelope: { ...wire, source_agent: wire.source_agent_id, sequence_number: 1 },
            sequenced_at: base.createdAt,
          },
        ],
        head_sequence: 1,
      };
    };
    await transport.push({ agentId: base.sourceAgent, events: [envelope] });
    const received = await transport.pull(0);
    assert.equal(
      transport.verifyEventSignature(received.events[0].envelope, rawKey),
      true,
      'REST normalization must preserve content hashes and recipient aliases',
    );
    assert.equal(
      client.verifyEventSignature({ ...envelope, payload: { amount: '0.01' } }, rawKey),
      false,
    );
    const mutations = [
      (p) => {
        p.nonce_b64u = Buffer.alloc(12).toString('base64url');
      },
      (p) => {
        p.ciphertext_b64u = 'AQID';
      },
      (p) => {
        p.tag_b64u = Buffer.alloc(16).toString('base64url');
      },
      (p) => {
        p.recipients[0].ct_b64u = 'AQID';
      },
      (p) => {
        p.recipients[0].recipient_kid = 7;
      },
      (p) => {
        p.recipients = [];
      },
      (p) => {
        p.enc_version = 99;
      },
      (p) => {
        p.hpke.kem = 'unsupported';
      },
      (p) => {
        p.nonce_b64u += '!';
      },
    ];
    if (profile !== 'legacy') {
      mutations.push((p) => {
        p.recipientWraps[0].wrappedKey = 'AQID';
      });
      mutations.push((p) => {
        p.keyWrapParams.scheme = 1;
      });
    }
    for (const mutate of mutations) {
      const changed = structuredClone(envelope);
      mutate(changed.payloadEncrypted);
      assert.equal(verifyPayloadIntegrity(changed), false, mutate.toString());
      assert.equal(
        client.verifyEventSignature(changed, rawKey),
        false,
        'unchanged signature cannot authenticate mutated content',
      );
    }
  });
}
