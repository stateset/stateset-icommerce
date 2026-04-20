/**
 * VES v1.0 Cryptographic Operations
 *
 * Implements:
 * - RFC 8785 JSON Canonicalization Scheme (JCS)
 * - Domain-separated hashing per VES spec
 * - Ed25519 signing for agent signatures
 * - Payload encryption (VES-ENC-1)
 *
 * When @stateset/embedded native module is available, performance-critical
 * functions (JCS canonicalization, Ed25519 signing/verification, AES-GCM,
 * Merkle root) are delegated to the Rust implementation for ~5-10x speedup.
 */

import crypto from 'crypto';

// =============================================================================
// Native Module (optional — falls back to JS)
// =============================================================================

/**
 * @typedef {{
 *   jcsCanonicalize?: (json: string) => string,
 *   jcs_canonicalize?: (json: string) => string,
 *   ed25519Sign?: (hash: Buffer, privateKey: Buffer) => Buffer,
 *   ed25519_sign?: (hash: Buffer, privateKey: Buffer) => Buffer,
 *   ed25519Verify?: (hash: Buffer, signature: Buffer, publicKey: Buffer) => boolean,
 *   ed25519_verify?: (hash: Buffer, signature: Buffer, publicKey: Buffer) => boolean,
 *   vesHybridGenerateSigningKeypair?: () => {
 *     ed25519PublicKey?: Buffer,
 *     ed25519_public_key?: Buffer,
 *     ed25519PrivateKey?: Buffer,
 *     ed25519_private_key?: Buffer,
 *     mlDsa65PublicKey?: Buffer,
 *     ml_dsa_65_public_key?: Buffer,
 *     mlDsa65Seed?: Buffer,
 *     ml_dsa_65_seed?: Buffer,
 *   },
 *   ves_hybrid_generate_signing_keypair?: () => {
 *     ed25519PublicKey?: Buffer,
 *     ed25519_public_key?: Buffer,
 *     ed25519PrivateKey?: Buffer,
 *     ed25519_private_key?: Buffer,
 *     mlDsa65PublicKey?: Buffer,
 *     ml_dsa_65_public_key?: Buffer,
 *     mlDsa65Seed?: Buffer,
 *     ml_dsa_65_seed?: Buffer,
 *   },
 *   vesHybridSignEventHash?: (hash: Buffer, ed25519PrivateKey: Buffer, mlDsa65Seed: Buffer) => {
 *     ed25519Signature?: Buffer,
 *     ed25519_signature?: Buffer,
 *     mlDsa65Signature?: Buffer,
 *     ml_dsa_65_signature?: Buffer,
 *   },
 *   ves_hybrid_sign_event_hash?: (hash: Buffer, ed25519PrivateKey: Buffer, mlDsa65Seed: Buffer) => {
 *     ed25519Signature?: Buffer,
 *     ed25519_signature?: Buffer,
 *     mlDsa65Signature?: Buffer,
 *     ml_dsa_65_signature?: Buffer,
 *   },
 *   vesHybridVerifyEventSignature?: (
 *     hash: Buffer,
 *     ed25519Signature: Buffer,
 *     mlDsa65Signature: Buffer,
 *     ed25519PublicKey: Buffer,
 *     mlDsa65PublicKey: Buffer,
 *   ) => boolean,
 *   ves_hybrid_verify_event_signature?: (
 *     hash: Buffer,
 *     ed25519Signature: Buffer,
 *     mlDsa65Signature: Buffer,
 *     ed25519PublicKey: Buffer,
 *     mlDsa65PublicKey: Buffer,
 *   ) => boolean,
 *   vesHybridGenerateRecipientKeypair?: (kid: number) => {
 *     kid?: number,
 *     x25519PublicKey?: Buffer,
 *     x25519_public_key?: Buffer,
 *     x25519PrivateKey?: Buffer,
 *     x25519_private_key?: Buffer,
 *     mlKem768PublicKey?: Buffer,
 *     ml_kem_768_public_key?: Buffer,
 *     mlKem768Seed?: Buffer,
 *     ml_kem_768_seed?: Buffer,
 *   },
 *   ves_hybrid_generate_recipient_keypair?: (kid: number) => {
 *     kid?: number,
 *     x25519PublicKey?: Buffer,
 *     x25519_public_key?: Buffer,
 *     x25519PrivateKey?: Buffer,
 *     x25519_private_key?: Buffer,
 *     mlKem768PublicKey?: Buffer,
 *     ml_kem_768_public_key?: Buffer,
 *     mlKem768Seed?: Buffer,
 *     ml_kem_768_seed?: Buffer,
 *   },
 *   vesHybridEncryptPayload?: (payloadJson: string, aadParams: any, recipients: any[]) => {
 *     payloadEncryptedJson?: string,
 *     payload_encrypted_json?: string,
 *     salt?: Buffer,
 *     payloadPlainHash?: Buffer,
 *     payload_plain_hash?: Buffer,
 *     payloadCipherHash?: Buffer,
 *     payload_cipher_hash?: Buffer,
 *   },
 *   ves_hybrid_encrypt_payload?: (payloadJson: string, aadParams: any, recipients: any[]) => {
 *     payloadEncryptedJson?: string,
 *     payload_encrypted_json?: string,
 *     salt?: Buffer,
 *     payloadPlainHash?: Buffer,
 *     payload_plain_hash?: Buffer,
 *     payloadCipherHash?: Buffer,
 *     payload_cipher_hash?: Buffer,
 *   },
 *   vesHybridDecryptPayload?: (
 *     payloadEncryptedJson: string,
 *     payloadAad: Buffer,
 *     recipientKid: number,
 *     recipientPrivateKey: {
 *       x25519PrivateKey?: Buffer,
 *       x25519_private_key?: Buffer,
 *       mlKem768Seed?: Buffer,
 *       ml_kem_768_seed?: Buffer,
 *     },
 *     expectedPlainHash: Buffer,
 *   ) => string,
 *   ves_hybrid_decrypt_payload?: (
 *     payloadEncryptedJson: string,
 *     payloadAad: Buffer,
 *     recipientKid: number,
 *     recipientPrivateKey: {
 *       x25519PrivateKey?: Buffer,
 *       x25519_private_key?: Buffer,
 *       mlKem768Seed?: Buffer,
 *       ml_kem_768_seed?: Buffer,
 *     },
 *     expectedPlainHash: Buffer,
 *   ) => string,
 *   merkleRoot?: (leaves: Buffer[]) => Buffer | Uint8Array,
 *   merkle_root?: (leaves: Buffer[]) => Buffer | Uint8Array,
 *   vesStrictGenerateSigningKeypair?: (...args: any[]) => any,
 *   ves_strict_generate_signing_keypair?: (...args: any[]) => any,
 *   vesStrictSignEventHash?: (...args: any[]) => any,
 *   ves_strict_sign_event_hash?: (...args: any[]) => any,
 *   vesStrictVerifyEventSignature?: (...args: any[]) => any,
 *   ves_strict_verify_event_signature?: (...args: any[]) => any,
 *   vesStrictGenerateRecipientKeypair?: (...args: any[]) => any,
 *   ves_strict_generate_recipient_keypair?: (...args: any[]) => any,
 *   vesStrictEncryptPayload?: (...args: any[]) => any,
 *   ves_strict_encrypt_payload?: (...args: any[]) => any,
 *   vesStrictDecryptPayload?: (...args: any[]) => any,
 *   ves_strict_decrypt_payload?: (...args: any[]) => any,
 *   vesHybridGenerateSigningPop?: (...args: any[]) => any,
 *   ves_hybrid_generate_signing_pop?: (...args: any[]) => any,
 *   vesHybridVerifySigningPop?: (...args: any[]) => any,
 *   ves_hybrid_verify_signing_pop?: (...args: any[]) => any,
 *   vesStrictGenerateSigningPop?: (...args: any[]) => any,
 *   ves_strict_generate_signing_pop?: (...args: any[]) => any,
 *   vesStrictVerifySigningPop?: (...args: any[]) => any,
 *   ves_strict_verify_signing_pop?: (...args: any[]) => any,
 * }} NativeCryptoCompat
 */

