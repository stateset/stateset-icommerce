# SQLite operations recovery runbook

This runbook covers a file-backed StateSet iCommerce store. It is designed for
an incident where the active database is unavailable, damaged, or must be
rolled back to a known-good backup. Never restore over a database that a live
`Commerce` process still has open.

## Preparation

- Back up with `commerce.maintenance().backup_to(...)`; do not copy a live
  SQLite file with `cp`. The API uses `VACUUM INTO` to capture committed WAL
  pages consistently.
- Store the `.db` and adjacent `.manifest.json` together in operator-owned,
  access-controlled storage. The manifest binds the backup with SHA-256 and
  records its schema and engine version.
- Regularly run the `Operations Recovery Evidence` workflow. Retain its
  `operations-recovery-<commit>-<attempt>` artifact with release evidence.

## Restore procedure

1. Declare the incident, stop writers, and preserve the active database plus
   its `-wal` and `-shm` sidecars for investigation. Record timestamps, engine
   version, store identifier, and the selected backup checksum.
2. Restore to a **new path** with
   `commerce.maintenance().restore_from(backup, target, options)`. Keep checksum
   verification enabled and `allow_newer_schema` disabled. A newer-schema
   rejection means the operator must use a compatible engine build.
   Native restore checks the exact staged bytes, SQLite integrity, actual
   migration metadata, and the manifest before publishing. Checksum bypass for
   a lost manifest still enforces integrity and the actual schema version.
   Existing files (including empty placeholders) require explicit native
   `overwrite`; symlinks and targets with `-wal`, `-shm`, or `-journal` entries
   are always refused. Stop every connection before overwriting an offline
   database. Do not remove journals from an active database to force a restore.
3. Open the restored path with a new `Commerce` instance. Run
   `PRAGMA integrity_check` and require the single result `ok`.
4. Verify the manifest schema version and migration count, then validate
   business invariants: representative customer/order reads, inventory totals,
   open payment/return work, and the audit/event tail.
5. Put the restored store behind a read-only canary first. Re-enable one writer,
   observe errors and invariants, then progressively restore traffic.
6. Keep the original files until the incident review and retention window are
   complete. Record the restore checksum, approver, test evidence, and cutover
   time in the incident log.

## CLI database manager recovery

`DatabaseManager.backup()` and `restore()` are asynchronous: **await both**.
Backup uses the native engine's consistent snapshot API, returns `manifestPath`
and `manifest` alongside `source`, `backup`, and `size`, and leaves a cached
source connection usable. It can also snapshot a managed `:memory:` store.
Keep the database and manifest together and immutable during recovery.

```javascript
import { DatabaseManager } from '@stateset/cli';

const manager = new DatabaseManager({ defaultPath: './store.db' });
const snapshot = await manager.backup(undefined, './backups');
const recovered = await manager.restore(snapshot.backup, './recovered-store.db');
// Validate recovered.restored, then coordinate application cutover separately.
```

Manager restore requires a **new file path**. It refuses existing files,
including empty files, symlinks, and SQLite WAL/SHM/journal sidecars. It always
verifies the manifest checksum and rejects a newer schema. Recovery first
writes to a temporary directory beside the destination, then publishes the
complete file with a hard link that cannot replace an existing destination.
The destination filesystem must support hard links. This replaces the previous
synchronous raw-file-copy behavior; callers must await completion and choose a
new recovery path instead of overwriting their active store.

The manager does not coordinate external processes or switch application
traffic. Reserve the destination for recovery and keep other readers and
writers away until validation and cutover are complete. Use the native engine
for maintenance; do not open a live commerce file through a separately bundled
SQLite library in the same process. Run auxiliary SQL inspection in a separate
process.

## Migration rollback procedure

The built-in engine schema is forward-only. Restore a verified pre-upgrade
backup to reverse an engine schema upgrade. The rollback procedure below is
for a separate application migration registry with explicit `down_sql`.

Application schema migrations are transactional and checksum-validated. Before
a production migration, rehearse migrate→rollback→remigrate against a recent,
sanitized restore. A rollback is available only when every migration above the
target supplies `down_sql`.

1. Stop writers and take a verified backup.
2. Run the migration against the rehearsal store and verify its status and
   application invariants.
3. Roll back to the explicitly approved version. Verify rollback order, schema
   status, retained data, and `PRAGMA integrity_check`.
