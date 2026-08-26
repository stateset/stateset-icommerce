//! Monetary types with currency safety.
//!
//! The [`Money`] type pairs an amount with a [`CurrencyCode`], preventing
//! accidental arithmetic between different currencies at the type level.

use alloc::string::String;
use core::fmt;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

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
    #[must_use]
    pub const fn amount(&self) -> Decimal {
        self.amount
    }

    /// Get the currency code.
    #[inline]
    #[must_use]
    pub const fn currency(&self) -> CurrencyCode {
        self.currency
    }

    /// Returns `true` if the amount is zero.
    #[inline]
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.amount.is_zero()
    }

    /// Returns `true` if the amount is positive.
    #[inline]
    #[must_use]
    pub const fn is_positive(&self) -> bool {
        self.amount.is_sign_positive() && !self.amount.is_zero()
    }

    /// Returns `true` if the amount is negative.
    #[inline]
    #[must_use]
    pub const fn is_negative(&self) -> bool {
        self.amount.is_sign_negative() && !self.amount.is_zero()
    }

    /// Add two monetary values.
    ///
    /// Returns `None` if the currencies don't match or if the underlying
    /// [`Decimal`] addition overflows. This method never panics.
    #[inline]
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        if self.currency != other.currency {
            return None;
        }
        let amount = self.amount.checked_add(other.amount)?;
        Some(Self { amount, currency: self.currency })
    }

    /// Subtract two monetary values.
    ///
    /// Returns `None` if the currencies don't match or if the underlying
    /// [`Decimal`] subtraction overflows. This method never panics.
    #[inline]
    #[must_use]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        if self.currency != other.currency {
            return None;
        }
        let amount = self.amount.checked_sub(other.amount)?;
        Some(Self { amount, currency: self.currency })
    }

    /// Round to a given number of decimal places.
    #[inline]
    #[must_use = "returns a new Money with rounded amount"]
    pub fn round_dp(self, dp: u32) -> Self {
        Self { amount: self.amount.round_dp(dp), currency: self.currency }
    }

    /// Multiply by a scalar factor (e.g., quantity or tax rate). Currency is preserved.
    ///
    /// Returns `None` if the underlying [`Decimal`] multiplication overflows.
    /// This method never panics.
    #[inline]
    #[must_use = "returns a new Money with scaled amount"]
    pub fn checked_mul_scalar(self, factor: Decimal) -> Option<Self> {
        let amount = self.amount.checked_mul(factor)?;
        Some(Self { amount, currency: self.currency })
    }

    /// Divide by a scalar.
    ///
    /// Returns `None` if the divisor is zero or if the underlying [`Decimal`]
    /// division overflows. This method never panics.
    #[must_use]
    pub fn checked_div_scalar(self, divisor: Decimal) -> Option<Self> {
        if divisor.is_zero() {
            return None;
        }
        let amount = self.amount.checked_div(divisor)?;
        Some(Self { amount, currency: self.currency })
    }

    /// Return the absolute value of this monetary amount.
    #[must_use = "returns a new Money with absolute amount"]
    pub fn abs(self) -> Self {
        Self { amount: self.amount.abs(), currency: self.currency }
    }

    /// Negate this monetary amount.
    #[must_use = "returns a new Money with negated amount"]
    pub fn negate(self) -> Self {
        Self { amount: -self.amount, currency: self.currency }
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

impl Default for CurrencyCode {
    /// Defaults to USD — the most common commerce currency.
    fn default() -> Self {
        Self::USD
    }
}

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

    /// Minor-unit scale for this currency — the number of decimal places a
    /// stored amount may carry.
    ///
    /// Zero-decimal currencies (JPY, KRW, VND) return `0`; the crypto codes
    /// BTC/ETH return `8`; every other code returns `2`. This is the single
    /// source of truth for invariant `commerce.money.scale_exceeds_currency`
    /// and mirrors `stateset_core::models::Currency::decimal_places`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use stateset_primitives::CurrencyCode;
    ///
    /// assert_eq!(CurrencyCode::USD.decimal_places(), 2);
    /// assert_eq!(CurrencyCode::JPY.decimal_places(), 0);
    /// ```
    #[inline]
    #[must_use]
    pub const fn decimal_places(&self) -> u8 {
        match &self.0 {
            b"JPY" | b"KRW" | b"VND" => 0,
            b"BTC" | b"ETH" => 8,
            _ => 2,
        }
    }

    /// Get the currency code as a string slice.
    #[inline]
    #[must_use]
    pub const fn as_str(&self) -> &str {
        match core::str::from_utf8(&self.0) {
            Ok(code) => code,
            Err(_) => panic!("CurrencyCode always stores validated ASCII uppercase bytes"),
        }
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

impl core::str::FromStr for CurrencyCode {
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
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CurrencyCodeError {
    /// Currency code must be exactly 3 characters.
    InvalidLength(usize),
    /// Currency code must contain only ASCII letters.
    InvalidCharacters,
}

impl fmt::Display for CurrencyCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(len) => {
                write!(f, "currency code must be exactly 3 characters, got {len}")
            }
            Self::InvalidCharacters => f.write_str("currency code must contain only ASCII letters"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CurrencyCodeError {}

// ---------------------------------------------------------------------------
// rusqlite integration
// ---------------------------------------------------------------------------

#[cfg(feature = "rusqlite")]
impl rusqlite::types::FromSql for CurrencyCode {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let s = value.as_str()?;
        s.parse::<Self>().map_err(|e| rusqlite::types::FromSqlError::Other(Box::new(e)))
    }
}

#[cfg(feature = "rusqlite")]
impl rusqlite::types::ToSql for CurrencyCode {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Borrowed(rusqlite::types::ValueRef::Text(
            self.as_str().as_bytes(),
        )))
    }
}

// ---------------------------------------------------------------------------
// sqlx-postgres integration
// ---------------------------------------------------------------------------

#[cfg(feature = "sqlx-postgres")]
impl sqlx::Type<sqlx::Postgres> for CurrencyCode {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <&str as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <&str as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "sqlx-postgres")]
impl<'q> sqlx::Encode<'q, sqlx::Postgres> for CurrencyCode {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        <&str as sqlx::Encode<'q, sqlx::Postgres>>::encode_by_ref(&self.as_str(), buf)
    }
}

