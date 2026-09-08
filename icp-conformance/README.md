# icp-conformance

Black-box conformance test suite for the Intelligent Commerce Protocol.
**Implementation-agnostic.** Any ICP implementation that wants the
"ICP-1.0 conformant" badge runs this suite against itself and publishes
the result.

The suite intentionally lives here, in the spec stewardship repo, rather
than inside any single implementation. That separation is what converts
"ICP" from a product into a protocol with multiple implementers.

## Status

ICP-1.0 — four vector families, four implementations (JS · Rust · Go ·
Python), all byte-identical in CI:

| Family | Covers | Cases |
|---|---|---|
| `01-aid-derivation` | §4.2 identity, §6.1 intent signing | 6 fields |
| `02-canonical-json` | RFC 8785 canonicalization | 20 sub-cases |
| `03-signature-verification` | §4.1/§5.2 envelope verification | 8 sub-cases |
| `04-escrow-lifecycle` | **§8 operational semantics**: full 30-cell transition matrix + 10 event-replay cases | 40 sub-cases |
| `05-intent-validation` | **§6 intent envelope**: structural validation across all 7 verbs, 7 error codes | 21 sub-cases |
| `06-quote-binding` | **§11.4 economic safety**: exact-decimal `max_total` ceiling (the quote-binding-attack mitigation) | 14 sub-cases |
| `07-settlement-receipts` | **§9 proof of payment**: co-signed (settler + receiving party) receipt verification | 8 sub-cases |
| `08-timing` | **§5.3 replay window**: strict-parse timestamps + `exp − iat ≤ 600s` + `exp < now` expiry | 10 sub-cases |
| `09-ceilings` | **§6.2/§6.6 economic safety**: `max_refund` and `max_per_payout` exact-decimal ceilings | 10 sub-cases |

`04`–`09` are the families that test what the protocol *does* rather than how
it hashes — two implementations that disagree on one escrow transition
disagree about who holds the money; two that disagree on whether an intent is
well-formed can't complete the handshake; two that compare `max_total` as
floats-or-strings rather than exact decimals will disagree on whether a Quote
is an overcharge (case `06/c09`: `9.9` ≤ `10.0`, which naive string comparison
gets wrong); two that disagree on a co-signed SettlementReceipt disagree
about whether a payment actually happened — the receipt is the §9 canonical
proof that tax and audit systems MUST treat as authoritative; and two that
parse timestamps leniently or botch the window arithmetic disagree on whether
a replayed, expired intent is still live. Remaining before ICP-1.0 Last Call:
the subscription `max_total_per_period` ceiling (§6.2 — same comparator),
intent-verb request/response flows (stateful counterparty responses), and the
`iat_in_future` clock-skew rule once §5.3 pins a value.

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

### Flags

| Flag | Default | Effect |
|---|---|---|
| `--profile <name>` | `icp-1.0-core` | Which profile in `profiles/` to run |
| `--iut <name>` | `reference-demo` | Which registry entry to exercise |
| `--vector <name>` | _(whole profile)_ | Run a single vector |
| `--registry <path>` | `iut-adapters/registry.json` | Point at an out-of-tree registry (adapters that live outside this repo) |
| `--fail-on-skip` | off | Treat a SKIP as a failure |
| `--verbose` | off | Print every matched field |

`--fail-on-skip` (equivalently `ICP_CONFORMANCE_FAIL_ON_SKIP=1`) exists
because a SKIP means *the adapter has no handler for that vector*. Without
the flag an IUT can list a profile in its `supports` array, implement none
of it, and still exit 0 — claiming a profile and passing it would be
different statements. CI runs every IUT with `--fail-on-skip` for both
profiles, so they are the same statement here.

## Profiles

The profiles that ship in `profiles/`:

| Profile | Vectors | Audience |
|---|---|---|
| `icp-1.0-core` | 01–09: identity, canonicalization, signatures, escrow lifecycle, intent verbs, quote binding, settlement receipts, timing, ceilings | every ICP implementation |
| `icp-1.0-commerce` | 10: commerce invariants — no over-refund/over-capture, returns bounded by shipped units, reservations that cannot oversell, balanced journal entries, currency scale (37 cases) | implementations that execute commerce, layered on top of `icp-1.0-core` |

All four adapters below pass both profiles with zero skips, gated per IUT
in CI.

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
| `reference-demo`   | `icp-spec/examples/01-aid-and-sign/demo.mjs` — JS, `node:crypto` | **core 9/9, commerce 1/1, 0 SKIP** |
| `stateset-rust`    | `crates/stateset-icp-iut` — Rust, `ed25519-dalek` + `x25519-dalek` + `serde_jcs` | **core 9/9, commerce 1/1, 0 SKIP** |
| `stateset-go`      | `crates/stateset-icp-iut-go` — Go, pure stdlib (`crypto/ed25519` + `crypto/ecdh`) | **core 9/9, commerce 1/1, 0 SKIP** |
| `stateset-python`  | `crates/stateset-icp-iut-py` — Python, `cryptography` library + stdlib | **core 9/9, commerce 1/1, 0 SKIP** |

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
3. Run `node runner/run.mjs --iut <your-adapter> --fail-on-skip`. (If your
   adapter lives outside this repo, keep your own registry file and pass
   `--registry <path>` instead of editing this one.)
4. If you pass all vectors in a profile **with zero skips**, you may publish
   your conformance result. Submit it to the public dashboard via PR to
   `dashboard/`.

## Runner tests

The runner is itself gated: `node --test test/runner.test.mjs` covers the
skip/exit-code contract and asserts the reference IUT completes both
profiles with no skips.

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
