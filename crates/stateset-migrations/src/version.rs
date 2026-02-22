//! Schema version tracking.

use std::fmt;

/// Represents the current schema version state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaVersion {
    /// The currently applied version (0 if no migrations applied).
    pub current: u32,
    /// The latest available version in the registry.
    pub latest: u32,
    /// Number of pending migrations.
    pub pending: u32,
}

impl SchemaVersion {
    /// Returns `true` if all migrations have been applied.
    #[must_use]
    pub const fn is_up_to_date(&self) -> bool {
        self.pending == 0
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_up_to_date() {
            write!(f, "v{} (up to date)", self.current)
        } else {
            write!(f, "v{} ({} pending)", self.current, self.pending)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_up_to_date_when_no_pending() {
        let v = SchemaVersion { current: 4, latest: 4, pending: 0 };
        assert!(v.is_up_to_date());
    }

    #[test]
    fn not_up_to_date_when_pending() {
        let v = SchemaVersion { current: 2, latest: 4, pending: 2 };
        assert!(!v.is_up_to_date());
    }

    #[test]
    fn display_up_to_date() {
        let v = SchemaVersion { current: 4, latest: 4, pending: 0 };
        assert_eq!(v.to_string(), "v4 (up to date)");
    }

    #[test]
    fn display_pending() {
        let v = SchemaVersion { current: 3, latest: 4, pending: 1 };
        assert_eq!(v.to_string(), "v3 (1 pending)");
    }

    #[test]
    fn display_multiple_pending() {
        let v = SchemaVersion { current: 1, latest: 5, pending: 4 };
        assert_eq!(v.to_string(), "v1 (4 pending)");
    }

    #[test]
    fn display_zero_current() {
        let v = SchemaVersion { current: 0, latest: 3, pending: 3 };
        assert_eq!(v.to_string(), "v0 (3 pending)");
    }

    #[test]
    fn fresh_database_version() {
        let v = SchemaVersion { current: 0, latest: 4, pending: 4 };
        assert!(!v.is_up_to_date());
        assert_eq!(v.current, 0);
        assert_eq!(v.latest, 4);
    }
}