#[cfg(feature = "sqlx-postgres")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for CurrencyCode {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let s = <&str as sqlx::Decode<'r, sqlx::Postgres>>::decode(value)?;
        s.parse::<Self>().map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rust_decimal_macros::dec;

    fn arb_currency() -> impl Strategy<Value = CurrencyCode> {
        prop_oneof![
            Just(CurrencyCode::USD),
            Just(CurrencyCode::EUR),
            Just(CurrencyCode::GBP),
            Just(CurrencyCode::JPY),
            Just(CurrencyCode::CAD),
            Just(CurrencyCode::AUD),
            Just(CurrencyCode::CHF),
            Just(CurrencyCode::CNY),
        ]
    }

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

    proptest! {
        #[test]
        fn checked_add_sub_are_inverses_for_same_currency(
            a_raw in -1_000_000i64..1_000_000,
            b_raw in -1_000_000i64..1_000_000,
            currency in arb_currency(),
        ) {
            let a = Money::new(Decimal::new(a_raw, 2), currency);
            let b = Money::new(Decimal::new(b_raw, 2), currency);
            let sum = a.checked_add(b).unwrap();
            let back = sum.checked_sub(b).unwrap();
            prop_assert_eq!(back, a);
        }
    }

    proptest! {
        #[test]
        fn checked_add_is_commutative_when_currency_matches(
            a_raw in -1_000_000i64..1_000_000,
            b_raw in -1_000_000i64..1_000_000,
            currency in arb_currency(),
        ) {
            let a = Money::new(Decimal::new(a_raw, 3), currency);
            let b = Money::new(Decimal::new(b_raw, 3), currency);
            prop_assert_eq!(a.checked_add(b), b.checked_add(a));
        }
    }

    proptest! {
        #[test]
        fn round_dp_is_idempotent(
            raw in -100_000_000i64..100_000_000,
            scale in 0u32..8,
            dp in 0u32..8,
            currency in arb_currency(),
        ) {
            let money = Money::new(Decimal::new(raw, scale), currency);
            let once = money.round_dp(dp);
            let twice = once.round_dp(dp);
            prop_assert_eq!(once, twice);
            prop_assert!(once.amount().scale() <= dp);
        }
    }

    proptest! {
        #[test]
        fn checked_add_rejects_currency_mismatch(
            a_raw in -1_000_000i64..1_000_000,
            b_raw in -1_000_000i64..1_000_000,
        ) {
            let usd = Money::new(Decimal::new(a_raw, 2), CurrencyCode::USD);
            let eur = Money::new(Decimal::new(b_raw, 2), CurrencyCode::EUR);
            prop_assert!(usd.checked_add(eur).is_none());
            prop_assert!(usd.checked_sub(eur).is_none());
        }
    }

    #[test]
    fn is_negative_returns_false_for_zero() {
        let zero = Money::zero(CurrencyCode::USD);
        assert!(!zero.is_negative());
        assert!(!zero.is_positive());
    }

    #[test]
    fn is_negative_returns_true_for_negative() {
        let money = Money::new(dec!(-5.00), CurrencyCode::USD);
        assert!(money.is_negative());
        assert!(!money.is_positive());
    }

    #[test]
    fn is_positive_returns_true_for_positive() {
        let money = Money::new(dec!(5.00), CurrencyCode::USD);
        assert!(money.is_positive());
        assert!(!money.is_negative());
    }

    #[test]
    fn checked_mul_scalar() {
        let money = Money::new(dec!(10.00), CurrencyCode::USD);
        let result = money.checked_mul_scalar(dec!(3)).unwrap();
        assert_eq!(result.amount(), dec!(30.00));
        assert_eq!(result.currency(), CurrencyCode::USD);
    }

    #[test]
    fn checked_mul_scalar_fractional() {
        let money = Money::new(dec!(100.00), CurrencyCode::USD);
        let result = money.checked_mul_scalar(dec!(0.0825)).unwrap(); // 8.25% tax
        assert_eq!(result.amount(), dec!(8.2500));
    }

    #[test]
    fn checked_div_scalar() {
        let money = Money::new(dec!(30.00), CurrencyCode::USD);
        let result = money.checked_div_scalar(dec!(3)).unwrap();
        assert_eq!(result.amount(), dec!(10.00));
        assert_eq!(result.currency(), CurrencyCode::USD);
    }

    #[test]
    fn checked_div_scalar_zero_returns_none() {
        let money = Money::new(dec!(30.00), CurrencyCode::USD);
        assert!(money.checked_div_scalar(dec!(0)).is_none());
    }

    #[test]
    fn abs_negative_becomes_positive() {
        let money = Money::new(dec!(-5.00), CurrencyCode::USD);
        let result = money.abs();
        assert_eq!(result.amount(), dec!(5.00));
    }

    #[test]
    fn abs_positive_stays_positive() {
        let money = Money::new(dec!(5.00), CurrencyCode::USD);
        let result = money.abs();
        assert_eq!(result.amount(), dec!(5.00));
    }

    #[test]
    fn negate_round_trip() {
        let money = Money::new(dec!(5.00), CurrencyCode::USD);
        let negated = money.negate();
        assert_eq!(negated.amount(), dec!(-5.00));
        let restored = negated.negate();
        assert_eq!(restored.amount(), dec!(5.00));
    }

    // -----------------------------------------------------------------------
    // Arithmetic-overflow guards: `checked_*` must return `None`, never panic.
    // -----------------------------------------------------------------------

    #[test]
    fn checked_add_returns_none_on_overflow() {
        let max = Money::new(Decimal::MAX, CurrencyCode::USD);
        let one = Money::new(Decimal::ONE, CurrencyCode::USD);
        // Decimal::MAX + 1 overflows the Decimal representation.
        assert!(max.checked_add(one).is_none());
        assert!(max.checked_add(max).is_none());
    }

    #[test]
    fn checked_sub_returns_none_on_overflow() {
        let min = Money::new(Decimal::MIN, CurrencyCode::USD);
        let one = Money::new(Decimal::ONE, CurrencyCode::USD);
        // Decimal::MIN - 1 overflows the Decimal representation.
        assert!(min.checked_sub(one).is_none());
        let max = Money::new(Decimal::MAX, CurrencyCode::USD);
        // Decimal::MIN - Decimal::MAX overflows.
        assert!(min.checked_sub(max).is_none());
    }

    #[test]
    fn checked_mul_scalar_returns_none_on_overflow() {
        let max = Money::new(Decimal::MAX, CurrencyCode::USD);
        // Decimal::MAX * 2 overflows the Decimal representation.
        assert!(max.checked_mul_scalar(dec!(2)).is_none());
        assert!(max.checked_mul_scalar(Decimal::MAX).is_none());
    }

    #[test]
    fn checked_div_scalar_returns_none_on_overflow() {
        let max = Money::new(Decimal::MAX, CurrencyCode::USD);
        // Dividing the largest magnitude by a tiny fraction overflows.
        assert!(max.checked_div_scalar(dec!(0.0000000000000000000000000001)).is_none());
    }

    proptest! {
        /// `checked_*` arithmetic is total: it must never panic, even at the
        /// extremes of the `Decimal` range. It either yields a value or `None`.
        #[test]
        fn checked_arithmetic_never_panics(
            a_raw in any::<i64>(),
            a_scale in 0u32..6,
            b_raw in any::<i64>(),
            b_scale in 0u32..6,
            currency in arb_currency(),
        ) {
            let a = Money::new(Decimal::new(a_raw, a_scale), currency);
            let b = Money::new(Decimal::new(b_raw, b_scale), currency);
            // None of these may panic regardless of inputs.
            let _ = a.checked_add(b);
            let _ = a.checked_sub(b);
            let _ = a.checked_mul_scalar(b.amount());
            let _ = a.checked_div_scalar(b.amount());
        }
    }

    proptest! {
        /// Even when operands are pinned to the `Decimal` extremes, the
        /// `checked_*` methods stay total (return `None`, never panic).
        #[test]
        fn checked_arithmetic_at_extremes_never_panics(
            factor_raw in any::<i64>(),
            factor_scale in 0u32..28,
            currency in arb_currency(),
        ) {
            let max = Money::new(Decimal::MAX, currency);
            let min = Money::new(Decimal::MIN, currency);
            let factor = Decimal::new(factor_raw, factor_scale);
            let _ = max.checked_add(max);
            let _ = max.checked_add(min);
            let _ = min.checked_sub(max);
            let _ = max.checked_mul_scalar(factor);
            let _ = min.checked_mul_scalar(factor);
            let _ = max.checked_div_scalar(factor);
            let _ = min.checked_div_scalar(factor);
        }
    }
}
