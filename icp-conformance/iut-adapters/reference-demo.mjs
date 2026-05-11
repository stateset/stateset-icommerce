#!/usr/bin/env node
// IUT adapter — reference implementation
//
// Wraps the same identity/signing logic as `icp-spec/examples/01-aid-and-sign/demo.mjs`,
// but in deterministic mode: keypairs are derived from seed bytes provided as input
// rather than freshly generated.
//
// Protocol: see ../iut.protocol.md
//
// Usage:
//   echo '{"test":"01-aid-derivation",...}' | node reference-demo.mjs 01-aid-derivation
//
// Behavior:
//   - Reads one JSON object from stdin.
//   - Performs the requested test.
//   - Writes one JSON object to stdout. Any other stdout output is a protocol violation.
//   - Logs (if any) go to stderr.

import { readFileSync } from 'node:fs';
import {
  createPrivateKey,
  createPublicKey,
  sign,
  verify,
  createHash,
} from 'node:crypto';

// ===========================================================================
// Helpers
// ===========================================================================

// PKCS#8 DER prefix for Ed25519 from raw 32-byte seed.
const ED25519_PKCS8_PREFIX = Buffer.from(
  '302e020100300506032b657004220420',
  'hex',
);
// PKCS#8 DER prefix for X25519 from raw 32-byte seed.
const X25519_PKCS8_PREFIX = Buffer.from(
  '302e020100300506032b656e04220420',
  'hex',
);

function privateKeyFromSeed(alg, seed) {
  const prefix = alg === 'ed25519' ? ED25519_PKCS8_PREFIX : X25519_PKCS8_PREFIX;
  const der = Buffer.concat([prefix, seed]);
  return createPrivateKey({ key: der, format: 'der', type: 'pkcs8' });
}

function extractRawPublicKey(keyObject) {
  // Both Ed25519 and X25519 SPKI exports are 44 bytes: 12-byte header + 32-byte raw key.
  const spki = keyObject.export({ format: 'der', type: 'spki' });
  if (spki.length !== 44) throw new Error(`unexpected SPKI length ${spki.length}`);
  return spki.subarray(12, 44);
}

function base58btcEncode(buf) {
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

function canonicalJson(value) {
  if (value === null || typeof value !== 'object') return JSON.stringify(value);
  if (Array.isArray(value)) return '[' + value.map(canonicalJson).join(',') + ']';
  const keys = Object.keys(value).sort();
  return '{' + keys.map((k) => JSON.stringify(k) + ':' + canonicalJson(value[k])).join(',') + '}';
}

// ===========================================================================
// Test handlers
// ===========================================================================

function run01AidDerivation(input) {
  const edSeed = Buffer.from(input.agent.ed25519_seed_hex, 'hex');
  const xSeed = Buffer.from(input.agent.x25519_seed_hex, 'hex');

  if (edSeed.length !== 32) throw new Error('ed25519_seed must be 32 bytes');
  if (xSeed.length !== 32) throw new Error('x25519_seed must be 32 bytes');

  const edPriv = privateKeyFromSeed('ed25519', edSeed);
  const xPriv = privateKeyFromSeed('x25519', xSeed);
  const edPub = createPublicKey(edPriv);
  const xPub = createPublicKey(xPriv);

  const edPubRaw = extractRawPublicKey(edPub);
  const xPubRaw = extractRawPublicKey(xPub);

  // AID per ICP-1.0 §4.2.
  const aidPayload = Buffer.concat([edPubRaw, Buffer.from([0x00]), xPubRaw]);
  const aidDigest = createHash('sha256').update(aidPayload).digest();
  const aid = `aid:v1:z${base58btcEncode(aidDigest)}`;

  // Build the Intent from the deterministic fields supplied in the input.
  // The adapter MUST plug the computed AID into `buyer` and into the
  // PrincipalBinding's `agent` field (since this Agent is self-binding for
  // the test).
  const intent = {
    ...input.intent,
    buyer: aid,
    principal_binding: { ...input.intent.principal_binding, agent: aid },
  };

  // Canonicalize and sign.
  const canonical = canonicalJson(intent);
  const sigBytes = sign(null, Buffer.from(canonical), edPriv);

  const out = {
    ed25519_pubkey_hex: edPubRaw.toString('hex'),
    x25519_pubkey_hex: xPubRaw.toString('hex'),
    aid,
    intent_canonical_string: canonical,
    intent_canonical_bytes_hex: Buffer.from(canonical).toString('hex'),
    intent_signature_hex: sigBytes.toString('hex'),
  };

  if (input.params?.verify_tamper_rejected) {
    const tampered = canonical.replace('29.99', '0.01');
    const ok = verify(null, Buffer.from(tampered), edPub, sigBytes);
    out.tamper_rejected = !ok;
  }

  return out;
}

function run02CanonicalJson(input) {
  if (!Array.isArray(input.cases)) throw new Error('input.cases must be an array');
  const canonical_strings = input.cases.map((c) => canonicalJson(c.value));
  return { canonical_strings, names: input.cases.map((c) => c.name) };
}

// ===========================================================================
// Main
// ===========================================================================

const testName = process.argv[2];
if (!testName) {
  process.stderr.write('FATAL: missing test name argument\n');
  process.exit(2);
}

let input;
try {
  input = JSON.parse(readFileSync(0, 'utf8'));
} catch (err) {
  process.stderr.write(`FATAL: invalid JSON on stdin: ${err.message}\n`);
  process.exit(2);
}

let output;
try {
  switch (testName) {
    case '01-aid-derivation':
      output = run01AidDerivation(input);
      break;
    case '02-canonical-json':
      output = run02CanonicalJson(input);
      break;
    default:
      process.stderr.write(JSON.stringify({ error: 'unsupported', reason: `no handler for ${testName}` }) + '\n');
      process.exit(2);
  }
} catch (err) {
  process.stderr.write(`FATAL: adapter error: ${err.message}\n${err.stack}\n`);
  process.exit(1);
}

process.stdout.write(JSON.stringify(output, null, 2) + '\n');
