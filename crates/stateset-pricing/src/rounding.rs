//! Rounding policies for monetary calculations.
//!
//! Different currencies have different numbers of minor units (e.g. 2 for USD,
//! 0 for JPY, 3 for BHD). This module provides configurable rounding with
//! multiple strategies.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Strategy for rounding monetary amounts.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::RoundingMode;
///
/// let mode = RoundingMode::HalfUp;
/// assert_eq!(format!("{mode:?}"), "HalfUp");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RoundingMode {
    /// Round half up — 0.5 rounds to 1 (standard commercial rounding).
    HalfUp,
    /// Banker's rounding — 0.5 rounds to nearest even.
    HalfEven,
    /// Truncate toward zero.
    Down,
    /// Always round away from zero (ceiling of absolute value).
    Up,
}

impl Default for RoundingMode {
    fn default() -> Self {
        Self::HalfUp
    }
}

/// A rounding policy combining a [`RoundingMode`] with the number of minor
/// units (decimal places) for a currency.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{RoundingPolicy, RoundingMode, round};
/// use rust_decimal_macros::dec;
///
/// let policy = RoundingPolicy::usd();
/// assert_eq!(round(dec!(1.235), &policy), dec!(1.24));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoundingPolicy {
    /// The rounding strategy.
    pub mode: RoundingMode,
    /// Number of decimal places (e.g. 2 for USD, 0 for JPY, 3 for BHD).
    pub minor_units: u32,
}

impl RoundingPolicy {
    /// Create a new rounding policy.
    #[must_use]
    pub const fn new(mode: RoundingMode, minor_units: u32) -> Self {
        Self { mode, minor_units }
    }

    /// US Dollar (2 decimal places, half-up).
    #[must_use]
    pub const fn usd() -> Self {
        Self { mode: RoundingMode::HalfUp, minor_units: 2 }
    }

    /// Euro (2 decimal places, half-up).
    #[must_use]
    pub const fn eur() -> Self {
        Self { mode: RoundingMode::HalfUp, minor_units: 2 }
    }

    /// Japanese Yen (0 decimal places, half-up).
    #[must_use]
    pub const fn jpy() -> Self {
        Self { mode: RoundingMode::HalfUp, minor_units: 0 }
    }

    /// Bahraini Dinar (3 decimal places, half-up).
    #[must_use]
    pub const fn bhd() -> Self {
        Self { mode: RoundingMode::HalfUp, minor_units: 3 }
    }

    /// British Pound Sterling (2 decimal places, half-up).
    #[must_use]
    pub const fn gbp() -> Self {
        Self { mode: RoundingMode::HalfUp, minor_units: 2 }
    }
}

impl Default for RoundingPolicy {
    fn default() -> Self {
        Self::usd()
    }
}

/// Look up the number of minor units for a given ISO 4217 currency code.
///
/// Returns 2 for most currencies, 0 for JPY/KRW/VND, 3 for BHD/KWD/OMR.
///
/// # Example
///
/// ```rust
/// use stateset_pricing::minor_units_for_currency;
///
/// assert_eq!(minor_units_for_currency("USD"), 2);
/// assert_eq!(minor_units_for_currency("JPY"), 0);
/// assert_eq!(minor_units_for_currency("BHD"), 3);
/// ```
#[must_use]
pub fn minor_units_for_currency(code: &str) -> u32 {
    match code.to_ascii_uppercase().as_str() {
        // Zero-decimal currencies
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF"
        | "UGX" | "UYI" | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        // Three-decimal currencies
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        // Everything else is 2
        _ => 2,
    }
}

