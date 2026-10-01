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
