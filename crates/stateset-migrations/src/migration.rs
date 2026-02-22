//! Core migration types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A database migration definition.
///
/// Each migration has a unique version number, a human-readable name,
/// the SQL to apply ("up"), and optionally the SQL to reverse it ("down").
/// A SHA-256 checksum is computed from the `up_sql` to detect tampering.
///
/// # Examples
///
/// ```
/// use stateset_migrations::Migration;
///
/// let m = Migration::new(1, "create_users", "CREATE TABLE users (id TEXT PRIMARY KEY);");
/// assert_eq!(m.version, 1);
/// assert!(!m.checksum.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Migration {
    /// Unique monotonically increasing version number.
    pub version: u32,
    /// Human-readable migration name (e.g., `"create_users"`).
    pub name: String,
    /// SQL to apply the migration.
    pub up_sql: String,
    /// Optional SQL to reverse the migration.
    pub down_sql: Option<String>,
    /// SHA-256 hex digest of `up_sql`.
    pub checksum: String,
}

impl Migration {
    /// Create a new migration without down SQL.
    ///
    /// The checksum is computed automatically from `up_sql`.
    #[must_use]
    pub fn new(version: u32, name: impl Into<String>, up_sql: impl Into<String>) -> Self {
        let up_sql = up_sql.into();
        let checksum = compute_checksum(&up_sql);
        Self { version, name: name.into(), up_sql, down_sql: None, checksum }
    }

    /// Create a new migration with down SQL for rollback support.
    #[must_use]
    pub fn with_down(
        version: u32,
        name: impl Into<String>,
        up_sql: impl Into<String>,
        down_sql: impl Into<String>,
    ) -> Self {
        let up_sql = up_sql.into();
        let checksum = compute_checksum(&up_sql);
        Self {
            version,
            name: name.into(),
            up_sql,
            down_sql: Some(down_sql.into()),
            checksum,
        }
    }

    /// Returns `true` if this migration supports rollback.
    #[must_use]
    pub const fn has_down(&self) -> bool {
        self.down_sql.is_some()
    }
}

/// A record of an applied migration stored in the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationRecord {
    /// The migration version that was applied.
    pub version: u32,
    /// The migration name.
    pub name: String,
    /// When the migration was applied.
    pub applied_at: DateTime<Utc>,
    /// The checksum recorded at apply time.
    pub checksum: String,
    /// How long the migration took to execute, in milliseconds.
    pub execution_time_ms: u64,
}

/// Compute the SHA-256 hex digest of the given SQL string.
#[must_use]
pub fn compute_checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Encode bytes as lowercase hex.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_computes_checksum() {
        let m = Migration::new(1, "test", "SELECT 1;");
        assert!(!m.checksum.is_empty());
        assert_eq!(m.checksum.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn same_sql_same_checksum() {
        let m1 = Migration::new(1, "a", "SELECT 1;");
        let m2 = Migration::new(2, "b", "SELECT 1;");
        assert_eq!(m1.checksum, m2.checksum);
    }

    #[test]
    fn different_sql_different_checksum() {
        let m1 = Migration::new(1, "a", "SELECT 1;");
        let m2 = Migration::new(2, "b", "SELECT 2;");
        assert_ne!(m1.checksum, m2.checksum);
    }

    #[test]
    fn with_down_sets_down_sql() {
        let m = Migration::with_down(1, "test", "CREATE TABLE t (id INT);", "DROP TABLE t;");
        assert!(m.has_down());
        assert_eq!(m.down_sql.as_deref(), Some("DROP TABLE t;"));
    }

    #[test]
    fn new_has_no_down_sql() {
        let m = Migration::new(1, "test", "SELECT 1;");
        assert!(!m.has_down());
        assert!(m.down_sql.is_none());
    }

    #[test]
    fn checksum_is_deterministic() {
        let sql = "CREATE TABLE users (id TEXT PRIMARY KEY);";
        let c1 = compute_checksum(sql);
        let c2 = compute_checksum(sql);
        assert_eq!(c1, c2);
    }

    #[test]
    fn checksum_is_sha256_hex() {
        let checksum = compute_checksum("hello");
        assert_eq!(checksum.len(), 64);
        assert!(checksum.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn migration_version_and_name() {
        let m = Migration::new(42, "add_index", "CREATE INDEX idx ON t(c);");
        assert_eq!(m.version, 42);
        assert_eq!(m.name, "add_index");
        assert_eq!(m.up_sql, "CREATE INDEX idx ON t(c);");
    }

    #[test]
    fn migration_record_fields() {
        let record = MigrationRecord {
            version: 1,
            name: "test".to_string(),
            applied_at: Utc::now(),
            checksum: "abc123".to_string(),
            execution_time_ms: 42,
        };
        assert_eq!(record.version, 1);
        assert_eq!(record.execution_time_ms, 42);
    }

    #[test]
    fn migration_serialization_roundtrip() {
        let m = Migration::with_down(1, "test", "UP;", "DOWN;");
        let json = serde_json::to_string(&m).unwrap();
        let m2: Migration = serde_json::from_str(&json).unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn migration_record_serialization_roundtrip() {
        let record = MigrationRecord {
            version: 3,
            name: "add_col".to_string(),
            applied_at: Utc::now(),
            checksum: "deadbeef".to_string(),
            execution_time_ms: 100,
        };
        let json = serde_json::to_string(&record).unwrap();
        let record2: MigrationRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record, record2);
    }

    #[test]
    fn empty_sql_still_has_checksum() {
        let m = Migration::new(1, "empty", "");
        assert_eq!(m.checksum.len(), 64);
    }

    #[test]
    fn hex_encode_known_value() {
        // SHA-256 of empty string is well-known
        let checksum = compute_checksum("");
        assert_eq!(
            checksum,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
