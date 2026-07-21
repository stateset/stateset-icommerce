//! Backup and restore for embedded SQLite databases.
//!
//! This module provides the recovery half of the embedded deployment story:
//! taking a consistent backup of a live database and restoring it safely.
//!
//! # Why not just copy the file?
//!
//! Copying `store.db` while writers are active produces a torn file: the
//! `-wal` and `-shm` sidecars hold committed pages that are not yet in the
//! main database, and page writes are not atomic with respect to `cp`.
//! [`backup_to`] uses SQLite's `VACUUM INTO`, which runs inside a read
//! transaction and writes a fully self-contained, already-compacted database
//! that is consistent as of the transaction snapshot, even under concurrent
//! writers.
//!
//! # Manifest
//!
//! Every backup is written with a sidecar manifest, `<backup>.manifest.json`,
//! carrying the schema version (the highest applied migration name), migration
//! count, engine version, creation time, source path, byte size and a SHA-256
//! of the backup file. [`restore_from`] verifies the checksum before touching
//! the target.
//!
//! # Restore safety rules
//!
//! 1. The manifest checksum must match the backup file byte-for-byte.
//! 2. A backup whose `schema_version` is *newer* than the newest migration this
//!    binary knows about is refused — restoring forward-migrated data into an
//!    older engine would let it read columns it does not understand and write
//!    rows the newer schema forbids.
//! 3. An existing, non-empty target is never overwritten unless
//!    [`RestoreOptions::overwrite`] is set.
//! 4. The restore is atomic: the backup is copied to a temporary file in the
//!    *same directory* as the target, fsynced, and then `rename`d into place,
//!    so a crash mid-restore leaves either the old database or the new one,
//!    never a half-written file.
//!
//! # Example
//!
//! ```ignore
//! use stateset_db::maintenance::{backup_to, restore_from, RestoreOptions};
//!
//! let report = backup_to(&conn, "./store.db", "./backups/store-2026-07-20.db")?;
//! println!("{} bytes, sha256 {}", report.manifest.size_bytes, report.manifest.checksum);
//!
//! let restored = restore_from(
//!     "./backups/store-2026-07-20.db",
//!     "./store.db",
//!     &RestoreOptions { overwrite: true, ..Default::default() },
//! )?;
//! ```

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Errors produced by backup and restore operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MaintenanceError {
    /// An I/O operation failed.
    #[error("io error at {path}: {source}")]
    Io {
        /// Path being operated on.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The SQLite engine reported an error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// The manifest could not be parsed or serialized.
    #[error("manifest error: {0}")]
    Manifest(String),
    /// The backup file did not match the checksum recorded in its manifest.
    #[error("checksum mismatch for {path}: manifest says {expected}, file is {actual}")]
    ChecksumMismatch {
        /// Backup file path.
        path: PathBuf,
        /// Checksum recorded in the manifest.
        expected: String,
        /// Checksum computed from the file on disk.
        actual: String,
    },
    /// The backup was taken by a newer engine with migrations this binary lacks.
    #[error(
        "backup schema version '{backup}' is newer than this engine supports (latest known: '{known}'); \
         restore with a build that includes the newer migrations"
    )]
    SchemaTooNew {
        /// Schema version recorded in the backup manifest.
        backup: String,
        /// Latest migration known to this binary.
        known: String,
    },
    /// The restore target already exists and `overwrite` was not set.
    #[error(
        "refusing to overwrite existing database at {path}; pass overwrite = true to replace it"
    )]
    TargetExists {
        /// Target path.
        path: PathBuf,
    },
}

impl From<MaintenanceError> for stateset_core::CommerceError {
    fn from(err: MaintenanceError) -> Self {
        Self::DatabaseError(err.to_string())
    }
}

type Result<T> = std::result::Result<T, MaintenanceError>;

fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> MaintenanceError {
    let path = path.into();
    move |source| MaintenanceError::Io { path, source }
}

