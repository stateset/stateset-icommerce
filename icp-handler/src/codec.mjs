// ICP wire codec — canonical JSON, AID derivation, signature verification.
// Zero-dep: only node:crypto. Mirrors the conformance reference IUT.

import {
  createPublicKey,
  verify as nodeVerify,
  sign as nodeSign,
  createPrivateKey,
  createHash,
  randomBytes,
} from 'node:crypto';

const ED25519_PKCS8_PREFIX = Buffer.from('302e020100300506032b657004220420', 'hex');
const ED25519_SPKI_PREFIX = Buffer.from('302a300506032b6570032100', 'hex');

export function privateKeyFromSeed(seed32) {
  if (seed32.length !== 32) throw new Error('seed must be 32 bytes');
  const der = Buffer.concat([ED25519_PKCS8_PREFIX, seed32]);
  return createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
}

export function publicKeyFromRaw(raw32) {
  if (raw32.length !== 32) throw new Error('public key must be 32 bytes');
  const der = Buffer.concat([ED25519_SPKI_PREFIX, raw32]);
  return createPublicKey({ key: der, format: 'der', type: 'spki' });
}

export function publicKeyToRaw(keyObject) {
  const spki = keyObject.export({ format: 'der', type: 'spki' });
  if (spki.length !== 44) throw new Error(`unexpected SPKI length ${spki.length}`);
  return spki.subarray(12, 44);
}

export function canonicalJson(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return '[' + value.map(canonicalJson).join(',') + ']';
  const keys = Object.keys(value).sort();
  return '{' + keys.map((k) => JSON.stringify(k) + ':' + canonicalJson(value[k])).join(',') + '}';
}

export function deriveAidFromPubkeys(edRaw, xRaw) {
  const buf = Buffer.concat([edRaw, Buffer.from([0x00]), xRaw]);
  const digest = createHash('sha256').update(buf).digest();
  return `aid:v1:z${base58btcEncode(digest)}`;
}

export function base58btcEncode(buf) {
  const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  let n = 0n;
  for (const b of buf) n = (n << 8n) | BigInt(b);
  let out = '';
  while (n > 0n) {
    const r = Number(n % 58n);
    n = n / 58n;
    out = ALPHABET[r] + out;
  }
  for (const b of buf) {
    if (b === 0) out = '1' + out;
    else break;
  }
  return out;
}

/**
 * Verify an ICP signature.
 * @param {string} canonical - canonical JSON of the signed payload
 * @param {string} signatureHex - hex-encoded Ed25519 signature
 * @param {Buffer} edPubRaw - 32-byte raw Ed25519 public key
 * @returns {boolean}
 */
export function verifyEd25519(canonical, signatureHex, edPubRaw) {
  try {
    const sig = Buffer.from(signatureHex, 'hex');
    if (sig.length !== 64) return false;
    const pub = publicKeyFromRaw(edPubRaw);
    return nodeVerify(null, Buffer.from(canonical), pub, sig);
  } catch (_) {
    return false;
  }
}

/** Sign canonical bytes with an Ed25519 private key. Returns hex. */
export function signEd25519(canonical, edPriv) {
  const sig = nodeSign(null, Buffer.from(canonical), edPriv);
  return sig.toString('hex');
}

export function newNonceHex() {
  return randomBytes(16).toString('hex');
}

export function newId(prefix) {
  // ULID-shaped 26-char Crockford base32. Not a real ULID; sufficient for IDs.
  const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
  const bytes = randomBytes(16);
  let bits = 0n;
  for (const b of bytes) bits = (bits << 8n) | BigInt(b);
  let s = '';
  for (let i = 0; i < 26; i++) {
    s = ALPHABET[Number(bits & 31n)] + s;
    bits >>= 5n;
  }
  return `${prefix}_${s}`;
}

/**
 * Resolve an AID to its raw Ed25519 public key. In the stub handler we accept
 * a directly-supplied `_pubkey_hex` field on Intents to short-circuit AID
 * resolution. In a real handler this calls a resolver (DNS-over-HTTPS,
 * .well-known endpoint, on-chain registry).
 *
 * NOTE: this helper does NOT bind the returned key to the claimed AID — see
 * `resolveAidPubkey`, which the handler uses on the verification path to
 * enforce ICP-1.0-DRAFT §4.2 (AID = Base58btc(SHA-256(ed_pk || 0x00 || x_pk))).
 */
