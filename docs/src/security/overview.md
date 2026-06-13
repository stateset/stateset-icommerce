# Security Overview

This page is the single landing for everything security-related in
StateSet iCommerce. It summarises the layered defenses and points you
to the canonical documents for each one.

## At-a-glance

| Layer                      | Tool / Practice                              | Where it runs           | Status |
| -------------------------- | -------------------------------------------- | ----------------------- | ------ |
| Memory safety              | `#![deny(unsafe_code)]` on all core crates   | rustc                   | ✓      |
| Panic hygiene              | `#![deny(clippy::unwrap_used)]` in 19 crates | clippy                  | ✓      |
| Lint posture               | clippy `pedantic` + 15 explicit lints        | CI                      | ✓      |
| Linker isolation           | OpenSSL banned, rustls only                  | `deny.toml`             | ✓      |
| Dep advisories             | `cargo-audit` against [RustSec][rustsec]     | CI on every PR          | ✓      |
| License + bans             | `cargo-deny`                                 | CI on every PR          | ✓      |
| Supply-chain audits        | `cargo-vet` + 6 trusted feeds                | `supply-chain/`         | ✓      |
| Updates                    | Dependabot (8 ecosystems)                    | weekly                  | ✓      |
| SBOM                       | CycloneDX via `anchore/sbom-action`          | CI on push to main      | ✓      |
| Static analysis (Rust)     | CodeQL                                       | CI on every PR          | ✓      |
| Static analysis (JS + GA)  | CodeQL                                       | CI on every PR          | ✓      |
| Secret scanning            | `gitleaks`                                   | CI on every push + PR   | ✓      |
| Fuzz coverage              | `cargo-fuzz`, 6 targets (crypto + protocol)  | nightly                 | ✓      |
| Pre-commit gates           | husky: prettier + ESLint + cargo fmt + clippy| local                   | ✓      |
| Signed releases            | sigstore `cosign` keyless on every `v*` tag  | CI on tag               | ✓      |
| Audit log                  | Per-event signed Merkle leaves (VES v1.0)    | runtime                 | ✓      |
| Hybrid signatures          | Ed25519 + ML-DSA-65                          | runtime (PQC feature)   | ✓ soft |
| Hybrid encryption          | X25519 + ML-KEM-768                          | runtime (PQC feature)   | ✓ soft |
| Post-quantum hard finality | (planned)                                    | —                       | ☐      |
| SOC 2 Type I               | (planned)                                    | —                       | ☐      |
| Third-party security audit | (planned)                                    | —                       | ☐      |

[rustsec]: https://rustsec.org

## Reporting a vulnerability

Email **security@stateset.io** — see [SECURITY.md](../../../SECURITY.md)
for the full process, SLA (48 h ack, 7 d initial assessment, 30 d
critical fix target), supported-versions table, and historical advisories.

## Code-level guarantees

- `#![deny(unsafe_code)]` is enforced on every core crate. Unsafe code
  exists only in the FFI bindings under `crates/stateset-ffi/`, where
  it is unavoidable and reviewed line-by-line.
- `cfg_attr(not(test), deny(clippy::unwrap_used))` is enforced on
  19 crates so production code can't panic on `.unwrap()` or `.expect()`
  on a `Result`.
- Workspace lints inherit `clippy::pedantic` plus 15 explicit lints
  including `dbg_macro`, `unimplemented`, `manual_string_new`,
  `redundant_clone`, `use_self`. See `Cargo.toml` `[workspace.lints]`.

## Cryptography (VES v1.0)

The Verifiable Encrypted Signatures protocol underpins the audit log,
event sync, and agent commerce primitives.

- [VES v1.0 Specification](ves.md) — full protocol spec, hashing
  domains, signing scheme, encryption AAD construction.
- [Security Architecture](architecture.md) — broader security model.
- [ERC-8004 Agent Identity](erc8004-identity.md) — agent registration,
  reputation, and verification flow.

Hybrid signatures (Ed25519 + ML-DSA-65) and hybrid encryption (X25519 +
ML-KEM-768) are available behind the `pqc` feature flag and used by
default in the agentic commerce paths. **Soft finality** today;
post-quantum *hard* finality is the largest planned gap (see below).

## Supply chain

