/**
 * Cryptographic Utilities for Blockchain Operations
 *
 * Provides Keccak256 (used by Ethereum) and secp256k1 operations.
 */

import crypto from 'crypto';

// =============================================================================
// KECCAK-256 (SHA-3 variant used by Ethereum)
// =============================================================================

/**
 * Keccak-256 constants
 */
const KECCAK_ROUNDS = 24;
const KECCAK_RC = [
  0x0000000000000001n,
  0x0000000000008082n,
  0x800000000000808an,
  0x8000000080008000n,
  0x000000000000808bn,
  0x0000000080000001n,
  0x8000000080008081n,
  0x8000000000008009n,
  0x000000000000008an,
  0x0000000000000088n,
  0x0000000080008009n,
  0x000000008000000an,
  0x000000008000808bn,
  0x800000000000008bn,
  0x8000000000008089n,
  0x8000000000008003n,
  0x8000000000008002n,
  0x8000000000000080n,
  0x000000000000800an,
  0x800000008000000an,
  0x8000000080008081n,
  0x8000000000008080n,
  0x0000000080000001n,
  0x8000000080008008n,
];

const KECCAK_ROTC = [
  1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];

const KECCAK_PILN = [
  10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

/**
 * Rotate left for 64-bit BigInt
 */
function rotl64(x, y) {
  return ((x << BigInt(y)) | (x >> BigInt(64 - y))) & 0xffffffffffffffffn;
}

/**
 * Keccak-f[1600] permutation
 */
function keccakF(state) {
  const bc = new Array(5);

  for (let round = 0; round < KECCAK_ROUNDS; round++) {
    // Theta
    for (let i = 0; i < 5; i++) {
      bc[i] = state[i] ^ state[i + 5] ^ state[i + 10] ^ state[i + 15] ^ state[i + 20];
    }

    for (let i = 0; i < 5; i++) {
      const t = bc[(i + 4) % 5] ^ rotl64(bc[(i + 1) % 5], 1);
      for (let j = 0; j < 25; j += 5) {
        state[j + i] ^= t;
      }
    }

    // Rho and Pi
    let t = state[1];
    for (let i = 0; i < 24; i++) {
      const j = KECCAK_PILN[i];
      bc[0] = state[j];
      state[j] = rotl64(t, KECCAK_ROTC[i]);
      t = bc[0];
    }

    // Chi
    for (let j = 0; j < 25; j += 5) {
      for (let i = 0; i < 5; i++) {
        bc[i] = state[j + i];
      }
      for (let i = 0; i < 5; i++) {
        state[j + i] ^= ~bc[(i + 1) % 5] & bc[(i + 2) % 5];
      }
    }

    // Iota
    state[0] ^= KECCAK_RC[round];
  }
}

/**
 * Keccak-256 hash function
 * @param {Buffer|Uint8Array} input - Data to hash
 * @returns {Buffer} 32-byte hash
 */
export function keccak256(input) {
  const data = Buffer.isBuffer(input) ? input : Buffer.from(input);

  // Rate for Keccak-256: 1088 bits = 136 bytes
  const rate = 136;

  // Initialize state (25 x 64-bit = 1600 bits)
  const state = new Array(25).fill(0n);

  // Absorb phase
  let offset = 0;
  while (offset < data.length) {
    const blockSize = Math.min(rate, data.length - offset);

    // XOR block into state
    for (let i = 0; i < blockSize; i++) {
      const stateIdx = Math.floor(i / 8);
      const byteIdx = i % 8;
      state[stateIdx] ^= BigInt(data[offset + i]) << BigInt(byteIdx * 8);
    }

    offset += blockSize;

    // Apply permutation if we have a full block or this is the last block
    if (blockSize === rate) {
      keccakF(state);
    }
  }

  // Padding (Keccak uses 0x01...0x80 padding)
  const lastBlockSize = data.length % rate;
  const padIdx = lastBlockSize;

  // Add padding
  state[Math.floor(padIdx / 8)] ^= 0x01n << BigInt((padIdx % 8) * 8);
  state[Math.floor((rate - 1) / 8)] ^= 0x80n << BigInt(((rate - 1) % 8) * 8);

  // Final permutation
  keccakF(state);

  // Squeeze phase - extract 256 bits (32 bytes)
  const output = Buffer.alloc(32);
  for (let i = 0; i < 32; i++) {
    const stateIdx = Math.floor(i / 8);
    const byteIdx = i % 8;
    output[i] = Number((state[stateIdx] >> BigInt(byteIdx * 8)) & 0xffn);
  }

  return output;
}

// =============================================================================
// SECP256K1 PUBLIC KEY DERIVATION
// =============================================================================

/**
 * Derive secp256k1 public key from private key using Node.js ECDH
 * @param {Buffer} privateKey - 32-byte private key
 * @returns {Buffer} 65-byte uncompressed public key (04 || x || y)
 */
export function secp256k1GetPublicKey(privateKey) {
  // Use ECDH to derive public key
  const ecdh = crypto.createECDH('secp256k1');
  ecdh.setPrivateKey(privateKey);

  // Get uncompressed public key (65 bytes: 04 prefix + 32-byte x + 32-byte y)
  return ecdh.getPublicKey();
}

/**
 * Derive Ethereum address from private key
 * Address = last 20 bytes of Keccak256(public_key[1:])
 *
 * @param {Buffer} privateKey - 32-byte private key
 * @returns {string} Ethereum address with 0x prefix and checksum
 */
export function privateKeyToEthAddress(privateKey) {
  // Get uncompressed public key (65 bytes)
  const publicKey = secp256k1GetPublicKey(privateKey);

  // Hash public key (excluding 0x04 prefix) with Keccak256
  const pubKeyHash = keccak256(publicKey.subarray(1));

  // Take last 20 bytes as address
  const addressBytes = pubKeyHash.subarray(-20);
  const addressHex = addressBytes.toString('hex');

  // Apply EIP-55 checksum
  return toChecksumAddress('0x' + addressHex);
}

/**
 * Convert address to EIP-55 checksum format
 * @param {string} address - Lowercase hex address with 0x prefix
 * @returns {string} Checksummed address
 */
export function toChecksumAddress(address) {
  const addr = address.toLowerCase().replace('0x', '');
  const hash = keccak256(Buffer.from(addr, 'utf8')).toString('hex');

  let checksummed = '0x';
  for (let i = 0; i < 40; i++) {
    if (parseInt(hash[i], 16) >= 8) {
      checksummed += addr[i].toUpperCase();
    } else {
      checksummed += addr[i];
    }
  }

  return checksummed;
}

/**
 * Validate Ethereum address format and optional checksum
 * @param {string} address - Address to validate
 * @returns {boolean} True if valid
 */
export function isValidEthAddress(address) {
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) {
    return false;
  }

  // If mixed case (EIP-55 checksum), verify checksum. Ignore the "0x" prefix when
  // checking case because `toUpperCase()` would convert it to "0X".
  const hex = address.slice(2);
  if (hex !== hex.toLowerCase() && hex !== hex.toUpperCase()) {
    return toChecksumAddress(address) === address;
  }

  return true;
}

