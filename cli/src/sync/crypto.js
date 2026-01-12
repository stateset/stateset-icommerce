/**
 * VES v1.0 Cryptographic Operations
 *
 * Implements:
 * - RFC 8785 JSON Canonicalization Scheme (JCS)
 * - Domain-separated hashing per VES spec
 * - Ed25519 signing for agent signatures
 * - Payload encryption (VES-ENC-1)
 */

import crypto from 'crypto';

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
    if (c === 0x22) result += '\\"';       // "
    else if (c === 0x5c) result += '\\\\'; // \
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
 * @param {any} value
 * @returns {string}
 */
export function canonicalizeJson(value) {
  if (value === null) return 'null';
  if (value === undefined) return 'null';

  const type = typeof value;

  if (type === 'boolean') return value ? 'true' : 'false';
  if (type === 'number') return canonicalizeNumber(value);
  if (type === 'string') return escapeString(value);

  if (Array.isArray(value)) {
    const items = value.map(v => canonicalizeJson(v));
    return '[' + items.join(',') + ']';
  }

  if (type === 'object') {
    // Sort keys lexicographically by UTF-16 code units
    const keys = Object.keys(value).sort();
    const pairs = keys.map(k => escapeString(k) + ':' + canonicalizeJson(value[k]));
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
 * @param {Buffer} [salt] - Optional 16-byte salt for encrypted payloads
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
 * Compute payload_cipher_hash per VES v1.0 Section 5.3
 * For plaintext events, returns 32 zero bytes
 * @param {Object} params - Encryption parameters (null for plaintext)
 * @returns {Buffer} - 32-byte hash
 */
export function computePayloadCipherHash(params = null) {
  if (!params) {
    return Buffer.alloc(32); // 32 zero bytes for plaintext
  }

  const { nonce, payloadAad, ciphertext, tag, recipientsHash } = params;

  const hasher = crypto.createHash('sha256');
  hasher.update(DOMAIN.PAYLOAD_CIPHER);
  hasher.update(nonce);       // 12 bytes
  hasher.update(payloadAad);  // 32 bytes
  hasher.update(ciphertext);  // variable
  hasher.update(tag);         // 16 bytes
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
 * @param {Buffer} eventSigningHash - 32-byte hash to sign
 * @param {Buffer} privateKey - 32-byte Ed25519 private key (seed)
 * @returns {Buffer} - 64-byte signature
 */
export function signEventHash(eventSigningHash, privateKey) {
  // Create key object from raw 32-byte seed
  const keyObj = crypto.createPrivateKey({
    key: Buffer.concat([
      // PKCS#8 Ed25519 private key header
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      privateKey
    ]),
    format: 'der',
    type: 'pkcs8'
  });

  return crypto.sign(null, eventSigningHash, keyObj);
}

/**
 * Verify an event signature
 * @param {Buffer} eventSigningHash - 32-byte hash that was signed
 * @param {Buffer} signature - 64-byte Ed25519 signature
 * @param {Buffer} publicKey - 32-byte Ed25519 public key
 * @returns {boolean}
 */
export function verifyEventSignature(eventSigningHash, signature, publicKey) {
  try {
    // Create key object from raw 32-byte public key
    const keyObj = crypto.createPublicKey({
      key: Buffer.concat([
        // SPKI Ed25519 public key header
        Buffer.from('302a300506032b6570032100', 'hex'),
        publicKey
      ]),
      format: 'der',
      type: 'spki'
    });

    return crypto.verify(null, eventSigningHash, keyObj, signature);
  } catch (e) {
    return false;
  }
}

// =============================================================================
// Payload Encryption (VES-ENC-1)
// =============================================================================

const NONCE_SIZE = 12;
const TAG_SIZE = 16;
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
 * @typedef {Object} EncryptionResult
 * @property {Object} payloadEncrypted - VES-ENC-1 structure
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
    payloadPlainHash
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
      ct_b64u: wrappedKey.toString('base64url')
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
    recipientsHash
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
      aead: 'AES-256-GCM'
    },
    recipients
  };

  return {
    payloadEncrypted,
    salt,
    payloadPlainHash,
    payloadCipherHash
  };
}

/**
 * Decrypt payload per VES-ENC-1
 * @param {Object} payloadEncrypted - VES-ENC-1 structure
 * @param {Buffer} payloadAad - 32-byte AAD
 * @param {number} recipientKid - Recipient key ID
 * @param {Buffer} recipientPrivateKey - 32-byte X25519 private key
 * @param {Buffer} expectedPlainHash - Expected payload_plain_hash
 * @returns {Object} - Decrypted JSON payload
 */
export function decryptPayload(payloadEncrypted, payloadAad, recipientKid, recipientPrivateKey, expectedPlainHash) {
  // Find recipient entry
  const recipient = payloadEncrypted.recipients.find(r => r.recipient_kid === recipientKid);
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

  // Verify payload_plain_hash
  const computedHash = computePayloadPlainHash(payload, salt);
  if (!computedHash.equals(expectedPlainHash)) {
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
      recipientPublicKey
    ]),
    format: 'der',
    type: 'spki'
  });

  const sharedSecret = crypto.diffieHellman({
    privateKey: ephemeral.privateKey,
    publicKey: recipientKeyObj
  });

  // Derive wrapping key using HKDF
  const wrappingKey = crypto.hkdfSync('sha256', sharedSecret, Buffer.alloc(0), info, 32);

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
      enc
    ]),
    format: 'der',
    type: 'spki'
  });

  const recipientPrivKeyObj = crypto.createPrivateKey({
    key: Buffer.concat([
      Buffer.from('302e020100300506032b656e04220420', 'hex'), // X25519 PKCS#8 header
      recipientPrivateKey
    ]),
    format: 'der',
    type: 'pkcs8'
  });

  // Compute shared secret
  const sharedSecret = crypto.diffieHellman({
    privateKey: recipientPrivKeyObj,
    publicKey: ephemeralPubKeyObj
  });

  // Derive wrapping key
  const wrappingKey = crypto.hkdfSync('sha256', sharedSecret, Buffer.alloc(0), info, 32);

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
 * Compute leaf hash per VES v1.0 Section 10.2
 * @param {Object} params
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
 * Compute receipt hash per VES v1.0 Section 6.4
 * @param {Object} params
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
