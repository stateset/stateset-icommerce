//! Error types for the migration framework.

use thiserror::Error;

/// Errors that can occur during migration operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum MigrationError {
    /// A migration with the same version is already registered.
    #[error(
        "version conflict: migration version {version} already registered as '{existing_name}', cannot register '{new_name}'"
    )]
    VersionConflict {
        /// The conflicting version number.
        version: u32,
        /// Name of the already-registered migration.
        existing_name: String,
        /// Name of the migration that failed to register.
        new_name: String,
    },

    /// An applied migration's checksum does not match the registered migration.
    #[error(
        "checksum mismatch for migration v{version} '{name}': expected {expected}, found {actual}"
    )]
    ChecksumMismatch {
        /// The migration version.
        version: u32,
        /// The migration name.
        name: String,
        /// The expected checksum (from the registry).
        expected: String,
        /// The actual checksum (from the database record).
        actual: String,
    },

    /// An underlying SQLite error occurred.
    #[error("SQL error: {0}")]
    SqlError(#[from] rusqlite::Error),

    /// A rollback failed (e.g., no down SQL provided).
    #[error("rollback failed for migration v{version} '{name}': {reason}")]
    RollbackFailed {
        /// The migration version.
        version: u32,
        /// The migration name.
        name: String,
        /// The reason the rollback failed.
        reason: String,
    },

    /// The migration has already been applied.
    #[error("migration v{version} '{name}' has already been applied")]
    AlreadyApplied {
        /// The migration version.
        version: u32,
        /// The migration name.
        name: String,
    },

    /// The migration definition is invalid.
    #[error("invalid migration: {reason}")]
    InvalidMigration {
        /// The reason the migration is invalid.
        reason: String,
    },

    /// A rollback was attempted but the migration has no down SQL.
    #[error("migration v{version} '{name}' has no down SQL for rollback")]
    NoDownMigration {
        /// The migration version.
        version: u32,
        /// The migration name.
        name: String,
    },
}

/// Convenience alias for migration results.
pub type Result<T> = std::result::Result<T, MigrationError>;
