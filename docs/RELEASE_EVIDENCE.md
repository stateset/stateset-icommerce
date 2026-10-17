# Release evidence checklist

Use this file as the release-candidate handoff. Link immutable CI artifacts;
do not mark a row complete from a local run alone.

| Gate                           | Required evidence                                                                                                                                                                                                         | Status  |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| Rust correctness               | CI URL for workspace tests, clippy, format, and invariant suite                                                                                                                                                           | Pending |
| Database coverage              | `database-coverage-<commit>-<attempt>` artifact; database-only line floor and independent backup/restore floors from `scripts/ci/check_db_coverage.py`                                                                    | Pending |
| Performance                    | `Perf Regression Gate` run URL and `perf-regression-<commit>-<attempt>` artifact with runner/toolchain provenance and raw Criterion estimates                                                                             | Pending |
| Binding compatibility          | Cross-binding vectors and generated binding inventory artifact                                                                                                                                                            | Pending |
| Storefront golden path         | `Storefront Golden Path` run URL and `storefront-golden-path-<commit>-<attempt>` artifact proving packed CLI, install, seed/query, launch doctor, private native merchant journey, audit, typecheck, and production build | Pending |
| Exact money                    | Boundary tests covering parse, serialization, arithmetic, and refunds                                                                                                                                                     | Pending |
| SQLite/Postgres parity         | Backend parity matrix artifact                                                                                                                                                                                            | Pending |
| MCP catalog                    | Generated catalog/profile counts and command-coverage artifact                                                                                                                                                            | Pending |
| MCP safety                     | Preview/apply, strict-kernel, auth, Host, and Origin negative tests                                                                                                                                                       | Pending |
| Storefront settlement          | Invalid token/from/to/amount, reverted, low-confirmation, and replay tests                                                                                                                                                | Pending |
| Protocol claims                | Versioned external conformance-suite artifact for every claimed protocol                                                                                                                                                  | Pending |
| Supply chain                   | SBOM, dependency audit, provenance/signature, and package smoke installs                                                                                                                                                  | Pending |
| Independent security review    | Reviewer/report for the release commit, findings and remediation verification; scope: [`SECURITY_REVIEW.md`](SECURITY_REVIEW.md)                                                                                          | Pending |
| Deployment durability and soak | Hardware/filesystem/sync policy, sustained workload and latency/error report, recovery-time and acknowledged-write-loss measurements from controlled storage failure                                                      | Pending |
| Operations                     | `Operations Recovery Evidence` run URL and `operations-recovery-<commit>-<attempt>` artifact; runbook: [`operations-recovery.md`](src/guides/operations-recovery.md)                                                      | Pending |

The operations artifact must include the `sqlite_process_recovery` check: real
worker termination before commit and after commit before acknowledgment, plus
four-process retry contention with exact stock and receipt-chain invariants.
It must also retain storage-exhaustion/checkpoint checks, audited receipt
reconciliation checks, and structured per-operation latency observations.
This bounded workload does not complete production load/soak, host power-loss,
external settlement, or independent security review requirements.

Release notes must identify skipped gates and their user-visible impact. A
planned feature, adjacent repository, or skipped platform is never counted as
passing evidence.

Storefront evidence must be initialized before work begins, retain the invocation
identity, bind its log hash, and match source fingerprints before/after the run.
The full level is `install-seed-doctor-journey-typecheck-build`; scaffold-only
checks cannot substitute for it. Native merchant settlement is simulated and
does not satisfy live provider verification. Inventory simulations likewise
prove conditional engine behavior, not demand forecast accuracy.
