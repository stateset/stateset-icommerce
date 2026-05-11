# icp-conformance

Black-box conformance test suite for the Intelligent Commerce Protocol.
**Implementation-agnostic.** Any ICP implementation that wants the
"ICP-1.0 conformant" badge runs this suite against itself and publishes
the result.

The suite intentionally lives here, in the spec stewardship repo, rather
than inside any single implementation. That separation is what converts
"ICP" from a product into a protocol with multiple implementers.

## Status

ICP-1.0 — bootstrap. First vector (`01-aid-derivation`) lands in this
release. Vector set will grow alongside the spec; full coverage targeted
for ICP-1.0 Last Call.

## How it works

Every test consists of:

1. A **vector** — a directory under `vectors/<spec-version>/<test-name>/`
   containing deterministic inputs and expected outputs.
2. An **IUT adapter** — a thin program (in any language) under
   `iut-adapters/` that exposes the implementation under test through a
   stable JSON-over-stdio protocol.
3. The **runner** — `runner/run.mjs`, which loads vectors, invokes the
   adapter, and compares output to expected.

Run:

```sh
node runner/run.mjs --profile icp-1.0-core --iut reference-demo
```

Expected output:

```
[01-aid-derivation] PASS — AID matches expected
[01-aid-derivation] PASS — signature verifies under expected pubkey
1/1 vectors PASS
```

Exit code 0 on full pass; non-zero with details on any failure.

## Profiles

| Profile | Vectors | Audience |
|---|---|---|
| `icp-1.0-core` | identity, signing, canonicalization, intent verbs | every ICP implementation |
| `icp-1.0-settler` | escrow lifecycle, SettlementReceipt, POR | Settler operators only |
| `icp-1.0-handler` | HTTP/MCP/gRPC binding semantics | handler implementations |

## IUT adapter protocol

An adapter MUST be invocable as:

```sh
<adapter-command> <test-name>
```

It reads a JSON object from **stdin** containing the test's `inputs`,
runs the implementation against those inputs, and writes a JSON object
to **stdout** containing the implementation's outputs. The runner
compares stdout JSON to the test's `expected.json`.

Adapters MUST NOT print to stdout anything other than the result JSON.
Logs go to stderr.

See `iut-adapters/iut.protocol.md` for the full contract.

## Adapters in this release

| Adapter | Implementation under test | Status |
|---|---|---|
| `reference-demo`   | `icp-spec/examples/01-aid-and-sign/demo.mjs` — JS, `node:crypto` | **icp-1.0-core: 2/2 PASS** |
| `stateset-rust`    | `crates/stateset-icp-iut` — Rust, `ed25519-dalek` + `x25519-dalek` + `serde_jcs` | **icp-1.0-core: 2/2 PASS** |
| `stateset-go`      | `crates/stateset-icp-iut-go` — Go, pure stdlib (`crypto/ed25519` + `crypto/ecdh`) | **icp-1.0-core: 2/2 PASS** |
| `stateset-python`  | `crates/stateset-icp-iut-py` — Python, `cryptography` library + stdlib | **icp-1.0-core: 2/2 PASS** |

The **four** adapters are completely independent (different languages,
different cryptography libraries, different canonicalization
implementations) and produce **byte-identical** wire bytes for every
vector input. CI (`.github/workflows/icp-conformance.yml`) enforces
this cross-IUT determinism on every PR.

The four languages span ICP's three core developer audiences:
- **Rust** — high-performance protocol stewards (reth/foundry tier engineering)
- **Go** — high-throughput backend systems (Stripe/Cloudflare class)
- **Python** — the agent-developer ecosystem (Anthropic SDK, OpenAI
  SDK, LangChain, LangGraph all Python-primary)
- **JavaScript** — frontend, browser, edge, and Node.js services

If your ecosystem isn't represented and you'd like to add an IUT,
write a thin adapter against `iut.protocol.md`, run the conformance
suite, and submit a PR. Two existing IUTs review your submission.

## Adding a new IUT

1. Write an adapter in any language. It needs to read inputs JSON from
   stdin and write outputs JSON to stdout per `iut.protocol.md`.
2. Add a row to `iut-adapters/registry.json`.
3. Run `node runner/run.mjs --iut <your-adapter>`.
4. If you pass all vectors in a profile, you may publish your conformance
   result. Submit it to the public dashboard via PR to `dashboard/`.

## Versioning

The suite is versioned **independently of the spec**. Suite version is
`<spec-major>.<spec-minor>.<suite-patch>`. Suite patches are
backward-compatible within a spec major version: a passing impl on
`1.0.5` will still pass on `1.0.7`. Adding new tests is a minor bump
(e.g. `1.0` → `1.1`); changing semantics of an existing test is a major
bump and breaks compatibility.

The current release is `icp-conformance 1.0.0` covering ICP-1.0.

## Why this design

We considered embedding conformance tests inside each implementation.
Rejected because:

- Implementations would write tests they can pass, not tests that
  expose ambiguity in the spec.
- "Conformant" claims would be self-graded.
- A second-implementation team would have to extract tests from the
  reference implementation's harness, which is friction-heavy.

A separate, vector-driven, language-agnostic suite eliminates all three
problems. It is the same model used by NIST CAVP for crypto, IETF for
TLS, W3C for the Web Platform Tests.