4. Remigrate the rehearsal store and repeat the checks. If any check fails,
   abort the production change and restore from the pre-migration backup.
5. In production, prefer a forward fix once new-version writes have occurred;
   destructive down migrations may discard columns or tables even when they
   execute successfully.

## Automated drill

The drill terminates real SQLite worker processes at two boundaries: after
dirty pages spill into the WAL before commit, and after a governed reservation
commits before the caller receives its receipt. Reopening must discard the
first transaction and replay the second without another reservation or event.
Four independent processes also retry 64 exact fractional reservations (256
attempts), verifying 64 effects, stock conservation, the receipt audit chain,
SQLite integrity, and foreign keys. This is a bounded contention check, not a
production throughput SLO or a host power-loss test. The default SQLite pool
uses WAL with `synchronous=NORMAL`; host power-loss durability requires a
separately reviewed storage and synchronous-mode configuration.

### Choose and inspect durability

Set `STATESET_SQLITE_SYNCHRONOUS=FULL` before starting the application to sync
the WAL on each commit. This applies to `SqliteDatabase::new`, including native
bindings, and to every connection the pool opens. `NORMAL` retains the existing
default; invalid settings, including `OFF`, fail before opening the store.
In Rust, use `SqliteDatabase::new_with_durability(&config,
SqliteDurability::Full)` to select a policy without changing the environment.
`db.storage_status()` reports the effective journal mode, numeric synchronous
mode (`1` NORMAL, `2` FULL), lock timeout, and WAL checkpoint threshold.

`FULL` depends on the filesystem and device honoring synchronization. It does
not certify host power-loss behavior. Revalidate after storage or deployment
changes. Shutdown now returns an error if a reader or writer blocks the WAL
checkpoint; close the other connections and retry rather than assuming the
checkpoint completed.

The drill bounds SQLite storage with `max_page_count` and forces `SQLITE_FULL`
during refund settlement. It verifies rollback, exact money, integrity and a
retry that applies once. This never fills the host disk and does not simulate
every OS/filesystem I/O error.

Refund retry keys also compare the payment, explicit exact amount, reason,
external identifier and notes. Changed fields return a conflict on both
backends. An omitted amount retains the original resolved remaining amount on
replay; it does not recalculate against the later payment balance. Concurrent
identical refund requests produce one refund and one creation event.

For a larger local workload, run
`STATESET_RECOVERY_KEYS=10000 cargo test --locked -p stateset-db --test sqlite_process_recovery multiple_processes -- --nocapture`.
The worker workload also records per-operation latency samples and reports
p50/p95/p99/max in microseconds, excluding worker startup. These samples mix
first executions and receipt replays. Overall elapsed time includes startup.
The standard evidence drill uses 64 keys and one round; its JSON retains
latencies, actual synchronous mode, toolchain, host and source provenance.
The hashed workload log retains raw samples; the evidence runner independently
recomputes the reported percentiles and rejects inconsistent observations.

Use `STATESET_RECOVERY_ROUNDS=10 STATESET_RECOVERY_KEYS=256` for repeated
contention: four processes make 10,240 attempts for 256 effects and 9,984
replays. Run separately with `STATESET_SQLITE_SYNCHRONOUS=NORMAL` and `FULL`
on the same hardware. Keys must be 1..100,000, rounds 1..1,000, and their product
at most 1,000,000; workers have a bounded completion timeout. Retain logs and
resource measurements and record errors rather than dropping failed runs.
These observations do not define a production SLO or complete a deployment soak.

The drill also exercises authenticated HTTP order creation, payment and partial
refund in a file-backed tenant store. Retry receipts live in that same tenant
database, so backing up the tenant preserves both business data and completed
retry responses. The tests verify replay after restart and restore, permission
checks before replay, and a response-storage failure after payment creation.

An unresolved retry receipt has `response_status = 0` and never expires. Stop
the original worker and reconcile the business record before modifying that
receipt. A retry with the same key returns 409; using a new key risks a second
mutation. A completed response can be recorded with the idempotency repository's
`complete` operation once verified. Delete a pending receipt only after proving
the original mutation did not occur and the original worker cannot resume.

### Inspect and resolve pending receipts

The native operator example opens an existing store without creating it or
running migrations. Inspection is read only and tenant scoped (an empty tenant
is explicit for single-tenant stores); it returns at most 1,000 oldest pending
receipt summaries, including their exact reservation timestamps:

