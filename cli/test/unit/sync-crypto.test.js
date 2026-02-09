import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  DOMAIN,
  ZERO_HASH,
  u32BE,
  u64BE,
  encodeString,
  uuidToBytes,
  hexToBuffer,
  bufferToHex,
  canonicalizeJson,
  computePayloadPlainHash,
  computePayloadCipherHash,
  computeEventSigningHash,
  computeLeafHash,
  computeNodeHash,
  computePadLeaf,
  computeStreamId,
  computeReceiptHash,
} from '../../src/sync/crypto.js';

// ============================================================================
// DOMAIN constants
// ============================================================================

describe('DOMAIN constants', () => {
  it('all domain constants are buffers', () => {
    for (const [key, val] of Object.entries(DOMAIN)) {
      assert.ok(Buffer.isBuffer(val), `DOMAIN.${key} should be a Buffer`);
    }
  });

  it('has expected domain keys', () => {
    const expected = [
      'PAYLOAD_PLAIN', 'PAYLOAD_AAD', 'PAYLOAD_CIPHER',
      'RECIPIENTS', 'EVENTSIG', 'LEAF', 'NODE', 'PAD_LEAF',
      'STREAM', 'RECEIPT',
    ];
    for (const key of expected) {
      assert.ok(DOMAIN[key], `DOMAIN.${key} should exist`);
    }
  });
});

describe('ZERO_HASH', () => {
  it('is 32 bytes of zeros', () => {
    assert.equal(ZERO_HASH.length, 32);
    assert.ok(ZERO_HASH.every((b) => b === 0));
  });
});

// ============================================================================
// Encoding helpers
// ============================================================================

describe('u32BE', () => {
  it('returns a 4-byte buffer', () => {
    const buf = u32BE(0);
    assert.equal(buf.length, 4);
  });

  it('encodes 0 as all zeros', () => {
    const buf = u32BE(0);
    assert.deepEqual([...buf], [0, 0, 0, 0]);
  });

  it('encodes 1 as [0,0,0,1]', () => {
    const buf = u32BE(1);
    assert.deepEqual([...buf], [0, 0, 0, 1]);
  });

  it('encodes 256 as [0,0,1,0]', () => {
    const buf = u32BE(256);
    assert.deepEqual([...buf], [0, 0, 1, 0]);
  });

  it('encodes max 32-bit value', () => {
    const buf = u32BE(0xFFFFFFFF);
    assert.deepEqual([...buf], [255, 255, 255, 255]);
  });
});

describe('u64BE', () => {
  it('returns an 8-byte buffer', () => {
    const buf = u64BE(0);
    assert.equal(buf.length, 8);
  });

  it('encodes 0 as all zeros', () => {
    const buf = u64BE(0);
    assert.ok(buf.every((b) => b === 0));
  });

  it('encodes 1 correctly', () => {
    const buf = u64BE(1);
    assert.deepEqual([...buf], [0, 0, 0, 0, 0, 0, 0, 1]);
  });

  it('accepts bigint', () => {
    const buf = u64BE(BigInt(42));
    assert.equal(buf.length, 8);
    assert.equal(buf[7], 42);
  });
});

describe('encodeString', () => {
  it('encodes empty string with 4-byte zero prefix', () => {
    const buf = encodeString('');
    assert.equal(buf.length, 4); // just the length prefix
    assert.deepEqual([...buf], [0, 0, 0, 0]);
  });

  it('encodes string with correct length prefix', () => {
    const buf = encodeString('abc');
    assert.equal(buf.length, 4 + 3);
    // Length prefix = 3
    assert.deepEqual([...buf.subarray(0, 4)], [0, 0, 0, 3]);
    assert.equal(buf.subarray(4).toString('utf8'), 'abc');
  });

  it('handles multi-byte UTF-8', () => {
    const buf = encodeString('\u00e9'); // e-acute (2 bytes in UTF-8)
    const len = buf.readUInt32BE(0);
    assert.equal(len, 2);
  });
});

describe('uuidToBytes', () => {
  it('converts a valid UUID to 16 bytes', () => {
    const buf = uuidToBytes('550e8400-e29b-41d4-a716-446655440000');
    assert.equal(buf.length, 16);
  });

  it('throws on invalid UUID', () => {
    assert.throws(() => uuidToBytes('not-a-uuid'), /Invalid UUID/);
  });

  it('throws on too-short UUID', () => {
    assert.throws(() => uuidToBytes('1234'), /Invalid UUID/);
  });

  it('produces correct bytes for known UUID', () => {
    const buf = uuidToBytes('00000000-0000-0000-0000-000000000001');
    assert.equal(buf[15], 1);
    assert.equal(buf[0], 0);
  });
});

