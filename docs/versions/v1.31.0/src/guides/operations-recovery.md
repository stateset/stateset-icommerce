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

## Migration rollback procedure

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

Run locally with:

```bash
./scripts/ci/run_operations_recovery_drill.sh
```

The command creates `artifacts/operations-recovery/evidence.json`, a summary,
and hash-bound logs. Local output is diagnostic only; release evidence must link
the immutable GitHub Actions run and its retained artifact.