/// Metadata written alongside every backup as `<backup>.manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct BackupManifest {
    /// Manifest format version.
    pub manifest_version: u32,
    /// Highest applied migration name, e.g. `066_search_configs`.
    pub schema_version: String,
    /// Number of migrations applied to the source database.
    pub migration_count: usize,
    /// Version of the engine that produced the backup.
    pub engine_version: String,
    /// When the backup was taken.
    pub created_at: DateTime<Utc>,
    /// Path of the database the backup was taken from.
    pub source_path: String,
    /// Size of the backup file in bytes.
    pub size_bytes: u64,
    /// Lowercase hex SHA-256 of the backup file.
    pub checksum: String,
}

/// The manifest format version this build writes and accepts.
pub const MANIFEST_VERSION: u32 = 1;

/// Result of a successful [`backup_to`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BackupReport {
    /// Path of the backup database file.
    pub backup_path: PathBuf,
    /// Path of the sidecar manifest.
    pub manifest_path: PathBuf,
    /// The manifest that was written.
    pub manifest: BackupManifest,
}

/// Options controlling [`restore_from`].
#[derive(Debug, Clone, Default)]
pub struct RestoreOptions {
    /// Replace an existing non-empty target database.
    pub overwrite: bool,
    /// Skip the manifest checksum verification (strongly discouraged;
    /// intended only for recovering a backup whose manifest was lost).
    pub skip_checksum: bool,
    /// Allow restoring a backup whose schema is newer than this binary knows.
    /// Unsafe — provided only as a break-glass escape hatch.
    pub allow_newer_schema: bool,
}

/// Result of a successful [`restore_from`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct RestoreReport {
    /// Path the database was restored to.
    pub target_path: PathBuf,
    /// Schema version of the restored database.
    pub schema_version: String,
    /// Size of the restored database in bytes.
    pub size_bytes: u64,
    /// Whether the checksum was verified against the manifest.
    pub checksum_verified: bool,
    /// Whether an existing database was replaced.
    pub replaced_existing: bool,
}

/// The conventional manifest path for a backup file.
#[must_use]
pub fn manifest_path_for(backup_path: &Path) -> PathBuf {
    let mut name = backup_path.as_os_str().to_os_string();
    name.push(".manifest.json");
    PathBuf::from(name)
}

/// Compute the lowercase hex `SHA-256` of a file, streaming it in 64 `KiB` chunks.
///
/// # Errors
///
/// Returns an error if the file cannot be opened or read.
pub fn file_checksum(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(io(path))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf).map_err(io(path))?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Read the applied schema version and migration count from a database.
fn applied_schema(conn: &Connection) -> Result<(String, usize)> {
    let table_exists: bool = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_migrations'",
        [],
        |row| row.get::<_, i64>(0),
    )? > 0;
    if !table_exists {
        return Ok((String::new(), 0));
    }
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))?;
    let latest: Option<String> =
        conn.query_row("SELECT MAX(name) FROM _migrations", [], |row| row.get(0))?;
    Ok((latest.unwrap_or_default(), usize::try_from(count).unwrap_or(0)))
}

