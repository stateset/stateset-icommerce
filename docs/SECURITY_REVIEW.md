# Independent security review handoff

Status: scope prepared; independent review and findings are outstanding. Passing
repository tests or dependency policy checks does not complete this review.
Record the reviewed commit, reviewer, report, and remediation verification in
`docs/RELEASE_EVIDENCE.md` before claiming independent audit coverage.

## Trust boundaries and review targets

| Boundary | Review targets | Acceptance evidence |
| --- | --- | --- |
| Remote agent to MCP HTTP | `cli/bin/stateset-mcp-http.js`, `cli/src/mcp/http-origins.js`, `http-api-keys.js` | Host rebinding refusal; exact scheme/host/port Origin matching; malformed/duplicate Origin rejection; authentication before tools; no key or response payload leakage |
| Model input to governed writes | `cli/src/kernel-config.js`, `kernel-tool-execution.js`, `mcp/tool-wrappers.js`, `mcp/v2-server.js` | Operator-owned principal/policy; original schema refinements; preview leaves store unchanged; authorization enforced before replay; ungoverned writes unavailable in strict mode |
| Native ABI to engine | `crates/stateset-ffi/src`, `crates/stateset-ffi/abi-fixtures` | Invalid/stale handles, ownership/lifetime and panic boundaries; decimal/currency/variant validation; C/C++/Python/Swift ABI checks |
| Backup bytes to active database | `crates/stateset-db/src/maintenance.rs`, `backup.rs` | Staged-image checksum/schema/integrity validation; symlink and sidecar refusal; concurrent publication; offline overwrite constraints and crash orphans |
| Operator resolution to retry receipt | `crates/stateset-db/src/reconciliation.rs`, `crates/stateset-db/examples/http_receipts.rs` | Tenant/key/fingerprint/reservation-time compare-and-set; required audit in same transaction; stale or invalid evidence cannot overwrite; no automatic re-execution or deletion |
| General audit coverage | Canonical migration `101_audit_log`, `crates/stateset-db/src/audit.rs`, repository call sites | Verify table upgrade/history preservation; distinguish opt-in general audit entries from governed receipts/domain facts; inventory mutations without audit coverage before making universal audit claims |
| Mutation to durable evidence | Both `kernel_outbox.rs` helpers, both customer repositories; `MUTATION_AUDIT_COVERAGE.md` | Reject silently suppressed facts, receipts and chain rows; preserve preview on failed promotion; customer edits, deletion, anonymization and address/default changes roll back when their fact cannot persist; inspect historical creation-email retention |
| Refund retry identity to money movement | `crates/stateset-db/src/refund_replay.rs`, both payment repositories | Changed payment, explicit amount or immutable details conflict; concurrent identical requests produce one refund/event; omitted amounts replay the originally resolved amount |
| Dependency source to shipped package | `Cargo.lock`, `supply-chain/`, `deny.toml`, `.github/workflows/ci.yml`, publish workflows | Identify unreviewed code reachable from default and optional builds; validate exemption scope and expiry; inspect provenance and fresh-consumer installs |

## Adversarial scenarios

- Attempt browser requests with a downgraded scheme, wrong port, opaque origin,
  duplicate headers, normalized path tricks, and hostile Host values. A full
  `--allowed-origin` URL pins the origin. A bare hostname is an explicit legacy
  rule allowing HTTP(S) on any port; flag broad rules during deployment review.
- Supply identity, policy or approval fields through model arguments. Verify
  they cannot replace operator configuration, expand capabilities or enable writes.
- Interrupt execution after business commit and before receipt persistence.
  Retrying the original key must preserve uncertainty or replay its response;
  it must never silently execute a second mutation.
- Exhaust SQLite's bounded page capacity during refund settlement, reject a
  ledger/subscription outbox insert, and reject a reconciliation audit insert.
  Inspect persisted business state after each failure, then retry.
- Restore an older database while an external payment remains settled. Verify
  the operator reconciles provider records before reopening writes. Local
  idempotency cannot undo or discover every external effect.

## Automated evidence to retain

Run the focused transport/security batch with `npm --prefix cli run test:mcp-safety`.

- MCP: `cli/test/unit/mcp-http-origins.test.js`, `mcp-http-api-keys.test.js`,
  `cli/test/integration/mcp-http-sandbox.test.js`, `mcp-stdio-protocol.test.js`,
  and strict-kernel/adversarial contract CI. Retain the `catalog-validation-proof`
  diagnostics: both HTTP/stdio protocol eras test every core tool advertising
  required inputs for a structured empty-input refusal and unchanged store.
  `mcp-v2-server.test.js` applies the same refusal contract to every required-input
  schema in the full catalog with all handlers replaced, so external/demo tool
  bodies never execute. This validates argument handling, not external services.
- Storage and reconciliation: `Operations Recovery Evidence`, including its
  source fingerprint, log hashes, storage-policy checks, refund storage failure,
  receipt reconciliation, and process latency observations.
- Native ABI and dependency policy: immutable FFI coverage/ABI, cargo-vet,
  cargo-deny, security-audit, SBOM and release-provenance workflow artifacts.

Classify findings by reachable impact and preconditions, attach a reproducer,
assign an owner, and require independent verification of remediation. Track
accepted risks with explicit scope, rationale, expiry, and operator guidance.
Never relabel bootstrap dependency exemptions as completed source audits.

## Remaining external evidence

An independent reviewer must supply the review report. Controlled host/storage
power-cut experiments and sustained deployment load/soak results must identify
hardware, filesystem, sync configuration, workload, recovery time, acknowledged
write loss, latency percentiles and errors. Repository fault injection and local
measurements do not establish those deployment guarantees.
