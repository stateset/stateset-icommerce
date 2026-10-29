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
//! 3. An existing target (including an empty file) is never overwritten unless
//!    [`RestoreOptions::overwrite`] is set.
//! 4. The restore is atomic: the backup is copied to a temporary file in the
//!    *same directory* as the target, checked, fsynced, and published without
//!    replacing a concurrently created file unless overwrite was requested.
//! 5. Symlinks, special files and targets with WAL/SHM/journal sidecars are
//!    refused, even with overwrite. Stop all connections before replacing a
//!    database; this API cannot coordinate other processes. A fresh path is
//!    preferred. Directory entries are synced on Unix before success.
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
use rusqlite::{Connection, OpenFlags};
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
    /// Replace an existing regular target file. All connections must be closed
    /// and SQLite sidecars must be absent; use a fresh path when possible.
    pub overwrite: bool,
    /// Skip manifest verification (for recovering a backup whose manifest was
    /// lost). The actual database integrity and schema are still checked.
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
/// file at `backup_path` or its manifest is rejected. A private snapshot is
/// synced before publication; its schema metadata comes from that snapshot,
/// so concurrent source migrations cannot change the manifest's meaning.
/// Keep both files together. An interrupted publication can leave an orphaned
/// snapshot without a manifest; choose a fresh path for the next attempt.
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
    let manifest_path = manifest_path_for(backup_path);
    reject_existing(backup_path)?;
    reject_existing(&manifest_path)?;
    reject_sidecars(backup_path)?;
    let dir = parent_directory(backup_path);
    let snapshot = tempfile::Builder::new()
        .prefix(".backup-")
        .tempfile_in(&dir)
        .map_err(io(&dir))?
        .into_temp_path();
    let literal = snapshot
        .to_str()
        .ok_or_else(|| MaintenanceError::Manifest("backup path is not UTF-8".into()))?;
    // VACUUM accepts a bound destination expression. SQLite allows the empty
    // private placeholder created above, but never writes the final path.
    conn.execute("VACUUM INTO ?1", [literal])?;
    fs::OpenOptions::new()
        .write(true)
        .open(&snapshot)
        .map_err(io(&snapshot))?
        .sync_all()
        .map_err(io(&snapshot))?;
    let (schema_version, migration_count) = inspect_image(&snapshot)?;
    let size_bytes = fs::metadata(&snapshot).map_err(io(&snapshot))?.len();
    let checksum = file_checksum(&snapshot)?;

    let manifest = BackupManifest {
        manifest_version: MANIFEST_VERSION,
        schema_version,
        migration_count,
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        created_at: Utc::now(),
        source_path: source_path.as_ref().to_string_lossy().into_owned(),
        size_bytes,
        checksum,
    };
    let encoded = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| MaintenanceError::Manifest(e.to_string()))?;
    let mut staged_manifest =
        tempfile::Builder::new().prefix(".backup-manifest-").tempfile_in(&dir).map_err(io(&dir))?;
    staged_manifest.write_all(&encoded).map_err(io(&manifest_path))?;
    staged_manifest.as_file().sync_all().map_err(io(&manifest_path))?;
    reject_sidecars(backup_path)?;
    snapshot.persist_noclobber(backup_path).map_err(|err| io(backup_path)(err.error))?;
    staged_manifest
        .persist_noclobber(&manifest_path)
        .map_err(|err| io(&manifest_path)(err.error))?;
    sync_directory(&dir)?;

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
    if manifest.manifest_version != MANIFEST_VERSION {
        return Err(MaintenanceError::Manifest(format!(
            "manifest version {} is unsupported; expected version {MANIFEST_VERSION}",
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
/// target exists without `overwrite`, or an I/O error. Symlinks and SQLite
/// sidecars are refused even with `overwrite`.
pub fn restore_from(
    backup_path: impl AsRef<Path>,
    target_path: impl AsRef<Path>,
    options: &RestoreOptions,
) -> Result<RestoreReport> {
    let backup_path = backup_path.as_ref();
    let target_path = target_path.as_ref();
    // Read the manifest once, then validate the bytes actually staged for
    // publication. Never validate one open of the source and install another.
    let manifest = if options.skip_checksum { None } else { Some(read_manifest(backup_path)?) };
    restore_image(backup_path, target_path, options, manifest.as_ref(), None)
}

// Legacy BackupManager metadata has a checksum but no migration count. Keep
// its recovery path on the same staged, integrity-checked publication code.
pub(crate) fn restore_legacy_image(
    backup_path: &Path,
    target_path: &Path,
    checksum: Option<&str>,
) -> Result<RestoreReport> {
    restore_image(backup_path, target_path, &RestoreOptions::default(), None, checksum)
}

fn restore_image(
    backup_path: &Path,
    target_path: &Path,
    options: &RestoreOptions,
    manifest: Option<&BackupManifest>,
    legacy_checksum: Option<&str>,
) -> Result<RestoreReport> {
    validate_target(target_path, options.overwrite)?;
    let dir = parent_directory(target_path);
    fs::create_dir_all(&dir).map_err(io(&dir))?;
    let mut staged =
        tempfile::Builder::new().prefix(".restore-").tempfile_in(&dir).map_err(io(&dir))?;
    let mut source = File::open(backup_path).map_err(io(backup_path))?;
    if !source.metadata().map_err(io(backup_path))?.is_file() {
        return Err(MaintenanceError::Manifest("backup must be a regular file".into()));
    }
    let (size_bytes, checksum) =
        copy_and_hash(&mut source, staged.as_file_mut()).map_err(io(staged.path()))?;
    staged.as_file().sync_all().map_err(io(staged.path()))?;
    if let Some(expected) = manifest.map(|manifest| manifest.checksum.as_str()).or(legacy_checksum)
        && checksum != expected
    {
        return Err(MaintenanceError::ChecksumMismatch {
            path: backup_path.to_path_buf(),
            expected: expected.to_owned(),
            actual: checksum,
        });
    }
    if let Some(manifest) = manifest
        && size_bytes != manifest.size_bytes
    {
        return Err(MaintenanceError::Manifest("backup size differs from manifest".into()));
    }

    let (schema_version, migration_count) = inspect_image(staged.path())?;
    // The schema gate applies even when the sidecar was lost or untrusted.
    if !options.allow_newer_schema && is_schema_newer_than_known(&schema_version) {
        return Err(MaintenanceError::SchemaTooNew {
            backup: schema_version,
            known: crate::migrations::latest_known_migration().to_owned(),
        });
    }
    if let Some(manifest) = manifest
        && (manifest.schema_version != schema_version
            || manifest.migration_count != migration_count)
    {
        return Err(MaintenanceError::Manifest(
            "backup schema or migration count differs from manifest".into(),
        ));
    }

    // Repeat the sidecar/type check immediately before installation. Default
    // publication is atomically no-clobber, including zero-byte placeholders
    // and dangling symlinks created while the copy was running.
    let replaced_existing = validate_target(target_path, options.overwrite)?;
    if options.overwrite {
        staged.persist(target_path).map_err(|err| io(target_path)(err.error))?;
    } else {
        staged.persist_noclobber(target_path).map_err(|err| {
            if err.error.kind() == std::io::ErrorKind::AlreadyExists {
                MaintenanceError::TargetExists { path: target_path.to_path_buf() }
            } else {
                io(target_path)(err.error)
            }
        })?;
    }
    sync_directory(&dir)?;
    Ok(RestoreReport {
        target_path: target_path.to_path_buf(),
        schema_version,
        size_bytes,
        checksum_verified: manifest.is_some() || legacy_checksum.is_some(),
        replaced_existing,
    })
}

fn parent_directory(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn reject_existing(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(MaintenanceError::TargetExists { path: path.to_path_buf() }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(io(path)(err)),
    }
}

fn reject_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        reject_existing(&PathBuf::from(name))?;
    }
    Ok(())
}

fn validate_target(path: &Path, overwrite: bool) -> Result<bool> {
    reject_sidecars(path)?;
    match fs::symlink_metadata(path) {
        Ok(meta) if overwrite && meta.file_type().is_file() => Ok(true),
        Ok(_) => Err(MaintenanceError::TargetExists { path: path.to_path_buf() }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(io(path)(err)),
    }
}

/// Check the actual staged image, without creating or migrating a database.
fn inspect_image(path: &Path) -> Result<(String, usize)> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut check = conn.prepare("PRAGMA integrity_check")?;
    let results = check
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if results != ["ok"] {
        return Err(MaintenanceError::Manifest(format!(
            "SQLite integrity check failed: {}",
            results.join("; ")
        )));
    }
    applied_schema(&conn)
}

/// Hash while streaming with bounded memory; the digest binds exactly the
/// image we publish, even if the source path is replaced during the copy.
fn copy_and_hash(
    source: &mut impl Read,
    target: &mut impl Write,
) -> std::io::Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        target.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        size += count as u64;
    }
    Ok((size, format!("{:x}", hasher.finalize())))
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path).map_err(io(path))?.sync_all().map_err(io(path))?;
    #[cfg(not(unix))]
    let _ = path;
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

        // The actual image, rather than just its sidecar, has a newer schema.
        let future = Connection::open(&backup).expect("open snapshot");
        future.execute("UPDATE _migrations SET name = '999_from_the_future' WHERE name = (SELECT MAX(name) FROM _migrations)", []).expect("future migration");
        drop(future);
        let manifest_path = manifest_path_for(&backup);
        let mut manifest = read_manifest(&backup).expect("manifest");
        manifest.schema_version = "999_from_the_future".to_owned();
        manifest.checksum = file_checksum(&backup).expect("checksum");
        manifest.size_bytes = fs::metadata(&backup).expect("metadata").len();
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
    fn restore_into_empty_placeholder_requires_explicit_overwrite() {
        let dir = tempfile::tempdir().expect("tempdir");
        let conn = seeded_db();
        let backup = dir.path().join("backup.db");
        backup_to(&conn, "memory", &backup).expect("backup");

        let target = dir.path().join("fresh.db");
        fs::write(&target, b"").expect("touch");
        let error = restore_from(&backup, &target, &RestoreOptions::default())
            .expect_err("refuse empty target");
        assert!(matches!(error, MaintenanceError::TargetExists { .. }));
        assert_eq!(fs::metadata(&target).expect("metadata").len(), 0);
        let report = restore_from(
            &backup,
            &target,
            &RestoreOptions { overwrite: true, ..Default::default() },
        )
        .expect("explicit overwrite");
        assert!(report.replaced_existing);
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
    fn restore_without_manifest_checks_actual_schema_and_integrity() {
        let dir = tempfile::tempdir().expect("tempdir");
        let backup = dir.path().join("backup.db");
        backup_to(&seeded_db(), "memory", &backup).expect("backup");
        fs::remove_file(manifest_path_for(&backup)).expect("remove manifest");
        let options = RestoreOptions { skip_checksum: true, ..Default::default() };
        let target = dir.path().join("restored.db");
        let report = restore_from(&backup, &target, &options).expect("lost manifest recovery");
        assert!(!report.checksum_verified);
        assert_eq!(report.schema_version, migrations::latest_known_migration());
        fs::remove_file(&target).expect("remove target");

        let conn = Connection::open(&backup).expect("open image");
        conn.execute("UPDATE _migrations SET name = '999_future' WHERE name = (SELECT MAX(name) FROM _migrations)", []).expect("new schema");
        drop(conn);
        assert!(matches!(
            restore_from(&backup, &target, &options),
            Err(MaintenanceError::SchemaTooNew { .. })
        ));
        assert!(!target.exists());

        fs::write(&backup, b"not a SQLite image").expect("corrupt image");
        assert!(restore_from(&backup, &target, &options).is_err());
        assert!(!target.exists());
        assert_no_staging_files(dir.path());
    }

    #[test]
    fn restore_rejects_inconsistent_manifest_metadata() {
        let dir = tempfile::tempdir().expect("tempdir");
        let backup = dir.path().join("backup.db");
        backup_to(&seeded_db(), "memory", &backup).expect("backup");
        let manifest = read_manifest(&backup).expect("manifest");
        for field in ["version", "size", "schema", "count"] {
            let mut modified = manifest.clone();
            match field {
                "version" => modified.manifest_version = 0,
                "size" => modified.size_bytes += 1,
                "schema" => modified.schema_version = "001_incorrect".into(),
                "count" => modified.migration_count += 1,
                _ => unreachable!(),
            }
            fs::write(manifest_path_for(&backup), serde_json::to_vec(&modified).expect("encode"))
                .expect("write");
            let target = dir.path().join("restored.db");
            assert!(
                matches!(
                    restore_from(&backup, &target, &RestoreOptions::default()),
                    Err(MaintenanceError::Manifest(_))
                ),
                "field {field}"
            );
            assert!(!target.exists());
            assert_no_staging_files(dir.path());
        }
    }

    #[test]
    fn restore_refuses_sidecars_without_deleting_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let backup = dir.path().join("backup.db");
        backup_to(&seeded_db(), "memory", &backup).expect("backup");
        let target = dir.path().join("live.db");
        fs::write(&target, b"existing database").expect("target");
        for suffix in ["-wal", "-shm", "-journal"] {
            let sidecar = dir.path().join(format!("live.db{suffix}"));
            fs::write(&sidecar, b"unreconciled journal").expect("sidecar");
            for overwrite in [false, true] {
                let options = RestoreOptions { overwrite, ..Default::default() };
                assert!(matches!(
                    restore_from(&backup, &target, &options),
                    Err(MaintenanceError::TargetExists { .. })
                ));
                assert_eq!(fs::read(&target).expect("target"), b"existing database");
                assert_eq!(fs::read(&sidecar).expect("sidecar"), b"unreconciled journal");
            }
            fs::remove_file(sidecar).expect("remove fixture");
        }
    }

    #[cfg(unix)]
    #[test]
    fn backup_and_restore_never_follow_destination_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().expect("tempdir");
        let victim = dir.path().join("victim.db");
        fs::write(&victim, b"precious bytes").expect("victim");
        let backup = dir.path().join("backup.db");
        backup_to(&seeded_db(), "memory", &backup).expect("backup");
        assert_eq!(fs::metadata(&backup).expect("metadata").permissions().mode() & 0o077, 0);
        assert_eq!(
            fs::metadata(manifest_path_for(&backup)).expect("metadata").permissions().mode()
                & 0o077,
            0
        );
        for destination in [&victim, &dir.path().join("missing.db")] {
            let target = dir.path().join("symlink.db");
            symlink(destination, &target).expect("symlink");
            for overwrite in [false, true] {
                assert!(matches!(
                    restore_from(
                        &backup,
                        &target,
                        &RestoreOptions { overwrite, ..Default::default() }
                    ),
                    Err(MaintenanceError::TargetExists { .. })
                ));
            }
            assert!(backup_to(&seeded_db(), "memory", &target).is_err());
            fs::remove_file(&target).expect("unlink");
        }
        assert_eq!(fs::read(&victim).expect("victim"), b"precious bytes");
    }

    #[test]
    fn concurrent_restores_publish_exactly_one_complete_image() {
        use std::sync::{Arc, Barrier};
        let dir = tempfile::tempdir().expect("tempdir");
        let backup = dir.path().join("backup.db");
        backup_to(&seeded_db(), "memory", &backup).expect("backup");
        let target = dir.path().join("restored.db");
        let barrier = Arc::new(Barrier::new(4));
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let (backup, target, barrier) = (backup.clone(), target.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    restore_from(&backup, &target, &RestoreOptions::default())
                })
            })
            .collect();
        let results: Vec<_> =
            workers.into_iter().map(|worker| worker.join().expect("worker")).collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        for error in results.into_iter().filter_map(std::result::Result::err) {
            assert!(matches!(error, MaintenanceError::TargetExists { .. }), "{error:?}");
        }
        assert_eq!(
            file_checksum(&target).expect("restored checksum"),
            file_checksum(&backup).expect("source checksum")
        );
        assert_no_staging_files(dir.path());
    }

    #[test]
    fn backup_refuses_existing_manifest_or_image_without_modification() {
        let dir = tempfile::tempdir().expect("tempdir");
        let backup = dir.path().join("backup.db");
        let manifest = manifest_path_for(&backup);
        fs::write(&manifest, b"previous manifest").expect("manifest");
        assert!(backup_to(&seeded_db(), "memory", &backup).is_err());
        assert!(!backup.exists());
        assert_eq!(fs::read(&manifest).expect("manifest"), b"previous manifest");
        fs::remove_file(&manifest).expect("remove fixture");
        fs::write(&backup, b"").expect("empty destination");
        assert!(backup_to(&seeded_db(), "memory", &backup).is_err());
        assert_eq!(fs::metadata(&backup).expect("metadata").len(), 0);
    }

    #[test]
    fn streaming_checksum_binds_the_copied_bytes_and_propagates_failures() {
        let data = vec![42_u8; 128 * 1024 + 17];
        let mut copied = Vec::new();
        let (size, hash) = copy_and_hash(&mut data.as_slice(), &mut copied).expect("copy");
        assert_eq!(copied, data);
        assert_eq!(size, data.len() as u64);
        assert_eq!(hash, format!("{:x}", Sha256::digest(&data)));
        struct FailingReader;
        impl Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("interrupted source"))
            }
        }
        assert!(copy_and_hash(&mut FailingReader, &mut Vec::new()).is_err());
        assert!(copy_and_hash(&mut data.as_slice(), &mut std::io::Cursor::new([0_u8; 4])).is_err());
    }

    fn assert_no_staging_files(directory: &Path) {
        for entry in fs::read_dir(directory).expect("directory") {
            let name = entry.expect("entry").file_name();
            assert!(
                !name.to_string_lossy().starts_with(".restore-"),
                "staging file left behind: {name:?}"
            );
            assert!(
                !name.to_string_lossy().starts_with(".backup-"),
                "staging file left behind: {name:?}"
            );
        }
    }

    #[test]
    fn schema_comparison_recognises_known_versions() {
        assert!(!is_schema_newer_than_known(""));
        assert!(!is_schema_newer_than_known(migrations::latest_known_migration()));
        assert!(!is_schema_newer_than_known("001_initial_schema"));
        assert!(is_schema_newer_than_known("999_future"));
    }
}
