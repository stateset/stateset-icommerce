// Phase 5 cross-binding compatibility test for the Node binding.
//
// Reads the language-neutral corpus at `bindings/test-vectors/v1.json`
// and asserts that the Node napi-rs binding produces byte-equal hex
// digests to Rust ground truth for every entry. Counterpart in Rust:
// `crates/stateset-crypto/tests/cross_binding_vectors.rs`.

const assert = require('assert');
const { test } = require('node:test');
const { createHash } = require('crypto');
const fs = require('fs');
const path = require('path');

const { jcsCanonicalize, merkleRoot } = require('../index.js');

const CORPUS_PATH = path.join(__dirname, '..', '..', 'test-vectors', 'v1.json');

function loadCorpus() {
  const raw = fs.readFileSync(CORPUS_PATH, 'utf8');
  const parsed = JSON.parse(raw);
  assert.strictEqual(parsed.version, 1, 'corpus version must be 1');
  return parsed;
}

function hex(buf) {
  return Buffer.from(buf).toString('hex');
}

// Domain-separation prefixes — must match `crates/stateset-crypto/src/lib.rs::domain::*`.
// Locked here to make the binding test self-contained; if Rust changes a prefix the
// vectors regenerate and this binding must update in lockstep.
const DOMAIN = {
  PAYLOAD_PLAIN: Buffer.from('VES_PAYLOAD_PLAIN_V1', 'utf8'),
};

// Compose `compute_payload_plain_hash` from the binding's `jcsCanonicalize`
// helper plus a domain-separated SHA-256. The Rust implementation is
// equivalent to:
//   sha256(domain.PAYLOAD_PLAIN || optional_salt || canonical_bytes)
function payloadPlainHash(payload, saltHex) {
  const canonical = jcsCanonicalize(JSON.stringify(payload));
  const hasher = createHash('sha256');
  hasher.update(DOMAIN.PAYLOAD_PLAIN);
  if (saltHex) {
    hasher.update(Buffer.from(saltHex, 'hex'));
  }
  hasher.update(Buffer.from(canonical, 'utf8'));
  return hasher.digest();
}

test('cross-binding corpus is present and version 1', () => {
  const corpus = loadCorpus();
  assert.ok(corpus.categories, 'corpus has categories block');
  assert.ok(Array.isArray(corpus.categories.canonical_json));
  assert.ok(Array.isArray(corpus.categories.payload_plain_hash));
  assert.ok(Array.isArray(corpus.categories.merkle_root));
});

test('canonical_json: every vector matches Rust ground truth via jcsCanonicalize', () => {
  const corpus = loadCorpus();
  for (const v of corpus.categories.canonical_json) {
    const canonical = jcsCanonicalize(JSON.stringify(v.input));
    const digest = createHash('sha256').update(canonical, 'utf8').digest();
    assert.strictEqual(
      hex(digest),
      v.expected_hex,
      `canonical_json/${v.id}: expected ${v.expected_hex}, got ${hex(digest)}`,
    );
  }
});

test('payload_plain_hash: every vector matches via composed domain SHA', () => {
  const corpus = loadCorpus();
  for (const v of corpus.categories.payload_plain_hash) {
    const digest = payloadPlainHash(v.input, v.salt_hex || null);
    assert.strictEqual(
      hex(digest),
      v.expected_hex,
      `payload_plain_hash/${v.id}: expected ${v.expected_hex}, got ${hex(digest)}`,
    );
  }
});

test('merkle_root: every vector matches via merkleRoot napi helper', () => {
  const corpus = loadCorpus();
  for (const v of corpus.categories.merkle_root) {
    const leaves = v.leaves_hex.map((h) => Buffer.from(h, 'hex'));
    const root = merkleRoot(leaves);
    assert.strictEqual(
      hex(root),
      v.expected_hex,
      `merkle_root/${v.id}: expected ${v.expected_hex}, got ${hex(root)}`,
    );
  }
});