describe('hexToBuffer', () => {
  it('converts hex string to buffer', () => {
    const buf = hexToBuffer('deadbeef');
    assert.equal(buf.length, 4);
    assert.equal(buf[0], 0xde);
  });

  it('handles 0x prefix', () => {
    const buf = hexToBuffer('0xdeadbeef');
    assert.equal(buf.length, 4);
    assert.equal(buf[0], 0xde);
  });
});

describe('bufferToHex', () => {
  it('returns 0x-prefixed hex string', () => {
    const result = bufferToHex(Buffer.from([0xde, 0xad]));
    assert.equal(result, '0xdead');
  });

  it('round-trips with hexToBuffer', () => {
    const original = Buffer.from([0x01, 0x02, 0x03, 0x04]);
    const hex = bufferToHex(original);
    const restored = hexToBuffer(hex);
    assert.deepEqual(restored, original);
  });
});

// ============================================================================
// canonicalizeJson (RFC 8785 JCS)
// ============================================================================

describe('canonicalizeJson', () => {
  it('canonicalizes null', () => {
    assert.equal(canonicalizeJson(null), 'null');
  });

  it('canonicalizes undefined as null', () => {
    assert.equal(canonicalizeJson(undefined), 'null');
  });

  it('canonicalizes booleans', () => {
    assert.equal(canonicalizeJson(true), 'true');
    assert.equal(canonicalizeJson(false), 'false');
  });

  it('canonicalizes integers', () => {
    assert.equal(canonicalizeJson(42), '42');
    assert.equal(canonicalizeJson(-1), '-1');
    assert.equal(canonicalizeJson(0), '0');
  });

  it('canonicalizes -0 as "0"', () => {
    assert.equal(canonicalizeJson(-0), '0');
  });

  it('canonicalizes strings with escaping', () => {
    assert.equal(canonicalizeJson('hello'), '"hello"');
    assert.equal(canonicalizeJson('a"b'), '"a\\"b"');
    assert.equal(canonicalizeJson('a\\b'), '"a\\\\b"');
  });

  it('escapes control characters', () => {
    assert.equal(canonicalizeJson('\n'), '"\\n"');
    assert.equal(canonicalizeJson('\t'), '"\\t"');
    assert.equal(canonicalizeJson('\r'), '"\\r"');
  });

  it('canonicalizes arrays', () => {
    assert.equal(canonicalizeJson([1, 2, 3]), '[1,2,3]');
    assert.equal(canonicalizeJson([]), '[]');
  });

  it('sorts object keys lexicographically', () => {
    const result = canonicalizeJson({ b: 2, a: 1 });
    assert.equal(result, '{"a":1,"b":2}');
  });

  it('handles nested objects', () => {
    const result = canonicalizeJson({ z: { b: 2, a: 1 }, a: 0 });
    assert.equal(result, '{"a":0,"z":{"a":1,"b":2}}');
  });

  it('throws on Infinity', () => {
    assert.throws(() => canonicalizeJson(Infinity), /Infinity/);
  });

  it('throws on NaN', () => {
    assert.throws(() => canonicalizeJson(NaN), /Infinity|NaN/);
  });
});

// ============================================================================
// Payload hashing
// ============================================================================

describe('computePayloadPlainHash', () => {
  it('returns 32-byte buffer', () => {
    const hash = computePayloadPlainHash({ hello: 'world' });
    assert.equal(hash.length, 32);
  });

  it('is deterministic for same input', () => {
    const h1 = computePayloadPlainHash({ a: 1 });
    const h2 = computePayloadPlainHash({ a: 1 });
    assert.deepEqual(h1, h2);
  });

  it('produces different hashes for different payloads', () => {
    const h1 = computePayloadPlainHash({ a: 1 });
    const h2 = computePayloadPlainHash({ a: 2 });
    assert.notDeepEqual(h1, h2);
  });

  it('accepts optional salt', () => {
    const salt = Buffer.alloc(16, 0x42);
    const h1 = computePayloadPlainHash({ x: 1 }, salt);
    const h2 = computePayloadPlainHash({ x: 1 });
    assert.notDeepEqual(h1, h2);
  });

  it('throws on invalid salt length', () => {
    assert.throws(() => computePayloadPlainHash({ x: 1 }, Buffer.alloc(8)), /16 bytes/);
  });
});

describe('computePayloadCipherHash', () => {
  it('returns 32 zero bytes for null params (plaintext)', () => {
    const hash = computePayloadCipherHash(null);
    assert.equal(hash.length, 32);
    assert.ok(hash.every((b) => b === 0));
  });

  it('returns non-zero hash for cipher params', () => {
    const hash = computePayloadCipherHash({
      nonce: Buffer.alloc(12, 1),
      payloadAad: Buffer.alloc(32, 2),
      ciphertext: Buffer.alloc(64, 3),
      tag: Buffer.alloc(16, 4),
      recipientsHash: Buffer.alloc(32, 5),
    });
    assert.equal(hash.length, 32);
    assert.ok(!hash.every((b) => b === 0));
  });
});

