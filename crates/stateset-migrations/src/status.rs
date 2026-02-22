//! Migration status reporting.

use crate::migration::MigrationRecord;
use crate::version::SchemaVersion;

/// Full status report of the migration state.
#[derive(Debug, Clone)]
pub struct MigrationStatus {
    /// The schema version summary.
    pub schema_version: SchemaVersion,
    /// All applied migrations.
    pub applied: Vec<MigrationRecord>,
    /// Names of pending migrations (not yet applied).
    pub pending: Vec<String>,
    /// Whether all applied migration checksums match the registry.
    pub checksum_valid: bool,
}

impl MigrationStatus {
    /// Returns `true` if the database is fully migrated and checksums are valid.
    #[must_use]
    pub const fn is_healthy(&self) -> bool {
        self.schema_version.is_up_to_date() && self.checksum_valid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn healthy_when_up_to_date_and_valid() {
        let status = MigrationStatus {
            schema_version: SchemaVersion { current: 2, latest: 2, pending: 0 },
            applied: vec![
                MigrationRecord {
                    version: 1,
                    name: "init".to_string(),
                    applied_at: Utc::now(),
                    checksum: "aaa".to_string(),
                    execution_time_ms: 10,
                },
                MigrationRecord {
                    version: 2,
                    name: "add_col".to_string(),
                    applied_at: Utc::now(),
                    checksum: "bbb".to_string(),
                    execution_time_ms: 5,
                },
            ],
            pending: vec![],
            checksum_valid: true,
        };
        assert!(status.is_healthy());
    }

    #[test]
    fn unhealthy_when_pending() {
        let status = MigrationStatus {
            schema_version: SchemaVersion { current: 1, latest: 2, pending: 1 },
            applied: vec![],
            pending: vec!["add_col".to_string()],
            checksum_valid: true,
        };
        assert!(!status.is_healthy());
    }

    #[test]
    fn unhealthy_when_checksum_invalid() {
        let status = MigrationStatus {
            schema_version: SchemaVersion { current: 2, latest: 2, pending: 0 },
            applied: vec![],
            pending: vec![],
            checksum_valid: false,
        };
        assert!(!status.is_healthy());
    }

    #[test]
    fn status_debug_impl() {
        let status = MigrationStatus {
            schema_version: SchemaVersion { current: 0, latest: 0, pending: 0 },
            applied: vec![],
            pending: vec![],
            checksum_valid: true,
        };
        let debug = format!("{status:?}");
        assert!(debug.contains("MigrationStatus"));
    }
}