// =============================================================================
// RIPEMD-160 (Used by Bitcoin/Zcash for address derivation)
// =============================================================================

/**
 * RIPEMD-160 constants
 */
const RIPEMD160_KL = [0x00000000, 0x5a827999, 0x6ed9eba1, 0x8f1bbcdc, 0xa953fd4e];
const RIPEMD160_KR = [0x50a28be6, 0x5c4dd124, 0x6d703ef3, 0x7a6d76e9, 0x00000000];

const RIPEMD160_RL = [
  [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
  [7, 4, 13, 1, 10, 6, 15, 3, 12, 0, 9, 5, 2, 14, 11, 8],
  [3, 10, 14, 4, 9, 15, 8, 1, 2, 7, 0, 6, 13, 11, 5, 12],
  [1, 9, 11, 10, 0, 8, 12, 4, 13, 3, 7, 15, 14, 5, 6, 2],
  [4, 0, 5, 9, 7, 12, 2, 10, 14, 1, 3, 8, 11, 6, 15, 13],
];

const RIPEMD160_RR = [
  [5, 14, 7, 0, 9, 2, 11, 4, 13, 6, 15, 8, 1, 10, 3, 12],
  [6, 11, 3, 7, 0, 13, 5, 10, 14, 15, 8, 12, 4, 9, 1, 2],
  [15, 5, 1, 3, 7, 14, 6, 9, 11, 8, 12, 2, 10, 0, 4, 13],
  [8, 6, 4, 1, 3, 11, 15, 0, 5, 12, 2, 13, 9, 7, 10, 14],
  [12, 15, 10, 4, 1, 5, 8, 7, 6, 2, 13, 14, 0, 3, 9, 11],
];

const RIPEMD160_SL = [
  [11, 14, 15, 12, 5, 8, 7, 9, 11, 13, 14, 15, 6, 7, 9, 8],
  [7, 6, 8, 13, 11, 9, 7, 15, 7, 12, 15, 9, 11, 7, 13, 12],
  [11, 13, 6, 7, 14, 9, 13, 15, 14, 8, 13, 6, 5, 12, 7, 5],
  [11, 12, 14, 15, 14, 15, 9, 8, 9, 14, 5, 6, 8, 6, 5, 12],
  [9, 15, 5, 11, 6, 8, 13, 12, 5, 12, 13, 14, 11, 8, 5, 6],
];

const RIPEMD160_SR = [
  [8, 9, 9, 11, 13, 15, 15, 5, 7, 7, 8, 11, 14, 14, 12, 6],
  [9, 13, 15, 7, 12, 8, 9, 11, 7, 7, 12, 7, 6, 15, 13, 11],
  [9, 7, 15, 11, 8, 6, 6, 14, 12, 13, 5, 14, 13, 13, 7, 5],
  [15, 5, 8, 11, 14, 14, 6, 14, 6, 9, 12, 9, 12, 5, 15, 8],
  [8, 5, 12, 9, 12, 5, 14, 6, 8, 13, 6, 5, 15, 13, 11, 11],
];

/**
 * Rotate left for 32-bit unsigned integer
 */
function rotl32(x, n) {
  return ((x << n) | (x >>> (32 - n))) >>> 0;
}

/**
 * RIPEMD-160 round function
 */
function ripemd160Round(j, x, y, z) {
  if (j < 16) return (x ^ y ^ z) >>> 0;
  if (j < 32) return ((x & y) | (~x & z)) >>> 0;
  if (j < 48) return ((x | ~y) ^ z) >>> 0;
  if (j < 64) return ((x & z) | (y & ~z)) >>> 0;
  return (x ^ (y | ~z)) >>> 0;
}

/**
 * RIPEMD-160 hash function
 * @param {Buffer|Uint8Array} input - Data to hash
 * @returns {Buffer} 20-byte hash
 */
export function ripemd160(input) {
  const data = Buffer.isBuffer(input) ? input : Buffer.from(input);

  // Initial hash values
  let h0 = 0x67452301;
  let h1 = 0xefcdab89;
  let h2 = 0x98badcfe;
  let h3 = 0x10325476;
  let h4 = 0xc3d2e1f0;

  // Pre-processing: adding padding bits
  const msgLen = data.length;
  const bitLen = BigInt(msgLen) * 8n;

  // Pad message to 64-byte boundary (512 bits)
  const padLen = msgLen % 64 < 56 ? 56 - (msgLen % 64) : 120 - (msgLen % 64);
  const padded = Buffer.alloc(msgLen + padLen + 8);
  data.copy(padded);
  padded[msgLen] = 0x80;

  // Append length in bits (little-endian 64-bit)
  padded.writeBigUInt64LE(bitLen, msgLen + padLen);

  // Process each 64-byte block
  for (let offset = 0; offset < padded.length; offset += 64) {
    const block = padded.subarray(offset, offset + 64);

    // Parse block into 16 32-bit little-endian words
    const X = new Array(16);
    for (let i = 0; i < 16; i++) {
      X[i] = block.readUInt32LE(i * 4);
    }

    // Initialize working variables
    let al = h0,
      bl = h1,
      cl = h2,
      dl = h3,
      el = h4;
    let ar = h0,
      br = h1,
      cr = h2,
      dr = h3,
      er = h4;

    // 80 rounds
    for (let j = 0; j < 80; j++) {
      const round = Math.floor(j / 16);

      // Left line
      const fl = ripemd160Round(j, bl, cl, dl);
      const rl = RIPEMD160_RL[round][j % 16];
      const sl = RIPEMD160_SL[round][j % 16];
      let tl = (al + fl + X[rl] + RIPEMD160_KL[round]) >>> 0;
      tl = (rotl32(tl, sl) + el) >>> 0;
      al = el;
      el = dl;
      dl = rotl32(cl, 10);
      cl = bl;
      bl = tl;

      // Right line
      const fr = ripemd160Round(79 - j, br, cr, dr);
      const rr = RIPEMD160_RR[round][j % 16];
      const sr = RIPEMD160_SR[round][j % 16];
      let tr = (ar + fr + X[rr] + RIPEMD160_KR[round]) >>> 0;
      tr = (rotl32(tr, sr) + er) >>> 0;
      ar = er;
      er = dr;
      dr = rotl32(cr, 10);
      cr = br;
      br = tr;
    }

    // Update hash values
    const t = (h1 + cl + dr) >>> 0;
    h1 = (h2 + dl + er) >>> 0;
    h2 = (h3 + el + ar) >>> 0;
    h3 = (h4 + al + br) >>> 0;
    h4 = (h0 + bl + cr) >>> 0;
    h0 = t;
  }

  // Produce final hash (little-endian)
  const output = Buffer.alloc(20);
  output.writeUInt32LE(h0, 0);
  output.writeUInt32LE(h1, 4);
  output.writeUInt32LE(h2, 8);
  output.writeUInt32LE(h3, 12);
  output.writeUInt32LE(h4, 16);

  return output;
}

// =============================================================================
// SHA256 DOUBLE HASH (Used for Bitcoin/Zcash checksums)
// =============================================================================

/**
 * Double SHA256 hash (SHA256(SHA256(data)))
 * Used for address checksum calculation in Bitcoin/Zcash
 * @param {Buffer|Uint8Array} input - Data to hash
 * @returns {Buffer} 32-byte hash
 */
export function sha256Double(input) {
  const first = crypto.createHash('sha256').update(input).digest();
  return crypto.createHash('sha256').update(first).digest();
}

// =============================================================================
// EXPORTS
// =============================================================================

export default {
  keccak256,
  secp256k1GetPublicKey,
  privateKeyToEthAddress,
  toChecksumAddress,
  isValidEthAddress,
  ripemd160,
  sha256Double,
};