```bash
cargo run --locked -p stateset-db --example http_receipts -- ./store.db list 'tenant:merchant'
```

Library callers can use `HttpIdempotencyRepository::list_pending` on SQLite
and PostgreSQL, or SQLite's `reconciliation::pending_http_receipts` on a read-only
connection. Inspection does not expire completed responses or change pending
state. An old timestamp alone does not establish that an operation failed.

After stopping the original worker and verifying business/provider records,
prepare operator-owned JSON matching `reconciliation::HttpReceiptResolution`:
`tenant`, `idempotency_key`, `request_fingerprint`, `reserved_at` (RFC3339 with
the original precision), `response_status` (200..599), `content_type` (or null),
`response_body` (the verified original bytes as a JSON byte array), `actor`, and
`evidence` (an incident/provider-evidence reference without secrets).

```bash
# Preview; opens the store read only.
cargo run --locked -p stateset-db --example http_receipts -- ./store.db resolve ./resolution.json
# Apply only after stopping the original worker and verifying the outcome.
cargo run --locked -p stateset-db --example http_receipts -- ./store.db resolve ./resolution.json --apply --worker-stopped
```

SQLite completion compares tenant, original key, fingerprint, exact reservation
time and pending status atomically. It writes the verified response and a
required audit entry in one transaction, with `FULL` synchronization in the
operator example. Audit failure leaves the receipt pending; stale evidence or
a second completion returns a conflict. Audit details retain operator/evidence,
tenant, fingerprint, original time, status and response hash, not the response
payload. No mutation is re-executed, and no pending receipt is deleted.

The helper cannot prove worker termination or external settlement. It is an
operator-only API, outside the MCP tool catalog; keep its database and evidence
files access controlled. PostgreSQL has pending inspection and its existing
fingerprint-checked completion API; this audited operator helper is SQLite only.

Canonical migration `101_audit_log` installs the helper's table and indexes on
new and upgraded engine stores, preserving compatible pre-existing history.
Stores that predate this migration must be opened through the engine or the
canonical migrator before using the operator helper; inspection/completion
commands deliberately do not run migrations. Missing or incompatible audit
storage causes completion to fail and leaves the receipt pending.

General SQLite audit logging is opt-in and best effort, without automatic
coverage of every repository mutation. Governed receipt audit chains and
domain outbox events are separate mechanisms. Export the process-wide
`stateset_db::audit::audit_write_failures()` monotonic counter and alert on
increases across all stores in the process. A zero count does not prove complete
audit coverage. Operations that require audit atomicity can call
`record_audit_required` inside their business transaction, as reconciliation does.

A backup only contains receipts and business records committed at its snapshot.
Reconcile post-backup external payments before admitting traffic to an older
restore; restoring a database cannot reverse charges at an external processor.

Run locally with:

```bash
./scripts/ci/run_operations_recovery_drill.sh
```

The command requires Python 3.8+, Git, and Rust on a Unix host. It creates
`artifacts/operations-recovery/evidence.json`, a summary, and logs under a unique
`runs/<invocation-id>/` directory. Override the output directory with
`RECOVERY_EVIDENCE_DIR`. A process lock prevents concurrent runs from overwriting
one another's evidence in the same directory.

The schema version 3 report starts as `running` before tool lookup or tests,
replacing any previous success. Normal failures record `failed`; SIGINT and
SIGTERM record `cancelled` and terminate the active check. An uncatchable kill
may leave `running`, which is not passing evidence. Each check records its exact
command, outcome, exit status when available, and the SHA-256 of its current log.
Only logs referenced by the current manifest belong to that invocation. A
successful exit without the expected proof markers and successful test counts
cannot pass the drill.

The report records the actual checkout commit and whether its source is dirty.
When `GITHUB_SHA` is supplied, it must match that checkout. Before and after the
checks, the runner fingerprints tracked files (including deletions and executable
bits), symlink targets, and non-ignored untracked files. Both fingerprints and
checkout commits must match for a passing result. Generated evidence is excluded
from the fingerprint and dirty status. Ignored files, external dependencies, and
changes made and reverted between snapshots are outside this check; it establishes
source provenance, not a reproducible build or protection against hostile edits.

Local output is diagnostic only; release evidence must link the immutable GitHub
Actions run and its retained artifact. A dirty local run does not certify the
committed revision by itself.
