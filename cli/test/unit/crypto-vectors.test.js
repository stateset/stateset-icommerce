/**
 * Cross-language crypto test vectors
 *
 * These tests verify that the JS implementation in cli/src/sync/crypto.js
 * produces IDENTICAL output to the Rust implementation in
 * crates/stateset-crypto/. The expected hex values are hardcoded and
 * shared between both test suites so any drift is caught immediately.
 *
 * Counterpart: crates/stateset-crypto/tests/test_vectors.rs
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  canonicalizeJson,
  u32BE,
  u64BE,
  encodeString,
  uuidToBytes,
  bufferToHex,
  computePayloadPlainHash,
  computeLegacyPayloadHash,
  computePayloadCipherHash,
  computeStreamId,
  computePadLeaf,
  computeNodeHash,
  computeEventSigningHash,
  computePayloadAad,
  computeLeafHash,
  computeReceiptHash,
  computeRecipientsHash,
  isNativeAvailable,
  ZERO_HASH,
} from '../../src/sync/crypto.js';

// ---------------------------------------------------------------------------
// Shared constants
// ---------------------------------------------------------------------------
const TEST_UUID = '550e8400-e29b-41d4-a716-446655440000';

// ---------------------------------------------------------------------------
// 1. JCS Canonicalization
// ---------------------------------------------------------------------------
describe('cross-lang: JCS canonicalization', () => {
  it('canonicalizes null', () => {
    assert.equal(canonicalizeJson(null), 'null');
  });

  it('canonicalizes true', () => {
    assert.equal(canonicalizeJson(true), 'true');
  });

  it('canonicalizes false', () => {
    assert.equal(canonicalizeJson(false), 'false');
  });

  it('canonicalizes integer 42', () => {
    assert.equal(canonicalizeJson(42), '42');
  });

  it('canonicalizes string "hello"', () => {
    assert.equal(canonicalizeJson('hello'), '"hello"');
  });

  it('sorts object keys lexicographically', () => {
    assert.equal(canonicalizeJson({ b: 2, a: 1 }), '{"a":1,"b":2}');
  });

  it('preserves array order', () => {
    assert.equal(canonicalizeJson([3, 1, 2]), '[3,1,2]');
  });

  it('handles nested objects with sorted keys', () => {
    assert.equal(canonicalizeJson({ z: { b: 2, a: 1 }, a: [] }), '{"a":[],"z":{"a":1,"b":2}}');
  });

  it('canonicalizes {key: "value"}', () => {
    assert.equal(canonicalizeJson({ key: 'value' }), '{"key":"value"}');
  });
});

// ---------------------------------------------------------------------------
// 2. Encoding helpers
// ---------------------------------------------------------------------------
describe('cross-lang: encoding helpers', () => {
  it('u32BE(0)', () => {
    assert.equal(bufferToHex(u32BE(0)), '0x00000000');
  });

  it('u32BE(1)', () => {
    assert.equal(bufferToHex(u32BE(1)), '0x00000001');
  });

  it('u32BE(256)', () => {
    assert.equal(bufferToHex(u32BE(256)), '0x00000100');
  });

  it('u32BE(MAX_U32)', () => {
    assert.equal(bufferToHex(u32BE(4294967295)), '0xffffffff');
  });

  it('u64BE(0)', () => {
    assert.equal(bufferToHex(u64BE(0)), '0x0000000000000000');
  });

  it('u64BE(1)', () => {
    assert.equal(bufferToHex(u64BE(1)), '0x0000000000000001');
  });

  it('u64BE(42)', () => {
    assert.equal(bufferToHex(u64BE(42)), '0x000000000000002a');
  });

  it('encodeString("hello") — length-prefixed', () => {
    assert.equal(bufferToHex(encodeString('hello')), '0x0000000568656c6c6f');
  });

  it('encodeString("") — zero length', () => {
    assert.equal(bufferToHex(encodeString('')), '0x00000000');
  });

  it('uuidToBytes — standard UUID', () => {
    assert.equal(bufferToHex(uuidToBytes(TEST_UUID)), '0x550e8400e29b41d4a716446655440000');
  });
});

// ---------------------------------------------------------------------------
// 3. Domain-separated hashing (deterministic)
// ---------------------------------------------------------------------------
describe('cross-lang: payload hashing', () => {
  it('computePayloadPlainHash({key:"value"}, null)', () => {
    const hash = computePayloadPlainHash({ key: 'value' }, null);
    assert.equal(
      bufferToHex(hash),
      '0x618fdef1f66e6d7ae46216d2b7a778898e02137c502255397d277dd3c8727bca',
    );
  });

  it('computePayloadPlainHash({key:"value"}, zeros_salt)', () => {
    const hash = computePayloadPlainHash({ key: 'value' }, Buffer.alloc(16, 0));
    assert.equal(
      bufferToHex(hash),
      '0xdf9c1da34c08c2c46e3ac7d850e8b90271a02cf0e8cc4e820dead6c89e7bbdf7',
    );
  });

  it('computeLegacyPayloadHash({key:"value"})', () => {
    const hash = computeLegacyPayloadHash({ key: 'value' });
    assert.equal(
      bufferToHex(hash),
      '0xe43abcf3375244839c012f9633f95862d232a95b00d5bc7348b3098b9fed7f32',
    );
  });

  it('computePayloadCipherHash(null) returns ZERO_HASH', () => {
    const hash = computePayloadCipherHash(null);
    assert.equal(
      bufferToHex(hash),
      '0x0000000000000000000000000000000000000000000000000000000000000000',
    );
    assert.ok(hash.equals(ZERO_HASH));
  });

  it('computePayloadCipherHash with params', () => {
    const hash = computePayloadCipherHash({
      nonce: Buffer.alloc(12, 0),
      payloadAad: Buffer.alloc(32, 1),
      ciphertext: Buffer.from('encrypted_data'),
      tag: Buffer.alloc(16, 2),
      recipientsHash: Buffer.alloc(32, 3),
    });
    assert.equal(
      bufferToHex(hash),
      '0xd75837f51bbb8cbdfddc4c3838e9e183939cbe506b4ada53ac56a6878c98631c',
    );
  });
});

// ---------------------------------------------------------------------------
// 4. Stream ID
// ---------------------------------------------------------------------------
describe('cross-lang: stream ID', () => {
  it('computeStreamId with TEST_UUID', () => {
    const id = computeStreamId(TEST_UUID, TEST_UUID);
    assert.equal(
      bufferToHex(id),
      '0x399cd60b39a2c65ab8a50de811a4d2a8efa8e191961a8fbd9bcc21174b0dd731',
    );
  });
});

// ---------------------------------------------------------------------------
// 5. Merkle hashing
// ---------------------------------------------------------------------------
describe('cross-lang: Merkle hashing', () => {
  it('computePadLeaf()', () => {
    const pad = computePadLeaf();
    assert.equal(
      bufferToHex(pad),
      '0xd9dd0e003ba5370a698013c48ed69c6c41d9ebc1236d44b280c52ceacfdad524',
    );
  });

  it('computeNodeHash(all-1s, all-2s)', () => {
    const left = Buffer.alloc(32, 1);
    const right = Buffer.alloc(32, 2);
    const hash = computeNodeHash(left, right);
    assert.equal(
      bufferToHex(hash),
      '0x5186fbc7094f70b9fc71bcf269fda0530c1c2bd675de918ef39562a6f18752fd',
    );
  });
});

// ---------------------------------------------------------------------------
// 6. Event signing hash
// ---------------------------------------------------------------------------
describe('cross-lang: event signing hash', () => {
  it('deterministic with known params', () => {
    const zeroHash32 = Buffer.alloc(32, 0);
    const hash = computeEventSigningHash({
      vesVersion: 1,
      tenantId: TEST_UUID,
      storeId: TEST_UUID,
      eventId: TEST_UUID,
      sourceAgentId: TEST_UUID,
      agentKeyId: 1,
      entityType: 'order',
      entityId: 'ord_001',
      eventType: 'order.created',
      createdAt: '2026-02-21T00:00:00Z',
      payloadKind: 0,
      payloadPlainHash: zeroHash32,
      payloadCipherHash: zeroHash32,
    });
    assert.equal(
      bufferToHex(hash),
      '0xdfc1efa1fb34966a13ed60a1d92a9f8ec56d4bf9bef521ed0808bcb43d069235',
    );
  });
});

// ---------------------------------------------------------------------------
// 7. Payload AAD
// ---------------------------------------------------------------------------
describe('cross-lang: payload AAD', () => {
  it('deterministic with known params', () => {
    const zeroHash32 = Buffer.alloc(32, 0);
    const aad = computePayloadAad({
      vesVersion: 1,
      tenantId: TEST_UUID,
      storeId: TEST_UUID,
      eventId: TEST_UUID,
      sourceAgentId: TEST_UUID,
      agentKeyId: 1,
      entityType: 'order',
      entityId: 'ord_001',
      eventType: 'order.created',
      createdAt: '2026-02-21T00:00:00Z',
      payloadPlainHash: zeroHash32,
    });
    assert.equal(
      bufferToHex(aad),
      '0xcdc1245d41bb28b1e9a5c49bfd76f32c276bf6c42f6cb68cd3990df80c4e7905',
    );
  });
});

// ---------------------------------------------------------------------------
// 8. Leaf hash
// ---------------------------------------------------------------------------
describe('cross-lang: leaf hash', () => {
  it('deterministic with known params', () => {
    const zeroHash32 = Buffer.alloc(32, 0);
    const hash = computeLeafHash({
      tenantId: TEST_UUID,
      storeId: TEST_UUID,
      sequenceNumber: 1,
      eventSigningHash: zeroHash32,
      agentSignature: Buffer.alloc(64, 0),
    });
    assert.equal(
      bufferToHex(hash),
      '0x6cefa2e2572cf1223d741e18caeb5dc3732b7e1e99fbab361883acd5be63fb48',
    );
  });
});

// ---------------------------------------------------------------------------
// 9. Receipt hash
// ---------------------------------------------------------------------------
describe('cross-lang: receipt hash', () => {
  it('deterministic with known params', () => {
    const zeroHash32 = Buffer.alloc(32, 0);
    const hash = computeReceiptHash({
      tenantId: TEST_UUID,
      storeId: TEST_UUID,
      eventId: TEST_UUID,
      sequenceNumber: 42,
      eventSigningHash: zeroHash32,
    });
    assert.equal(
      bufferToHex(hash),
      '0x90be3aa44a2d74ea2688c5d583053247acdd47e2bbbd80db73b683cb7329638a',
    );
  });
});

// ---------------------------------------------------------------------------
// 10. Recipients hash
// ---------------------------------------------------------------------------
describe('cross-lang: recipients hash', () => {
  it('sorts by recipient_kid and produces deterministic hash', () => {
    const hash = computeRecipientsHash([
      { recipient_kid: 2, enc_b64u: 'a', ct_b64u: 'b' },
      { recipient_kid: 1, enc_b64u: 'c', ct_b64u: 'd' },
    ]);
    assert.equal(
      bufferToHex(hash),
      '0x9209fbf107e6f97f3fe2c4179d90e8ab7be79d1528eeaeb82b83f2b832c91d94',
    );
  });
});

// ---------------------------------------------------------------------------
// 11. Merkle root computation
// ---------------------------------------------------------------------------
import { computeMerkleRoot } from '../../src/sync/crypto.js';

describe('cross-lang: computeMerkleRoot', () => {
  it('empty tree returns pad_leaf', () => {
    const root = computeMerkleRoot([]);
    const padLeaf = computePadLeaf();
    assert.equal(bufferToHex(root), bufferToHex(padLeaf));
  });

  it('single leaf returns leaf itself', () => {
    const leaf = Buffer.alloc(32, 42);
    const root = computeMerkleRoot([leaf]);
    assert.equal(bufferToHex(root), bufferToHex(leaf));
  });

  it('two leaves = node_hash(a, b)', () => {
    const a = Buffer.alloc(32, 1);
    const b = Buffer.alloc(32, 2);
    const root = computeMerkleRoot([a, b]);
    const expected = computeNodeHash(a, b);
    assert.equal(bufferToHex(root), bufferToHex(expected));
  });

  it('three leaves pads to four', () => {
    const a = Buffer.alloc(32, 1);
    const b = Buffer.alloc(32, 2);
    const c = Buffer.alloc(32, 3);
    const pad = computePadLeaf();
    const root = computeMerkleRoot([a, b, c]);
    const expected = computeNodeHash(computeNodeHash(a, b), computeNodeHash(c, pad));
    assert.equal(bufferToHex(root), bufferToHex(expected));
  });

  it('four leaves', () => {
    const a = Buffer.alloc(32, 1);
    const b = Buffer.alloc(32, 2);
    const c = Buffer.alloc(32, 3);
    const d = Buffer.alloc(32, 4);
    const root = computeMerkleRoot([a, b, c, d]);
    const expected = computeNodeHash(computeNodeHash(a, b), computeNodeHash(c, d));
    assert.equal(bufferToHex(root), bufferToHex(expected));
  });

  it('is deterministic', () => {
    const leaves = Array.from({ length: 7 }, (_, i) => Buffer.alloc(32, i + 1));
    const root1 = computeMerkleRoot(leaves);
    const root2 = computeMerkleRoot(leaves);
    assert.equal(bufferToHex(root1), bufferToHex(root2));
  });
});

// ---------------------------------------------------------------------------
// 12. Native module status
// ---------------------------------------------------------------------------
describe('cross-lang: native module', () => {
  it('isNativeAvailable() returns a boolean', () => {
    assert.equal(typeof isNativeAvailable(), 'boolean');
  });
});
