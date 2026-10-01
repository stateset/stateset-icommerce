import {
  computePayloadPlainHash,
  computePayloadCipherHash,
  computePayloadAad,
  computeRecipientsHash,
} from './crypto.js';

function hashBytes(value) {
  if (typeof value === 'string') {
    if (!/^(?:0x)?[0-9a-fA-F]{64}$/.test(value)) throw new Error('Invalid payload hash');
    return Buffer.from(value.replace(/^0x/, ''), 'hex');
  }
  if (!(value instanceof Uint8Array) || value.length !== 32)
    throw new Error('Invalid payload hash');
  return Buffer.from(value);
}

function base64Bytes(value, length) {
  if (typeof value !== 'string' || !/^[A-Za-z0-9_-]+$/.test(value))
    throw new Error('Invalid encrypted bytes');
  const bytes = Buffer.from(value, 'base64url');
  if (bytes.toString('base64url') !== value || (length !== undefined && bytes.length !== length)) {
    throw new Error('Invalid encrypted byte length/encoding');
  }
  return bytes;
}

// Verify the content addressed by the signed hashes without requiring a
// recipient private key. Salted plaintext hashes are checked during decryption.
export function verifyPayloadIntegrity(envelope) {
  try {
    const plainHash = hashBytes(envelope.payloadPlainHash);
    const cipherHash = hashBytes(envelope.payloadCipherHash);
    const kind = envelope.payloadKind ?? 0;
    if (kind === 0) {
      return (
        !envelope.payloadEncrypted &&
        cipherHash.equals(Buffer.alloc(32)) &&
        computePayloadPlainHash(envelope.payload).equals(plainHash)
      );
    }
    if (kind !== 1) return false;
    // Ciphertext integrity does not authenticate a parallel cleartext payload.
    if (
      envelope.payload !== null &&
      envelope.payload !== undefined &&
      (typeof envelope.payload !== 'object' ||
        Array.isArray(envelope.payload) ||
        Object.keys(envelope.payload).length !== 0)
    )
      return false;
    const encrypted = envelope.payloadEncrypted;
    if (!encrypted || encrypted.aead !== 'AES-256-GCM') return false;
    const version = encrypted.enc_version;
    if (!Number.isInteger(version)) return false;
    const expected = {
      1: { scheme: 1, mode: 'base', kem: 'X25519-HKDF-SHA256' },
      2: { scheme: 3, mode: 'hybrid-base', kem: 'x25519+mlkem768' },
      3: { scheme: 2, mode: 'pqc-base', kem: 'mlkem768' },
    }[version];
    if (
      !expected ||
      encrypted.hpke?.mode !== expected.mode ||
      encrypted.hpke?.kem !== expected.kem ||
      encrypted.hpke?.kdf !== 'HKDF-SHA256' ||
      encrypted.hpke?.aead !== 'AES-256-GCM'
    )
      return false;
    for (const params of [encrypted.keyWrapParams, encrypted.key_wrap_params]) {
      if (
        params &&
        (Number(params.scheme) !== expected.scheme ||
          params.kdf !== 'HKDF-SHA256' ||
          params.aead !== 'AES-256-GCM')
      )
        return false;
    }
    const recipients = encrypted.recipients;
    if (!Array.isArray(recipients) || recipients.length === 0) return false;
    const byKid = new Map();
    for (const recipient of recipients) {
      const kid = recipient.recipient_kid;
      if (!Number.isSafeInteger(kid) || kid < 0 || kid > 0xffffffff || byKid.has(kid)) return false;
      byKid.set(kid, recipient);
    }
    // Auxiliary transport aliases must describe the hashed recipient list.
    for (const wraps of [encrypted.recipientWraps, encrypted.recipient_wraps]) {
      if (wraps === undefined) continue;
      if (!Array.isArray(wraps) || wraps.length !== recipients.length) return false;
      const seen = new Set();
      for (const wrap of wraps) {
        const kid = wrap.recipientKid ?? wrap.recipient_kid;
        const original = byKid.get(kid);
        if (
          !original ||
          seen.has(kid) ||
          Number(wrap.wrapScheme ?? wrap.wrap_scheme) !== expected.scheme
        )
          return false;
        seen.add(kid);
        const pairs = [
          [wrap.wrappedKey ?? wrap.wrapped_key_b64u, original.ct_b64u],
          [
            wrap.x25519Enc ?? wrap.x25519_enc_b64u,
            version === 1 ? original.enc_b64u : original.x25519_enc_b64u,
          ],
          [wrap.mlKemCiphertext ?? wrap.ml_kem_ciphertext_b64u, original.mlkem_ct_b64u],
          [wrap.wrapNonce ?? wrap.wrap_nonce_b64u, original.wrap_nonce_b64u],
        ];
        if (pairs.some(([alias, value]) => (alias ?? null) !== (value ?? null))) return false;
      }
    }
    const payloadAad = computePayloadAad({
      ...envelope,
      sourceAgentId: envelope.sourceAgent,
      payloadPlainHash: plainHash,
    });
    return computePayloadCipherHash({
      nonce: base64Bytes(encrypted.nonce_b64u, 12),
      tag: base64Bytes(encrypted.tag_b64u, 16),
      ciphertext: base64Bytes(encrypted.ciphertext_b64u),
      payloadAad,
      recipientsHash: computeRecipientsHash(recipients),
    }).equals(cipherHash);
  } catch {
    return false;
  }
}
