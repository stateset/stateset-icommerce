import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { loadSync } from '@grpc/proto-loader';
import { GrpcSequencerClient } from '../../src/sync/grpc-client.js';
import { UnifiedSequencerClient } from '../../src/sync/unified-client.js';
import { SyncEngine } from '../../src/sync/engine.js';
import { toProtoTimestamp } from '../../src/sync/grpc-timestamp.js';
import { assertEventMatchesSecurityProfile } from '../../src/sync/pqc.js';

const definition = loadSync(
  fileURLToPath(new URL('../../src/sync/proto/sequencer_v2.proto', import.meta.url)),
  { keepCase: true, longs: String, defaults: true },
);
const stream = definition['stateset.sequencer.v2.Sequencer'].SyncStream;
const config = { url: 'localhost:50051', tenantId: 'tenant', storeId: 'store', agentId: 'agent' };

function strictEvent(overrides = {}) {
  return event({
    agentSignature: undefined,
    agentSignatureScheme: 2,
    agentSignatureBundle: { mlDsa65Signature: Buffer.alloc(3309, 3) },
    ...overrides,
  });
}

// Capture real protobuf messages at the last boundary before a network write.
function transportHarness(profile, mode) {
  const client = new GrpcSequencerClient({ ...config, securityProfile: profile });
  const writes = [];
  client.connected = true;
  client._createMetadata = () => ({});
  client.client = {
    push: (request, _metadata, callback) => {
      const rpc = definition['stateset.sequencer.v2.Sequencer'].Push;
      writes.push(rpc.requestDeserialize(rpc.requestSerialize(request)));
      callback(null, { sequence_end: 1 });
    },
  };
  client.syncStream = {
    write: (message) => {
      writes.push(stream.requestDeserialize(stream.requestSerialize(message)).push);
    },
  };
  return {
    writes,
    send: async (events) =>
      mode === 'unary' ? client.pushEvents(events) : client.pushEventsViaStream(events),
  };
}

for (const mode of ['unary', 'stream']) {
  for (const profile of ['hybrid', 'pqc-strict']) {
    const make = profile === 'hybrid' ? event : strictEvent;
    const wrapScheme = profile === 'hybrid' ? 3 : 2;
    for (const wraps of [undefined, [], {}, [null]]) {
      test(`${mode} ${profile} rejects missing/malformed recipient wraps: ${JSON.stringify(wraps)}`, async () => {
        const { writes, send } = transportHarness(profile, mode);
        await assert.rejects(
          send([
            make({
              payloadKind: 1,
              payloadEncrypted: { keyWrapParams: { scheme: wrapScheme }, recipientWraps: wraps },
            }),
          ]),
        );
        assert.equal(writes.length, 0);
      });
    }
    test(`${mode} ${profile} validates the snake_case encryption alias`, async () => {
      const { writes, send } = transportHarness(profile, mode);
      await assert.rejects(
        send([make({ payloadKind: undefined, payload_kind: 1 })]),
        /requires payloadEncrypted/,
      );
      await assert.rejects(
        send([make({ payloadKind: undefined, payload_kind: 99 })]),
        /Unsupported event payload kind/,
      );
      assert.equal(writes.length, 0);
    });
    test(`${mode} ${profile} preserves valid snake_case encrypted payloads`, async () => {
      const { writes, send } = transportHarness(profile, mode);
      await send([
        make({
          payloadKind: undefined,
          payload_kind: 1,
          payload_encrypted: {
            key_wrap_params: { scheme: wrapScheme },
            ciphertext_b64u: 'AQID',
            recipient_wraps: [
              {
                recipient_kid: 1,
                wrap_scheme: wrapScheme,
                ml_kem_ciphertext: Buffer.alloc(1088, 1),
                wrapped_key: Buffer.alloc(48, 2),
                ...(profile === 'hybrid' ? { x25519_enc: Buffer.alloc(32, 3) } : {}),
              },
            ],
          },
        }),
      ]);
      const [wire] = writes[0].events;
      assert.equal(wire.payload_kind, 2);
      assert.equal(wire.payload.length, 0);
      assert.deepEqual(wire.payload_encrypted.ciphertext, Buffer.from([1, 2, 3]));
      assert.equal(wire.payload_encrypted.recipient_wraps[0].wrap_scheme, wrapScheme);
    });
  }
  test(`${mode} preserves source author and protocol version`, async () => {
    const { writes, send } = transportHarness('hybrid', mode);
    await send([event({ sourceAgent: 'original-author', vesVersion: 7 })]);
    assert.equal(writes[0].events[0].source_agent, 'original-author');
    assert.equal(writes[0].events[0].ves_version, 7);
    assert.equal(writes[0].agent_id, config.agentId, 'batch sender remains configured agent');
  });
  test(`${mode} refuses tenant or store rerouting before sending any batch`, async () => {
    const { writes, send } = transportHarness('hybrid', mode);
    for (const field of ['tenantId', 'storeId']) {
      await assert.rejects(send([event(), event({ [field]: 'other' })]), /configured destination/);
    }
    assert.equal(writes.length, 0);
  });
  test(`${mode} strict profile never fabricates Ed25519 material`, async () => {
    const { writes, send } = transportHarness('pqc-strict', mode);
    await send([strictEvent()]);
    assert.equal(writes[0].events[0].agent_signature.length, 0);
    await assert.rejects(
      send([strictEvent({ signature: Buffer.alloc(64, 2) })]),
      /rejects Ed25519/,
    );
    assert.equal(writes.length, 1);
  });
}

