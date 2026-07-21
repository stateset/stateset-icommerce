//! Backup, restore and structured export/import.
//!
//! Reached via [`Commerce::maintenance`](crate::Commerce::maintenance).
//!
//! Two complementary tools:
//!
//! - **Backup / restore** ([`Maintenance::backup_to`], [`Maintenance::restore_from`])
//!   move a byte-exact, consistent image of the SQLite database. This is the
//!   disaster-recovery path: identity, history and every table are preserved.
//!   SQLite-backed instances only.
//! - **Export / import** ([`Maintenance::export_to_file`], [`Maintenance::import_from_file`])
//!   move business records as versioned JSON. Backend-independent, diffable,
//!   and survives schema changes — but IDs are re-minted on import. See
//!   [`stateset_db::portability`] for exactly what is and is not covered.
//!
//! # Example
//!
//! ```ignore
//! use stateset_embedded::Commerce;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Nightly backup.
//! let report = commerce.maintenance().backup_to("./backups/store-nightly.db")?;
//! println!("sha256 {}", report.manifest.checksum);
//!
//! // Portable snapshot.
//! commerce.maintenance().export_to_file("./exports/store.json")?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{CommerceError, Result};
use stateset_db::Database;
use std::path::Path;
use std::sync::Arc;

#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

pub use stateset_db::portability::{
    ConflictPolicy, ExportOptions, ExportReport, ImportOptions, ImportReport,
};

#[cfg(feature = "sqlite")]
pub use stateset_db::maintenance::{BackupManifest, BackupReport, RestoreOptions, RestoreReport};

/// Backup, restore, export and import operations.
pub struct Maintenance {
    db: Arc<dyn Database>,
    #[cfg(feature = "sqlite")]
    sqlite: Option<Arc<SqliteDatabase>>,
    #[cfg(feature = "sqlite")]
    sqlite_path: Option<String>,
}

impl std::fmt::Debug for Maintenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Maintenance").finish_non_exhaustive()
    }
}

impl Maintenance {
    #[cfg(feature = "sqlite")]
    pub(crate) fn new(
        db: Arc<dyn Database>,
        sqlite: Option<Arc<SqliteDatabase>>,
        sqlite_path: Option<String>,
    ) -> Self {
        Self { db, sqlite, sqlite_path }
    }

    #[cfg(not(feature = "sqlite"))]
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether file-level backup and restore are available.
    ///
    /// False for PostgreSQL and externally-supplied databases — use
    /// [`Self::export_to_file`] for those, or the backend's own tooling
    /// (`pg_dump`).
    #[must_use]
    pub const fn supports_backup(&self) -> bool {
        #[cfg(feature = "sqlite")]
        {
            self.sqlite.is_some()
        }
        #[cfg(not(feature = "sqlite"))]
        {
            false
        }
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_handle(&self) -> Result<&Arc<SqliteDatabase>> {
        self.sqlite.as_ref().ok_or_else(|| {
            CommerceError::ValidationError(
                "backup and restore require a SQLite-backed Commerce instance".to_owned(),
            )
        })
    }

    /// Take a consistent backup to `backup_path`, writing a sidecar manifest.
    ///
    /// Safe to call while other threads are writing: the backup is taken with
    /// `VACUUM INTO` inside a read transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the instance is not SQLite-backed, or when the
    /// backup or its checksum verification fails.
    #[cfg(feature = "sqlite")]
    pub fn backup_to(&self, backup_path: impl AsRef<Path>) -> Result<BackupReport> {
        let handle = self.sqlite_handle()?;
        let conn = handle.conn()?;
        let source = self.sqlite_path.clone().unwrap_or_else(|| ":memory:".to_owned());
        stateset_db::maintenance::backup_to(&conn, source, backup_path).map_err(Into::into)
    }

    /// Restore a backup over `target_path`.
    ///
    /// This does **not** affect the currently-open database handle: restore
    /// writes a file, and the process must reopen [`crate::Commerce`] against
    /// the restored path to see the new data. Restoring over the path this
    /// instance currently has open is rejected, because the open connection
    /// pool would keep serving pages from the replaced file.
    ///
    /// # Errors
    ///
    /// See [`stateset_db::maintenance::restore_from`]. Also errors when
    /// `target_path` is the database this instance has open.
    #[cfg(feature = "sqlite")]
    pub fn restore_from(
        &self,
        backup_path: impl AsRef<Path>,
        target_path: impl AsRef<Path>,
        options: &RestoreOptions,
    ) -> Result<RestoreReport> {
        let target = target_path.as_ref();
        if let Some(open_path) = &self.sqlite_path {
            let same = Path::new(open_path)
                .canonicalize()
                .ok()
                .zip(target.canonicalize().ok())
                .is_some_and(|(a, b)| a == b);
            if same {
                return Err(CommerceError::ValidationError(format!(
                    "refusing to restore over '{open_path}', which this Commerce instance has \
                     open; restore to a new path and reopen, or close this instance first"
                )));
            }
        }
        stateset_db::maintenance::restore_from(backup_path, target, options).map_err(Into::into)
    }

    /// Write a structured JSON export to `writer`.
    ///
    /// # Errors
    ///
    /// Returns an error if a repository read or the write fails.
    pub fn export_all<W: std::io::Write>(
        &self,
        writer: &mut W,
        options: &ExportOptions,
    ) -> Result<ExportReport> {
        stateset_db::portability::export_all(self.db.as_ref(), writer, options)
    }

    /// Write a structured JSON export to `path`, creating parent directories.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or the export fails.
    pub fn export_to_file(&self, path: impl AsRef<Path>) -> Result<ExportReport> {
        self.export_to_file_with(path, &ExportOptions::default())
    }

    /// [`Self::export_to_file`] with explicit options.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created or the export fails.
    pub fn export_to_file_with(
        &self,
        path: impl AsRef<Path>,
        options: &ExportOptions,
    ) -> Result<ExportReport> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| {
                CommerceError::DatabaseError(format!(
                    "cannot create export directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        let file = std::fs::File::create(path).map_err(|e| {
            CommerceError::DatabaseError(format!("cannot create {}: {e}", path.display()))
        })?;
        let mut writer = std::io::BufWriter::new(file);
        self.export_all(&mut writer, options)
    }

    /// Read a structured JSON export from `reader` and replay it.
    ///
    /// # Errors
    ///
    /// See [`stateset_db::portability::import_all`].
    pub fn import_all<R: std::io::Read>(
        &self,
        reader: &mut R,
        options: &ImportOptions,
    ) -> Result<ImportReport> {
        stateset_db::portability::import_all(self.db.as_ref(), reader, options)
    }

    /// Read a structured JSON export from `path` and replay it.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the import fails.
    pub fn import_from_file(
        &self,
        path: impl AsRef<Path>,
        options: &ImportOptions,
    ) -> Result<ImportReport> {
        let path = path.as_ref();
        let file = std::fs::File::open(path).map_err(|e| {
            CommerceError::DatabaseError(format!("cannot open {}: {e}", path.display()))
        })?;
        let mut reader = std::io::BufReader::new(file);
        self.import_all(&mut reader, options)
    }

    /// Domains the export covers, in export order.
    #[must_use]
    pub fn exportable_domains(&self) -> Vec<&'static str> {
        stateset_db::portability::exportable_domains()
    }

    /// Domains the import can write.
    #[must_use]
    pub fn importable_domains(&self) -> Vec<&'static str> {
        stateset_db::portability::importable_domains()
    }
}
