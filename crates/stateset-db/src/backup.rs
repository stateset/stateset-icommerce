//! Backup and restore utilities for embedded SQLite databases
//!
//! Provides safe backup and restore operations for production deployments.
//!
//! # Features
//!
//! - Full database backups (VACUUM INTO)
//! - Point-in-time recovery
//! - Incremental backups
//! - Backup verification
//! - Restore with integrity checks

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use stateset_core::CommerceError;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Backup configuration
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Number of backups to retain
    pub retain_count: usize,
    /// Whether to compress backups
    pub compress: bool,
    /// Whether to verify backups after creation
    pub verify: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            retain_count: 7, // Keep 7 days of backups
            compress: true,
            verify: true,
        }
    }
}

/// Backup metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupMetadata {
    /// Backup filename
    pub filename: String,
    /// Timestamp when backup was created
    pub created_at: DateTime<Utc>,
    /// Size of backup in bytes
    pub size_bytes: u64,
    /// Database schema version
    pub schema_version: i32,
    /// Number of tables
    pub table_count: usize,
    /// Whether backup is compressed
    pub compressed: bool,
    /// Backup checksum (SHA256)
    pub checksum: String,
}

/// Backup result
#[derive(Debug)]
pub struct BackupResult {
    /// Backup metadata
    pub metadata: BackupMetadata,
    /// Path to backup file
    pub path: PathBuf,
}

/// Restore result
#[derive(Debug)]
pub struct RestoreResult {
    /// Timestamp of restore
    pub restored_at: DateTime<Utc>,
    /// Number of tables restored
    pub table_count: usize,
    /// Whether restore was successful
    pub success: bool,
}

/// Backup manager for SQLite databases
#[derive(Debug)]
pub struct BackupManager {
    config: BackupConfig,
    backup_dir: PathBuf,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new<P: AsRef<Path>>(backup_dir: P, config: BackupConfig) -> Result<Self, io::Error> {
        let backup_dir = backup_dir.as_ref().to_path_buf();
        fs::create_dir_all(&backup_dir)?;
        Ok(Self { config, backup_dir })
    }

    /// Create a full backup of the database
    ///
    /// # Example
    ///
    /// ```ignore
    /// use stateset_db::backup::BackupManager;
    ///
    /// let manager = BackupManager::new("./backups", BackupConfig::default())?;
    /// let result = manager.backup(conn, "store.db")?;
    /// println!("Backup created: {}", result.path.display());
    /// ```
    pub fn backup<V: AsRef<Path>>(
        &self,
        conn: &Connection,
        database_path: V,
    ) -> Result<BackupResult, CommerceError> {
        let timestamp = Utc::now();
        let timestamp_str = timestamp.format("%Y%m%d_%H%M%S").to_string();
        let db_name =
            database_path.as_ref().file_stem().and_then(|s| s.to_str()).unwrap_or("database");

        // Validate that the derived db_name is a safe filename component.
        // Reject path separators and `..` to prevent directory-traversal attacks
        // via crafted database_path arguments.
        if db_name.contains(std::path::MAIN_SEPARATOR)
            || db_name.contains('/')
            || db_name.contains('\\')
            || db_name.contains("..")
            || db_name.contains('\0')
        {
            return Err(CommerceError::DatabaseError(format!(
                "Backup path escapes backup directory: database name '{db_name}' contains path separators or traversal sequences"
            )));
        }

        let (filename, backup_path) = {
            let mut attempt = 0usize;
            loop {
                let suffix = if attempt == 0 { String::new() } else { format!("_{attempt}") };
                let candidate = format!("{db_name}_{timestamp_str}{suffix}.db");
                let path = self.backup_dir.join(&candidate);
                if !path.exists() {
                    break (candidate, path);
                }
                attempt += 1;
            }
        };

        // Create backup using VACUUM INTO (path is validated above)
        let backup_file = backup_path.display().to_string();
        conn.execute_batch(&format!("VACUUM INTO '{}';", backup_file.replace('\'', "''")))
            .map_err(|e| CommerceError::DatabaseError(format!("Backup failed: {e}")))?;

        // Get backup metadata
        let size_bytes = fs::metadata(&backup_path).map(|m| m.len()).unwrap_or(0);

        let table_count: usize = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |r| r.get(0))
            .unwrap_or(0);

