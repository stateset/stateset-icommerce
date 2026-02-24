//! Monetary types with currency safety.
//!
//! The [`Money`] type pairs an amount with a [`CurrencyCode`], preventing
//! accidental arithmetic between different currencies at the type level.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A monetary amount paired with its currency.
///
/// This type ensures that amounts always carry their currency context,
/// preventing accidental mixing of currencies in arithmetic operations.
///
/// # Example
///
/// ```rust
/// use stateset_primitives::{Money, CurrencyCode};
/// use rust_decimal_macros::dec;
///
/// let price = Money::new(dec!(29.99), CurrencyCode::USD);
/// assert_eq!(price.amount(), dec!(29.99));
/// assert_eq!(price.currency(), CurrencyCode::USD);
/// assert_eq!(format!("{}", price), "29.99 USD");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[must_use]
pub struct Money {
    amount: Decimal,
    currency: CurrencyCode,
}

impl Money {
    /// Create a new monetary value.
    #[inline]
    pub const fn new(amount: Decimal, currency: CurrencyCode) -> Self {
        Self { amount, currency }
    }

    /// Create a zero amount in the given currency.
    #[inline]
    pub const fn zero(currency: CurrencyCode) -> Self {
        Self { amount: Decimal::ZERO, currency }
    }

    /// Get the amount.
    #[inline]
    pub const fn amount(&self) -> Decimal {
        self.amount
    }

    /// Get the currency code.
    #[inline]
    pub const fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// Returns `true` if the amount is zero.
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    /// Returns `true` if the amount is positive.
    #[inline]
    pub const fn is_positive(&self) -> bool {
        self.amount.is_sign_positive() && !self.amount.is_zero()
    }

    /// Returns `true` if the amount is negative.
    #[inline]
    pub const fn is_negative(&self) -> bool {
        self.amount.is_sign_negative()
    }

    /// Add two monetary values. Returns `None` if currencies don't match.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        if self.currency != other.currency {
            return None;
        }
        Some(Self { amount: self.amount + other.amount, currency: self.currency })
    }

    /// Subtract two monetary values. Returns `None` if currencies don't match.
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        if self.currency != other.currency {
            return None;
        }
        Some(Self { amount: self.amount - other.amount, currency: self.currency })
    }

    /// Round to a given number of decimal places.
    #[inline]
    #[must_use = "returns a new Money with rounded amount"]
    pub fn round_dp(self, dp: u32) -> Self {
        Self { amount: self.amount.round_dp(dp), currency: self.currency }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

// ---------------------------------------------------------------------------
// CurrencyCode
// ---------------------------------------------------------------------------

/// ISO 4217 three-letter currency code.
///
/// Stored as 3 ASCII uppercase bytes for zero-allocation comparisons and copies.
///
/// # Example
///
/// ```rust
/// use stateset_primitives::CurrencyCode;
///
/// let usd = CurrencyCode::USD;
/// assert_eq!(usd.as_str(), "USD");
///
/// let parsed: CurrencyCode = "EUR".parse().unwrap();
/// assert_eq!(parsed, CurrencyCode::EUR);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurrencyCode([u8; 3]);

impl CurrencyCode {
    // Common currency code constants
    /// United States Dollar
    pub const USD: Self = Self(*b"USD");
    /// Euro
    pub const EUR: Self = Self(*b"EUR");
    /// British Pound Sterling
    pub const GBP: Self = Self(*b"GBP");
    /// Japanese Yen
    pub const JPY: Self = Self(*b"JPY");
    /// Canadian Dollar
    pub const CAD: Self = Self(*b"CAD");
    /// Australian Dollar
    pub const AUD: Self = Self(*b"AUD");
    /// Swiss Franc
    pub const CHF: Self = Self(*b"CHF");
    /// Chinese Yuan
    pub const CNY: Self = Self(*b"CNY");

    /// Create a currency code from 3 ASCII uppercase bytes.
    ///
    /// Returns `None` if any byte is not ASCII uppercase.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 3]) -> Option<Self> {
        if bytes[0].is_ascii_uppercase()
            && bytes[1].is_ascii_uppercase()
            && bytes[2].is_ascii_uppercase()
        {
            Some(Self(bytes))
        } else {
            None
        }
    }

    /// Get the currency code as a string slice.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        // SAFETY: We validate ASCII uppercase in all constructors.
        std::str::from_utf8(&self.0).expect("CurrencyCode is always valid ASCII")
    }
}

impl fmt::Debug for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CurrencyCode({})", self.as_str())
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for CurrencyCode {
    type Err = CurrencyCodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        if bytes.len() != 3 {
            return Err(CurrencyCodeError::InvalidLength(s.len()));
        }
        let arr = [bytes[0], bytes[1], bytes[2]];
        // Uppercase before validating
        let arr =
            [arr[0].to_ascii_uppercase(), arr[1].to_ascii_uppercase(), arr[2].to_ascii_uppercase()];
        Self::from_bytes(arr).ok_or(CurrencyCodeError::InvalidCharacters)
    }
}

impl Serialize for CurrencyCode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CurrencyCode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Error parsing a [`CurrencyCode`].
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum CurrencyCodeError {
    /// Currency code must be exactly 3 characters.
    #[error("currency code must be exactly 3 characters, got {0}")]
    InvalidLength(usize),
    /// Currency code must contain only ASCII letters.
    #[error("currency code must contain only ASCII letters")]
    InvalidCharacters,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn money_display() {
        let m = Money::new(dec!(42.50), CurrencyCode::USD);
        assert_eq!(m.to_string(), "42.50 USD");
    }

    #[test]
    fn money_checked_add_same_currency() {
        let a = Money::new(dec!(10.00), CurrencyCode::USD);
        let b = Money::new(dec!(5.50), CurrencyCode::USD);
        let sum = a.checked_add(b).unwrap();
        assert_eq!(sum.amount(), dec!(15.50));
    }

    #[test]
    fn money_checked_add_different_currency() {
        let a = Money::new(dec!(10.00), CurrencyCode::USD);
        let b = Money::new(dec!(5.50), CurrencyCode::EUR);
        assert!(a.checked_add(b).is_none());
    }

    #[test]
    fn currency_code_parse() {
        let usd: CurrencyCode = "USD".parse().unwrap();
        assert_eq!(usd, CurrencyCode::USD);

        let lower: CurrencyCode = "eur".parse().unwrap();
        assert_eq!(lower, CurrencyCode::EUR);
    }

    #[test]
    fn currency_code_invalid() {
        assert!("US".parse::<CurrencyCode>().is_err()); // too short
        assert!("USDX".parse::<CurrencyCode>().is_err()); // too long
        assert!("U$D".parse::<CurrencyCode>().is_err()); // non-alpha
    }

    #[test]
    fn currency_code_serde_roundtrip() {
        let code = CurrencyCode::GBP;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"GBP\"");
        let parsed: CurrencyCode = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, code);
    }

    #[test]
    fn money_zero() {
        let z = Money::zero(CurrencyCode::JPY);
        assert!(z.is_zero());
        assert!(!z.is_positive());
        assert!(!z.is_negative());
    }
}
