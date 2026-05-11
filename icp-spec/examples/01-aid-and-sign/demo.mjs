// ICP-1.0 example 01 — AID derivation and Intent signing
//
// Zero-dependency Node.js demo of the core ICP-1.0 identity model:
//   1. Generate Ed25519 + X25519 keypairs (the Agent's identity)
//   2. Derive the AID per spec §4.2
//   3. Build a purchase.create Intent (spec §6.1)
//   4. Sign it with Ed25519
//   5. Verify the signature
//
// Run:   node demo.mjs
// Reqs:  Node 18+ (uses node:crypto Ed25519 + X25519 stock)
//
// This demo uses canonical JSON for clarity. Production ICP uses canonical
// CBOR (RFC 8949 §4.2.2). The demo's signing approach is correct; only the
// serialization differs. See `icp-spec/schemas/canonicalization.md`
// (forthcoming) for the full CBOR rules.

import {
  generateKeyPairSync,
  createPublicKey,
  createPrivateKey,
  sign,
  verify,
  createHash,
  randomBytes,
} from 'node:crypto';

// ---------------------------------------------------------------------------
// Step 1: keypairs
// ---------------------------------------------------------------------------

const ed = generateKeyPairSync('ed25519');
const x = generateKeyPairSync('x25519');

// node:crypto exports keys wrapped in DER. For Ed25519 and X25519 the raw
// 32-byte public key is the last 32 bytes of the SPKI export.
const edPubRaw = extractRawPublicKey(ed.publicKey, 'ed25519');
const xPubRaw = extractRawPublicKey(x.publicKey, 'x25519');

// ---------------------------------------------------------------------------
// Step 2: AID per spec §4.2
//   AID = "aid:v1:" + multibase_base58btc( SHA-256(ed_pk || 0x00 || x_pk) )
// ---------------------------------------------------------------------------

const aidPayload = Buffer.concat([edPubRaw, Buffer.from([0x00]), xPubRaw]);
const aidDigest = createHash('sha256').update(aidPayload).digest();
const aid = `aid:v1:z${base58btcEncode(aidDigest)}`;

console.log('=== Identity ===');
console.log('Ed25519 pub (hex):', edPubRaw.toString('hex'));
console.log('X25519  pub (hex):', xPubRaw.toString('hex'));
console.log('AID:               ', aid);
console.log();

// ---------------------------------------------------------------------------
// Step 3: build Intent per spec §6.1 (purchase.create)
// ---------------------------------------------------------------------------

const merchantAid = 'aid:v1:zMerchantPlaceholderForDemoOnlyDoNotShip';
const now = new Date();
const exp = new Date(now.getTime() + 600 * 1000); // §5.3: ≤600s for Intents

const intent = {
  // Order matters: this is the canonical JSON shape used in the demo.
  // (Real ICP signs canonical CBOR; the per-key ordering rules are in
  // schemas/canonicalization.md.)
  v: 'icp-1.0',
  verb: 'purchase.create',
  intent_id: `icp_int_${ulidLike()}`,
  buyer: aid,
  merchant: merchantAid,
  settler: 'settler:stateset.usdc.base-sepolia', // testnet bootstrap Settler
  items: [
    {
      sku: 'WIDGET-001',
      quantity: 2,
      unit_price: { amount: '29.99', currency: 'USDC' },
    },
  ],
  max_total: { amount: '65.00', currency: 'USDC' },
  expiry: exp.toISOString(),
  principal_binding: {
    // For demo: a self-binding (Agent IS the Principal). Real Intents carry
    // a separately-signed PrincipalBinding from the legal entity authorizing
    // the agent. See schemas/intent.purchase.create.schema.json.
    principal: 'did:web:example.com:demo-principal',
    agent: aid,
    authority: {
      max_per_intent: { amount: '500.00', currency: 'USDC' },
      verbs: ['purchase.create'],
    },
    expiry: new Date(now.getTime() + 86400 * 1000).toISOString(),
    revocation: 'https://example.com/.well-known/icp-revocation',
    signature: {
      alg: 'ed25519',
      kid: 'demo-self-binding',
      sig: '<would-be-signed-by-principal-key>',
    },
  },
  nonce: randomBytes(16).toString('hex'),
  iat: now.toISOString(),
  exp: exp.toISOString(),
};