for (const material of [true, 1, {}, [], '   ', Buffer.alloc(0)]) {
  test(`rejects malformed required signature material: ${JSON.stringify(material)}`, () => {
    assert.throws(
      () =>
        assertEventMatchesSecurityProfile(
          strictEvent({
            agentSignatureBundle: { mlDsa65Signature: material },
          }),
          'pqc-strict',
        ),
      /requires an ML-DSA-65 signature component/,
    );
  });
}

function event(overrides = {}) {
  return {
    eventId: 'event',
    entityType: 'order',
    entityId: 'order',
    eventType: 'order.created',
    tenantId: 'tenant',
    storeId: 'store',
    sourceAgent: 'agent',
    payload: { amount: '19.99' },
    payloadKind: 0,
    payloadPlainHash: Buffer.alloc(32, 1),
    payloadCipherHash: Buffer.alloc(32),
    agentKeyId: 7,
    agentSignature: Buffer.alloc(64, 2),
    agentSignatureScheme: 3,
    agentSignatureBundle: {
      ed25519Signature: Buffer.alloc(64, 2),
      mlDsa65Signature: Buffer.alloc(3309, 3),
    },
    createdAt: new Date('2026-01-02T03:04:05.123Z'),
    createdAtRaw: '2026-01-02T03:04:05.123456789+00:00',
    ...overrides,
  };
}

for (const [input, seconds, nanos] of [
  ['2026-01-02T03:04:05.123456789Z', 1767323045, 123456789],
  ['2026-01-02T05:04:05.123456+02:00', 1767323045, 123456000],
  ['1969-12-31T23:59:59.999999999Z', -1, 999999999],
  ['1970-01-01T00:00:00Z', 0, 0],
  [0, 0, 0],
  [-1, -1, 999000000],
  [new Date('2026-01-02T03:04:05.123Z'), 1767323045, 123000000],
  ['0001-01-01T00:00:00Z', -62135596800, 0],
  ['9999-12-31T23:59:59.999999999Z', 253402300799, 999999999],
]) {
  test(`protobuf preserves timestamp instant: ${input}`, () => {
    assert.deepEqual(toProtoTimestamp(input), { seconds, nanos });
  });
}

for (const input of [
  '',
  'nonsense',
  '2026-02-30T00:00:00Z',
  '2026-01-01T24:00:00Z',
  '2026-01-01T00:00:00.1234567890Z',
  '2026-01-01T00:00:00+24:00',
  '2026-01-01T00:00:00+00:60',
  '0000-01-01T00:00:00Z',
  '0001-01-01T00:00:00+01:00',
  '9999-12-31T23:59:59-01:00',
  NaN,
  Infinity,
  0.5,
  null,
  undefined,
  new Date(NaN),
]) {
  test(`rejects invalid timestamp: ${input}`, () => {
    assert.throws(() => toProtoTimestamp(input), /timestamp/);
  });
}

