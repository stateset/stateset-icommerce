#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUTPUT_DIR="${RECOVERY_EVIDENCE_DIR:-${REPO_ROOT}/artifacts/operations-recovery}"
mkdir -p "${OUTPUT_DIR}"

BACKUP_LOG="${OUTPUT_DIR}/backup-restore.log"
MIGRATION_LOG="${OUTPUT_DIR}/migration-rollback.log"

cd "${REPO_ROOT}"

echo "Running file-backed backup/restore recovery drill"
cargo test --locked -p stateset-embedded --features sqlite \
  --test maintenance_accessor backup_then_restore_reproduces_the_data \
  -- --exact --nocapture 2>&1 | tee "${BACKUP_LOG}"

echo "Running file-backed migrate/rollback/remigrate rehearsal"
cargo test --locked -p stateset-migrations \
  sqlite::tests::migrate_then_rollback_then_remigrate \
  -- --exact --nocapture 2>&1 | tee "${MIGRATION_LOG}"

if ! grep -q 'recovery-proof .* integrity=ok' "${BACKUP_LOG}"; then
  echo "error: backup/restore proof marker missing" >&2
  exit 1
fi
if ! grep -q 'migration-proof .* data_preserved=true integrity=ok' "${MIGRATION_LOG}"; then
  echo "error: migration rollback proof marker missing" >&2
  exit 1
fi

COMMIT_SHA="${GITHUB_SHA:-$(git rev-parse HEAD)}"
RUN_ID="${GITHUB_RUN_ID:-local}"
RUN_ATTEMPT="${GITHUB_RUN_ATTEMPT:-local}"
CREATED_AT="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
BACKUP_LOG_SHA256="$(sha256sum "${BACKUP_LOG}" | cut -d ' ' -f 1)"
MIGRATION_LOG_SHA256="$(sha256sum "${MIGRATION_LOG}" | cut -d ' ' -f 1)"
RUSTC_VERSION="$(rustc --version)"
CARGO_VERSION="$(cargo --version)"

cat > "${OUTPUT_DIR}/evidence.json" <<EOF
{
  "schema_version": 1,
  "result": "passed",
  "created_at": "${CREATED_AT}",
  "commit_sha": "${COMMIT_SHA}",
  "github_run_id": "${RUN_ID}",
  "github_run_attempt": "${RUN_ATTEMPT}",
  "checks": {
    "backup_restore": {
      "result": "passed",
      "assertions": ["manifest checksum verified", "schema and migration count match", "domain data restored", "PRAGMA integrity_check=ok"],
      "log": "backup-restore.log",
      "log_sha256": "${BACKUP_LOG_SHA256}"
    },
    "migration_rollback": {
      "result": "passed",
      "assertions": ["file-backed migrate", "rollback order verified", "retained data preserved", "remigrate healthy", "PRAGMA integrity_check=ok"],
      "log": "migration-rollback.log",
      "log_sha256": "${MIGRATION_LOG_SHA256}"
    }
  },
  "toolchain": {
    "rustc": "${RUSTC_VERSION}",
    "cargo": "${CARGO_VERSION}"
  },
  "runbook": "docs/src/guides/operations-recovery.md"
}
EOF

cat > "${OUTPUT_DIR}/README.md" <<EOF
# Operations recovery evidence

- Result: **passed**
- Commit: \`${COMMIT_SHA}\`
- Generated: ${CREATED_AT}
- GitHub run: ${RUN_ID}, attempt ${RUN_ATTEMPT}
- Backup/restore log SHA-256: \`${BACKUP_LOG_SHA256}\`
- Migration rollback log SHA-256: \`${MIGRATION_LOG_SHA256}\`
- Runbook: \`docs/src/guides/operations-recovery.md\`

The machine-readable assertion inventory is in \`evidence.json\`. The two raw
test transcripts are hash-bound by that file.
EOF

echo "Operations recovery evidence written to ${OUTPUT_DIR}"