/// Round a [`Decimal`] according to the given [`RoundingPolicy`].
///
/// # Example
///
/// ```rust
/// use stateset_pricing::{round, RoundingPolicy, RoundingMode};
/// use rust_decimal_macros::dec;
///
/// // Half-up: 2.345 -> 2.35
/// let policy = RoundingPolicy::new(RoundingMode::HalfUp, 2);
/// assert_eq!(round(dec!(2.345), &policy), dec!(2.35));
///
/// // Banker's (half-even): 2.345 -> 2.34  (4 is even)
/// let policy = RoundingPolicy::new(RoundingMode::HalfEven, 2);
/// assert_eq!(round(dec!(2.345), &policy), dec!(2.34));
///
/// // Down (truncate): 2.349 -> 2.34
/// let policy = RoundingPolicy::new(RoundingMode::Down, 2);
/// assert_eq!(round(dec!(2.349), &policy), dec!(2.34));
///
/// // Up (ceiling): 2.341 -> 2.35
/// let policy = RoundingPolicy::new(RoundingMode::Up, 2);
/// assert_eq!(round(dec!(2.341), &policy), dec!(2.35));
/// ```
#[must_use]
pub fn round(amount: Decimal, policy: &RoundingPolicy) -> Decimal {
    let dp = policy.minor_units;
    match policy.mode {
        RoundingMode::HalfUp => round_half_up(amount, dp),
        RoundingMode::HalfEven => round_half_even(amount, dp),
        RoundingMode::Down => round_down(amount, dp),
        RoundingMode::Up => round_up(amount, dp),
    }
}