- **`cargo-vet`** (config in `supply-chain/`) imports trusted-feed
  audits from Mozilla, Google, Embark Studios, Bytecode Alliance,
  Zcash, and ISRG. The `supply-chain` workflow runs on every PR
  (advisory mode at bootstrap; tightening over time as exemptions
  shrink).
- **`cargo-deny`** enforces the license allowlist and bans MySQL +
  OpenSSL at the workspace level; `deny.toml` is the source of truth.
- **`cargo-audit`** runs against [RustSec][rustsec] on every PR.
  Two long-standing advisory ignores are documented inline in CI
  with the by-design risk-zero justification.
- **CycloneDX SBOM** is generated and uploaded as an artifact on every
  push to main (`.github/workflows/sbom.yml`).

## Static analysis

- **CodeQL Rust + JavaScript + GitHub Actions** runs on every PR
  (`.github/workflows/ci.yml` `codeql` job, matrix-driven).
- **`gitleaks`** scans the full git history on every push and PR
  (`.github/workflows/gitleaks.yml`).
- **`cargo-fuzz`** runs nightly on three targets on `stateset-crypto`
  (canonicalize_json, compute_payload_plain_hash, compute_merkle_root) —
  90-second soak each, persistent crash artifacts uploaded
  (`.github/workflows/fuzz-nightly.yml`).

## Signed releases

Every annotated `v*` tag triggers `.github/workflows/release-sign.yml`,
which builds a deterministic source tarball + CycloneDX SBOM, signs the
SHA256SUMS with **`cosign` keyless OIDC** (no long-lived signing key;
identity is the workflow run's ephemeral GitHub Actions token, recorded
in the public Rekor transparency log), and attaches everything to the
GitHub Release.

The full verification recipe (`gh release download` → `cosign verify-blob`
→ `sha256sum -c`) is in [SECURITY.md § Signed releases](../../../SECURITY.md#signed-releases-sigstore--cosign).

## Local pre-commit gates

`.husky/pre-commit` runs:

1. `npm run format:check` (Prettier) on all staged files.
2. ESLint on `cli/` and `admin/` if Node is available.
3. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets
   -- -D warnings` *only when staged changes touch `*.rs` or `*.toml`*.
   Skippable via `SKIP_RUST=1 git commit ...` for non-Rust contributors.
   Gracefully no-ops if `cargo` isn't on PATH.

## Known gaps (honest)

In keeping with the [Trust Foundation](../../../TRUST_FOUNDATION.md), we
publish the gaps explicitly so adopters can size them against their own
threat model:

- **Post-quantum hard finality.** The hybrid signature/encryption
  primitives are implemented but the *protocol-level* hard-finality
  requirement (every log root signed under a PQC key, every replay
  re-verifiable post-quantum) is still soft. See
  [`docs/PQC_INITIAL_SPEC.md`](../../../docs/PQC_INITIAL_SPEC.md).
- **SOC 2 Type I.** Audit primitives exist (signed event log, deterministic
  replay, redacted CLI replay log) but no third-party SOC 2 control
  mapping or attestation is in place.
- **Independent third-party security audit.** Nothing has been engaged
  yet. Internal review and the security-tooling stack above are the
  current evidence; an external audit is the next escalation.
- **Formal verification.** The Merkle/sync convergence properties and
  the policy-DSL evaluator are good candidates for TLA+ / Kani / Coq
  but no proofs have been written. Current evidence: the proptest
  suites in `crates/stateset-policy/tests/proptest_operator.rs` and
  `crates/stateset-sync/tests/proptest_conflict.rs`.

## Reference materials

- [`SECURITY.md`](../../../SECURITY.md) — vulnerability reporting
  process, supported versions, advisory history, signed-release
  verification recipe.
- [`TRUST_FOUNDATION.md`](../../../TRUST_FOUNDATION.md) — what we sign,
  what we verify, and the explicit gap inventory.
- [`deny.toml`](../../../deny.toml) — license allowlist, banned crates,
  advisory-handling policy.
- [`supply-chain/`](../../../supply-chain/) — `cargo-vet` config,
  trusted import feeds, exemption ledger.
- The `.github/workflows/` directory — every CI gate above is a
  one-file workflow you can read end-to-end.