/// Take a consistent backup of `conn` into `backup_path` and write its manifest.
///
/// The parent directory of `backup_path` is created if needed. Any existing
/// file at `backup_path` is rejected by SQLite's `VACUUM INTO`, which refuses
/// to overwrite — pick a fresh path per backup.
///
/// # Errors
///
/// Returns an error if the vacuum fails, the file cannot be written, or the
/// post-write checksum verification fails.
pub fn backup_to(
    conn: &Connection,
    source_path: impl AsRef<Path>,
    backup_path: impl AsRef<Path>,
) -> Result<BackupReport> {
    let backup_path = backup_path.as_ref();
    if let Some(parent) = backup_path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(io(parent))?;
    }

    // VACUUM INTO takes a string literal, not a bound parameter, so the path is
    // escaped by doubling single quotes. Embedded NULs would truncate the
    // literal, so reject them outright.
    let literal = backup_path.to_string_lossy().into_owned();
    if literal.contains('\0') {
        return Err(MaintenanceError::Manifest("backup path contains a NUL byte".to_owned()));
    }
    conn.execute_batch(&format!("VACUUM INTO '{}';", literal.replace('\'', "''")))?;

    let (schema_version, migration_count) = applied_schema(conn)?;
    let size_bytes = fs::metadata(backup_path).map_err(io(backup_path))?.len();
    let checksum = file_checksum(backup_path)?;

    let manifest = BackupManifest {
        manifest_version: MANIFEST_VERSION,
        schema_version,
        migration_count,
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        created_at: Utc::now(),
        source_path: source_path.as_ref().to_string_lossy().into_owned(),
        size_bytes,
        checksum: checksum.clone(),
    };

    let manifest_path = manifest_path_for(backup_path);
    let encoded = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| MaintenanceError::Manifest(e.to_string()))?;
    write_file_durably(&manifest_path, &encoded)?;

    // Verify what actually landed on disk, not what we think we wrote.
    let verified = file_checksum(backup_path)?;
    if verified != checksum {
        return Err(MaintenanceError::ChecksumMismatch {
            path: backup_path.to_path_buf(),
            expected: checksum,
            actual: verified,
        });
    }

    Ok(BackupReport { backup_path: backup_path.to_path_buf(), manifest_path, manifest })
}

/// Load and parse the manifest sidecar for a backup file.
///
/// # Errors
///
/// Returns an error if the manifest is missing, unreadable, malformed, or
/// written in an unsupported manifest format version.
pub fn read_manifest(backup_path: &Path) -> Result<BackupManifest> {
    let path = manifest_path_for(backup_path);
    let bytes = fs::read(&path).map_err(io(&path))?;
    let manifest: BackupManifest =
        serde_json::from_slice(&bytes).map_err(|e| MaintenanceError::Manifest(e.to_string()))?;
    if manifest.manifest_version > MANIFEST_VERSION {
        return Err(MaintenanceError::Manifest(format!(
            "manifest version {} is newer than supported version {MANIFEST_VERSION}",
            manifest.manifest_version
        )));
    }
    Ok(manifest)
}

/// True when `candidate` is a migration name this binary does not know about
/// and which sorts after everything it does know. Migration names are
/// zero-padded and monotonically increasing, so lexicographic order is the
/// application order.
fn is_schema_newer_than_known(candidate: &str) -> bool {
    if candidate.is_empty() {
        return false;
    }
    let known = crate::migrations::known_migration_names();
    if known.contains(&candidate) {
        return false;
    }
    known.last().is_none_or(|latest| candidate > *latest)
}