// ============================================================================
// Event signing
// ============================================================================

describe('computeEventSigningHash', () => {
  const UUID = '550e8400-e29b-41d4-a716-446655440000';
  const baseParams = {
    vesVersion: 1,
    tenantId: UUID,
    storeId: UUID,
    eventId: UUID,
    sourceAgentId: UUID,
    agentKeyId: 1,
    entityType: 'order',
    entityId: 'ORD-1',
    eventType: 'created',
    createdAt: '2024-01-01T00:00:00Z',
    payloadKind: 0,
    payloadPlainHash: Buffer.alloc(32, 1),
    payloadCipherHash: Buffer.alloc(32, 0),
  };

  it('returns 32-byte buffer', () => {
    const hash = computeEventSigningHash(baseParams);
    assert.equal(hash.length, 32);
  });

  it('is deterministic', () => {
    const h1 = computeEventSigningHash(baseParams);
    const h2 = computeEventSigningHash(baseParams);
    assert.deepEqual(h1, h2);
  });

  it('changes with different entity types', () => {
    const h1 = computeEventSigningHash(baseParams);
    const h2 = computeEventSigningHash({ ...baseParams, entityType: 'customer' });
    assert.notDeepEqual(h1, h2);
  });
});

// ============================================================================
// Merkle tree operations
// ============================================================================

describe('computeLeafHash', () => {
  it('returns 32-byte buffer', () => {
    const hash = computeLeafHash({
      tenantId: '550e8400-e29b-41d4-a716-446655440000',
      storeId: '550e8400-e29b-41d4-a716-446655440000',
      sequenceNumber: 1,
      eventSigningHash: Buffer.alloc(32, 0xaa),
      agentSignature: Buffer.alloc(64, 0xbb),
    });
    assert.equal(hash.length, 32);
  });
});

describe('computeNodeHash', () => {
  it('returns 32-byte buffer', () => {
    const left = Buffer.alloc(32, 1);
    const right = Buffer.alloc(32, 2);
    const hash = computeNodeHash(left, right);
    assert.equal(hash.length, 32);
  });

  it('is order-dependent', () => {
    const a = Buffer.alloc(32, 1);
    const b = Buffer.alloc(32, 2);
    const h1 = computeNodeHash(a, b);
    const h2 = computeNodeHash(b, a);
    assert.notDeepEqual(h1, h2);
  });
});

describe('computePadLeaf', () => {
  it('returns 32-byte buffer', () => {
    const hash = computePadLeaf();
    assert.equal(hash.length, 32);
  });

  it('is deterministic', () => {
    const h1 = computePadLeaf();
    const h2 = computePadLeaf();
    assert.deepEqual(h1, h2);
  });
});

// ============================================================================
// Stream ID
// ============================================================================

describe('computeStreamId', () => {
  const UUID = '550e8400-e29b-41d4-a716-446655440000';
  const UUID2 = '660e8400-e29b-41d4-a716-446655440000';

  it('returns 32-byte buffer', () => {
    const id = computeStreamId(UUID, UUID);
    assert.equal(id.length, 32);
  });

  it('is deterministic', () => {
    const id1 = computeStreamId(UUID, UUID);
    const id2 = computeStreamId(UUID, UUID);
    assert.deepEqual(id1, id2);
  });

  it('differs for different store IDs', () => {
    const id1 = computeStreamId(UUID, UUID);
    const id2 = computeStreamId(UUID, UUID2);
    assert.notDeepEqual(id1, id2);
  });
});

// ============================================================================
// Receipt hash
// ============================================================================

describe('computeReceiptHash', () => {
  const UUID = '550e8400-e29b-41d4-a716-446655440000';

  it('returns 32-byte buffer', () => {
    const hash = computeReceiptHash({
      tenantId: UUID,
      storeId: UUID,
      eventId: UUID,
      sequenceNumber: 42,
      eventSigningHash: Buffer.alloc(32, 0xff),
    });
    assert.equal(hash.length, 32);
  });

  it('is deterministic', () => {
    const params = {
      tenantId: UUID,
      storeId: UUID,
      eventId: UUID,
      sequenceNumber: 1,
      eventSigningHash: Buffer.alloc(32, 0),
    };
    const h1 = computeReceiptHash(params);
    const h2 = computeReceiptHash(params);
    assert.deepEqual(h1, h2);
  });

  it('changes with different sequence numbers', () => {
    const base = {
      tenantId: UUID,
      storeId: UUID,
      eventId: UUID,
      eventSigningHash: Buffer.alloc(32, 0),
    };
    const h1 = computeReceiptHash({ ...base, sequenceNumber: 1 });
    const h2 = computeReceiptHash({ ...base, sequenceNumber: 2 });
    assert.notDeepEqual(h1, h2);
  });
});
