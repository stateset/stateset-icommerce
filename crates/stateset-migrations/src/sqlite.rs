//! SQLite migrator implementation.
//!
//! Provides the [`SqliteMigrator`] for running migrations against a SQLite
//! database using transactions and recording applied migrations in a
//! `_migrations` metadata table.

use std::time::Instant;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension};

use crate::error::{MigrationError, Result};
use crate::migration::MigrationRecord;
use crate::registry::MigrationRegistry;
use crate::status::MigrationStatus;
use crate::version::SchemaVersion;

/// Name of the migrations metadata table.
const MIGRATIONS_TABLE: &str = "_migrations";

/// SQLite database migrator.
///
/// Applies registered migrations to a SQLite database, recording each applied
/// migration in a `_migrations` table for tracking and checksum validation.
///
/// # Examples
///
/// ```
/// use stateset_migrations::{Migration, MigrationRegistry, SqliteMigrator};
///
/// let registry = MigrationRegistry::builder()
///     .add(Migration::new(1, "init", "CREATE TABLE test (id INTEGER PRIMARY KEY);"))
///     .build()
///     .unwrap();
///
/// let migrator = SqliteMigrator::new(registry);
/// let conn = rusqlite::Connection::open_in_memory().unwrap();
/// let applied = migrator.migrate(&conn).unwrap();
/// assert_eq!(applied.len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct SqliteMigrator {
    registry: MigrationRegistry,
}

impl SqliteMigrator {
    /// Create a new migrator with the given registry.
    #[must_use]
    pub const fn new(registry: MigrationRegistry) -> Self {
        Self { registry }
    }

    /// Get a reference to the underlying registry.
    #[must_use]
    pub const fn registry(&self) -> &MigrationRegistry {
        &self.registry
    }