/** @type {NativeCryptoCompat | null} */
let _native = null;

/**
 * @param {unknown} error
 * @returns {string}
 */
function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

try {
  _native = /** @type {NativeCryptoCompat} */ (
    /** @type {unknown} */ (await import('@stateset/embedded'))
  );
} catch (nativeErr) {
  console.debug(
    'native crypto module not available, using JS fallback:',
    messageFromError(nativeErr),
  );
}

/**
 * @returns {((json: string) => string) | null}
 */
function getNativeJcsCanonicalize() {
  return _native?.jcsCanonicalize || _native?.jcs_canonicalize || null;
}

/**
 * @returns {((hash: Buffer, privateKey: Buffer) => Buffer) | null}
 */
function getNativeEd25519Sign() {
  return _native?.ed25519Sign || _native?.ed25519_sign || null;
}

/**
 * @returns {((hash: Buffer, signature: Buffer, publicKey: Buffer) => boolean) | null}
 */
function getNativeEd25519Verify() {
  return _native?.ed25519Verify || _native?.ed25519_verify || null;
}

/**
 * @returns {((leaves: Buffer[]) => Buffer | Uint8Array) | null}
 */
function getNativeMerkleRoot() {
  return _native?.merkleRoot || _native?.merkle_root || null;
}

function getNativeHybridGenerateSigningKeypair() {
  return (
    _native?.vesHybridGenerateSigningKeypair || _native?.ves_hybrid_generate_signing_keypair || null
  );
}

function getNativeHybridSignEventHash() {
  return _native?.vesHybridSignEventHash || _native?.ves_hybrid_sign_event_hash || null;
}

function getNativeHybridVerifyEventSignature() {
  return (
    _native?.vesHybridVerifyEventSignature || _native?.ves_hybrid_verify_event_signature || null
  );
}

function getNativeHybridGenerateRecipientKeypair() {
  return (
    _native?.vesHybridGenerateRecipientKeypair ||
    _native?.ves_hybrid_generate_recipient_keypair ||
    null
  );
}

function getNativeHybridEncryptPayload() {
  return _native?.vesHybridEncryptPayload || _native?.ves_hybrid_encrypt_payload || null;
}

function getNativeHybridDecryptPayload() {
  return _native?.vesHybridDecryptPayload || _native?.ves_hybrid_decrypt_payload || null;
}

function getNativeStrictGenerateSigningKeypair() {
  return (
    _native?.vesStrictGenerateSigningKeypair || _native?.ves_strict_generate_signing_keypair || null
  );
}

function getNativeStrictSignEventHash() {
  return _native?.vesStrictSignEventHash || _native?.ves_strict_sign_event_hash || null;
}

function getNativeStrictVerifyEventSignature() {
  return (
    _native?.vesStrictVerifyEventSignature || _native?.ves_strict_verify_event_signature || null
  );
}

function getNativeStrictGenerateRecipientKeypair() {
  return (
    _native?.vesStrictGenerateRecipientKeypair ||
    _native?.ves_strict_generate_recipient_keypair ||
    null
  );
}

function getNativeStrictEncryptPayload() {
  return _native?.vesStrictEncryptPayload || _native?.ves_strict_encrypt_payload || null;
}

function getNativeStrictDecryptPayload() {
  return _native?.vesStrictDecryptPayload || _native?.ves_strict_decrypt_payload || null;
}

function getNativeHybridGenerateSigningPop() {
  return _native?.vesHybridGenerateSigningPop || _native?.ves_hybrid_generate_signing_pop || null;
}

function getNativeHybridVerifySigningPop() {
  return _native?.vesHybridVerifySigningPop || _native?.ves_hybrid_verify_signing_pop || null;
}

function getNativeStrictGenerateSigningPop() {
  return _native?.vesStrictGenerateSigningPop || _native?.ves_strict_generate_signing_pop || null;
}

function getNativeStrictVerifySigningPop() {
  return _native?.vesStrictVerifySigningPop || _native?.ves_strict_verify_signing_pop || null;
}

/**
 * @param {unknown} value
 * @returns {Buffer}
 */
function toBuffer(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  if (typeof value === 'string') {
    return Buffer.from(value);
  }
  if (Array.isArray(value)) {
    return Buffer.from(value);
  }
  return Buffer.alloc(0);
}

/**
 * @param {unknown} value
 * @returns {Buffer}
 */
function toBinaryInput(value) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  if (typeof value === 'string') {
    return hexToBuffer(value);
  }
  if (Array.isArray(value)) {
    return Buffer.from(value);
  }
  return Buffer.alloc(0);
}

/**
 * @param {any} value
 * @param {string} camelCase
 * @param {string} snakeCase
 * @returns {unknown}
 */
function readHybridField(value, camelCase, snakeCase) {
  return value?.[camelCase] ?? value?.[snakeCase] ?? null;
}

// =============================================================================
// Domain Separation Prefixes (must match sequencer)
// =============================================================================

export const DOMAIN = {
  PAYLOAD_PLAIN: Buffer.from('VES_PAYLOAD_PLAIN_V1'),
  PAYLOAD_AAD: Buffer.from('VES_PAYLOAD_AAD_V1'),
  PAYLOAD_CIPHER: Buffer.from('VES_PAYLOAD_CIPHER_V1'),
  RECIPIENTS: Buffer.from('VES_RECIPIENTS_V1'),
  EVENTSIG: Buffer.from('VES_EVENTSIG_V1'),
  LEAF: Buffer.from('VES_LEAF_V1'),
  NODE: Buffer.from('VES_NODE_V1'),
  PAD_LEAF: Buffer.from('VES_PAD_LEAF_V1'),
  STREAM: Buffer.from('VES_STREAM_V1'),
  RECEIPT: Buffer.from('VES_RECEIPT_V1'),
};

/**
 * Zero hash (32 bytes of 0x00) - used for plaintext payloads in cipher hash field
 */
export const ZERO_HASH = Buffer.alloc(32, 0);

// =============================================================================
// Encoding Helpers
// =============================================================================

/**
 * Encode u32 as big-endian bytes
 * @param {number} n
 * @returns {Buffer}
 */
export function u32BE(n) {
  const buf = Buffer.alloc(4);
  buf.writeUInt32BE(n >>> 0);
  return buf;
}

/**
 * Encode u64 as big-endian bytes
 * @param {number|bigint} n
 * @returns {Buffer}
 */
export function u64BE(n) {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64BE(BigInt(n));
  return buf;
}

/**
 * Encode string with length prefix (VES ENC_STR)
 * @param {string} s
 * @returns {Buffer}
 */
export function encodeString(s) {
  const strBuf = Buffer.from(s, 'utf8');
  return Buffer.concat([u32BE(strBuf.length), strBuf]);
}

/**
 * Convert UUID string to 16-byte buffer
 * @param {string} uuid
 * @returns {Buffer}
 */
export function uuidToBytes(uuid) {
  const hex = uuid.replace(/-/g, '');
  if (hex.length !== 32) {
    throw new Error(`Invalid UUID: ${uuid}`);
  }
  return Buffer.from(hex, 'hex');
}

/**
 * Convert hex string (with or without 0x prefix) to buffer
 * @param {string} hex
 * @returns {Buffer}
 */
export function hexToBuffer(hex) {
  if (hex.startsWith('0x')) hex = hex.slice(2);
  return Buffer.from(hex, 'hex');
}

/**
 * Convert buffer to hex string with 0x prefix
 * @param {Buffer} buf
 * @returns {string}
 */
export function bufferToHex(buf) {
  return '0x' + buf.toString('hex');
}

