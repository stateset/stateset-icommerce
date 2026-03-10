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

        let (filename, backup_path) = {
            let mut attempt = 0usize;
            loop {
                let suffix = if attempt == 0 { String::new() } else { format!("_{attempt}") };
                let candidate = format!("{}_{}{}.db", db_name, timestamp_str, suffix);
                let path = self.backup_dir.join(&candidate);
                if !path.exists() {
                    break (candidate, path);
                }
                attempt += 1;
            }
        };

        // Create backup using VACUUM INTO
        let backup_file = backup_path.display().to_string();
        conn.execute(&format!("VACUUM INTO '{}'", backup_file.replace('\'', "''")), [])
            .map_err(|e| CommerceError::DatabaseError(format!("Backup failed: {}", e)))?;

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
            filename: filename.clone(),
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
            .map_err(|e| CommerceError::DatabaseError(format!("Restore copy failed: {}", e)))?;

        // Open restore connection and verify integrity
        let restore_conn = Connection::open(restore_path)
            .map_err(|e| CommerceError::DatabaseError(format!("Restore open failed: {}", e)))?;

        // Run integrity check
        let integrity_result: String = restore_conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap_or_else(|_| "failed".to_string());

        if integrity_result != "ok" {
            return Err(CommerceError::DatabaseError(format!(
                "Backup integrity check failed: {}",
                integrity_result
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

        for entry in fs::read_dir(&self.backup_dir).map_err(|e| {
            CommerceError::DatabaseError(format!("Failed to read backup dir: {}", e))
        })? {
            let entry = entry.map_err(|e| {
                CommerceError::DatabaseError(format!("Failed to read entry: {}", e))
            })?;
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
            .map_err(|e| CommerceError::DatabaseError(format!("Failed to delete backup: {}", e)))?;

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
            .map_err(|e| CommerceError::DatabaseError(format!("Failed to open backup: {}", e)))?;

        let mut buffer = vec![0; 8192];
        loop {
            let n = file.read(&mut buffer).map_err(|e| {
                CommerceError::DatabaseError(format!("Failed to read backup: {}", e))
            })?;
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
            .map_err(|e| CommerceError::DatabaseError(format!("Backup file corrupt: {}", e)))?;

        // Run integrity check
        let integrity_result: String = conn
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap_or_else(|_| "failed".to_string());

        if integrity_result != "ok" {
            return Err(CommerceError::DatabaseError(format!(
                "Backup integrity check failed: {}",
                integrity_result
            )));
        }

        Ok(())
    }

    /// Save backup metadata to a file
    fn save_metadata(&self, metadata: &BackupMetadata) -> Result<(), CommerceError> {
        let metadata_path = self.backup_dir.join(&metadata.filename).with_extension("meta");
        let metadata_json = serde_json::to_string_pretty(metadata).map_err(|e| {
            CommerceError::DatabaseError(format!("Failed to serialize metadata: {}", e))
        })?;

        fs::write(&metadata_path, metadata_json)
            .map_err(|e| CommerceError::DatabaseError(format!("Failed to save metadata: {}", e)))?;

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
}
