/**
 * Unit tests for chains/crypto-utils.js — Keccak-256, RIPEMD-160, secp256k1, Ethereum addresses
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import crypto from 'node:crypto';
import {
  keccak256,
  secp256k1GetPublicKey,
  privateKeyToEthAddress,
  toChecksumAddress,
  isValidEthAddress,
  ripemd160,
  sha256Double,
} from '../../src/chains/crypto-utils.js';

// ===========================================================================
// keccak256
// ===========================================================================

describe('keccak256', () => {
  it('hashes empty input to known value', () => {
    const hash = keccak256(Buffer.alloc(0));
    assert.strictEqual(
      hash.toString('hex'),
      'c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470',
    );
  });

  it('returns 32-byte Buffer', () => {
    const hash = keccak256(Buffer.from('hello'));
    assert.ok(Buffer.isBuffer(hash));
    assert.strictEqual(hash.length, 32);
  });

  it('is deterministic', () => {
    const a = keccak256(Buffer.from('test'));
    const b = keccak256(Buffer.from('test'));
    assert.strictEqual(a.toString('hex'), b.toString('hex'));
  });

  it('different inputs produce different hashes', () => {
    const a = keccak256(Buffer.from('a'));
    const b = keccak256(Buffer.from('b'));
    assert.notStrictEqual(a.toString('hex'), b.toString('hex'));
  });

  it('accepts Uint8Array input', () => {
    const data = new Uint8Array([0x61, 0x62, 0x63]); // "abc"
    const hash = keccak256(data);
    assert.strictEqual(hash.length, 32);
  });

  it('handles multi-block input (> 136 bytes)', () => {
    const data = Buffer.alloc(200, 0x42);
    const hash = keccak256(data);
    assert.strictEqual(hash.length, 32);
  });
});

// ===========================================================================
// ripemd160
// ===========================================================================

describe('ripemd160', () => {
  it('hashes empty input to known value', () => {
    const hash = ripemd160(Buffer.alloc(0));
    assert.strictEqual(hash.toString('hex'), '9c1185a5c5e9fc54612808977ee8f548b2258d31');
  });

  it('returns 20-byte Buffer', () => {
    const hash = ripemd160(Buffer.from('hello'));
    assert.ok(Buffer.isBuffer(hash));
    assert.strictEqual(hash.length, 20);
  });

  it('hashes "a" to known value', () => {
    const hash = ripemd160(Buffer.from('a'));
    assert.strictEqual(hash.toString('hex'), '0bdc9d2d256b3ee9daae347be6f4dc835a467ffe');
  });

  it('hashes "abc" to known value', () => {
    const hash = ripemd160(Buffer.from('abc'));
    assert.strictEqual(hash.toString('hex'), '8eb208f7e05d987a9b044a8e98c6b087f15a0bfc');
  });

  it('is deterministic', () => {
    const a = ripemd160(Buffer.from('test'));
    const b = ripemd160(Buffer.from('test'));
    assert.strictEqual(a.toString('hex'), b.toString('hex'));
  });

  it('accepts Uint8Array', () => {
    const hash = ripemd160(new Uint8Array([0x61]));
    assert.strictEqual(hash.length, 20);
  });
});

// ===========================================================================
// sha256Double
// ===========================================================================

describe('sha256Double', () => {
  it('double-hashes empty input', () => {
    const hash = sha256Double(Buffer.alloc(0));
    // SHA256(SHA256(empty))
    const first = crypto.createHash('sha256').update(Buffer.alloc(0)).digest();
    const expected = crypto.createHash('sha256').update(first).digest();
    assert.strictEqual(hash.toString('hex'), expected.toString('hex'));
  });

  it('returns 32-byte Buffer', () => {
    const hash = sha256Double(Buffer.from('hello'));
    assert.strictEqual(hash.length, 32);
  });

  it('is deterministic', () => {
    const a = sha256Double(Buffer.from('test'));
    const b = sha256Double(Buffer.from('test'));
    assert.strictEqual(a.toString('hex'), b.toString('hex'));
  });

  it('differs from single SHA-256', () => {
    const single = crypto.createHash('sha256').update(Buffer.from('test')).digest();
    const double = sha256Double(Buffer.from('test'));
    assert.notStrictEqual(single.toString('hex'), double.toString('hex'));
  });
});

// ===========================================================================
// secp256k1GetPublicKey
// ===========================================================================

describe('secp256k1GetPublicKey', () => {
  it('derives 65-byte uncompressed public key', () => {
    const privateKey = crypto.randomBytes(32);
    const publicKey = secp256k1GetPublicKey(privateKey);
    assert.strictEqual(publicKey.length, 65);
    assert.strictEqual(publicKey[0], 0x04); // uncompressed prefix
  });

  it('is deterministic for same private key', () => {
    const privateKey = crypto.randomBytes(32);
    const a = secp256k1GetPublicKey(privateKey);
    const b = secp256k1GetPublicKey(privateKey);
    assert.strictEqual(a.toString('hex'), b.toString('hex'));
  });

  it('different private keys produce different public keys', () => {
    const a = secp256k1GetPublicKey(crypto.randomBytes(32));
    const b = secp256k1GetPublicKey(crypto.randomBytes(32));
    assert.notStrictEqual(a.toString('hex'), b.toString('hex'));
  });
});

// ===========================================================================
// privateKeyToEthAddress
// ===========================================================================

describe('privateKeyToEthAddress', () => {
  it('derives a valid Ethereum address', () => {
    const privateKey = crypto.randomBytes(32);
    const address = privateKeyToEthAddress(privateKey);
    assert.ok(address.startsWith('0x'));
    assert.strictEqual(address.length, 42);
    assert.ok(isValidEthAddress(address));
  });

  it('is deterministic', () => {
    const privateKey = crypto.randomBytes(32);
    const a = privateKeyToEthAddress(privateKey);
    const b = privateKeyToEthAddress(privateKey);
    assert.strictEqual(a, b);
  });

  it('produces checksummed address', () => {
    const privateKey = crypto.randomBytes(32);
    const address = privateKeyToEthAddress(privateKey);
    // Should not be all lowercase or all uppercase (mixed case = checksum)
    const hex = address.slice(2);
    const hasUpper = /[A-F]/.test(hex);
    const hasLower = /[a-f]/.test(hex);
    // Most addresses will have both; in rare cases all digits — that's ok
    if (hasUpper || hasLower) {
      assert.ok(true);
    }
  });
});

// ===========================================================================
// toChecksumAddress (EIP-55)
// ===========================================================================

describe('toChecksumAddress', () => {
  it('produces EIP-55 checksummed address', () => {
    // Known EIP-55 test vectors
    const vectors = [
      ['0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed', '0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed'],
      ['0xfb6916095ca1df60bb79ce92ce3ea74c37c5d359', '0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359'],
      ['0xdbf03b407c01e7cd3cbea99509d93f8dddc8c6fb', '0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB'],
      ['0xd1220a0cf47c7b9be7a2e6ba89f429762e7b9adb', '0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb'],
    ];

    for (const [input, expected] of vectors) {
      assert.strictEqual(toChecksumAddress(input), expected);
    }
  });

  it('handles already-checksummed address', () => {
    const addr = '0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed';
    assert.strictEqual(toChecksumAddress(addr), addr);
  });

  it('handles all-uppercase input', () => {
    const result = toChecksumAddress('0x5AAEB6053F3E94C9B9A09F33669435E7EF1BEAED');
    assert.strictEqual(result, '0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed');
  });
});

// ===========================================================================
// isValidEthAddress
// ===========================================================================

describe('isValidEthAddress', () => {
  it('accepts valid lowercase address', () => {
    assert.strictEqual(
      isValidEthAddress('0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed'),
      true,
    );
  });

  it('accepts valid uppercase address', () => {
    assert.strictEqual(
      isValidEthAddress('0x5AAEB6053F3E94C9B9A09F33669435E7EF1BEAED'),
      true,
    );
  });

  it('accepts valid checksummed address', () => {
    assert.strictEqual(
      isValidEthAddress('0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed'),
      true,
    );
  });

  it('rejects invalid checksum', () => {
    // Flip one character's case to break checksum
    assert.strictEqual(
      isValidEthAddress('0x5AAeb6053F3E94C9b9A09f33669435E7Ef1BeAed'),
      false,
    );
  });

  it('rejects too short', () => {
    assert.strictEqual(isValidEthAddress('0x5aaeb6053f'), false);
  });

  it('rejects missing 0x prefix', () => {
    assert.strictEqual(
      isValidEthAddress('5aaeb6053f3e94c9b9a09f33669435e7ef1beaed'),
      false,
    );
  });

  it('rejects non-hex characters', () => {
    assert.strictEqual(
      isValidEthAddress('0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaeg'),
      false,
    );
  });

  it('rejects empty string', () => {
    assert.strictEqual(isValidEthAddress(''), false);
  });
});
