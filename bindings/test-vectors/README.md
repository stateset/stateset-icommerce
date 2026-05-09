# Cross-binding compatibility test vectors

This directory holds canonical test vectors that **every** language binding
must produce identical output for. The Rust core is the ground truth; each
binding consumes the same JSON file and asserts byte-equal results.

## Why

Phase 5 of the engineering elevation plan calls for a "shared compatibility
test corpus — JSON test vectors generated from Rust ground truth — and run
across Node/Python/Go/Java/Kotlin/Swift/.NET/Ruby/PHP/WASM bindings." This
directory is that corpus.

Without it, a subtle change in (say) JSON canonicalization would silently
diverge bindings from each other. With it, any divergence is a CI failure.

## Files

- `v1.json` — version 1 vectors. Stable: rows may be appended, never
  removed or reordered. Renaming the file requires a major-version bump
  on the bindings.

## Format

```json
{
  "version": 1,
  "description": "...",
  "categories": {
    "canonical_json": [
      { "id": "<stable-slug>", "input": <serde_json::Value>, "expected_hex": "<hex>" }
    ],
    "payload_plain_hash": [
      { "id": "<stable-slug>", "input": <serde_json::Value>,
        "salt_hex": "<32-hex chars or null>", "expected_hex": "<64-hex>" }
    ],
    "merkle_root": [
      { "id": "<stable-slug>", "leaves_hex": ["<64-hex>", ...],
        "expected_hex": "<64-hex>" }
    ]
  }
}
```

Each `expected_hex` is a lowercase hex string. The Rust integration test
at `crates/stateset-crypto/tests/test_vectors.rs` enforces that the file
matches what `stateset-crypto` produces. Bindings consume the same file
and assert their output equals `expected_hex`.

## Adding a new vector

1. Append an entry to the appropriate category in `v1.json` with a stable
   `id` and a placeholder `expected_hex` value (any 64-char hex will do).
2. Run `cargo test --package stateset-crypto --test test_vectors -- --nocapture`.
   The test emits the actual computed value in its failure message.
3. Update `expected_hex` with the actual value. Re-run; test should pass.
4. Each binding's test suite picks up the new vector automatically.

## Adding a new category

Coordinate with the bindings owner — adding a category requires every
binding to gain support for it. Once landed, the new category should
ship in `v1.json`; do *not* create `v2.json` for additive changes.

## Verification recipe

```bash
# Rust ground truth (always runs in CI)
cargo test --package stateset-crypto --test cross_binding_vectors

# Node binding (already wired)
cd bindings/node && node --test test/cross-binding-vectors.js

# Python binding (already wired — requires `maturin develop` first)
cd bindings/python && pytest tests/test_cross_binding_vectors.py

# Go binding (already wired — requires `cargo build --release -p stateset-go` first)
cd bindings/go/stateset && go test -v -run TestCorpus -run TestCanonical -run TestPayload -run TestMerkle ./

# WASM binding (already wired — requires `wasm-pack build --release --target nodejs --out-dir pkg-node` first)
cd bindings/wasm && node --test test/cross-binding-vectors.js

# Java binding (already wired — requires `cargo build --release -p stateset-java` first)
# Local needs JDK 11+ and gradle. CI exercises this in the `jvm-bindings` job.
cd bindings/java/java && gradle test --tests CryptoVectorTests

# Kotlin binding (already wired — requires `cargo build --release -p stateset-kotlin` first)
# Local needs JDK 11+ and gradle. CI exercises this in the `jvm-bindings` job.
cd bindings/kotlin/kotlin && gradle test --tests CryptoVectorTest

# .NET binding (already wired — requires `cargo build --release -p stateset-dotnet` first)
# Local needs dotnet 8+. CI exercises this in the `dotnet-bindings` job.
cd bindings/dotnet/tests && dotnet test --filter FullyQualifiedName~CryptoVectorTests

# Swift binding (already wired — requires `cargo build --release -p stateset-swift` first)
# Local needs Swift 5.7+ on macOS. CI exercises this in the `swift-bindings` job.
cd bindings/swift && swift test --filter CryptoVectorTests

# Ruby binding (already wired)
# Local needs Ruby 3.0+ with dev headers (ruby.h) and `bundle install` first.
# CI exercises this in the `ruby-bindings` job which runs `bundle exec rake`.
cd bindings/ruby && bundle exec rake compile && bundle exec rspec spec/crypto_vector_spec.rb

# PHP binding (already wired)
# Local needs PHP 8.1+ with dev headers and Composer.
# CI exercises this in the extended `php-bindings` job (now compiles + tests).
cd bindings/php && cargo build --features runtime --release && \
  php -d extension="$PWD/target/release/libstateset_embedded.so" \
    vendor/bin/phpunit --filter CryptoVectorTest tests/CryptoVectorTest.php

# All 10 bindings now wired. Phase 5 complete.
# follow the 3-step recipe — read v1.json, run binding primitives,
# assert byte-equal hex against expected_hex.
```

## Wiring a new binding

For each binding, expose three primitives that delegate to
`stateset-crypto` (or re-implement against the same JCS spec):

| Primitive          | Inputs                                | Output             |
|--------------------|---------------------------------------|--------------------|
| `jcs_canonicalize` | JSON value                            | canonical bytes    |
| `payload_plain_hash` | JSON value, optional 16-byte salt   | 32-byte digest     |
| `merkle_root`      | list of 32-byte leaves                | 32-byte digest     |

Then write a test that loads `v1.json`, iterates the three categories,
runs the binding's primitives, and asserts byte-equal hex vs. `expected_hex`.

The domain prefix used by `payload_plain_hash` is hardcoded in each binding
to match `crates/stateset-crypto/src/lib.rs::domain::PAYLOAD_PLAIN`
(`b"VES_PAYLOAD_PLAIN_V1"`). If Rust ever changes a domain prefix the
corresponding bindings must update in lockstep.