test('engine streaming preserves the signed envelope through actual protobuf serialization', () => {
  const client = new GrpcSequencerClient(config);
  const writes = [];
  client.syncStream = {
    write: (message) => writes.push(stream.requestDeserialize(stream.requestSerialize(message))),
  };
  const unified = new UnifiedSequencerClient({ config: {} });
  unified._client = client;
  unified._transport = 'grpc';
  unified._streamActive = true;
  // Exercise the real engine -> unified -> gRPC path without opening a socket.
  const engine = Object.create(SyncEngine.prototype);
  engine._streamingEnabled = true;
  engine.client = unified;
  const input = event();
  assert.equal(engine.pushViaStream([input]), true);
  assert.equal(writes.length, 1);
  const [wire] = writes[0].push.events;
  assert.equal(wire.agent_key_id, input.agentKeyId);
  assert.equal(wire.agent_signature_scheme, 3);
  assert.deepEqual(wire.agent_signature, input.agentSignature);
  assert.deepEqual(
    wire.agent_signature_bundle.ed25519_signature,
    input.agentSignatureBundle.ed25519Signature,
  );
  assert.deepEqual(
    wire.agent_signature_bundle.ml_dsa_65_signature,
    input.agentSignatureBundle.mlDsa65Signature,
  );
  assert.deepEqual(wire.payload_plain_hash, input.payloadPlainHash);
  assert.deepEqual(wire.payload_cipher_hash, input.payloadCipherHash);
  assert.deepEqual(JSON.parse(wire.payload.toString()), { amount: '19.99' });
  assert.deepEqual(wire.created_at, { seconds: '1767323045', nanos: 123456789 });
  assert.ok(input.createdAt instanceof Date, 'caller object remains unchanged');
});

for (const securityProfile of ['hybrid', 'pqc-strict']) {
  test(`${securityProfile} refuses a mixed-profile stream batch before writing`, () => {
    const client = new GrpcSequencerClient({ ...config, securityProfile });
    let writes = 0;
    client.syncStream = { write: () => writes++ };
    const valid =
      securityProfile === 'hybrid'
        ? event()
        : event({
            agentSignatureScheme: 2,
            agentSignature: null,
            agentSignatureBundle: { mlDsa65Signature: Buffer.alloc(3309, 3) },
          });
    assert.throws(
      () => client.pushEventsViaStream([valid, event({ agentSignatureScheme: 1 })]),
      /requires SIGNATURE_SCHEME/,
    );
    assert.equal(writes, 0);
    client.pushEventsViaStream([valid]);
    assert.equal(writes, 1);
  });
}

test('hybrid stream refuses legacy encrypted payloads without writing', () => {
  const client = new GrpcSequencerClient(config);
  let writes = 0;
  client.syncStream = { write: () => writes++ };
  assert.throws(
    () => client.pushEventsViaStream([event({ payloadKind: 1 })]),
    /requires payloadEncrypted/,
  );
  assert.throws(
    () =>
      client.pushEventsViaStream([
        event({ payloadKind: 1, payloadEncrypted: { recipients: [{ recipient_kid: 1 }] } }),
      ]),
    /Hybrid profile requires/,
  );
  assert.equal(writes, 0);
});

test('invalid timestamp in a batch prevents the entire stream write', () => {
  const client = new GrpcSequencerClient(config);
  let writes = 0;
  client.syncStream = { write: () => writes++ };
  assert.throws(
    () => client.pushEventsViaStream([event(), event({ createdAtRaw: 'invalid' })]),
    /timestamp/,
  );
  assert.equal(writes, 0);
});

test('engine streaming retains encrypted payload and recipient material on the wire', () => {
  const client = new GrpcSequencerClient(config);
  let wire;
  client.syncStream = {
    write: (message) => {
      [wire] = stream.requestDeserialize(stream.requestSerialize(message)).push.events;
    },
  };
  const engine = Object.create(SyncEngine.prototype);
  engine._streamingEnabled = true;
  engine.client = {
    isStreaming: () => true,
    pushViaStream: (events) => client.pushEventsViaStream(events),
  };
  const encrypted = {
    enc_version: 1,
    aead: 'AES-256-GCM',
    nonce_b64u: 'AQID',
    ciphertext_b64u: 'BAUG',
    tag_b64u: 'BwgJ',
    keyWrapParams: { scheme: 3, kdf: 'HKDF-SHA256', aead: 'AES-256-GCM' },
    recipientWraps: [
      {
        recipientKid: 4,
        wrapScheme: 3,
        x25519Enc: 'AQID',
        mlKemCiphertext: 'BAUG',
        wrappedKey: 'BwgJ',
        wrapNonce: 'CgsM',
      },
    ],
  };
  engine.pushViaStream([event({ payloadKind: 1, payloadEncrypted: encrypted })]);
  assert.equal(wire.payload_kind, 2);
  assert.equal(wire.payload.length, 0, 'plaintext is never transmitted for an encrypted event');
  assert.equal(wire.payload_encrypted.key_wrap_params.scheme, 3);
  assert.equal(wire.payload_encrypted.recipient_wraps.length, 1);
  assert.deepEqual(
    wire.payload_encrypted.recipient_wraps[0].ml_kem_ciphertext,
    Buffer.from([4, 5, 6]),
  );
  assert.deepEqual(wire.payload_encrypted.ciphertext, Buffer.from([4, 5, 6]));
});