// =============================================================================
// JSON Canonicalization (RFC 8785 JCS)
// =============================================================================

/**
 * Canonicalize a number per RFC 8785
 * @param {number} n
 * @returns {string}
 */
function canonicalizeNumber(n) {
  if (!Number.isFinite(n)) {
    throw new Error('JCS does not support Infinity or NaN');
  }
  if (Object.is(n, -0)) return '0';
  if (Number.isInteger(n) && Math.abs(n) < Number.MAX_SAFE_INTEGER) {
    return n.toString();
  }
  // Use exponential notation for very large/small numbers
  const s = n.toString();
  if (s.includes('e')) {
    // Normalize exponential notation
    const [mantissa, exp] = s.split('e');
    const expNum = parseInt(exp, 10);
    return `${mantissa}e${expNum >= 0 ? '+' : ''}${expNum}`;
  }
  return s;
}

/**
 * Escape a string for JSON per RFC 8785
 * @param {string} s
 * @returns {string}
 */
function escapeString(s) {
  let result = '"';
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c === 0x22)
      result += '\\"'; // "
    else if (c === 0x5c)
      result += '\\\\'; // \
    else if (c < 0x20) {
      // Control characters
      if (c === 0x08) result += '\\b';
      else if (c === 0x09) result += '\\t';
      else if (c === 0x0a) result += '\\n';
      else if (c === 0x0c) result += '\\f';
      else if (c === 0x0d) result += '\\r';
      else result += '\\u' + c.toString(16).padStart(4, '0');
    } else {
      result += s[i];
    }
  }
  return result + '"';
}

/**
 * Canonicalize JSON value per RFC 8785 JCS
 *
 * Uses native Rust implementation when available for ~5x speedup.
 * @param {any} value
 * @returns {string}
 */
export function canonicalizeJson(value) {
  const nativeJcsCanonicalize = getNativeJcsCanonicalize();
  if (nativeJcsCanonicalize) {
    try {
      return nativeJcsCanonicalize(JSON.stringify(value));
    } catch (nativeErr) {
      console.debug('native crypto call failed, using JS fallback:', messageFromError(nativeErr));
    }
  }
  if (value === null) return 'null';
  if (value === undefined) return 'null';

  const type = typeof value;

  if (type === 'boolean') return value ? 'true' : 'false';
  if (type === 'number') return canonicalizeNumber(value);
  if (type === 'string') return escapeString(value);

  if (Array.isArray(value)) {
    const items = value.map((v) => canonicalizeJson(v));
    return '[' + items.join(',') + ']';
  }

  if (type === 'object') {
    // Sort keys lexicographically by UTF-16 code units
    const keys = Object.keys(value).sort();
    const pairs = keys.map((k) => escapeString(k) + ':' + canonicalizeJson(value[k]));
    return '{' + pairs.join(',') + '}';
  }

  throw new Error(`Cannot canonicalize type: ${type}`);
}

// =============================================================================
// Payload Hashing
// =============================================================================

/**
 * Compute payload_plain_hash per VES v1.0 Section 5.2
 * @param {Object} payload - JSON payload
 * @param {Buffer | null} [salt] - Optional 16-byte salt for encrypted payloads
 * @returns {Buffer} - 32-byte hash
 */
export function computePayloadPlainHash(payload, salt = null) {
  const canonical = canonicalizeJson(payload);
  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.PAYLOAD_PLAIN);
  if (salt) {
    if (salt.length !== 16) throw new Error('Salt must be 16 bytes');
    hasher.update(salt);
  }
  hasher.update(canonical);
  return hasher.digest();
}

/**
 * Compute legacy payload hash (no domain prefix)
 * Used for compatibility with legacy gRPC API that uses EventEnvelope
 * @param {Object} payload - JSON payload
 * @returns {Buffer} - 32-byte hash
 */
export function computeLegacyPayloadHash(payload) {
  const canonical = canonicalizeJson(payload);
  const hasher = crypto.createHash('sha256');
  hasher.update(canonical);
  return hasher.digest();
}

/**
 * @typedef {{
 *   nonce: Buffer,
 *   payloadAad: Buffer,
 *   ciphertext: Buffer,
 *   tag: Buffer,
 *   recipientsHash: Buffer,
 * }} PayloadCipherHashParams
 */

/**
 * Compute payload_cipher_hash per VES v1.0 Section 5.3
 * For plaintext events, returns 32 zero bytes
 * @param {PayloadCipherHashParams | null} params - Encryption parameters (null for plaintext)
 * @returns {Buffer} - 32-byte hash
 */
export function computePayloadCipherHash(params = null) {
  if (!params) {
    return Buffer.alloc(32); // 32 zero bytes for plaintext
  }

  const { nonce, payloadAad, ciphertext, tag, recipientsHash } = params;

  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.PAYLOAD_CIPHER);
  hasher.update(nonce); // 12 bytes
  hasher.update(payloadAad); // 32 bytes
  hasher.update(ciphertext); // variable
  hasher.update(tag); // 16 bytes
  hasher.update(recipientsHash); // 32 bytes

  return hasher.digest();
}

// =============================================================================
// Event Signing
// =============================================================================

/**
 * @typedef {Object} EventSigningParams
 * @property {number} vesVersion
 * @property {string} tenantId - UUID string
 * @property {string} storeId - UUID string
 * @property {string} eventId - UUID string
 * @property {string} sourceAgentId - UUID string
 * @property {number} agentKeyId
 * @property {string} entityType
 * @property {string} entityId
 * @property {string} eventType
 * @property {string} createdAt - RFC 3339 timestamp
 * @property {number} payloadKind - 0 for plaintext, 1 for encrypted
 * @property {Buffer} payloadPlainHash - 32 bytes
 * @property {Buffer} payloadCipherHash - 32 bytes
 */

/**
 * Compute event signing hash per VES v1.0 Section 6.2
 * @param {EventSigningParams} params
 * @returns {Buffer} - 32-byte hash
 */
export function computeEventSigningHash(params) {
  const hasher = crypto.createHash('sha256');

  hasher.update(DOMAIN.EVENTSIG);
  hasher.update(u32BE(params.vesVersion));
  hasher.update(uuidToBytes(params.tenantId));
  hasher.update(uuidToBytes(params.storeId));
  hasher.update(uuidToBytes(params.eventId));
  hasher.update(uuidToBytes(params.sourceAgentId));
  hasher.update(u32BE(params.agentKeyId));
  hasher.update(encodeString(params.entityType));
  hasher.update(encodeString(params.entityId));
  hasher.update(encodeString(params.eventType));
  hasher.update(encodeString(params.createdAt));
  hasher.update(u32BE(params.payloadKind));
  hasher.update(params.payloadPlainHash);
  hasher.update(params.payloadCipherHash);

  return hasher.digest();
}

/**
 * Sign an event signing hash with Ed25519
 *
 * Uses native Rust implementation when available.
 * @param {Buffer} eventSigningHash - 32-byte hash to sign
 * @param {Buffer} privateKey - 32-byte Ed25519 private key (seed)
 * @returns {Buffer} - 64-byte signature
 */
export function signEventHash(eventSigningHash, privateKey) {
  const nativeEd25519Sign = getNativeEd25519Sign();
  if (nativeEd25519Sign) {
    try {
      return nativeEd25519Sign(eventSigningHash, privateKey);
    } catch (nativeErr) {
      console.debug('native crypto call failed, using JS fallback:', messageFromError(nativeErr));
    }
  }
  // Create key object from raw 32-byte seed
  const keyObj = crypto.createPrivateKey({
    key: Buffer.concat([
      // PKCS#8 Ed25519 private key header
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      privateKey,
    ]),
    format: 'der',
    type: 'pkcs8',
  });

  return crypto.sign(null, eventSigningHash, keyObj);
}

