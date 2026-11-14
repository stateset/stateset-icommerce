# icp-conformance

Black-box conformance test suite for the Intelligent Commerce Protocol.
**Implementation-agnostic.** Any ICP implementation that wants the
"ICP-1.0 conformant" badge runs this suite against itself and publishes
the result.

The suite intentionally lives here, in the spec stewardship repo, rather
than inside any single implementation. That separation is what converts
"ICP" from a product into a protocol with multiple implementers.

## Status

ICP-1.0 — ten vector families in two profiles, four implementations (JS ·
Rust · Go · Python), all byte-identical in CI:

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
| `10-commerce-invariants` | **commerce execution** (`icp-1.0-commerce` profile): no over-refund/over-capture, returns bounded by shipped units, reservations that cannot oversell, balanced journal entries, currency scale | 37 sub-cases |

`01`–`09` make up the `icp-1.0-core` profile; `10` is the `icp-1.0-commerce`
profile (see [Profiles](#profiles)). Each family directory holds a
`description.md` stating what it tests and the adapter's input/output shape.

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
ICP Conformance Runner — profile=icp-1.0-core iut=reference-demo vectors=9
Implementation: icp-spec/examples/01-aid-and-sign/demo.mjs (deterministic mode)

[01-aid-derivation] PASS — 6 fields match expected
[02-canonical-json] PASS — 2 fields match expected
[03-signature-verification] PASS — 2 fields match expected
[04-escrow-lifecycle] PASS — 2 fields match expected
[05-intent-validation] PASS — 1 field match expected
[06-quote-binding] PASS — 1 field match expected
[07-settlement-receipts] PASS — 1 field match expected
[08-timing] PASS — 1 field match expected
[09-ceilings] PASS — 1 field match expected

Result: 9 PASS, 0 FAIL, 0 SKIP (of 9 total)
```

Exit code 0 on full pass; non-zero with details on any failure.

The runner needs only Node.js (>= 20.20.0) — no dependencies and no build
step. `reference-demo` is pure JavaScript and runs as-is; the other three
adapters in the shipped registry point at binaries built elsewhere in the
StateSet monorepo and are not usable outside it.

### Flags

| Flag | Default | Effect |
|---|---|---|
| `--profile <name>` | `icp-1.0-core` | Which profile in `profiles/` to run |
| `--iut <name>` | `reference-demo` | Which registry entry to exercise |
| `--vector <name>` | _(whole profile)_ | Run a single vector |
| `--registry <path>` | `iut-adapters/registry.json` | Point at an out-of-tree registry (adapters that live outside this repo) |
| `--report <path>` | _(none)_ | Also write a JSON conformance report to `<path>` (see [`CONFORMANCE-REPORT.md`](CONFORMANCE-REPORT.md)); stdout and the exit code are unchanged |
| `--fail-on-skip` | off | Treat a SKIP as a failure |
| `--verbose` | off | Print every matched field |
| `--version` | — | Print the suite version and exit |

Exit codes: `0` every selected vector passed; `1` at least one failed (or was
skipped, under `--fail-on-skip`); `2` runner error (unknown IUT or profile,
unreadable registry, unwritable report).

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

## Testing your own implementation

You do not need to edit anything in this directory, or build anything else
in the repository it lives in.

1. Write an adapter in any language. It reads the vector's inputs JSON from
   stdin and writes its outputs JSON to stdout, per
   [`iut-adapters/iut.protocol.md`](iut-adapters/iut.protocol.md). Each
   family's `description.md` gives the exact input and output shape.
2. Describe it in a registry file of your own, kept next to your adapter.
   Use **absolute paths** in `command`: the runner starts every adapter with
   this suite's directory as the working directory, so a relative path is
   resolved against the suite, not against your registry file.

   ```json
   {
     "my-impl": {
       "command": ["/abs/path/to/my-adapter"],
       "language": "zig",
       "implementation": "my-impl 0.3.1 (commit abc1234)",
       "version": "0.1.0",
       "supports": ["icp-1.0-core"]
     }
   }
   ```

3. Run each profile you claim, with skips fatal, and keep the report:

   ```sh
   node runner/run.mjs --registry /abs/path/to/registry.json --iut my-impl \
     --profile icp-1.0-core --fail-on-skip --report results/my-impl-core.json
   ```

   `results/` is git-ignored here. Use `--vector <name>` to iterate on one
   family at a time.
4. If every vector in a profile passes **with zero skips** (exit code 0 under
   `--fail-on-skip`), you may state that your implementation passes that
   profile of `icp-conformance <suite version>`. Publish the JSON report next
   to the claim — in your own repository or release notes, or attached to an
   issue or pull request on this repository if you would like the result
   reviewed. The report is what makes the claim checkable: it names the suite
   version, the profile, and the result of every vector. There is no hosted
   results dashboard.

To contribute an adapter to this repository instead, add it to
`iut-adapters/registry.json` and send a pull request with the reports
attached.

### Using the suite outside this repository

`package.json` describes the suite as `@stateset/icp-conformance` with an
`icp-conformance` bin, so a checkout or tarball of this directory is
self-contained: the vectors, profiles, runner, adapter contract and the
`reference-demo` adapter travel together, and `icp-conformance --version`
reports which release you are running. The package is marked `private` and
has **not been published to a registry**; until it is, take it from this
repository (clone it, or `npm pack` this directory and install the tarball).

## Runner tests

The runner is itself gated: `node --test test/runner.test.mjs` covers the
skip/exit-code contract, `--version`, the `--report` document (including a
deliberately failing adapter fixture), and asserts the reference IUT
completes both profiles with no skips.

## Versioning

The suite is versioned **independently of the spec**. Suite version is
`<spec-major>.<spec-minor>.<suite-patch>`. Suite patches are
backward-compatible within a spec major version: a passing impl on
`1.0.5` will still pass on `1.0.7`. Adding new tests is a minor bump
(e.g. `1.0` → `1.1`); changing semantics of an existing test is a major
bump and breaks compatibility.

The current release is `icp-conformance 1.0.0` covering ICP-1.0. The version
lives in `package.json`; `node runner/run.mjs --version` prints it and every
`--report` document records it.

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