// ---------------------------------------------------------------------------
// Step 4: canonicalize and sign
// ---------------------------------------------------------------------------

const canonical = canonicalJson(intent);
const signature = sign(null, Buffer.from(canonical), ed.privateKey);

console.log('=== Intent ===');
console.log(canonical.slice(0, 400) + (canonical.length > 400 ? '\n... (truncated)' : ''));
console.log();
console.log('=== Signature ===');
console.log('Bytes (hex):', signature.toString('hex'));
console.log('Length:', signature.length, 'bytes');
console.log();

// ---------------------------------------------------------------------------
// Step 5: verify (round-trip)
// ---------------------------------------------------------------------------

const ok = verify(null, Buffer.from(canonical), ed.publicKey, signature);
console.log('=== Verification ===');
console.log('Round-trip verify:', ok ? 'PASS ✓' : 'FAIL ✗');

// Demonstrate negative case: tampered payload MUST fail verification.
const tampered = canonical.replace('29.99', '0.01');
const tamperedOk = verify(null, Buffer.from(tampered), ed.publicKey, signature);
console.log('Tampered payload reject:', tamperedOk ? 'FAIL ✗ (security bug!)' : 'PASS ✓');

if (!ok || tamperedOk) {
  process.exitCode = 1;
}

// ===========================================================================
// Helpers (no external deps)
// ===========================================================================

function extractRawPublicKey(keyObject, alg) {
  // SPKI DER for Ed25519 and X25519 is a fixed 12-byte prefix + 32-byte pubkey.
  const spki = keyObject.export({ format: 'der', type: 'spki' });
  if (spki.length !== 44) {
    throw new Error(`unexpected SPKI length ${spki.length} for ${alg}`);
  }
  return spki.subarray(12, 44);
}

function base58btcEncode(buf) {
  // RFC draft-msporny-base58 / Bitcoin Base58. Tiny implementation:
  // arbitrary-precision base conversion, then the leading-zero
  // preservation rule (0x00 bytes → leading '1' chars).
  const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  let n = 0n;
  for (const b of buf) n = (n << 8n) | BigInt(b);
  let out = '';
  while (n > 0n) {
    const r = Number(n % 58n);
    n = n / 58n;
    out = ALPHABET[r] + out;
  }
  // Preserve leading zero bytes as leading '1's (Base58btc convention).
  for (const b of buf) {
    if (b === 0) out = '1' + out;
    else break;
  }
  return out;
}

function ulidLike() {
  // ULID-shaped (26 chars, Crockford base32, time-ordered). Simplified:
  // not a real ULID library — the demo only needs a unique-per-process
  // identifier with the right shape for the schema regex.
  const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
  const bytes = randomBytes(16);
  let bits = 0n;
  for (const b of bytes) bits = (bits << 8n) | BigInt(b);
  let s = '';
  for (let i = 0; i < 26; i++) {
    s = ALPHABET[Number(bits & 31n)] + s;
    bits >>= 5n;
  }
  return s;
}

function canonicalJson(value) {
  // RFC 8785 JCS subset: lexicographic key ordering, no whitespace,
  // UTF-8, no insignificant trailing zeros (handled by JSON.stringify
  // for the simple shapes used in the demo).
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return '[' + value.map(canonicalJson).join(',') + ']';
  }
  const keys = Object.keys(value).sort();
  const entries = keys.map((k) => JSON.stringify(k) + ':' + canonicalJson(value[k]));
  return '{' + entries.join(',') + '}';
}