/**
 * Verify an event signature
 *
 * Uses native Rust implementation when available.
 * @param {Buffer} eventSigningHash - 32-byte hash that was signed
 * @param {Buffer} signature - 64-byte Ed25519 signature
 * @param {Buffer} publicKey - 32-byte Ed25519 public key
 * @returns {boolean}
 */
export function verifyEventSignature(eventSigningHash, signature, publicKey) {
  const nativeEd25519Verify = getNativeEd25519Verify();
  if (nativeEd25519Verify) {
    try {
      return nativeEd25519Verify(eventSigningHash, signature, publicKey);
    } catch (nativeErr) {
      console.debug('native crypto call failed, using JS fallback:', messageFromError(nativeErr));
    }
  }
  try {
    // Create key object from raw 32-byte public key
    const keyObj = crypto.createPublicKey({
      key: Buffer.concat([
        // SPKI Ed25519 public key header
        Buffer.from('302a300506032b6570032100', 'hex'),
        publicKey,
      ]),
      format: 'der',
      type: 'spki',
    });

    return crypto.verify(null, eventSigningHash, keyObj, signature);
  } catch (err) {
    console.debug('[sync-crypto] Signature verification failed:', messageFromError(err));
    return false;
  }
}

/**
 * Check whether native hybrid PQC helpers are available.
 * @returns {boolean}
 */
export function hasNativeHybridPqcSupport() {
  return Boolean(
    getNativeHybridGenerateSigningKeypair() &&
    getNativeHybridSignEventHash() &&
    getNativeHybridGenerateRecipientKeypair() &&
    getNativeHybridEncryptPayload(),
  );
}

/**
 * Check whether native hybrid PQC verification is available.
 * @returns {boolean}
 */
export function hasNativeHybridPqcVerificationSupport() {
  return Boolean(getNativeHybridVerifyEventSignature());
}

/**
 * Check whether native hybrid PQC payload decryption is available.
 * @returns {boolean}
 */
export function hasNativeHybridPqcDecryptionSupport() {
  return Boolean(getNativeHybridDecryptPayload());
}

/**
 * Generate a hybrid Ed25519 + ML-DSA-65 signing keypair using native Rust bindings.
 * @returns {{
 *   ed25519PublicKey: Buffer,
 *   ed25519PrivateKey: Buffer,
 *   mlDsa65PublicKey: Buffer,
 *   mlDsa65Seed: Buffer,
 * }}
 */
export function generateHybridSigningKeypair() {
  const nativeFn = getNativeHybridGenerateSigningKeypair();
  if (!nativeFn) {
    throw new Error('Hybrid PQC signing key generation requires native @stateset/embedded support');
  }

  const result = nativeFn();
  return {
    ed25519PublicKey: toBuffer(readHybridField(result, 'ed25519PublicKey', 'ed25519_public_key')),
    ed25519PrivateKey: toBuffer(
      readHybridField(result, 'ed25519PrivateKey', 'ed25519_private_key'),
    ),
    mlDsa65PublicKey: toBuffer(readHybridField(result, 'mlDsa65PublicKey', 'ml_dsa_65_public_key')),
    mlDsa65Seed: toBuffer(readHybridField(result, 'mlDsa65Seed', 'ml_dsa_65_seed')),
  };
}

/**
 * Sign an event hash with the hybrid Ed25519 + ML-DSA-65 profile.
 * @param {Buffer} eventSigningHash
 * @param {{ed25519PrivateKey: Buffer, mlDsa65Seed: Buffer}} privateKeyBundle
 * @returns {{ed25519Signature: Buffer, mlDsa65Signature: Buffer}}
 */
export function signEventHashHybrid(eventSigningHash, privateKeyBundle) {
  const nativeFn = getNativeHybridSignEventHash();
  if (!nativeFn) {
    throw new Error('Hybrid PQC signing requires native @stateset/embedded support');
  }

  const result = nativeFn(
    eventSigningHash,
    privateKeyBundle.ed25519PrivateKey,
    privateKeyBundle.mlDsa65Seed,
  );
  return {
    ed25519Signature: toBuffer(readHybridField(result, 'ed25519Signature', 'ed25519_signature')),
    mlDsa65Signature: toBuffer(readHybridField(result, 'mlDsa65Signature', 'ml_dsa_65_signature')),
  };
}

/**
 * Verify a hybrid Ed25519 + ML-DSA-65 signature bundle.
 * @param {Buffer} eventSigningHash
 * @param {{ed25519Signature: Buffer | string, mlDsa65Signature: Buffer | string}} signatureBundle
 * @param {{ed25519PublicKey: Buffer | string, mlDsa65PublicKey: Buffer | string}} publicKeyBundle
 * @returns {boolean}
 */
export function verifyEventSignatureHybrid(eventSigningHash, signatureBundle, publicKeyBundle) {
  const nativeFn = getNativeHybridVerifyEventSignature();
  if (!nativeFn) {
    throw new Error('Hybrid PQC signature verification requires native @stateset/embedded support');
  }

  const ed25519Signature = readHybridField(
    signatureBundle,
    'ed25519Signature',
    'ed25519_signature',
  );
  const mlDsa65Signature = readHybridField(
    signatureBundle,
    'mlDsa65Signature',
    'ml_dsa_65_signature',
  );
  const ed25519PublicKey = readHybridField(
    publicKeyBundle,
    'ed25519PublicKey',
    'ed25519_public_key',
  );
  const mlDsa65PublicKey = readHybridField(
    publicKeyBundle,
    'mlDsa65PublicKey',
    'ml_dsa_65_public_key',
  );

  if (!ed25519Signature || !mlDsa65Signature || !ed25519PublicKey || !mlDsa65PublicKey) {
    return false;
  }

  return nativeFn(
    eventSigningHash,
    toBinaryInput(ed25519Signature),
    toBinaryInput(mlDsa65Signature),
    toBinaryInput(ed25519PublicKey),
    toBinaryInput(mlDsa65PublicKey),
  );
}

/**
 * Generate a hybrid X25519 + ML-KEM-768 recipient keypair using native Rust bindings.
 * @param {number} kid
 * @returns {{
 *   kid: number,
 *   x25519PublicKey: Buffer,
 *   x25519PrivateKey: Buffer,
 *   mlKem768PublicKey: Buffer,
 *   mlKem768Seed: Buffer,
 * }}
 */
export function generateHybridRecipientKeypair(kid) {
  const nativeFn = getNativeHybridGenerateRecipientKeypair();
  if (!nativeFn) {
    throw new Error(
      'Hybrid PQC recipient key generation requires native @stateset/embedded support',
    );
  }

  const result = nativeFn(kid);
  return {
    kid: result.kid ?? kid,
    x25519PublicKey: toBuffer(readHybridField(result, 'x25519PublicKey', 'x25519_public_key')),
    x25519PrivateKey: toBuffer(readHybridField(result, 'x25519PrivateKey', 'x25519_private_key')),
    mlKem768PublicKey: toBuffer(
      readHybridField(result, 'mlKem768PublicKey', 'ml_kem_768_public_key'),
    ),
    mlKem768Seed: toBuffer(readHybridField(result, 'mlKem768Seed', 'ml_kem_768_seed')),
  };
}

/**
 * Encrypt a payload with hybrid X25519 + ML-KEM-768 recipient wrapping.
 * @param {Object} payload
 * @param {PayloadAadParams} aadParams
 * @param {Array<{kid: number, x25519PublicKey: Buffer, mlKem768PublicKey: Buffer}>} recipientKeys
 * @returns {EncryptionResult}
 */