export function pubkeyForAid(aid, hintHex) {
  if (hintHex) return Buffer.from(hintHex, 'hex');
  throw new Error(
    `cannot resolve ${aid}: no resolver configured and no _pubkey_hex hint provided`,
  );
}

/**
 * Error thrown when a supplied public key does not derive to the claimed AID.
 * The `.code` field maps to the registered `auth.*` error namespace.
 */
export class AidBindingError extends Error {
  constructor(message, code = 'auth.aid_resolution_failed') {
    super(message);
    this.name = 'AidBindingError';
    this.code = code;
  }
}

/**
 * Resolve an AID to its raw Ed25519 public key AND verify the AID→pubkey
 * binding per ICP-1.0-DRAFT §4.2.
 *
 * The handler receives the Agent's key material out-of-band via two
 * convenience fields on the Intent envelope:
 *   - `_pubkey_hex`   — raw hex of the Agent's Ed25519 public key (REQUIRED).
 *   - `_x_pubkey_hex` — raw hex of the Agent's X25519 public key (OPTIONAL but
 *                       REQUIRED to verify the binding; without it the full AID
 *                       cannot be re-derived).
 *
 * Binding rule:
 *   - If both keys are supplied, the handler re-derives
 *       aid' = aid:v1:z + Base58btc(SHA-256(ed_pk || 0x00 || x_pk))
 *     and rejects (throws `AidBindingError`) unless `aid' === aid`. This closes
 *     the "any key verifies as any AID" hole — a forged `_pubkey_hex` no longer
 *     impersonates an arbitrary AID.
 *   - If only `_pubkey_hex` is supplied, the binding CANNOT be checked (the
 *     X25519 half of the preimage is absent). The handler rejects with
 *     `auth.aid_resolution_failed` for any `aid:v1:z…`-shaped AID, because a
 *     spec AID is only meaningful when its derivation can be verified. Callers
 *     that genuinely cannot supply the X25519 key must register through a real
 *     resolver (out of scope for the reference handler).
 *
 * @param {string} aid              The claimed Agent AID (`intent.buyer`/`seller`).
 * @param {string} edHintHex        Raw hex of the Ed25519 public key.
 * @param {string} [xHintHex]       Raw hex of the X25519 public key.
 * @returns {Buffer} the 32-byte raw Ed25519 public key, bound to `aid`.
 * @throws {AidBindingError} if the binding cannot be established or fails.
 */
export function resolveAidPubkey(aid, edHintHex, xHintHex) {
  if (!edHintHex) {
    throw new AidBindingError(
      `cannot resolve ${aid}: no resolver configured and no _pubkey_hex hint provided`,
    );
  }
  const edRaw = Buffer.from(edHintHex, 'hex');
  if (edRaw.length !== 32) {
    throw new AidBindingError(`_pubkey_hex must be 32 bytes, got ${edRaw.length}`);
  }

  // A spec AID (`aid:v1:z…`) can only be honored if we can re-derive it.
  const isSpecAid = typeof aid === 'string' && aid.startsWith('aid:v1:z');

  if (!xHintHex) {
    if (isSpecAid) {
      throw new AidBindingError(
        `cannot verify AID binding for ${aid}: _x_pubkey_hex (X25519 public key) is required to re-derive the AID per §4.2`,
      );
    }
    // Non-spec AID with no X key: nothing to bind against; return the key as-is.
    return edRaw;
  }

  const xRaw = Buffer.from(xHintHex, 'hex');
  if (xRaw.length !== 32) {
    throw new AidBindingError(`_x_pubkey_hex must be 32 bytes, got ${xRaw.length}`);
  }

  const derived = deriveAidFromPubkeys(edRaw, xRaw);
  if (derived !== aid) {
    throw new AidBindingError(
      `AID binding failed: supplied pubkeys derive to ${derived}, not the claimed ${aid}`,
    );
  }
  return edRaw;
}
