//! Strongly-typed SKU (Stock Keeping Unit) identifier.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A validated product SKU (Stock Keeping Unit).
///
/// SKUs are non-empty strings used to uniquely identify products in inventory.
/// This newtype ensures that empty strings are never used as SKUs.
///
/// # Example
///
/// ```rust
/// use stateset_primitives::Sku;
///
/// let sku = Sku::new("SKU-001").unwrap();
/// assert_eq!(sku.as_str(), "SKU-001");
///
/// // Empty SKUs are rejected
/// assert!(Sku::new("").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sku(String);

impl Sku {
    /// Create a new SKU from a string.
    ///
    /// Returns an error if the string is empty or contains only whitespace.
    pub fn new(s: impl Into<String>) -> Result<Self, SkuError> {
        let s = s.into();
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(SkuError::Empty);
        }
        if trimmed.len() > 128 {
            return Err(SkuError::TooLong(trimmed.len()));
        }
        Ok(Self(trimmed.to_string()))
    }

    /// Get the SKU as a string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume and return the inner string.
    #[inline]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for Sku {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Sku {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Sku {
    type Error = SkuError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<Sku> for String {
    fn from(sku: Sku) -> Self {
        sku.0
    }
}

/// Error creating a [`Sku`].
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum SkuError {
    /// SKU string was empty or whitespace-only.
    #[error("SKU cannot be empty")]
    Empty,
    /// SKU string exceeded the maximum length.
    #[error("SKU too long ({0} chars, max 128)")]
    TooLong(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_sku() {
        let sku = Sku::new("SKU-001").unwrap();
        assert_eq!(sku.as_str(), "SKU-001");
        assert_eq!(sku.to_string(), "SKU-001");
    }

    #[test]
    fn empty_sku_rejected() {
        assert!(Sku::new("").is_err());
        assert!(Sku::new("   ").is_err());
    }

    #[test]
    fn long_sku_rejected() {
        let long = "X".repeat(129);
        assert!(Sku::new(long).is_err());
    }

    #[test]
    fn sku_trims_whitespace() {
        let sku = Sku::new("  SKU-001  ").unwrap();
        assert_eq!(sku.as_str(), "SKU-001");
    }

    #[test]
    fn serde_roundtrip() {
        let sku = Sku::new("WIDGET-42").unwrap();
        let json = serde_json::to_string(&sku).unwrap();
        assert_eq!(json, "\"WIDGET-42\"");
        let parsed: Sku = serde_json::from_str(&json).unwrap();
        assert_eq!(sku, parsed);
    }

    #[test]
    fn serde_rejects_empty() {
        let result = serde_json::from_str::<Sku>("\"\"");
        assert!(result.is_err());
    }
}