export function encryptPayloadHybrid(payload, aadParams, recipientKeys) {
  const nativeFn = getNativeHybridEncryptPayload();
  if (!nativeFn) {
    throw new Error('Hybrid PQC payload encryption requires native @stateset/embedded support');
  }
  if (recipientKeys.length === 0) {
    throw new Error('At least one recipient required');
  }

  const payloadPlainHash = computePayloadPlainHash(payload);
  const result = nativeFn(
    JSON.stringify(payload),
    {
      vesVersion: aadParams.vesVersion,
      tenantId: aadParams.tenantId,
      storeId: aadParams.storeId,
      eventId: aadParams.eventId,
      sourceAgentId: aadParams.sourceAgentId,
      agentKeyId: aadParams.agentKeyId,
      entityType: aadParams.entityType,
      entityId: aadParams.entityId,
      eventType: aadParams.eventType,
      createdAt: aadParams.createdAt,
      payloadPlainHash,
    },
    recipientKeys.map((recipient) => ({
      kid: recipient.kid,
      x25519PublicKey: recipient.x25519PublicKey,
      mlKem768PublicKey: recipient.mlKem768PublicKey,
    })),
  );

  const payloadEncrypted = JSON.parse(
    result.payloadEncryptedJson ?? result.payload_encrypted_json ?? '{}',
  );
  const recipientEntries = Array.isArray(payloadEncrypted.recipients)
    ? /** @type {any[]} */ (payloadEncrypted.recipients)
    : [];
  payloadEncrypted.keyWrapParams = {
    scheme: 3,
    kdf: 'HKDF-SHA256',
    aead: 'AES-256-GCM',
  };
  payloadEncrypted.recipientWraps = recipientEntries.map((recipient) => ({
    recipientKid: recipient.recipient_kid,
    wrapScheme: 3,
    x25519Enc: recipient.x25519_enc_b64u ?? null,
    mlKemCiphertext: recipient.mlkem_ct_b64u ?? null,
    wrapNonce: recipient.wrap_nonce_b64u ?? null,
    wrappedKey: recipient.ct_b64u ?? null,
  }));

  return {
    payloadEncrypted,
    salt: toBuffer(result.salt),
    payloadPlainHash: toBuffer(result.payloadPlainHash ?? result.payload_plain_hash),
    payloadCipherHash: toBuffer(result.payloadCipherHash ?? result.payload_cipher_hash),
  };
}

/**
 * Decrypt a payload previously encrypted by {@link encryptPayloadHybrid}.
 * @param {PayloadEncryptedStructure & { recipientWraps?: Array<Object>, keyWrapParams?: Object }} payloadEncrypted
 * @param {Buffer | string} payloadAad
 * @param {number} recipientKid
 * @param {{x25519PrivateKey: Buffer | string, mlKem768Seed: Buffer | string}} recipientPrivateKeyBundle
 * @param {Buffer | string} expectedPlainHash
 * @returns {unknown}
 */
export function decryptPayloadHybrid(
  payloadEncrypted,
  payloadAad,
  recipientKid,
  recipientPrivateKeyBundle,
  expectedPlainHash,
) {
  const nativeFn = getNativeHybridDecryptPayload();
  if (!nativeFn) {
    throw new Error('Hybrid PQC payload decryption requires native @stateset/embedded support');
  }

  const x25519PrivateKey = readHybridField(
    recipientPrivateKeyBundle,
    'x25519PrivateKey',
    'x25519_private_key',
  );
  const mlKem768Seed = readHybridField(
    recipientPrivateKeyBundle,
    'mlKem768Seed',
    'ml_kem_768_seed',
  );

  if (!x25519PrivateKey || !mlKem768Seed) {
    throw new Error('Hybrid recipient private key bundle requires X25519 and ML-KEM-768 material');
  }

  return JSON.parse(
    nativeFn(
      JSON.stringify(payloadEncrypted),
      toBinaryInput(payloadAad),
      recipientKid,
      {
        x25519PrivateKey: toBinaryInput(x25519PrivateKey),
        mlKem768Seed: toBinaryInput(mlKem768Seed),
      },
      toBinaryInput(expectedPlainHash),
    ),
  );
}

// =============================================================================
// PQC-Strict Operations (ML-DSA-65 only, ML-KEM-768 only)
// =============================================================================

/**
 * Generate an ML-DSA-65-only signing keypair for PQC-strict mode.
 * @returns {{ mlDsa65PublicKey: Buffer, mlDsa65Seed: Buffer }}
 */
export function generateStrictSigningKeypair() {
  const nativeFn = getNativeStrictGenerateSigningKeypair();
  if (!nativeFn) {
    throw new Error('PQC-strict signing key generation requires native @stateset/embedded support');
  }
  const result = nativeFn();
  return {
    mlDsa65PublicKey: toBuffer(readHybridField(result, 'mlDsa65PublicKey', 'ml_dsa_65_public_key')),
    mlDsa65Seed: toBuffer(readHybridField(result, 'mlDsa65Seed', 'ml_dsa_65_seed')),
  };
}

/**
 * Sign an event hash with ML-DSA-65 only (PQC-strict mode).
 * @param {Buffer} eventSigningHash
 * @param {{ mlDsa65Seed: Buffer }} privateKeyBundle
 * @returns {Buffer} ML-DSA-65 signature bytes
 */
export function signEventHashStrict(eventSigningHash, privateKeyBundle) {
  const nativeFn = getNativeStrictSignEventHash();
  if (!nativeFn) {
    throw new Error('PQC-strict signing requires native @stateset/embedded support');
  }
  return toBuffer(nativeFn(eventSigningHash, privateKeyBundle.mlDsa65Seed));
}

/**
 * Verify an ML-DSA-65-only event signature (PQC-strict mode).
 * @param {Buffer} eventSigningHash
 * @param {Buffer} mlDsa65Signature
 * @param {{ mlDsa65PublicKey: Buffer }} publicKeyBundle
 * @returns {boolean}
 */
export function verifyEventSignatureStrict(eventSigningHash, mlDsa65Signature, publicKeyBundle) {
  const nativeFn = getNativeStrictVerifyEventSignature();
  if (!nativeFn) {
    throw new Error('PQC-strict signature verification requires native @stateset/embedded support');
  }
  const pk = readHybridField(publicKeyBundle, 'mlDsa65PublicKey', 'ml_dsa_65_public_key');
  if (!pk || !mlDsa65Signature) {
    return false;
  }
  return nativeFn(eventSigningHash, toBinaryInput(mlDsa65Signature), toBinaryInput(pk));
}

/**
 * Generate an ML-KEM-768-only recipient keypair for PQC-strict mode.
 * @param {number} kid
 * @returns {{ kid: number, mlKem768PublicKey: Buffer, mlKem768Seed: Buffer }}
 */
export function generateStrictRecipientKeypair(kid) {
  const nativeFn = getNativeStrictGenerateRecipientKeypair();
  if (!nativeFn) {
    throw new Error(
      'PQC-strict recipient key generation requires native @stateset/embedded support',
    );
  }
  const result = nativeFn(kid);
  return {
    kid: result.kid,
    mlKem768PublicKey: toBuffer(
      readHybridField(result, 'mlKem768PublicKey', 'ml_kem_768_public_key'),
    ),
    mlKem768Seed: toBuffer(readHybridField(result, 'mlKem768Seed', 'ml_kem_768_seed')),
  };
}

/**
 * Encrypt a payload with ML-KEM-768-only recipient wrapping (PQC-strict mode).
 * @param {Object} payload
 * @param {PayloadAadParams} aadParams
 * @param {Array<{kid: number, mlKem768PublicKey: Buffer}>} recipientKeys
 * @returns {EncryptionResult}
 */