/// Restore a backup to `target_path` atomically.
///
/// See the [module docs](self) for the full list of safety rules.
///
/// # Errors
///
/// Returns [`MaintenanceError::ChecksumMismatch`] if the backup does not match
/// its manifest, [`MaintenanceError::SchemaTooNew`] if the backup requires
/// migrations this binary lacks, [`MaintenanceError::TargetExists`] if a
/// non-empty target exists without `overwrite`, or an I/O error.
pub fn restore_from(
    backup_path: impl AsRef<Path>,
    target_path: impl AsRef<Path>,
    options: &RestoreOptions,
) -> Result<RestoreReport> {
    let backup_path = backup_path.as_ref();
    let target_path = target_path.as_ref();

    let backup_size = fs::metadata(backup_path).map_err(io(backup_path))?.len();

    // 1. Manifest + checksum.
    let mut schema_version = String::new();
    let mut checksum_verified = false;
    if options.skip_checksum {
        // Still read the manifest opportunistically for the schema gate.
        if let Ok(manifest) = read_manifest(backup_path) {
            schema_version = manifest.schema_version;
        }
    } else {
        let manifest = read_manifest(backup_path)?;
        let actual = file_checksum(backup_path)?;
        if actual != manifest.checksum {
            return Err(MaintenanceError::ChecksumMismatch {
                path: backup_path.to_path_buf(),
                expected: manifest.checksum,
                actual,
            });
        }
        checksum_verified = true;
        schema_version = manifest.schema_version;
    }

    // 2. Forward-restore gate.
    if !options.allow_newer_schema && is_schema_newer_than_known(&schema_version) {
        return Err(MaintenanceError::SchemaTooNew {
            backup: schema_version,
            known: crate::migrations::latest_known_migration().to_owned(),
        });
    }

    // 3. Overwrite gate. A zero-byte target is treated as absent: SQLite
    //    creates one on first connect, so refusing it would make restore into
    //    a freshly-opened path impossible.
    let existing_len = fs::metadata(target_path).map(|m| m.len()).ok();
    let replaced_existing = matches!(existing_len, Some(len) if len > 0);
    if replaced_existing && !options.overwrite {
        return Err(MaintenanceError::TargetExists { path: target_path.to_path_buf() });
    }

    // 4. Atomic install: temp file in the same directory, fsync, rename.
    let dir = target_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    fs::create_dir_all(&dir).map_err(io(&dir))?;
    let temp_name = format!(
        ".{}.restore-{}.tmp",
        target_path.file_name().map_or_else(|| "database".into(), |n| n.to_string_lossy()),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    );
    let temp_path = dir.join(temp_name);

    let copy_result = (|| -> Result<()> {
        let bytes = fs::read(backup_path).map_err(io(backup_path))?;
        write_file_durably(&temp_path, &bytes)
    })();
    if let Err(err) = copy_result {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }

    if let Err(err) = fs::rename(&temp_path, target_path).map_err(io(target_path)) {
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }
    // Durably record the directory entry so the rename survives a crash.
    if let Ok(handle) = File::open(&dir) {
        let _ = handle.sync_all();
    }

    // Stale WAL/SHM sidecars from the replaced database would be replayed on
    // top of the restored file and silently corrupt it.
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = target_path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }

    Ok(RestoreReport {
        target_path: target_path.to_path_buf(),
        schema_version,
        size_bytes: backup_size,
        checksum_verified,
        replaced_existing,
    })
}