        // Get schema version
        let schema_version: i32 =
            conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);

        // Calculate checksum if configured
        let checksum =
            if self.config.verify { self.calculate_checksum(&backup_path)? } else { String::new() };

        let metadata = BackupMetadata {
            filename,
            created_at: timestamp,
            size_bytes,
            schema_version,
            table_count,
            compressed: self.config.compress,
            checksum,
        };

        // Verify backup if configured
        if self.config.verify {
            self.verify_backup(&backup_path)?;
        }

        // Save metadata
        self.save_metadata(&metadata)?;

        // Clean up old backups
        self.cleanup_old_backups(db_name)?;

        Ok(BackupResult { metadata, path: backup_path })
    }

    /// Restore database from a backup
    pub fn restore<V: AsRef<Path>>(
        &self,
        _conn: &Connection,
        backup_path: V,
        restore_path: V,
    ) -> Result<RestoreResult, CommerceError> {
        let backup_path = backup_path.as_ref();
        let restore_path = restore_path.as_ref();

        // Verify backup exists
        if !backup_path.exists() {
            return Err(CommerceError::DatabaseError(format!(
                "Backup file not found: {}",
                backup_path.display()
            )));
        }

        // Copy backup to restore location
        fs::copy(backup_path, restore_path)
            .map_err(|e| CommerceError::DatabaseError(format!("Restore copy failed: {e}")))?;

        // Open restore connection and verify integrity
        let restore_conn = Connection::open(restore_path)
            .map_err(|e| CommerceError::DatabaseError(format!("Restore open failed: {e}")))?;

        // Run integrity check
        let integrity_result: String = restore_conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap_or_else(|_| "failed".to_string());

        if integrity_result != "ok" {
            return Err(CommerceError::DatabaseError(format!(
                "Backup integrity check failed: {integrity_result}"
            )));
        }

        // Get table count
        let table_count: usize = restore_conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |r| r.get(0))
            .unwrap_or(0);

        Ok(RestoreResult { restored_at: Utc::now(), table_count, success: true })
    }

    /// List available backups
    pub fn list_backups(&self) -> Result<Vec<BackupMetadata>, CommerceError> {
        let mut backups = Vec::new();

        for entry in fs::read_dir(&self.backup_dir)
            .map_err(|e| CommerceError::DatabaseError(format!("Failed to read backup dir: {e}")))?
        {
            let entry = entry
                .map_err(|e| CommerceError::DatabaseError(format!("Failed to read entry: {e}")))?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("db") {
                if let Some(metadata) = self.load_metadata(&path) {
                    backups.push(metadata);
                }
            }
        }

        // Sort by creation date (newest first)
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    /// Delete a backup
    pub fn delete_backup<P: AsRef<Path>>(&self, backup_path: P) -> Result<(), CommerceError> {
        let backup_path = backup_path.as_ref();
        fs::remove_file(backup_path)
            .map_err(|e| CommerceError::DatabaseError(format!("Failed to delete backup: {e}")))?;

        // Delete metadata file
        let metadata_path = backup_path.with_extension("meta");
        if metadata_path.exists() {
            fs::remove_file(&metadata_path).ok();
        }

        Ok(())
    }

    /// Calculate SHA256 checksum of a file
    fn calculate_checksum<P: AsRef<Path>>(&self, path: P) -> Result<String, CommerceError> {
        use sha2::{Digest, Sha256};

        let mut hash = Sha256::new();
        let mut file = fs::File::open(path)
            .map_err(|e| CommerceError::DatabaseError(format!("Failed to open backup: {e}")))?;

        let mut buffer = vec![0; 8192];
        loop {
            let n = file
                .read(&mut buffer)
                .map_err(|e| CommerceError::DatabaseError(format!("Failed to read backup: {e}")))?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }

        Ok(hex::encode(hash.finalize()))
    }

    /// Verify backup integrity
    fn verify_backup<P: AsRef<Path>>(&self, backup_path: P) -> Result<(), CommerceError> {
        let backup_path = backup_path.as_ref();

        // Try to open backup file
        let conn = Connection::open(backup_path)
            .map_err(|e| CommerceError::DatabaseError(format!("Backup file corrupt: {e}")))?;

        // Run integrity check
        let integrity_result: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap_or_else(|_| "failed".to_string());

        if integrity_result != "ok" {
            return Err(CommerceError::DatabaseError(format!(
                "Backup integrity check failed: {integrity_result}"
            )));
        }

        Ok(())
    }

    /// Save backup metadata to a file
    fn save_metadata(&self, metadata: &BackupMetadata) -> Result<(), CommerceError> {
        let metadata_path = self.backup_dir.join(&metadata.filename).with_extension("meta");
        let metadata_json = serde_json::to_string_pretty(metadata).map_err(|e| {
            CommerceError::DatabaseError(format!("Failed to serialize metadata: {e}"))
        })?;

        fs::write(&metadata_path, metadata_json)
            .map_err(|e| CommerceError::DatabaseError(format!("Failed to save metadata: {e}")))?;

        Ok(())
    }

    /// Load backup metadata from a file
    fn load_metadata<P: AsRef<Path>>(&self, backup_path: P) -> Option<BackupMetadata> {
        let backup_path = backup_path.as_ref();
        let metadata_path = backup_path.with_extension("meta");

        if !metadata_path.exists() {
            return None;
        }

        let metadata_json = fs::read_to_string(&metadata_path).ok()?;
        serde_json::from_str(&metadata_json).ok()
    }

    /// Clean up old backups based on retain count
    fn cleanup_old_backups(&self, db_name: &str) -> Result<(), CommerceError> {
        let backups = self.list_backups()?;
        let db_backups: Vec<_> =
            backups.into_iter().filter(|b| b.filename.starts_with(db_name)).collect();

        // Remove excess backups
        for backup in db_backups.iter().skip(self.config.retain_count) {
            let backup_path = self.backup_dir.join(&backup.filename);
            self.delete_backup(backup_path)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a test database with one table and one row.
    fn create_test_db(dir: &Path) -> (PathBuf, Connection) {
        let db_path = dir.join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)", []).unwrap();
        conn.execute("INSERT INTO test VALUES (1, 'hello')", []).unwrap();
        (db_path, conn)
    }

    // ------------------------------------------------------------------
    // Original tests (preserved)
    // ------------------------------------------------------------------

    #[test]
    fn test_backup_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let backup_dir = temp_dir.path().join("backups");

        // Create test database
        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)", []).unwrap();
        conn.execute("INSERT INTO test VALUES (1, 'test')", []).unwrap();

        // Create backup
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert!(result.path.exists());
        assert!(result.metadata.table_count > 0);
        assert!(!result.metadata.checksum.is_empty());
    }

    #[test]
    fn test_list_backups() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let backups = manager.list_backups().unwrap();

        assert_eq!(backups.len(), 0);
    }

    #[test]
    fn test_backup_retention() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let backup_dir = temp_dir.path().join("backups");

        // Create test database
        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, value TEXT)", []).unwrap();

        // Create backups with retention of 2
        let config = BackupConfig { retain_count: 2, ..Default::default() };
        let manager = BackupManager::new(&backup_dir, config).unwrap();

        // Create 3 backups
        for _ in 0..3 {
            manager.backup(&conn, &db_path).unwrap();
        }

        // Only 2 should remain
        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 2);
    }

    // ------------------------------------------------------------------
    // New tests
    // ------------------------------------------------------------------

    // ---- BackupConfig defaults ----

    #[test]
    fn test_backup_config_default_values() {
        let cfg = BackupConfig::default();
        assert_eq!(cfg.retain_count, 7);
        assert!(cfg.compress);
        assert!(cfg.verify);
    }

    #[test]
    fn test_backup_config_clone() {
        let cfg = BackupConfig { retain_count: 3, compress: false, verify: false };
        #[allow(clippy::redundant_clone)]
        let cfg2 = cfg.clone();
        assert_eq!(cfg2.retain_count, 3);
        assert!(!cfg2.compress);
        assert!(!cfg2.verify);
    }

    // ---- BackupManager::new ----

    #[test]
    fn test_new_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("a").join("b").join("c");
        assert!(!backup_dir.exists());

        let _manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        assert!(backup_dir.exists());
    }

    // ---- Path traversal prevention ----

    #[test]
    fn test_path_traversal_dotdot_in_stem_rejected() {
        let temp_dir = TempDir::new().unwrap();
        let (_, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        // Craft a path whose file_stem literally contains ".."
        // e.g. a file named "..something.db" — file_stem = "..something"
        let malicious = temp_dir.path().join("..secret.db");
        let result = manager.backup(&conn, &malicious);
        assert!(result.is_err(), "Stem containing '..' should be blocked");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("escapes"), "Error should mention escaping, got: {err_msg}");
    }

    #[test]
    fn test_path_traversal_safe_stems_work() {
        let temp_dir = TempDir::new().unwrap();
        let (_, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        // file_stem of "../../escape.db" is "escape" (safe after stem extraction)
        let result = manager.backup(&conn, Path::new("../../escape.db"));
        assert!(result.is_ok(), "file_stem extraction strips directory components");

        // "sub/dir/evil.db" has file_stem "evil" (safe)
        let result = manager.backup(&conn, Path::new("sub/dir/safe.db"));
        assert!(result.is_ok(), "Normal nested paths produce safe stems");
    }

    #[test]
    fn test_backup_name_with_dots_but_no_traversal_ok() {
        let temp_dir = TempDir::new().unwrap();
        let (_, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        // A name like "my.store.db" should work fine — no traversal
        let safe_name = temp_dir.path().join("my.store.db");
        let result = manager.backup(&conn, &safe_name);
        assert!(result.is_ok());
    }

    #[test]
    fn test_backup_safe_by_construction() {
        let temp_dir = TempDir::new().unwrap();
        let (_, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, Path::new("store.db")).unwrap();

        // The backup file must always be under backup_dir
        assert!(
            result.path.starts_with(&backup_dir),
            "Backup path must be under backup_dir: {}",
            result.path.display()
        );
    }

    // ---- Backup verification with corrupt file ----

    #[test]
    fn test_verify_backup_catches_corruption() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        // Write garbage bytes to a file
        let corrupt_path = backup_dir.join("corrupt.db");
        fs::write(&corrupt_path, b"this is not a valid sqlite database").unwrap();

        let result = manager.verify_backup(&corrupt_path);
        assert!(result.is_err(), "Corrupt file should fail verification");
    }

    #[test]
    fn test_verify_backup_succeeds_for_valid_db() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        // Verify should succeed on the newly created backup
        assert!(manager.verify_backup(&result.path).is_ok());
    }

    // ---- Restore from backup (round trip) ----

    #[test]
    fn test_backup_restore_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let backup_result = manager.backup(&conn, &db_path).unwrap();

        // Restore to a new location
        let restore_path = temp_dir.path().join("restored.db");
        let restore_result = manager.restore(&conn, &backup_result.path, &restore_path).unwrap();

        assert!(restore_result.success);
        assert!(restore_result.table_count > 0);

        // Open restored DB and verify data
        let restored_conn = Connection::open(&restore_path).unwrap();
        let value: String = restored_conn
            .query_row("SELECT value FROM test WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(value, "hello");
    }

    #[test]
    fn test_restore_nonexistent_backup_fails() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let conn = Connection::open_in_memory().unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        let missing = backup_dir.join("does_not_exist.db");
        let restore_path = temp_dir.path().join("restored.db");
        let result = manager.restore(&conn, &missing, &restore_path);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("not found"), "Expected 'not found' in: {err_msg}");
    }

    #[test]
    fn test_restore_corrupt_backup_fails_integrity() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let conn = Connection::open_in_memory().unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        // Write garbage to a "backup" file
        let corrupt = backup_dir.join("corrupt.db");
        fs::write(&corrupt, b"garbage data here").unwrap();

        let restore_path = temp_dir.path().join("restored.db");
        let result = manager.restore(&conn, &corrupt, &restore_path);
        assert!(result.is_err());
    }

    // ---- Metadata persistence ----

    #[test]
    fn test_metadata_save_and_load_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        let metadata = BackupMetadata {
            filename: "store_20260317_120000.db".to_string(),
            created_at: Utc::now(),
            size_bytes: 4096,
            schema_version: 42,
            table_count: 10,
            compressed: true,
            checksum: "abc123".to_string(),
        };

        manager.save_metadata(&metadata).unwrap();

        let loaded = manager
            .load_metadata(backup_dir.join("store_20260317_120000.db"))
            .expect("metadata should load");

        assert_eq!(loaded.filename, metadata.filename);
        assert_eq!(loaded.size_bytes, 4096);
        assert_eq!(loaded.schema_version, 42);
        assert_eq!(loaded.table_count, 10);
        assert!(loaded.compressed);
        assert_eq!(loaded.checksum, "abc123");
    }

    #[test]
    fn test_load_metadata_returns_none_for_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        let result = manager.load_metadata(backup_dir.join("nonexistent.db"));
        assert!(result.is_none());
    }

    #[test]
    fn test_load_metadata_returns_none_for_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        let meta_path = backup_dir.join("bad.meta");
        fs::write(&meta_path, b"not json at all").unwrap();

        let result = manager.load_metadata(backup_dir.join("bad.db"));
        assert!(result.is_none());
    }

    // ---- Empty database backup ----

    #[test]
    fn test_backup_empty_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("empty.db");
        let backup_dir = temp_dir.path().join("backups");

        // Open a database with no user tables
        let conn = Connection::open(&db_path).unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert!(result.path.exists());
        // An empty database has 0 user tables
        assert_eq!(result.metadata.table_count, 0);
        assert!(result.metadata.size_bytes > 0); // file still has header
    }

    #[test]
    fn test_backup_database_with_multiple_tables() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("multi.db");
        let backup_dir = temp_dir.path().join("backups");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE t1 (id INTEGER PRIMARY KEY)", []).unwrap();
        conn.execute("CREATE TABLE t2 (id INTEGER PRIMARY KEY)", []).unwrap();
        conn.execute("CREATE TABLE t3 (id INTEGER PRIMARY KEY)", []).unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert_eq!(result.metadata.table_count, 3);
    }

    // ---- Delete backup ----

    #[test]
    fn test_delete_backup_removes_file_and_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        let backup_path = &result.path;
        let meta_path = backup_path.with_extension("meta");

        assert!(backup_path.exists());
        assert!(meta_path.exists());

        manager.delete_backup(backup_path).unwrap();

        assert!(!backup_path.exists());
        assert!(!meta_path.exists());
    }

    #[test]
    fn test_delete_nonexistent_backup_fails() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        let missing = backup_dir.join("no_such_file.db");
        let result = manager.delete_backup(&missing);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_backup_without_metadata_file_still_succeeds() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        // Create a bare .db file with no .meta companion
        let fake_backup = backup_dir.join("orphan.db");
        fs::write(&fake_backup, b"fake").unwrap();
        assert!(fake_backup.exists());

        manager.delete_backup(&fake_backup).unwrap();
        assert!(!fake_backup.exists());
    }

    // ---- List backups ordering ----

    #[test]
    fn test_list_backups_returns_newest_first() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let config = BackupConfig { retain_count: 100, ..Default::default() };
        let manager = BackupManager::new(&backup_dir, config).unwrap();

        // Create multiple backups — they'll have monotonically increasing timestamps
        let mut created_filenames = Vec::new();
        for _ in 0..3 {
            let result = manager.backup(&conn, &db_path).unwrap();
            created_filenames.push(result.metadata.filename.clone());
        }

        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 3);
        // Newest first means the last created should be first in the list
        for i in 0..backups.len() - 1 {
            assert!(
                backups[i].created_at >= backups[i + 1].created_at,
                "Backups should be sorted newest first"
            );
        }
    }

    // ---- Concurrent backup filenames (_1 suffix) ----

    #[test]
    fn test_concurrent_backup_filenames_differ() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        // Use high retain count so cleanup doesn't interfere
        let config = BackupConfig { retain_count: 100, ..Default::default() };
        let manager = BackupManager::new(&backup_dir, config).unwrap();

        // Create two backups rapidly — they should get unique filenames via the
        // _1, _2 suffix logic when the timestamp is identical.
        let r1 = manager.backup(&conn, &db_path).unwrap();
        let r2 = manager.backup(&conn, &db_path).unwrap();

        assert_ne!(
            r1.metadata.filename, r2.metadata.filename,
            "Filenames must differ even in the same second"
        );
        assert!(r1.path.exists());
        assert!(r2.path.exists());
    }

    #[test]
    fn test_three_rapid_backups_all_unique_filenames() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let config = BackupConfig { retain_count: 100, ..Default::default() };
        let manager = BackupManager::new(&backup_dir, config).unwrap();

        let mut filenames = std::collections::HashSet::new();
        for _ in 0..5 {
            let r = manager.backup(&conn, &db_path).unwrap();
            assert!(
                filenames.insert(r.metadata.filename.clone()),
                "Duplicate filename: {}",
                r.metadata.filename
            );
        }
        assert_eq!(filenames.len(), 5);
    }

    // ---- Checksum ----

    #[test]
    fn test_checksum_is_deterministic() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        // Write a known file
        let path = backup_dir.join("fixed.bin");
        fs::write(&path, b"deterministic content").unwrap();

        let c1 = manager.calculate_checksum(&path).unwrap();
        let c2 = manager.calculate_checksum(&path).unwrap();
        assert_eq!(c1, c2);
        assert_eq!(c1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_checksum_differs_for_different_content() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        let p1 = backup_dir.join("file_a.bin");
        let p2 = backup_dir.join("file_b.bin");
        fs::write(&p1, b"content A").unwrap();
        fs::write(&p2, b"content B").unwrap();

        let c1 = manager.calculate_checksum(&p1).unwrap();
        let c2 = manager.calculate_checksum(&p2).unwrap();
        assert_ne!(c1, c2);
    }

    #[test]
    fn test_checksum_missing_file_errors() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();

        let result = manager.calculate_checksum(backup_dir.join("nope.bin"));
        assert!(result.is_err());
    }

    // ---- Backup metadata fields ----

    #[test]
    fn test_backup_metadata_has_correct_schema_version() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("versioned.db");
        let backup_dir = temp_dir.path().join("backups");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE t1 (id INTEGER)", []).unwrap();
        conn.execute_batch("PRAGMA user_version = 99;").unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert_eq!(result.metadata.schema_version, 99);
    }

    #[test]
    fn test_backup_without_verify_has_empty_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let config = BackupConfig { retain_count: 7, compress: false, verify: false };
        let manager = BackupManager::new(&backup_dir, config).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert!(result.metadata.checksum.is_empty());
    }

    #[test]
    fn test_backup_result_path_is_under_backup_dir() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert!(result.path.starts_with(&backup_dir), "Backup path should be under backup_dir");
    }

    #[test]
    fn test_backup_filename_contains_db_stem() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("mystore.db");
        let backup_dir = temp_dir.path().join("backups");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE orders (id INTEGER)", []).unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert!(
            result.metadata.filename.starts_with("mystore_"),
            "Filename should start with the db stem: {}",
            result.metadata.filename
        );
    }

    // ---- Cleanup old backups ----

    #[test]
    fn test_cleanup_respects_retain_count_of_one() {
        let temp_dir = TempDir::new().unwrap();
        let (db_path, conn) = create_test_db(temp_dir.path());
        let backup_dir = temp_dir.path().join("backups");

        let config = BackupConfig { retain_count: 1, ..Default::default() };
        let manager = BackupManager::new(&backup_dir, config).unwrap();

        for _ in 0..5 {
            manager.backup(&conn, &db_path).unwrap();
        }

        let backups = manager.list_backups().unwrap();
        assert_eq!(backups.len(), 1, "Only 1 backup should be retained");
    }

    // ---- BackupMetadata serde ----

    #[test]
    fn test_backup_metadata_serialization_round_trip() {
        let metadata = BackupMetadata {
            filename: "test_20260317_100000.db".to_string(),
            created_at: Utc::now(),
            size_bytes: 1024,
            schema_version: 5,
            table_count: 3,
            compressed: false,
            checksum: "deadbeef".to_string(),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: BackupMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.filename, metadata.filename);
        assert_eq!(deserialized.size_bytes, metadata.size_bytes);
        assert_eq!(deserialized.schema_version, metadata.schema_version);
        assert_eq!(deserialized.table_count, metadata.table_count);
        assert_eq!(deserialized.compressed, metadata.compressed);
        assert_eq!(deserialized.checksum, metadata.checksum);
    }

    // ---- BackupResult / RestoreResult Debug ----

    #[test]
    fn test_backup_result_debug() {
        let result = BackupResult {
            metadata: BackupMetadata {
                filename: "f.db".into(),
                created_at: Utc::now(),
                size_bytes: 0,
                schema_version: 0,
                table_count: 0,
                compressed: false,
                checksum: String::new(),
            },
            path: PathBuf::from("/tmp/f.db"),
        };
        let dbg = format!("{result:?}");
        assert!(dbg.contains("BackupResult"));
    }

    #[test]
    fn test_restore_result_debug() {
        let result = RestoreResult { restored_at: Utc::now(), table_count: 5, success: true };
        let dbg = format!("{result:?}");
        assert!(dbg.contains("RestoreResult"));
        assert!(dbg.contains("true"));
    }

    // ---- Edge cases ----

    #[test]
    fn test_backup_large_data_set() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("big.db");
        let backup_dir = temp_dir.path().join("backups");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, data TEXT)", []).unwrap();

        // Insert 500 rows
        for i in 0..500 {
            conn.execute(
                "INSERT INTO items (id, data) VALUES (?1, ?2)",
                rusqlite::params![i, format!("data_{i}")],
            )
            .unwrap();
        }

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        assert!(result.path.exists());
        assert!(result.metadata.size_bytes > 0);
        assert_eq!(result.metadata.table_count, 1);

        // Verify restoration preserves all rows
        let restore_path = temp_dir.path().join("big_restored.db");
        manager.restore(&conn, &result.path, &restore_path).unwrap();

        let restored = Connection::open(&restore_path).unwrap();
        let count: i64 =
            restored.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 500);
    }

    #[test]
    fn test_backup_in_memory_database() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");

        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE mem_table (id INTEGER PRIMARY KEY)", []).unwrap();
        conn.execute("INSERT INTO mem_table VALUES (1)", []).unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        // database_path is just used for naming — use a synthetic name
        let result = manager.backup(&conn, Path::new("in_memory.db")).unwrap();

        assert!(result.path.exists());
        assert!(result.metadata.filename.starts_with("in_memory_"));

        // Verify the backed-up file is a valid SQLite database
        let check_conn = Connection::open(&result.path).unwrap();
        let count: i64 =
            check_conn.query_row("SELECT COUNT(*) FROM mem_table", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_backup_preserves_indexes_and_views() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("schema.db");
        let backup_dir = temp_dir.path().join("backups");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)", []).unwrap();
        conn.execute("CREATE INDEX idx_name ON items (name)", []).unwrap();
        conn.execute("CREATE VIEW item_names AS SELECT name FROM items", []).unwrap();

        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let result = manager.backup(&conn, &db_path).unwrap();

        let restored = Connection::open(&result.path).unwrap();
        // Verify index exists
        let idx_count: i64 = restored
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_name'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx_count, 1);

        // Verify view exists
        let view_count: i64 = restored
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='view' AND name='item_names'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(view_count, 1);
    }

    #[test]
    fn test_backup_debug_format() {
        let temp_dir = TempDir::new().unwrap();
        let backup_dir = temp_dir.path().join("backups");
        let manager = BackupManager::new(&backup_dir, BackupConfig::default()).unwrap();
        let dbg = format!("{manager:?}");
        assert!(dbg.contains("BackupManager"));
    }

    #[test]
    fn test_backup_metadata_debug() {
        let meta = BackupMetadata {
            filename: "x.db".into(),
            created_at: Utc::now(),
            size_bytes: 42,
            schema_version: 1,
            table_count: 2,
            compressed: true,
            checksum: "abc".into(),
        };
        let dbg = format!("{meta:?}");
        assert!(dbg.contains("BackupMetadata"));
    }

    #[test]
    fn test_backup_metadata_clone() {
        let meta = BackupMetadata {
            filename: "x.db".into(),
            created_at: Utc::now(),
            size_bytes: 42,
            schema_version: 1,
            table_count: 2,
            compressed: true,
            checksum: "abc".into(),
        };
        let clone = meta.clone();
        assert_eq!(clone.filename, meta.filename);
        assert_eq!(clone.checksum, meta.checksum);
    }
}