export function encryptPayloadStrict(payload, aadParams, recipientKeys) {
  const nativeFn = getNativeStrictEncryptPayload();
  if (!nativeFn) {
    throw new Error('PQC-strict payload encryption requires native @stateset/embedded support');
  }
  if (recipientKeys.length === 0) {
    throw new Error('At least one recipient required');
  }

  const payloadPlainHash = computePayloadPlainHash(payload);
  const result = nativeFn(
    JSON.stringify(payload),
    {
      vesVersion: aadParams.vesVersion,
      tenantId: aadParams.tenantId,
      storeId: aadParams.storeId,
      eventId: aadParams.eventId,
      sourceAgentId: aadParams.sourceAgentId,
      agentKeyId: aadParams.agentKeyId,
      entityType: aadParams.entityType,
      entityId: aadParams.entityId,
      eventType: aadParams.eventType,
      createdAt: aadParams.createdAt,
      payloadPlainHash,
    },
    recipientKeys.map((r) => ({
      kid: r.kid,
      mlKem768PublicKey: r.mlKem768PublicKey,
    })),
  );

  const payloadEncrypted = JSON.parse(
    result.payloadEncryptedJson ?? result.payload_encrypted_json ?? '{}',
  );
  const recipientEntries = Array.isArray(payloadEncrypted.recipients)
    ? /** @type {any[]} */ (payloadEncrypted.recipients)
    : [];
  payloadEncrypted.keyWrapParams = {
    scheme: 2, // KEY_WRAP_SCHEME_ML_KEM_768
    kdf: 'HKDF-SHA256',
    aead: 'AES-256-GCM',
  };
  payloadEncrypted.recipientWraps = recipientEntries.map((recipient) => ({
    recipientKid: recipient.recipient_kid,
    wrapScheme: 2,
    mlKemCiphertext: recipient.mlkem_ct_b64u ?? null,
    wrapNonce: recipient.wrap_nonce_b64u ?? null,
    wrappedKey: recipient.ct_b64u ?? null,
  }));

  return {
    payloadEncrypted,
    salt: toBuffer(result.salt),
    payloadPlainHash: toBuffer(result.payloadPlainHash ?? result.payload_plain_hash),
    payloadCipherHash: toBuffer(result.payloadCipherHash ?? result.payload_cipher_hash),
  };
}

/**
 * Decrypt a payload encrypted with ML-KEM-768-only wrapping (PQC-strict mode).
 * @param {Object} payloadEncrypted
 * @param {Buffer | string} payloadAad
 * @param {number} recipientKid
 * @param {{ mlKem768Seed: Buffer | string }} recipientPrivateKeyBundle
 * @param {Buffer | string} expectedPlainHash
 * @returns {unknown}
 */
export function decryptPayloadStrict(
  payloadEncrypted,
  payloadAad,
  recipientKid,
  recipientPrivateKeyBundle,
  expectedPlainHash,
) {
  const nativeFn = getNativeStrictDecryptPayload();
  if (!nativeFn) {
    throw new Error('PQC-strict payload decryption requires native @stateset/embedded support');
  }

  const mlKem768Seed = readHybridField(
    recipientPrivateKeyBundle,
    'mlKem768Seed',
    'ml_kem_768_seed',
  );
  if (!mlKem768Seed) {
    throw new Error('PQC-strict decryption requires ML-KEM-768 seed material');
  }

  return JSON.parse(
    nativeFn(
      JSON.stringify(payloadEncrypted),
      toBinaryInput(payloadAad),
      recipientKid,
      { mlKem768Seed: toBinaryInput(mlKem768Seed) },
      toBinaryInput(expectedPlainHash),
    ),
  );
}

/**
 * Generate a hybrid signing proof-of-possession bundle.
 * @param {{ ed25519PrivateKey: Buffer, mlDsa65Seed: Buffer, ed25519PublicKey: Buffer, mlDsa65PublicKey: Buffer }} keyMaterial
 * @returns {{ ed25519Signature: Buffer, mlDsa65Signature: Buffer }}
 */
export function generateHybridSigningPop(keyMaterial) {
  const nativeFn = getNativeHybridGenerateSigningPop();
  if (!nativeFn) {
    throw new Error('Hybrid PoP generation requires native @stateset/embedded support');
  }
  const result = nativeFn(
    keyMaterial.ed25519PrivateKey,
    keyMaterial.mlDsa65Seed,
    keyMaterial.ed25519PublicKey,
    keyMaterial.mlDsa65PublicKey,
  );
  return {
    ed25519Signature: toBuffer(readHybridField(result, 'ed25519Signature', 'ed25519_signature')),
    mlDsa65Signature: toBuffer(readHybridField(result, 'mlDsa65Signature', 'ml_dsa_65_signature')),
  };
}

/**
 * Verify a hybrid signing proof-of-possession bundle.
 * @param {{ ed25519Signature: Buffer, mlDsa65Signature: Buffer }} pop
 * @param {{ ed25519PublicKey: Buffer, mlDsa65PublicKey: Buffer }} publicKeyBundle
 * @returns {boolean}
 */
export function verifyHybridSigningPop(pop, publicKeyBundle) {
  const nativeFn = getNativeHybridVerifySigningPop();
  if (!nativeFn) {
    throw new Error('Hybrid PoP verification requires native @stateset/embedded support');
  }
  return nativeFn(
    toBinaryInput(pop.ed25519Signature),
    toBinaryInput(pop.mlDsa65Signature),
    toBinaryInput(publicKeyBundle.ed25519PublicKey),
    toBinaryInput(publicKeyBundle.mlDsa65PublicKey),
  );
}

const POP_DOMAIN = Buffer.from('VES_POP_V1');

/**
 * Compute the PoP challenge hash: SHA-256("VES_POP_V1" || publicKeyBytes).
 * @param {Buffer} publicKeyBytes - Concatenated public key material.
 * @returns {Buffer} 32-byte challenge hash.
 */
function popChallenge(publicKeyBytes) {
  return crypto.createHash('sha256').update(POP_DOMAIN).update(publicKeyBytes).digest();
}

/**
 * Generate a PQC-strict signing proof-of-possession.
 * Signs SHA-256("VES_POP_V1" || ml_dsa_65_public_key) with ML-DSA-65.
 * Prefers native NAPI binding when available, falls back to JS.
 * @param {{ mlDsa65Seed: Buffer, mlDsa65PublicKey: Buffer }} keyMaterial
 * @returns {Buffer} ML-DSA-65 PoP signature bytes
 */
export function generateStrictSigningPop(keyMaterial) {
  const nativeFn = getNativeStrictGenerateSigningPop();
  if (nativeFn) {
    return toBuffer(nativeFn(keyMaterial.mlDsa65Seed, keyMaterial.mlDsa65PublicKey));
  }
  // JS fallback: compute challenge and sign
  const challenge = popChallenge(keyMaterial.mlDsa65PublicKey);
  return signEventHashStrict(challenge, { mlDsa65Seed: keyMaterial.mlDsa65Seed });
}

/**
 * Verify a PQC-strict signing proof-of-possession.
 * Prefers native NAPI binding when available, falls back to JS.
 * @param {Buffer} pop - ML-DSA-65 signature bytes.
 * @param {{ mlDsa65PublicKey: Buffer }} publicKeyBundle
 * @returns {boolean}
 */
export function verifyStrictSigningPop(pop, publicKeyBundle) {
  const pk = readHybridField(publicKeyBundle, 'mlDsa65PublicKey', 'ml_dsa_65_public_key');
  if (!pk) return false;

  const nativeFn = getNativeStrictVerifySigningPop();
  if (nativeFn) {
    return nativeFn(toBinaryInput(pop), toBinaryInput(pk));
  }
  // JS fallback
  const challenge = popChallenge(toBinaryInput(pk));
  return verifyEventSignatureStrict(challenge, pop, publicKeyBundle);
}

// =============================================================================
// Payload Encryption (VES-ENC-1)
// =============================================================================

const NONCE_SIZE = 12;
const SALT_SIZE = 16;
const KEY_SIZE = 32;

/**
 * @typedef {Object} PayloadAadParams
 * @property {number} vesVersion
 * @property {string} tenantId
 * @property {string} storeId
 * @property {string} eventId
 * @property {string} sourceAgentId
 * @property {number} agentKeyId
 * @property {string} entityType
 * @property {string} entityId
 * @property {string} eventType
 * @property {string} createdAt
 * @property {Buffer} payloadPlainHash
 */

