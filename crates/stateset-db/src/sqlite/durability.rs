//! SQLite commit synchronization policy, applied to every pooled connection.

use stateset_core::CommerceError;

/// Environment override used by [`super::SqliteDatabase::new`], including bindings.
pub const SQLITE_SYNCHRONOUS_ENV: &str = "STATESET_SQLITE_SYNCHRONOUS";

/// WAL synchronization policy. Neither mode protects against storage that lies
/// about synchronization. `Full` syncs the WAL on each commit; `Normal` may lose
/// recent committed transactions after host power loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SqliteDurability {
    /// Existing default: consistent after process crashes, fewer syncs.
    #[default]
    Normal,
    /// Synchronize each commit for applications requiring stronger durability.
    Full,
}

impl SqliteDurability {
    /// Parse an operator setting, rejecting unknown or unsafe modes.
    pub fn from_setting(value: Option<&str>) -> Result<Self, CommerceError> {
        match value.map(str::trim) {
            None => Ok(Self::Normal),
            Some(value) if value.eq_ignore_ascii_case("NORMAL") => Ok(Self::Normal),
            Some(value) if value.eq_ignore_ascii_case("FULL") => Ok(Self::Full),
            Some(_) => Err(CommerceError::ValidationError(format!(
                "{SQLITE_SYNCHRONOUS_ENV} must be NORMAL or FULL"
            ))),
        }
    }

    pub(super) fn from_env() -> Result<Self, CommerceError> {
        match std::env::var(SQLITE_SYNCHRONOUS_ENV) {
            Ok(value) => Self::from_setting(Some(&value)),
            Err(std::env::VarError::NotPresent) => Self::from_setting(None),
            Err(std::env::VarError::NotUnicode(_)) => Err(CommerceError::ValidationError(format!(
                "{SQLITE_SYNCHRONOUS_ENV} must be NORMAL or FULL"
            ))),
        }
    }

    pub(super) const fn synchronous_pragma(self) -> &'static str {
        match self {
            Self::Normal => "PRAGMA synchronous = NORMAL;",
            Self::Full => "PRAGMA synchronous = FULL;",
        }
    }
}

/// Effective storage settings read from a pooled connection, for operator diagnostics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SqliteStorageStatus {
    /// Actual journal mode; the engine configures WAL.
    pub journal_mode: String,
    /// SQLite numeric synchronous mode: 1 = NORMAL, 2 = FULL.
    pub synchronous: u32,
    /// Lock wait timeout in milliseconds.
    pub busy_timeout_ms: u64,
    /// Automatic WAL checkpoint threshold in pages.
    pub wal_autocheckpoint_pages: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durability_settings_fail_closed() {
        assert_eq!(SqliteDurability::from_setting(None).unwrap(), SqliteDurability::Normal);
        assert_eq!(SqliteDurability::from_setting(Some(" full ")).unwrap(), SqliteDurability::Full);
        assert_eq!(
            SqliteDurability::from_setting(Some("normal")).unwrap(),
            SqliteDurability::Normal
        );
        for value in ["", "OFF", "0", "1", "EXTRA", "FULL; PRAGMA foreign_keys=OFF", "FUL"] {
            assert!(SqliteDurability::from_setting(Some(value)).is_err(), "{value}");
        }
    }
}