/// Round half-up: 0.5 rounds away from zero.
fn round_half_up(amount: Decimal, dp: u32) -> Decimal {
    // rust_decimal's round_dp uses MidpointAwayFromZero which is half-up
    amount.round_dp_with_strategy(dp, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
}

/// Banker's rounding: 0.5 rounds to nearest even.
fn round_half_even(amount: Decimal, dp: u32) -> Decimal {
    amount.round_dp_with_strategy(dp, rust_decimal::RoundingStrategy::MidpointNearestEven)
}

/// Truncate toward zero.
fn round_down(amount: Decimal, dp: u32) -> Decimal {
    amount.round_dp_with_strategy(dp, rust_decimal::RoundingStrategy::ToZero)
}

/// Round away from zero (ceiling of absolute value).
fn round_up(amount: Decimal, dp: u32) -> Decimal {
    let truncated = round_down(amount, dp);
    if truncated == amount {
        amount
    } else if amount.is_sign_negative() {
        truncated - scale_unit(dp)
    } else {
        truncated + scale_unit(dp)
    }
}

/// Get the smallest unit for a given number of decimal places.
/// e.g. dp=2 -> 0.01, dp=0 -> 1, dp=3 -> 0.001
fn scale_unit(dp: u32) -> Decimal {
    Decimal::new(1, dp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ---- HalfUp ----

    #[test]
    fn half_up_rounds_5_up() {
        let p = RoundingPolicy::new(RoundingMode::HalfUp, 2);
        assert_eq!(round(dec!(2.345), &p), dec!(2.35));
    }

    #[test]
    fn half_up_rounds_4_down() {
        let p = RoundingPolicy::new(RoundingMode::HalfUp, 2);
        assert_eq!(round(dec!(2.344), &p), dec!(2.34));
    }

    #[test]
    fn half_up_negative() {
        let p = RoundingPolicy::new(RoundingMode::HalfUp, 2);
        assert_eq!(round(dec!(-2.345), &p), dec!(-2.35));
    }

    #[test]
    fn half_up_exact() {
        let p = RoundingPolicy::new(RoundingMode::HalfUp, 2);
        assert_eq!(round(dec!(2.34), &p), dec!(2.34));
    }

    #[test]
    fn half_up_jpy() {
        let p = RoundingPolicy::jpy();
        assert_eq!(round(dec!(100.5), &p), dec!(101));
        assert_eq!(round(dec!(100.4), &p), dec!(100));
    }

    #[test]
    fn half_up_bhd() {
        let p = RoundingPolicy::bhd();
        assert_eq!(round(dec!(1.2345), &p), dec!(1.235));
        assert_eq!(round(dec!(1.2344), &p), dec!(1.234));
    }

    // ---- HalfEven (Banker's) ----

    #[test]
    fn half_even_rounds_to_even_up() {
        let p = RoundingPolicy::new(RoundingMode::HalfEven, 2);
        // 2.355 -> 2.36 (6 is even)
        assert_eq!(round(dec!(2.355), &p), dec!(2.36));
    }

    #[test]
    fn half_even_rounds_to_even_down() {
        let p = RoundingPolicy::new(RoundingMode::HalfEven, 2);
        // 2.345 -> 2.34 (4 is even)
        assert_eq!(round(dec!(2.345), &p), dec!(2.34));
    }

    #[test]
    fn half_even_not_midpoint() {
        let p = RoundingPolicy::new(RoundingMode::HalfEven, 2);
        assert_eq!(round(dec!(2.346), &p), dec!(2.35));
    }

    // ---- Down (Truncate) ----

    #[test]
    fn down_truncates_positive() {
        let p = RoundingPolicy::new(RoundingMode::Down, 2);
        assert_eq!(round(dec!(2.349), &p), dec!(2.34));
    }

    #[test]
    fn down_truncates_negative() {
        let p = RoundingPolicy::new(RoundingMode::Down, 2);
        assert_eq!(round(dec!(-2.349), &p), dec!(-2.34));
    }

    #[test]
    fn down_exact() {
        let p = RoundingPolicy::new(RoundingMode::Down, 2);
        assert_eq!(round(dec!(2.34), &p), dec!(2.34));
    }

    // ---- Up (Ceiling) ----

    #[test]
    fn up_rounds_up_positive() {
        let p = RoundingPolicy::new(RoundingMode::Up, 2);
        assert_eq!(round(dec!(2.341), &p), dec!(2.35));
    }

    #[test]
    fn up_rounds_up_negative() {
        let p = RoundingPolicy::new(RoundingMode::Up, 2);
        assert_eq!(round(dec!(-2.341), &p), dec!(-2.35));
    }

    #[test]
    fn up_exact_no_change() {
        let p = RoundingPolicy::new(RoundingMode::Up, 2);
        assert_eq!(round(dec!(2.34), &p), dec!(2.34));
    }

    #[test]
    fn up_jpy() {
        let p = RoundingPolicy::new(RoundingMode::Up, 0);
        assert_eq!(round(dec!(100.1), &p), dec!(101));
    }

    // ---- minor_units_for_currency ----

    #[test]
    fn minor_units_standard() {
        assert_eq!(minor_units_for_currency("USD"), 2);
        assert_eq!(minor_units_for_currency("EUR"), 2);
        assert_eq!(minor_units_for_currency("GBP"), 2);
    }

    #[test]
    fn minor_units_zero_decimal() {
        assert_eq!(minor_units_for_currency("JPY"), 0);
        assert_eq!(minor_units_for_currency("KRW"), 0);
        assert_eq!(minor_units_for_currency("VND"), 0);
    }

    #[test]
    fn minor_units_three_decimal() {
        assert_eq!(minor_units_for_currency("BHD"), 3);
        assert_eq!(minor_units_for_currency("KWD"), 3);
        assert_eq!(minor_units_for_currency("OMR"), 3);
    }

    #[test]
    fn minor_units_case_insensitive() {
        assert_eq!(minor_units_for_currency("jpy"), 0);
        assert_eq!(minor_units_for_currency("Bhd"), 3);
    }

    #[test]
    fn minor_units_unknown_defaults_to_two() {
        assert_eq!(minor_units_for_currency("XYZ"), 2);
    }

    // ---- Policy constructors ----

    #[test]
    fn usd_policy() {
        let p = RoundingPolicy::usd();
        assert_eq!(p.minor_units, 2);
        assert_eq!(p.mode, RoundingMode::HalfUp);
    }

    #[test]
    fn jpy_policy() {
        let p = RoundingPolicy::jpy();
        assert_eq!(p.minor_units, 0);
    }

    #[test]
    fn bhd_policy() {
        let p = RoundingPolicy::bhd();
        assert_eq!(p.minor_units, 3);
    }

    #[test]
    fn default_policy_is_usd() {
        assert_eq!(RoundingPolicy::default(), RoundingPolicy::usd());
    }

    // ---- Zero amount ----

    #[test]
    fn round_zero() {
        for mode in [
            RoundingMode::HalfUp,
            RoundingMode::HalfEven,
            RoundingMode::Down,
            RoundingMode::Up,
        ] {
            let p = RoundingPolicy::new(mode, 2);
            assert_eq!(round(Decimal::ZERO, &p), Decimal::ZERO);
        }
    }

    // ---- Large values ----

    #[test]
    fn round_large_value() {
        let p = RoundingPolicy::usd();
        assert_eq!(round(dec!(999999999.999), &p), dec!(1000000000.00));
    }
}