/**
 * Compute payload AAD per VES-ENC-1
 * @param {PayloadAadParams} params
 * @returns {Buffer} - 32-byte AAD hash
 */
export function computePayloadAad(params) {
  const hasher = crypto.createHash('sha256');

  hasher.update(DOMAIN.PAYLOAD_AAD);
  hasher.update(u32BE(params.vesVersion));
  hasher.update(uuidToBytes(params.tenantId));
  hasher.update(uuidToBytes(params.storeId));
  hasher.update(uuidToBytes(params.eventId));
  hasher.update(uuidToBytes(params.sourceAgentId));
  hasher.update(u32BE(params.agentKeyId));
  hasher.update(encodeString(params.entityType));
  hasher.update(encodeString(params.entityId));
  hasher.update(encodeString(params.eventType));
  hasher.update(encodeString(params.createdAt));
  hasher.update(params.payloadPlainHash);

  return hasher.digest();
}

/**
 * Compute recipients hash per VES-ENC-1
 * @param {Array<{recipient_kid: number, enc_b64u: string, ct_b64u: string}>} recipients
 * @returns {Buffer} - 32-byte hash
 */
export function computeRecipientsHash(recipients) {
  // Sort by recipient_kid for deterministic ordering
  const sorted = [...recipients].sort((a, b) => a.recipient_kid - b.recipient_kid);
  const canonical = canonicalizeJson(sorted);

  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.RECIPIENTS);
  hasher.update(canonical);

  return hasher.digest();
}

/**
 * @typedef {{ recipient_kid: number, enc_b64u: string, ct_b64u: string }} EncryptedRecipient
 * @typedef {{
 *   enc_version: number,
 *   aead: string,
 *   nonce_b64u: string,
 *   ciphertext_b64u: string,
 *   tag_b64u: string,
 *   hpke: {
 *     mode: string,
 *     kem: string,
 *     kdf: string,
 *     aead: string,
 *   },
 *   recipients: EncryptedRecipient[],
 * }} PayloadEncryptedStructure
 */

/**
 * @typedef {Object} EncryptionResult
 * @property {PayloadEncryptedStructure} payloadEncrypted - VES-ENC-1 structure
 * @property {Buffer} salt - 16-byte salt used
 * @property {Buffer} payloadPlainHash - 32-byte hash
 * @property {Buffer} payloadCipherHash - 32-byte hash
 */

/**
 * Encrypt payload per VES-ENC-1
 * @param {Object} payload - JSON payload to encrypt
 * @param {PayloadAadParams} aadParams - Parameters for AAD computation
 * @param {Array<{kid: number, publicKey: Buffer}>} recipientKeys - Recipient X25519 public keys
 * @returns {EncryptionResult}
 */
export function encryptPayload(payload, aadParams, recipientKeys) {
  if (recipientKeys.length === 0) {
    throw new Error('At least one recipient required');
  }

  // Generate random values
  const salt = crypto.randomBytes(SALT_SIZE);
  const dek = crypto.randomBytes(KEY_SIZE);
  const nonce = crypto.randomBytes(NONCE_SIZE);

  // Compute payload_plain_hash with salt
  const payloadPlainHash = computePayloadPlainHash(payload, salt);

  // Compute AAD
  const payloadAad = computePayloadAad({
    ...aadParams,
    payloadPlainHash,
  });

  // Prepare plaintext: salt || JCS(payload)
  const canonical = canonicalizeJson(payload);
  const plaintext = Buffer.concat([salt, Buffer.from(canonical, 'utf8')]);

  // Encrypt with AES-256-GCM
  const cipher = crypto.createCipheriv('aes-256-gcm', dek, nonce);
  cipher.setAAD(payloadAad);

  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  const tag = cipher.getAuthTag();

  // Wrap DEK for each recipient using ECDH + HKDF
  // Note: Full HPKE would require additional library, using simplified ECDH here
  const recipients = recipientKeys.map(({ kid, publicKey }) => {
    const { enc, wrappedKey } = wrapDek(dek, publicKey, payloadAad);
    return {
      recipient_kid: kid,
      enc_b64u: enc.toString('base64url'),
      ct_b64u: wrappedKey.toString('base64url'),
    };
  });

  // Sort recipients by kid
  recipients.sort((a, b) => a.recipient_kid - b.recipient_kid);

  // Compute recipients hash
  const recipientsHash = computeRecipientsHash(recipients);

  // Compute payload_cipher_hash
  const payloadCipherHash = computePayloadCipherHash({
    nonce,
    payloadAad,
    ciphertext,
    tag,
    recipientsHash,
  });

  // Build encrypted payload structure
  const payloadEncrypted = {
    enc_version: 1,
    aead: 'AES-256-GCM',
    nonce_b64u: nonce.toString('base64url'),
    ciphertext_b64u: ciphertext.toString('base64url'),
    tag_b64u: tag.toString('base64url'),
    hpke: {
      mode: 'base',
      kem: 'X25519-HKDF-SHA256',
      kdf: 'HKDF-SHA256',
      aead: 'AES-256-GCM',
    },
    recipients,
  };

  return {
    payloadEncrypted,
    salt,
    payloadPlainHash,
    payloadCipherHash,
  };
}

/**
 * Decrypt payload per VES-ENC-1
 * @param {PayloadEncryptedStructure} payloadEncrypted - VES-ENC-1 structure
 * @param {Buffer} payloadAad - 32-byte AAD
 * @param {number} recipientKid - Recipient key ID
 * @param {Buffer} recipientPrivateKey - 32-byte X25519 private key
 * @param {Buffer} expectedPlainHash - Expected payload_plain_hash
 * @returns {unknown} - Decrypted JSON payload
 */
export function decryptPayload(
  payloadEncrypted,
  payloadAad,
  recipientKid,
  recipientPrivateKey,
  expectedPlainHash,
) {
  // Find recipient entry
  const recipient = payloadEncrypted.recipients.find((r) => r.recipient_kid === recipientKid);
  if (!recipient) {
    throw new Error(`Recipient ${recipientKid} not found`);
  }

  // Unwrap DEK
  const enc = Buffer.from(recipient.enc_b64u, 'base64url');
  const wrappedKey = Buffer.from(recipient.ct_b64u, 'base64url');
  const dek = unwrapDek(enc, wrappedKey, recipientPrivateKey, payloadAad);

  // Decode encrypted payload
  const nonce = Buffer.from(payloadEncrypted.nonce_b64u, 'base64url');
  const ciphertext = Buffer.from(payloadEncrypted.ciphertext_b64u, 'base64url');
  const tag = Buffer.from(payloadEncrypted.tag_b64u, 'base64url');

  // Decrypt with AES-256-GCM
  const decipher = crypto.createDecipheriv('aes-256-gcm', dek, nonce);
  decipher.setAAD(payloadAad);
  decipher.setAuthTag(tag);

  const plaintext = Buffer.concat([decipher.update(ciphertext), decipher.final()]);

  // Extract salt and JSON
  const salt = plaintext.subarray(0, SALT_SIZE);
  const jsonBytes = plaintext.subarray(SALT_SIZE);
  const payload = JSON.parse(jsonBytes.toString('utf8'));

  // Verify payload_plain_hash (timing-safe comparison)
  const computedHash = computePayloadPlainHash(payload, salt);
  if (!crypto.timingSafeEqual(computedHash, expectedPlainHash)) {
    throw new Error('Payload hash mismatch');
  }

  return payload;
}

// =============================================================================
// ECDH Key Wrapping (Simplified HPKE)
// =============================================================================

/**
 * Wrap DEK using X25519 ECDH + HKDF + AES-256-GCM
 * @param {Buffer} dek - 32-byte DEK to wrap
 * @param {Buffer} recipientPublicKey - 32-byte X25519 public key
 * @param {Buffer} info - Context info (payload_aad)
 * @returns {{enc: Buffer, wrappedKey: Buffer}}
 */
