//! Permission levels for authorization decisions.
//!
//! Mirrors the JS `PERMISSION_LEVELS` object from `cli/src/permissions.js` but
//! provides compile-time safety and ordering.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Permission levels ordered from least to most privileged.
///
/// # Ordering
///
/// `None` < `Read` < `Preview` < `Write` < `Delete` < `Admin`
///
/// ```rust
/// use stateset_authz::PermissionLevel;
///
/// assert!(PermissionLevel::Admin > PermissionLevel::Write);
/// assert!(PermissionLevel::Read < PermissionLevel::Delete);
/// assert_eq!(PermissionLevel::None as u8, 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[non_exhaustive]
#[repr(u8)]
pub enum PermissionLevel {
    /// No operations allowed.
    None = 0,
    /// List, get, query operations.
    Read = 1,
    /// Read plus show what would happen (dry-run).
    Preview = 2,
    /// Create, update operations.
    Write = 3,
    /// Cancel, void, delete operations.
    Delete = 4,
    /// Bulk operations, settings, full access.
    Admin = 5,
}

impl PermissionLevel {
    /// Returns `true` if this level is at least as privileged as `required`.
    ///
    /// ```rust
    /// use stateset_authz::PermissionLevel;
    ///
    /// assert!(PermissionLevel::Admin.has_at_least(PermissionLevel::Write));
    /// assert!(PermissionLevel::Write.has_at_least(PermissionLevel::Write));
    /// assert!(!PermissionLevel::Read.has_at_least(PermissionLevel::Write));
    /// ```
    #[must_use]
    pub fn has_at_least(self, required: Self) -> bool {
        self >= required
    }

    /// Returns all variants in ascending order.
    ///
    /// ```rust
    /// use stateset_authz::PermissionLevel;
    ///
    /// let all = PermissionLevel::all();
    /// assert_eq!(all.len(), 6);
    /// assert_eq!(all[0], PermissionLevel::None);
    /// assert_eq!(all[5], PermissionLevel::Admin);
    /// ```
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::None, Self::Read, Self::Preview, Self::Write, Self::Delete, Self::Admin]
    }
}

impl fmt::Display for PermissionLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::None => "none",
            Self::Read => "read",
            Self::Preview => "preview",
            Self::Write => "write",
            Self::Delete => "delete",
            Self::Admin => "admin",
        };
        f.write_str(s)
    }
}

/// Error returned when parsing an invalid permission level string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsePermissionLevelError(String);

impl fmt::Display for ParsePermissionLevelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid permission level: '{}'", self.0)
    }
}

impl std::error::Error for ParsePermissionLevelError {}

impl FromStr for PermissionLevel {
    type Err = ParsePermissionLevelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "read" => Ok(Self::Read),
            "preview" => Ok(Self::Preview),
            "write" => Ok(Self::Write),
            "delete" => Ok(Self::Delete),
            "admin" => Ok(Self::Admin),
            _ => Err(ParsePermissionLevelError(s.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Ordering --

    #[test]
    fn ordering_none_is_lowest() {
        for &level in PermissionLevel::all().iter().skip(1) {
            assert!(PermissionLevel::None < level, "None should be less than {level}");
        }
    }

    #[test]
    fn ordering_admin_is_highest() {
        for &level in PermissionLevel::all().iter().rev().skip(1) {
            assert!(PermissionLevel::Admin > level, "Admin should be greater than {level}");
        }
    }

    #[test]
    fn ordering_is_total() {
        let all = PermissionLevel::all();
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert!(all[i] < all[j], "{} should be < {}", all[i], all[j]);
            }
        }
    }

    #[test]
    fn repr_values_are_sequential() {
        assert_eq!(PermissionLevel::None as u8, 0);
        assert_eq!(PermissionLevel::Read as u8, 1);
        assert_eq!(PermissionLevel::Preview as u8, 2);
        assert_eq!(PermissionLevel::Write as u8, 3);
        assert_eq!(PermissionLevel::Delete as u8, 4);
        assert_eq!(PermissionLevel::Admin as u8, 5);
    }

    // -- has_at_least --

    #[test]
    fn has_at_least_same_level() {
        for &level in PermissionLevel::all() {
            assert!(level.has_at_least(level));
        }
    }

    #[test]
    fn has_at_least_higher_passes() {
        assert!(PermissionLevel::Admin.has_at_least(PermissionLevel::None));
        assert!(PermissionLevel::Admin.has_at_least(PermissionLevel::Read));
        assert!(PermissionLevel::Write.has_at_least(PermissionLevel::Read));
        assert!(PermissionLevel::Delete.has_at_least(PermissionLevel::Write));
    }

    #[test]
    fn has_at_least_lower_fails() {
        assert!(!PermissionLevel::None.has_at_least(PermissionLevel::Read));
        assert!(!PermissionLevel::Read.has_at_least(PermissionLevel::Write));
        assert!(!PermissionLevel::Preview.has_at_least(PermissionLevel::Delete));
    }

    // -- Display --

    #[test]
    fn display_lowercase() {
        assert_eq!(PermissionLevel::None.to_string(), "none");
        assert_eq!(PermissionLevel::Read.to_string(), "read");
        assert_eq!(PermissionLevel::Preview.to_string(), "preview");
        assert_eq!(PermissionLevel::Write.to_string(), "write");
        assert_eq!(PermissionLevel::Delete.to_string(), "delete");
        assert_eq!(PermissionLevel::Admin.to_string(), "admin");
    }

    // -- FromStr --

    #[test]
    fn from_str_roundtrip() {
        for &level in PermissionLevel::all() {
            let s = level.to_string();
            let parsed: PermissionLevel = s.parse().unwrap();
            assert_eq!(parsed, level);
        }
    }

    #[test]
    fn from_str_case_insensitive() {
        assert_eq!("ADMIN".parse::<PermissionLevel>().unwrap(), PermissionLevel::Admin);
        assert_eq!("Read".parse::<PermissionLevel>().unwrap(), PermissionLevel::Read);
        assert_eq!("WRITE".parse::<PermissionLevel>().unwrap(), PermissionLevel::Write);
    }

    #[test]
    fn from_str_invalid() {
        let err = "superadmin".parse::<PermissionLevel>().unwrap_err();
        assert_eq!(err.to_string(), "invalid permission level: 'superadmin'");
    }

    // -- Serde --

    #[test]
    fn serde_roundtrip() {
        for &level in PermissionLevel::all() {
            let json = serde_json::to_string(&level).unwrap();
            let parsed: PermissionLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, level);
        }
    }

    #[test]
    fn serde_json_representation() {
        let json = serde_json::to_string(&PermissionLevel::Admin).unwrap();
        assert_eq!(json, "\"Admin\"");
    }

    // -- all() --

    #[test]
    fn all_returns_six_variants() {
        assert_eq!(PermissionLevel::all().len(), 6);
    }

    #[test]
    fn all_is_sorted() {
        let all = PermissionLevel::all();
        for window in all.windows(2) {
            assert!(window[0] < window[1]);
        }
    }
}
