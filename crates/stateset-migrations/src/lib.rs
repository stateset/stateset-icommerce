#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet Migrations
//!
//! A schema migration framework for StateSet iCommerce, with first-class
//! SQLite support and extensibility for PostgreSQL.
//!
//! ## Overview
//!
//! This crate provides:
//!
//! - **[`Migration`]** and **[`MigrationRecord`]** types for defining and
//!   tracking database schema changes
//! - **[`MigrationRegistry`]** for managing ordered, versioned migrations with
//!   checksum validation
//! - **[`SqliteMigrator`]** for applying/rolling back migrations against SQLite
//! - **Built-in migrations** for the full StateSet iCommerce schema (V1–V4)
//! - **[`SchemaVersion`]** and **[`MigrationStatus`]** for reporting
//!
//! ## Quick Start
//!
//! ```
//! use stateset_migrations::{builtin_registry, SqliteMigrator};
//!
//! let registry = builtin_registry().unwrap();
//! let migrator = SqliteMigrator::new(registry);
//!
//! let conn = rusqlite::Connection::open_in_memory().unwrap();
//! let applied = migrator.migrate(&conn).unwrap();
//! println!("Applied {} migrations", applied.len());
//!
//! let status = migrator.status(&conn).unwrap();
//! println!("Schema: {}", status.schema_version);
//! ```
//!
//! ## Custom Migrations
//!
//! You can extend the built-in registry with your own migrations:
//!
//! ```
//! use stateset_migrations::{Migration, MigrationRegistry};
//!
//! let registry = MigrationRegistry::builder()
//!     .add(Migration::new(1, "create_users", "CREATE TABLE users (id TEXT PRIMARY KEY);"))
//!     .add(Migration::with_down(
//!         2,
//!         "add_email",
//!         "ALTER TABLE users ADD COLUMN email TEXT;",
//!         "SELECT 1; -- SQLite cannot drop columns easily",
//!     ))
//!     .build()
//!     .unwrap();
//! ```

pub mod builtins;
pub mod error;
pub mod migration;
pub mod registry;
pub mod sqlite;
pub mod status;
pub mod version;

// Re-export primary types for convenience.
pub use builtins::builtin_registry;
pub use error::MigrationError;
pub use migration::{Migration, MigrationRecord};
pub use registry::MigrationRegistry;
pub use sqlite::SqliteMigrator;
pub use status::MigrationStatus;
pub use version::SchemaVersion;