    /// Ensure the migrations metadata table exists.
    fn ensure_migrations_table(conn: &Connection) -> Result<()> {
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {MIGRATIONS_TABLE} (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                checksum TEXT NOT NULL,
                execution_time_ms INTEGER NOT NULL DEFAULT 0
            )"
        ))?;
        Ok(())
    }

    /// Load all applied migration records from the database.
    fn load_applied(conn: &Connection) -> Result<Vec<MigrationRecord>> {
        let mut stmt = conn.prepare(&format!(
            "SELECT version, name, applied_at, checksum, execution_time_ms
             FROM {MIGRATIONS_TABLE}
             ORDER BY version"
        ))?;

        let rows = stmt
            .query_map([], |row| {
                let version: u32 = row.get(0)?;
                let name: String = row.get(1)?;
                let applied_at_str: String = row.get(2)?;
                let checksum: String = row.get(3)?;
                let execution_time_ms: u64 = row.get(4)?;
                Ok((version, name, applied_at_str, checksum, execution_time_ms))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut records = Vec::with_capacity(rows.len());
        for (version, name, applied_at_str, checksum, execution_time_ms) in rows {
            let applied_at = parse_datetime(&applied_at_str).map_err(|err| {
                MigrationError::InvalidMigration {
                    reason: format!(
                        "invalid applied_at timestamp for migration v{version} '{name}': {err}",
                    ),
                }
            })?;
            records.push(MigrationRecord {
                version,
                name,
                applied_at,
                checksum,
                execution_time_ms,
            });
        }

        Ok(records)
    }

    fn is_version_conflict(error: &rusqlite::Error) -> bool {
        match error {
            rusqlite::Error::SqliteFailure(code, msg) => {
                code.code == rusqlite::ErrorCode::ConstraintViolation
                    && msg.as_deref().is_some_and(|text| text.contains("_migrations.version"))
            }
            _ => false,
        }
    }

    /// Run all pending migrations within transactions.
    ///
    /// Returns the list of newly applied migration records.
    ///
    /// Each migration runs in its own transaction. If a migration fails,
    /// all previously applied migrations in this call remain committed,
    /// and the error is returned.
    pub fn migrate(&self, conn: &Connection) -> Result<Vec<MigrationRecord>> {
        Self::ensure_migrations_table(conn)?;
        let applied = Self::load_applied(conn)?;

        // Validate existing checksums before applying new migrations
        self.registry.validate_checksums(&applied)?;

        let pending = self.registry.pending(&applied);
        let mut newly_applied = Vec::with_capacity(pending.len());

        for migration in pending {
            let start = Instant::now();

            let tx = conn.unchecked_transaction()?;
            let already_applied = tx
                .query_row(
                    &format!("SELECT 1 FROM {MIGRATIONS_TABLE} WHERE version = ?1 LIMIT 1",),
                    rusqlite::params![migration.version],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if already_applied {
                tx.rollback()?;
                continue;
            }

            if let Err(exec_err) = tx.execute_batch(&migration.up_sql) {
                // Another migrator may have committed this version while we were running.
                let now_applied = tx
                    .query_row(
                        &format!("SELECT 1 FROM {MIGRATIONS_TABLE} WHERE version = ?1 LIMIT 1",),
                        rusqlite::params![migration.version],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some();
                if now_applied {
                    tx.rollback()?;
                    continue;
                }
                return Err(exec_err.into());
            }

            let insert_result = tx.execute(
                &format!(
                    "INSERT INTO {MIGRATIONS_TABLE} (version, name, applied_at, checksum, execution_time_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5)"
                ),
                rusqlite::params![
                    migration.version,
                    migration.name,
                    Utc::now().to_rfc3339(),
                    migration.checksum,
                    start.elapsed().as_millis() as u64,
                ],
            );

            if let Err(insert_err) = insert_result {
                if Self::is_version_conflict(&insert_err) {
                    tx.rollback()?;
                    continue;
                }
                return Err(insert_err.into());
            }

            tx.commit()?;

            let execution_time_ms = start.elapsed().as_millis() as u64;
            newly_applied.push(MigrationRecord {
                version: migration.version,
                name: migration.name.clone(),
                applied_at: Utc::now(),
                checksum: migration.checksum.clone(),
                execution_time_ms,
            });
        }

        Ok(newly_applied)
    }

    /// Rollback applied migrations down to (but not including) `target_version`.
    ///
    /// Runs the `down_sql` of each migration in reverse order. Returns the
    /// list of rolled-back migration records.
    pub fn rollback(&self, conn: &Connection, target_version: u32) -> Result<Vec<MigrationRecord>> {
        Self::ensure_migrations_table(conn)?;
        let applied = Self::load_applied(conn)?;

        let current_version = applied.iter().map(|r| r.version).max().unwrap_or(0);
        if target_version >= current_version {
            return Ok(vec![]);
        }

        let to_rollback = self.registry.range_reverse(target_version, current_version);
        let mut rolled_back = Vec::new();

        for migration in to_rollback {
            let down_sql =
                migration.down_sql.as_ref().ok_or_else(|| MigrationError::NoDownMigration {
                    version: migration.version,
                    name: migration.name.clone(),
                })?;

            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(down_sql).map_err(|e| MigrationError::RollbackFailed {
                version: migration.version,
                name: migration.name.clone(),
                reason: e.to_string(),
            })?;
            tx.execute(
                &format!("DELETE FROM {MIGRATIONS_TABLE} WHERE version = ?1"),
                rusqlite::params![migration.version],
            )?;
            tx.commit()?;

            rolled_back.push(MigrationRecord {
                version: migration.version,
                name: migration.name.clone(),
                applied_at: Utc::now(),
                checksum: migration.checksum.clone(),
                execution_time_ms: 0,
            });
        }

        Ok(rolled_back)
    }

    /// Get the current migration status.
    pub fn status(&self, conn: &Connection) -> Result<MigrationStatus> {
        Self::ensure_migrations_table(conn)?;
        let applied = Self::load_applied(conn)?;

        let current = applied.iter().map(|r| r.version).max().unwrap_or(0);
        let latest = self.registry.latest_version().unwrap_or(0);
        let pending_migrations = self.registry.pending(&applied);
        let pending_names: Vec<String> =
            pending_migrations.iter().map(|m| m.name.clone()).collect();
        let pending_count = pending_names.len() as u32;

        let checksum_valid = self.registry.validate_checksums(&applied).is_ok();

        Ok(MigrationStatus {
            schema_version: SchemaVersion { current, latest, pending: pending_count },
            applied,
            pending: pending_names,
            checksum_valid,
        })
    }

    /// Validate that all applied migration checksums match the registry.
    pub fn validate(&self, conn: &Connection) -> Result<()> {
        Self::ensure_migrations_table(conn)?;
        let applied = Self::load_applied(conn)?;
        self.registry.validate_checksums(&applied)
    }
}

/// Parse a datetime string in RFC 3339 format.
fn parse_datetime(s: &str) -> std::result::Result<DateTime<Utc>, chrono::ParseError> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::Migration;
    use chrono::Datelike;

    fn test_registry() -> MigrationRegistry {
        MigrationRegistry::builder()
            .add(Migration::with_down(
                1,
                "create_users",
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
                "DROP TABLE IF EXISTS users;",
            ))
            .add(Migration::with_down(
                2,
                "create_posts",
                "CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER, title TEXT);",
                "DROP TABLE IF EXISTS posts;",
            ))
            .add(Migration::with_down(
                3,
                "create_comments",
                "CREATE TABLE comments (id INTEGER PRIMARY KEY, post_id INTEGER, body TEXT);",
                "DROP TABLE IF EXISTS comments;",
            ))
            .build()
            .unwrap()
    }

    fn memory_conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn migrate_fresh_database() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        let applied = migrator.migrate(&conn).unwrap();
        assert_eq!(applied.len(), 3);
        assert_eq!(applied[0].version, 1);
        assert_eq!(applied[1].version, 2);
        assert_eq!(applied[2].version, 3);
    }

    #[test]
    fn migrate_partially_applied() {
        let conn = memory_conn();
        let reg1 = MigrationRegistry::builder()
            .add(Migration::new(
                1,
                "create_users",
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
            ))
            .build()
            .unwrap();
        let migrator1 = SqliteMigrator::new(reg1);
        migrator1.migrate(&conn).unwrap();

        // Now use full registry
        let migrator2 = SqliteMigrator::new(test_registry());
        let applied = migrator2.migrate(&conn).unwrap();
        assert_eq!(applied.len(), 2); // Only v2 and v3
        assert_eq!(applied[0].version, 2);
        assert_eq!(applied[1].version, 3);
    }

    #[test]
    fn migrate_idempotent() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();

        // Run again — should apply nothing
        let applied = migrator.migrate(&conn).unwrap();
        assert!(applied.is_empty());
    }

    #[test]
    fn rollback_to_version() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();

        let rolled_back = migrator.rollback(&conn, 1).unwrap();
        assert_eq!(rolled_back.len(), 2); // v3 and v2 rolled back
        assert_eq!(rolled_back[0].version, 3);
        assert_eq!(rolled_back[1].version, 2);
    }

    #[test]
    fn rollback_to_zero() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();

        let rolled_back = migrator.rollback(&conn, 0).unwrap();
        assert_eq!(rolled_back.len(), 3);
    }

    #[test]
    fn rollback_noop_when_at_target() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();

        let rolled_back = migrator.rollback(&conn, 3).unwrap();
        assert!(rolled_back.is_empty());
    }

    #[test]
    fn rollback_noop_when_above_current() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();

        let rolled_back = migrator.rollback(&conn, 99).unwrap();
        assert!(rolled_back.is_empty());
    }

    #[test]
    fn rollback_no_down_sql_fails() {
        let reg = MigrationRegistry::builder()
            .add(Migration::new(1, "create_stuff", "CREATE TABLE stuff (id INTEGER PRIMARY KEY);"))
            .build()
            .unwrap();
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(reg);
        migrator.migrate(&conn).unwrap();

        let err = migrator.rollback(&conn, 0).unwrap_err();
        assert!(matches!(err, MigrationError::NoDownMigration { version: 1, .. }));
    }

    #[test]
    fn status_fresh_database() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        let status = migrator.status(&conn).unwrap();

        assert_eq!(status.schema_version.current, 0);
        assert_eq!(status.schema_version.latest, 3);
        assert_eq!(status.schema_version.pending, 3);
        assert!(!status.schema_version.is_up_to_date());
        assert!(status.applied.is_empty());
        assert_eq!(status.pending.len(), 3);
        assert!(status.checksum_valid);
    }

    #[test]
    fn status_after_full_migration() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();

        let status = migrator.status(&conn).unwrap();
        assert_eq!(status.schema_version.current, 3);
        assert_eq!(status.schema_version.latest, 3);
        assert_eq!(status.schema_version.pending, 0);
        assert!(status.schema_version.is_up_to_date());
        assert_eq!(status.applied.len(), 3);
        assert!(status.pending.is_empty());
        assert!(status.checksum_valid);
        assert!(status.is_healthy());
    }

    #[test]
    fn status_after_partial_migration() {
        let conn = memory_conn();
        let reg = MigrationRegistry::builder()
            .add(Migration::new(
                1,
                "create_users",
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
            ))
            .build()
            .unwrap();
        SqliteMigrator::new(reg).migrate(&conn).unwrap();

        let migrator = SqliteMigrator::new(test_registry());
        let status = migrator.status(&conn).unwrap();
        assert_eq!(status.schema_version.current, 1);
        assert_eq!(status.schema_version.pending, 2);
    }

    #[test]
    fn validate_ok() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();
        assert!(migrator.validate(&conn).is_ok());
    }

    #[test]
    fn validate_detects_tampered_checksum() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();

        // Tamper with a checksum in the database
        conn.execute(
            &format!("UPDATE {MIGRATIONS_TABLE} SET checksum = 'tampered' WHERE version = 1"),
            [],
        )
        .unwrap();

        let err = migrator.validate(&conn).unwrap_err();
        assert!(matches!(err, MigrationError::ChecksumMismatch { version: 1, .. }));
    }

    #[test]
    fn migrate_then_rollback_then_remigrate() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("migration-rehearsal.db");
        let conn = Connection::open(&db_path).unwrap();
        let migrator = SqliteMigrator::new(test_registry());

        // Apply all
        migrator.migrate(&conn).unwrap();
        assert_eq!(migrator.status(&conn).unwrap().schema_version.current, 3);
        conn.execute("INSERT INTO users (id, name) VALUES (1, 'Ada')", []).unwrap();
        conn.execute("INSERT INTO posts (id, user_id, title) VALUES (1, 1, 'proof')", []).unwrap();

        // Rollback to v1. Data owned by the retained migration must survive,
        // while schema introduced above the target must disappear.
        let rolled_back = migrator.rollback(&conn, 1).unwrap();
        assert_eq!(rolled_back.iter().map(|record| record.version).collect::<Vec<_>>(), [3, 2]);
        assert_eq!(migrator.status(&conn).unwrap().schema_version.current, 1);
        let user_name: String =
            conn.query_row("SELECT name FROM users WHERE id = 1", [], |row| row.get(0)).unwrap();
        assert_eq!(user_name, "Ada");
        let posts_table: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'posts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(posts_table, 0);

        // Re-migrate
        let applied = migrator.migrate(&conn).unwrap();
        assert_eq!(applied.len(), 2);
        let status = migrator.status(&conn).unwrap();
        assert_eq!(status.schema_version.current, 3);
        assert!(status.is_healthy());
        let user_name: String =
            conn.query_row("SELECT name FROM users WHERE id = 1", [], |row| row.get(0)).unwrap();
        assert_eq!(user_name, "Ada");
        conn.execute("INSERT INTO posts (id, user_id, title) VALUES (2, 1, 'restored')", [])
            .unwrap();
        let integrity: String =
            conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
        assert_eq!(integrity, "ok");

        println!(
            "migration-proof path={} rollback=3,2 target=1 remigrated=2,3 data_preserved=true integrity={integrity}",
            db_path.display()
        );
    }

    #[test]
    fn execution_time_is_recorded() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        let applied = migrator.migrate(&conn).unwrap();

        for record in &applied {
            // Execution time should be non-negative (typically 0 for trivial SQL)
            assert!(record.execution_time_ms < 10_000); // sanity: less than 10 seconds
        }
    }

    #[test]
    fn migrations_table_schema() {
        let conn = memory_conn();
        SqliteMigrator::ensure_migrations_table(&conn).unwrap();

        // Verify the table exists and has the expected columns
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({MIGRATIONS_TABLE})")).unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert!(columns.contains(&"version".to_string()));
        assert!(columns.contains(&"name".to_string()));
        assert!(columns.contains(&"applied_at".to_string()));
        assert!(columns.contains(&"checksum".to_string()));
        assert!(columns.contains(&"execution_time_ms".to_string()));
    }

    #[test]
    fn checksum_mismatch_prevents_migration() {
        let conn = memory_conn();

        // Apply v1 with one registry
        let reg1 = MigrationRegistry::builder()
            .add(Migration::new(
                1,
                "create_users",
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);",
            ))
            .build()
            .unwrap();
        SqliteMigrator::new(reg1).migrate(&conn).unwrap();

        // Try to migrate with a different v1 SQL
        let reg2 = MigrationRegistry::builder()
            .add(Migration::new(
                1,
                "create_users",
                "CREATE TABLE users (id TEXT PRIMARY KEY, name TEXT, email TEXT);",
            ))
            .add(Migration::new(2, "create_posts", "CREATE TABLE posts (id INTEGER PRIMARY KEY);"))
            .build()
            .unwrap();

        let err = SqliteMigrator::new(reg2).migrate(&conn).unwrap_err();
        assert!(matches!(err, MigrationError::ChecksumMismatch { version: 1, .. }));
    }

    #[test]
    fn empty_registry_migrate_is_noop() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(MigrationRegistry::new());
        let applied = migrator.migrate(&conn).unwrap();
        assert!(applied.is_empty());
    }

    #[test]
    fn registry_accessor() {
        let reg = test_registry();
        let migrator = SqliteMigrator::new(reg.clone());
        assert_eq!(migrator.registry().len(), reg.len());
    }

    #[test]
    fn parse_datetime_valid_rfc3339() {
        let dt = parse_datetime("2024-01-15T10:30:00+00:00").unwrap();
        assert_eq!(dt.year(), 2024);
    }

    #[test]
    fn parse_datetime_invalid_returns_error() {
        assert!(parse_datetime("not-a-date").is_err());
    }

    #[test]
    fn status_fails_on_invalid_applied_timestamp() {
        let conn = memory_conn();
        SqliteMigrator::ensure_migrations_table(&conn).unwrap();
        conn.execute(
            &format!(
                "INSERT INTO {MIGRATIONS_TABLE} (version, name, applied_at, checksum, execution_time_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)"
            ),
            rusqlite::params![1u32, "bad_time", "not-a-date", "checksum", 0u64],
        )
        .unwrap();

        let migrator = SqliteMigrator::new(MigrationRegistry::new());
        let err = migrator.status(&conn).unwrap_err();
        assert!(matches!(err, MigrationError::InvalidMigration { .. }));
    }

    #[test]
    fn migrate_records_correct_checksums() {
        let conn = memory_conn();
        let registry = test_registry();
        let migrator = SqliteMigrator::new(registry.clone());
        migrator.migrate(&conn).unwrap();

        let status = migrator.status(&conn).unwrap();
        for record in &status.applied {
            let expected = registry.get(record.version).unwrap();
            assert_eq!(record.checksum, expected.checksum);
        }
    }

    #[test]
    fn rollback_then_status_shows_pending() {
        let conn = memory_conn();
        let migrator = SqliteMigrator::new(test_registry());
        migrator.migrate(&conn).unwrap();
        migrator.rollback(&conn, 1).unwrap();

        let status = migrator.status(&conn).unwrap();
        assert_eq!(status.schema_version.current, 1);
        assert_eq!(status.schema_version.pending, 2);
        assert!(!status.is_healthy());
    }
}
