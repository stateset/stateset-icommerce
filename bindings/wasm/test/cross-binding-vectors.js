// Cross-binding compatibility test for the WASM binding.
//
// Reads the language-neutral corpus at `bindings/test-vectors/v1.json` and
// asserts the WASM binding produces byte-equal hex digests to Rust ground
// truth for every entry. Counterparts:
//   - Rust:   crates/stateset-crypto/tests/cross_binding_vectors.rs
//   - Node:   bindings/node/test/cross-binding-vectors.js
//   - Python: bindings/python/tests/test_cross_binding_vectors.py
//   - Go:     bindings/go/stateset/crypto_test.go
//
// Requires `wasm-pack build --release --target nodejs --out-dir pkg-node` to
// have run first (produces ../pkg-node/stateset_embedded_wasm.js).

const assert = require('assert');
const { test } = require('node:test');
const { createHash } = require('crypto');
const fs = require('fs');
const path = require('path');

const {
  jcsCanonicalize,
  merkleRoot,
  payloadPlainHash,
} = require('../pkg-node/stateset_embedded_wasm.js');

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

test('corpus is present and version 1', () => {
  const corpus = loadCorpus();
  assert.ok(corpus.categories);
  assert.ok(Array.isArray(corpus.categories.canonical_json));
  assert.ok(Array.isArray(corpus.categories.payload_plain_hash));
  assert.ok(Array.isArray(corpus.categories.merkle_root));
});

test('canonical_json: every vector matches Rust ground truth via WASM jcsCanonicalize', () => {
  const corpus = loadCorpus();
  for (const v of corpus.categories.canonical_json) {
    const canonical = jcsCanonicalize(JSON.stringify(v.input));
    const digest = createHash('sha256').update(Buffer.from(canonical)).digest();
    assert.strictEqual(
      hex(digest),
      v.expected_hex,
      `canonical_json/${v.id}: expected ${v.expected_hex}, got ${hex(digest)}`,
    );
  }
});

test('payload_plain_hash: every vector matches via WASM payloadPlainHash', () => {
  const corpus = loadCorpus();
  for (const v of corpus.categories.payload_plain_hash) {
    const salt = v.salt_hex ? new Uint8Array(Buffer.from(v.salt_hex, 'hex')) : undefined;
    const digest = payloadPlainHash(JSON.stringify(v.input), salt);
    assert.strictEqual(
      hex(digest),
      v.expected_hex,
      `payload_plain_hash/${v.id}: expected ${v.expected_hex}, got ${hex(digest)}`,
    );
  }
});

test('merkle_root: every vector matches via WASM merkleRoot', () => {
  const corpus = loadCorpus();
  for (const v of corpus.categories.merkle_root) {
    const leaves = v.leaves_hex.map((h) => new Uint8Array(Buffer.from(h, 'hex')));
    const root = merkleRoot(leaves);
    assert.strictEqual(
      hex(root),
      v.expected_hex,
      `merkle_root/${v.id}: expected ${v.expected_hex}, got ${hex(root)}`,
    );
  }
});