function wrapDek(dek, recipientPublicKey, info) {
  // Generate ephemeral X25519 key pair
  const ephemeral = crypto.generateKeyPairSync('x25519');

  // Get raw ephemeral public key (enc)
  const encDer = ephemeral.publicKey.export({ type: 'spki', format: 'der' });
  const enc = encDer.subarray(-32);

  // Compute shared secret via ECDH
  const recipientKeyObj = crypto.createPublicKey({
    key: Buffer.concat([
      Buffer.from('302a300506032b656e032100', 'hex'), // X25519 SPKI header
      recipientPublicKey,
    ]),
    format: 'der',
    type: 'spki',
  });

  const sharedSecret = crypto.diffieHellman({
    privateKey: ephemeral.privateKey,
    publicKey: recipientKeyObj,
  });

  // Derive wrapping key using HKDF
  const wrappingKey = Buffer.from(
    crypto.hkdfSync('sha256', sharedSecret, Buffer.alloc(0), info, 32),
  );

  // Wrap DEK with AES-256-GCM
  const wrapNonce = crypto.randomBytes(12);
  const wrapCipher = crypto.createCipheriv('aes-256-gcm', wrappingKey, wrapNonce);
  const wrapped = Buffer.concat([wrapCipher.update(dek), wrapCipher.final()]);
  const wrapTag = wrapCipher.getAuthTag();

  // wrappedKey = nonce || ciphertext || tag
  const wrappedKey = Buffer.concat([wrapNonce, wrapped, wrapTag]);

  return { enc, wrappedKey };
}

/**
 * Unwrap DEK using X25519 ECDH + HKDF + AES-256-GCM
 * @param {Buffer} enc - 32-byte ephemeral public key
 * @param {Buffer} wrappedKey - nonce || ciphertext || tag
 * @param {Buffer} recipientPrivateKey - 32-byte X25519 private key
 * @param {Buffer} info - Context info (payload_aad)
 * @returns {Buffer} - 32-byte DEK
 */
function unwrapDek(enc, wrappedKey, recipientPrivateKey, info) {
  // Create key objects
  const ephemeralPubKeyObj = crypto.createPublicKey({
    key: Buffer.concat([
      Buffer.from('302a300506032b656e032100', 'hex'), // X25519 SPKI header
      enc,
    ]),
    format: 'der',
    type: 'spki',
  });

  const recipientPrivKeyObj = crypto.createPrivateKey({
    key: Buffer.concat([
      Buffer.from('302e020100300506032b656e04220420', 'hex'), // X25519 PKCS#8 header
      recipientPrivateKey,
    ]),
    format: 'der',
    type: 'pkcs8',
  });

  // Compute shared secret
  const sharedSecret = crypto.diffieHellman({
    privateKey: recipientPrivKeyObj,
    publicKey: ephemeralPubKeyObj,
  });

  // Derive wrapping key
  const wrappingKey = Buffer.from(
    crypto.hkdfSync('sha256', sharedSecret, Buffer.alloc(0), info, 32),
  );

  // Unwrap DEK
  const wrapNonce = wrappedKey.subarray(0, 12);
  const ciphertext = wrappedKey.subarray(12, -16);
  const wrapTag = wrappedKey.subarray(-16);

  const decipher = crypto.createDecipheriv('aes-256-gcm', wrappingKey, wrapNonce);
  decipher.setAuthTag(wrapTag);

  return Buffer.concat([decipher.update(ciphertext), decipher.final()]);
}

// =============================================================================
// Merkle Tree Hashing
// =============================================================================

/**
 * @typedef {{
 *   tenantId: string,
 *   storeId: string,
 *   sequenceNumber: number | bigint,
 *   eventSigningHash: Buffer,
 *   agentSignature: Buffer,
 * }} LeafHashParams
 */

/**
 * Compute leaf hash per VES v1.0 Section 10.2
 * @param {LeafHashParams} params
 * @returns {Buffer} - 32-byte hash
 */
export function computeLeafHash(params) {
  const { tenantId, storeId, sequenceNumber, eventSigningHash, agentSignature } = params;

  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.LEAF);
  hasher.update(uuidToBytes(tenantId));
  hasher.update(uuidToBytes(storeId));
  hasher.update(u64BE(sequenceNumber));
  hasher.update(eventSigningHash);
  hasher.update(agentSignature);

  return hasher.digest();
}

/**
 * Compute Merkle node hash per VES v1.0 Section 10.4
 * @param {Buffer} left - 32-byte left child hash
 * @param {Buffer} right - 32-byte right child hash
 * @returns {Buffer} - 32-byte hash
 */
export function computeNodeHash(left, right) {
  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.NODE);
  hasher.update(left);
  hasher.update(right);
  return hasher.digest();
}

/**
 * Compute padding leaf per VES v1.0 Section 10.3
 * @returns {Buffer} - 32-byte hash
 */
export function computePadLeaf() {
  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.PAD_LEAF);
  return hasher.digest();
}

/**
 * Compute stream ID per VES v1.0 Section 12.2
 * @param {string} tenantId
 * @param {string} storeId
 * @returns {Buffer} - 32-byte hash
 */
export function computeStreamId(tenantId, storeId) {
  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.STREAM);
  hasher.update(uuidToBytes(tenantId));
  hasher.update(uuidToBytes(storeId));
  return hasher.digest();
}

// =============================================================================
// Receipt Verification
// =============================================================================

/**
 * @typedef {{
 *   tenantId: string,
 *   storeId: string,
 *   eventId: string,
 *   sequenceNumber: number | bigint,
 *   eventSigningHash: Buffer,
 * }} ReceiptHashParams
 */

/**
 * Compute receipt hash per VES v1.0 Section 6.4
 * @param {ReceiptHashParams} params
 * @returns {Buffer} - 32-byte hash
 */
export function computeReceiptHash(params) {
  const { tenantId, storeId, eventId, sequenceNumber, eventSigningHash } = params;

  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.RECEIPT);
  hasher.update(uuidToBytes(tenantId));
  hasher.update(uuidToBytes(storeId));
  hasher.update(uuidToBytes(eventId));
  hasher.update(u64BE(sequenceNumber));
  hasher.update(eventSigningHash);

  return hasher.digest();
}

// =============================================================================
// Merkle Root Computation
// =============================================================================

/**
 * Compute Merkle root from an array of leaf hashes per VES v1.0 Section 10.
 * Pads to next power of 2 with pad_leaf, then bottom-up pairwise hashing.
 * @param {Buffer[]} leaves - Array of 32-byte leaf hashes
 * @returns {Buffer} - 32-byte Merkle root
 */
export function computeMerkleRoot(leaves) {
  // Delegate to native if available
  const nativeMerkleRoot = getNativeMerkleRoot();
  if (nativeMerkleRoot) {
    return Buffer.from(nativeMerkleRoot(leaves.map((l) => Buffer.from(l))));
  }

  if (leaves.length === 0) return computePadLeaf();
  if (leaves.length === 1) return Buffer.from(leaves[0]);

  // Pad to next power of 2
  let nextPow2 = 1;
  while (nextPow2 < leaves.length) nextPow2 <<= 1;
  const padLeaf = Buffer.from(computePadLeaf());
  /** @type {Buffer[]} */
  const layer = leaves.map((l) => Buffer.from(l));
  while (layer.length < nextPow2) layer.push(padLeaf);

  // Bottom-up merge
  while (layer.length > 1) {
    /** @type {Buffer[]} */
    const nextLayer = [];
    for (let i = 0; i < layer.length; i += 2) {
      nextLayer.push(computeNodeHash(layer[i], layer[i + 1]));
    }
    layer.length = 0;
    layer.push(...nextLayer);
  }

  return layer[0];
}

// =============================================================================
// Native Module Status
// =============================================================================

/**
 * Check whether the native Rust crypto module is available
 * @returns {boolean}
 */
export function isNativeAvailable() {
  return _native !== null;
}