/// Write `bytes` to `path`, fsyncing the file before returning.
fn write_file_durably(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = File::create(path).map_err(io(path))?;
    file.write_all(bytes).map_err(io(path))?;
    file.sync_all().map_err(io(path))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;

    fn seeded_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("open memory db");
        migrations::run_migrations(&mut conn).expect("migrate");
        conn
    }

    #[test]
    fn backup_writes_manifest_with_verified_checksum() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        let backup = dir.path().join("backup.db");

        let report = backup_to(&conn, "memory", &backup).expect("backup");

        assert!(backup.exists());
        assert!(report.manifest_path.exists());
        assert_eq!(report.manifest.manifest_version, MANIFEST_VERSION);
        assert_eq!(report.manifest.engine_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(report.manifest.schema_version, migrations::latest_known_migration());
        // Not every migration applies in every build (e.g. `027_vector_search`
        // is skipped when the sqlite-vec extension is unavailable), so assert a
        // bound rather than exact equality.
        assert!(report.manifest.migration_count > 0);
        assert!(report.manifest.migration_count <= migrations::known_migration_names().len());
        assert_eq!(report.manifest.checksum, file_checksum(&backup).expect("checksum"));
        assert!(report.manifest.size_bytes > 0);
    }

    #[test]
    fn restore_rejects_corrupted_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        let backup = dir.path().join("backup.db");
        backup_to(&conn, "memory", &backup).expect("backup");

        // Flip a byte well past the header so the manifest no longer matches.
        let mut bytes = fs::read(&backup).expect("read");
        let idx = bytes.len() / 2;
        bytes[idx] ^= 0xFF;
        fs::write(&backup, &bytes).expect("write");

        let err = restore_from(&backup, dir.path().join("restored.db"), &RestoreOptions::default())
            .expect_err("should refuse corrupted backup");
        assert!(matches!(err, MaintenanceError::ChecksumMismatch { .. }), "got {err:?}");
    }

    #[test]
    fn restore_refuses_backup_from_newer_engine() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        let backup = dir.path().join("backup.db");
        backup_to(&conn, "memory", &backup).expect("backup");

        // Rewrite the manifest claiming a migration far beyond what we know.
        let manifest_path = manifest_path_for(&backup);
        let mut manifest = read_manifest(&backup).expect("manifest");
        manifest.schema_version = "999_from_the_future".to_owned();
        fs::write(&manifest_path, serde_json::to_vec(&manifest).expect("encode")).expect("write");

        let target = dir.path().join("restored.db");
        let err = restore_from(&backup, &target, &RestoreOptions::default())
            .expect_err("should refuse newer schema");
        assert!(matches!(err, MaintenanceError::SchemaTooNew { .. }), "got {err:?}");
        assert!(!target.exists(), "target must not be created on refusal");

        // Break-glass override still works.
        let opts = RestoreOptions { allow_newer_schema: true, ..Default::default() };
        restore_from(&backup, &target, &opts).expect("override restores");
        assert!(target.exists());
    }

    #[test]
    fn restore_refuses_to_clobber_existing_target() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        let backup = dir.path().join("backup.db");
        backup_to(&conn, "memory", &backup).expect("backup");

        let target = dir.path().join("live.db");
        fs::write(&target, b"precious existing data").expect("write");

        let err = restore_from(&backup, &target, &RestoreOptions::default())
            .expect_err("should refuse to overwrite");
        assert!(matches!(err, MaintenanceError::TargetExists { .. }), "got {err:?}");
        assert_eq!(fs::read(&target).expect("read"), b"precious existing data");

        let opts = RestoreOptions { overwrite: true, ..Default::default() };
        let report = restore_from(&backup, &target, &opts).expect("overwrite restores");
        assert!(report.replaced_existing);
        assert!(report.checksum_verified);
        assert_ne!(fs::read(&target).expect("read"), b"precious existing data");
    }

    #[test]
    fn restore_into_empty_placeholder_is_allowed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        let backup = dir.path().join("backup.db");
        backup_to(&conn, "memory", &backup).expect("backup");

        let target = dir.path().join("fresh.db");
        fs::write(&target, b"").expect("touch");
        let report =
            restore_from(&backup, &target, &RestoreOptions::default()).expect("restore into empty");
        assert!(!report.replaced_existing);
    }

    #[test]
    fn restore_leaves_no_temp_files_and_is_openable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        conn.execute("INSERT INTO customers (id, email, first_name, last_name) VALUES ('11111111-1111-1111-1111-111111111111', 'a@b.co', 'A', 'B')", [])
            .ok();
        let backup = dir.path().join("backup.db");
        backup_to(&conn, "memory", &backup).expect("backup");

        let target = dir.path().join("restored.db");
        let report = restore_from(&backup, &target, &RestoreOptions::default()).expect("restore");
        assert_eq!(report.schema_version, migrations::latest_known_migration());
        assert!(report.checksum_verified);
        assert_eq!(report.size_bytes, fs::metadata(&target).expect("meta").len());

        // No `.restore-*.tmp` residue.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".restore-"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");

        // The restored file is a valid, migrated SQLite database.
        let restored = Connection::open(&target).expect("open restored");
        let (version, count) = applied_schema(&restored).expect("schema");
        assert_eq!(version, migrations::latest_known_migration());
        assert!(count > 0 && count <= migrations::known_migration_names().len());
    }

    #[test]
    fn restore_missing_manifest_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        let backup = dir.path().join("backup.db");
        backup_to(&conn, "memory", &backup).expect("backup");
        fs::remove_file(manifest_path_for(&backup)).expect("remove manifest");

        let err = restore_from(&backup, dir.path().join("t.db"), &RestoreOptions::default())
            .expect_err("no manifest");
        assert!(matches!(err, MaintenanceError::Io { .. }), "got {err:?}");
    }

    #[test]
    fn schema_comparison_recognises_known_versions() {
        assert!(!is_schema_newer_than_known(""));
        assert!(!is_schema_newer_than_known(migrations::latest_known_migration()));
        assert!(!is_schema_newer_than_known("001_initial_schema"));
        assert!(is_schema_newer_than_known("999_future"));
    }
}
