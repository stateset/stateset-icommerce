//! Split payment calculation logic.
//!
//! Implements percentage and fixed splits with rounding drift prevention.
//! All arithmetic uses [`rust_decimal::Decimal`] with 6 decimal places
//! (matching USDC's precision).

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// Number of decimal places for USDC-like assets.
pub const DECIMALS_DP: u32 = 6;

/// Multiplier for converting between decimal and smallest-unit representations.
pub const DECIMALS_MULTIPLIER: u64 = 1_000_000;

/// The type of split being performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SplitType {
    /// Each recipient receives a percentage of the remaining amount (after platform fee).
    Percentage,
    /// Each recipient receives a fixed amount.
    Fixed,
}

/// A recipient in a split payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipient {
    /// Wallet address.
    pub address: String,
    /// Share percentage (for percentage splits). Must be in 0..=100 range.
    pub percent: Option<Decimal>,
    /// Fixed amount (for fixed splits).
    pub amount: Option<Decimal>,
}

/// A computed share for one recipient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitShare {
    /// Wallet address.
    pub address: String,
    /// Computed share amount (6 dp).
    pub amount: Decimal,
}

/// Result of a split calculation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitResult {
    /// Individual shares for each recipient.
    pub shares: Vec<SplitShare>,
    /// Platform fee amount.
    pub platform_fee: Decimal,
    /// Total distributed (shares + platform fee). Must equal the input total.
    pub total_distributed: Decimal,
}

fn validate_platform_fee_pct(platform_fee_pct: Decimal) -> A2AResult<()> {
    if platform_fee_pct < Decimal::ZERO {
        return Err(A2AError::validation("platform_fee_pct must be >= 0"));
    }
    if platform_fee_pct > dec!(100) {
        return Err(A2AError::validation("platform_fee_pct must be <= 100"));
    }
    Ok(())
}

/// Calculate a percentage-based split.
///
/// The last recipient receives the remainder after all others have been
/// allocated, preventing rounding drift.
///
/// # Errors
///
/// - [`A2AError::Validation`] if `recipients` has fewer than 2 entries.
/// - [`A2AError::Validation`] if `total` is not positive.
/// - [`A2AError::Validation`] if any recipient is missing a `percent` field.
/// - [`A2AError::PercentageSumMismatch`] if percentages do not sum to 100.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use stateset_a2a::splits::{Recipient, calculate_percentage_split};
///
/// let recipients = vec![
///     Recipient { address: "0xAlice".into(), percent: Some(dec!(50)), amount: None },
///     Recipient { address: "0xBob".into(), percent: Some(dec!(30)), amount: None },
///     Recipient { address: "0xCharlie".into(), percent: Some(dec!(20)), amount: None },
/// ];
///
/// let result = calculate_percentage_split(dec!(100), dec!(2.5), &recipients).unwrap();
/// assert_eq!(result.total_distributed, dec!(100));
/// assert_eq!(result.platform_fee, dec!(2.500000));
/// assert_eq!(result.shares.len(), 3);
/// ```
pub fn calculate_percentage_split(
    total: Decimal,
    platform_fee_pct: Decimal,
    recipients: &[Recipient],
) -> A2AResult<SplitResult> {
    // --- Validation ---
    if total <= Decimal::ZERO {
        return Err(A2AError::validation("total must be greater than 0"));
    }
    validate_platform_fee_pct(platform_fee_pct)?;
    if recipients.len() < 2 {
        return Err(A2AError::validation("recipients must have at least 2 entries"));
    }

    // Validate all recipients have a percent
    for (i, r) in recipients.iter().enumerate() {
        let Some(percent) = r.percent else {
            return Err(A2AError::validation(format!(
                "recipients[{i}].percent is required for percentage splits"
            )));
        };
        if !(Decimal::ZERO..=dec!(100)).contains(&percent) {
            return Err(A2AError::validation(format!(
                "recipients[{i}].percent must be in range [0, 100]"
            )));
        }
    }

    // Validate percentages sum to 100
    let percent_sum: Decimal = recipients.iter().map(|r| r.percent.unwrap_or_default()).sum();
    let tolerance = dec!(0.001);
    if (percent_sum - dec!(100)).abs() > tolerance {
        return Err(A2AError::PercentageSumMismatch { actual: percent_sum });
    }

    // --- Platform fee ---
    let fee = (total * platform_fee_pct / dec!(100)).round_dp(DECIMALS_DP);
    let remaining = total - fee;

    // --- Compute shares ---
    let mut allocated = Decimal::ZERO;
    let mut shares = Vec::with_capacity(recipients.len());

    for (i, r) in recipients.iter().enumerate() {
        let pct = r.percent.unwrap_or_default();
        let share = if i == recipients.len() - 1 {
            // Last recipient gets the remainder to prevent rounding drift
            remaining - allocated
        } else {
            (remaining * pct / dec!(100)).round_dp(DECIMALS_DP)
        };
        allocated += share;
        shares.push(SplitShare { address: r.address.clone(), amount: share });
    }

    Ok(SplitResult { shares, platform_fee: fee, total_distributed: allocated + fee })
}

/// Calculate a fixed-amount split.
///
/// Validates that the fixed amounts sum to `total - platform_fee`.
///
/// # Errors
///
/// - [`A2AError::Validation`] if `recipients` has fewer than 2 entries.
/// - [`A2AError::Validation`] if `total` is not positive.
/// - [`A2AError::Validation`] if any recipient is missing an `amount` field.
/// - [`A2AError::FixedSumMismatch`] if fixed amounts don't sum to the remaining amount.
///
/// # Example
///
/// ```
/// use rust_decimal_macros::dec;
/// use stateset_a2a::splits::{Recipient, calculate_fixed_split};
///
/// let recipients = vec![
///     Recipient { address: "0xAlice".into(), percent: None, amount: Some(dec!(60)) },
///     Recipient { address: "0xBob".into(), percent: None, amount: Some(dec!(40)) },
/// ];
///
/// let result = calculate_fixed_split(dec!(100), dec!(0), &recipients).unwrap();
/// assert_eq!(result.total_distributed, dec!(100));
/// ```
pub fn calculate_fixed_split(
    total: Decimal,
    platform_fee_pct: Decimal,
    recipients: &[Recipient],
) -> A2AResult<SplitResult> {
    // --- Validation ---
    if total <= Decimal::ZERO {
        return Err(A2AError::validation("total must be greater than 0"));
    }
    validate_platform_fee_pct(platform_fee_pct)?;
    if recipients.len() < 2 {
        return Err(A2AError::validation("recipients must have at least 2 entries"));
    }

    for (i, r) in recipients.iter().enumerate() {
        let Some(amount) = r.amount else {
            return Err(A2AError::validation(format!(
                "recipients[{i}].amount is required for fixed splits"
            )));
        };
        if amount < Decimal::ZERO {
            return Err(A2AError::validation(format!(
                "recipients[{i}].amount must be >= 0 for fixed splits"
            )));
        }
    }

    // --- Platform fee ---
    let fee = (total * platform_fee_pct / dec!(100)).round_dp(DECIMALS_DP);
    let remaining = total - fee;

    // --- Validate fixed amounts sum to remaining ---
    let mut rounded_amounts: Vec<Decimal> =
        recipients.iter().map(|r| r.amount.unwrap_or_default().round_dp(DECIMALS_DP)).collect();
    let fixed_sum: Decimal = rounded_amounts.iter().copied().sum();

    // Allow 1 smallest-unit tolerance for rounding
    let one_unit = Decimal::new(1, DECIMALS_DP);
    let residual = remaining - fixed_sum;
    if residual.abs() > one_unit {
        return Err(A2AError::FixedSumMismatch { expected: remaining, actual: fixed_sum });
    }

    if let Some(last) = rounded_amounts.last_mut() {
        let adjusted = (*last + residual).round_dp(DECIMALS_DP);
        if adjusted < Decimal::ZERO {
            return Err(A2AError::validation(
                "residual normalization would make last recipient amount negative",
            ));
        }
        *last = adjusted;
    }

    // --- Build shares ---
    let mut shares = Vec::with_capacity(recipients.len());
    let mut distributed = Decimal::ZERO;

    for (r, amount) in recipients.iter().zip(rounded_amounts.into_iter()) {
        distributed += amount;
        shares.push(SplitShare { address: r.address.clone(), amount });
    }

    Ok(SplitResult { shares, platform_fee: fee, total_distributed: distributed + fee })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn make_pct_recipients(pcts: &[(&str, Decimal)]) -> Vec<Recipient> {
        pcts.iter()
            .map(|(addr, pct)| Recipient {
                address: (*addr).to_string(),
                percent: Some(*pct),
                amount: None,
            })
            .collect()
    }

    fn make_fixed_recipients(amounts: &[(&str, Decimal)]) -> Vec<Recipient> {
        amounts
            .iter()
            .map(|(addr, amt)| Recipient {
                address: (*addr).to_string(),
                percent: None,
                amount: Some(*amt),
            })
            .collect()
    }

    // ===== Percentage split tests =====

    #[test]
    fn percentage_split_basic_two_way() {
        let r = make_pct_recipients(&[("0xA", dec!(50)), ("0xB", dec!(50))]);
        let result = calculate_percentage_split(dec!(100), dec!(0), &r).unwrap();

        assert_eq!(result.shares.len(), 2);
        assert_eq!(result.shares[0].amount, dec!(50));
        assert_eq!(result.shares[1].amount, dec!(50));
        assert_eq!(result.platform_fee, dec!(0));
        assert_eq!(result.total_distributed, dec!(100));
    }

    #[test]
    fn percentage_split_three_way_with_platform_fee() {
        let r = make_pct_recipients(&[
            ("0xAlice", dec!(50)),
            ("0xBob", dec!(30)),
            ("0xCharlie", dec!(20)),
        ]);
        let result = calculate_percentage_split(dec!(100), dec!(2.5), &r).unwrap();

        assert_eq!(result.platform_fee, dec!(2.500000));
        assert_eq!(result.total_distributed, dec!(100));
        assert_eq!(result.shares.len(), 3);

        // Alice: 97.5 * 50% = 48.75
        assert_eq!(result.shares[0].amount, dec!(48.750000));
        // Bob: 97.5 * 30% = 29.25
        assert_eq!(result.shares[1].amount, dec!(29.250000));
        // Charlie gets remainder: 97.5 - 48.75 - 29.25 = 19.5
        assert_eq!(result.shares[2].amount, dec!(19.500000));
    }

    #[test]
    fn percentage_split_rounding_drift_prevention() {
        // 3-way even split of 100 with no fee: 33.333333 + 33.333333 + 33.333334
        let r = make_pct_recipients(&[
            ("0xA", dec!(33.333)),
            ("0xB", dec!(33.333)),
            ("0xC", dec!(33.334)),
        ]);
        let result = calculate_percentage_split(dec!(100), dec!(0), &r).unwrap();

        // Total must be exactly 100 regardless of rounding
        assert_eq!(result.total_distributed, dec!(100));
    }

    #[test]
    fn percentage_split_large_amount() {
        let r = make_pct_recipients(&[("0xA", dec!(70)), ("0xB", dec!(30))]);
        let result = calculate_percentage_split(dec!(1000000), dec!(1), &r).unwrap();

        assert_eq!(result.platform_fee, dec!(10000.000000));
        assert_eq!(result.total_distributed, dec!(1000000));
    }

    #[test]
    fn percentage_split_tiny_amount() {
        let r = make_pct_recipients(&[("0xA", dec!(60)), ("0xB", dec!(40))]);
        let result = calculate_percentage_split(dec!(0.000001), dec!(0), &r).unwrap();
        assert_eq!(result.total_distributed, dec!(0.000001));
    }

    #[test]
    fn percentage_split_rejects_single_recipient() {
        let r = make_pct_recipients(&[("0xA", dec!(100))]);
        let err = calculate_percentage_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn percentage_split_rejects_zero_total() {
        let r = make_pct_recipients(&[("0xA", dec!(50)), ("0xB", dec!(50))]);
        let err = calculate_percentage_split(dec!(0), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn percentage_split_rejects_negative_total() {
        let r = make_pct_recipients(&[("0xA", dec!(50)), ("0xB", dec!(50))]);
        let err = calculate_percentage_split(dec!(-10), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn percentage_split_rejects_wrong_sum() {
        let r = make_pct_recipients(&[("0xA", dec!(50)), ("0xB", dec!(40))]);
        let err = calculate_percentage_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::PercentageSumMismatch { .. }));
    }

    #[test]
    fn percentage_split_rejects_missing_percent() {
        let r = vec![
            Recipient { address: "0xA".into(), percent: Some(dec!(50)), amount: None },
            Recipient { address: "0xB".into(), percent: None, amount: None },
        ];
        let err = calculate_percentage_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn percentage_split_many_recipients() {
        // 10 recipients, each 10%
        let r: Vec<_> = (0..10)
            .map(|i| Recipient {
                address: format!("0x{i:02}"),
                percent: Some(dec!(10)),
                amount: None,
            })
            .collect();
        let result = calculate_percentage_split(dec!(1000), dec!(5), &r).unwrap();
        assert_eq!(result.total_distributed, dec!(1000));
        assert_eq!(result.shares.len(), 10);
    }

    #[test]
    fn percentage_split_uneven_three_way() {
        // Test the specific case from the JS code: 100 total, 2.5% fee, 50/30/20 split
        let r = make_pct_recipients(&[
            ("0xAlice", dec!(50)),
            ("0xBob", dec!(30)),
            ("0xCharlie", dec!(20)),
        ]);
        let result = calculate_percentage_split(dec!(100), dec!(2.5), &r).unwrap();

        // Fee = 100 * 2.5% = 2.5
        // Remaining = 97.5
        // Alice: 97.5 * 50% = 48.75
        // Bob:   97.5 * 30% = 29.25
        // Charlie: 97.5 - 48.75 - 29.25 = 19.5
        assert_eq!(result.platform_fee, dec!(2.500000));
        assert_eq!(result.shares[0].amount, dec!(48.750000));
        assert_eq!(result.shares[1].amount, dec!(29.250000));
        assert_eq!(result.shares[2].amount, dec!(19.500000));
        assert_eq!(result.total_distributed, dec!(100));
    }

    #[test]
    fn percentage_split_rejects_negative_platform_fee_pct() {
        let r = make_pct_recipients(&[("0xA", dec!(50)), ("0xB", dec!(50))]);
        let err = calculate_percentage_split(dec!(100), dec!(-1), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn percentage_split_rejects_platform_fee_pct_above_100() {
        let r = make_pct_recipients(&[("0xA", dec!(50)), ("0xB", dec!(50))]);
        let err = calculate_percentage_split(dec!(100), dec!(100.000001), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn percentage_split_rejects_negative_recipient_percent() {
        let r = make_pct_recipients(&[("0xA", dec!(110)), ("0xB", dec!(-10))]);
        let err = calculate_percentage_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn percentage_split_rejects_recipient_percent_above_100() {
        let r = make_pct_recipients(&[("0xA", dec!(120)), ("0xB", dec!(-20))]);
        let err = calculate_percentage_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    // ===== Fixed split tests =====

    #[test]
    fn fixed_split_basic() {
        let r = make_fixed_recipients(&[("0xA", dec!(60)), ("0xB", dec!(40))]);
        let result = calculate_fixed_split(dec!(100), dec!(0), &r).unwrap();

        assert_eq!(result.shares.len(), 2);
        assert_eq!(result.shares[0].amount, dec!(60));
        assert_eq!(result.shares[1].amount, dec!(40));
        assert_eq!(result.total_distributed, dec!(100));
    }

    #[test]
    fn fixed_split_with_platform_fee() {
        // 100 total, 5% fee = 5, remaining = 95
        let r = make_fixed_recipients(&[("0xA", dec!(55)), ("0xB", dec!(40))]);
        let result = calculate_fixed_split(dec!(100), dec!(5), &r).unwrap();

        assert_eq!(result.platform_fee, dec!(5.000000));
        assert_eq!(result.total_distributed, dec!(100));
    }

    #[test]
    fn fixed_split_rejects_wrong_sum() {
        let r = make_fixed_recipients(&[("0xA", dec!(50)), ("0xB", dec!(40))]);
        let err = calculate_fixed_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::FixedSumMismatch { .. }));
    }

    #[test]
    fn fixed_split_rejects_single_recipient() {
        let r = make_fixed_recipients(&[("0xA", dec!(100))]);
        let err = calculate_fixed_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn fixed_split_rejects_missing_amount() {
        let r = vec![
            Recipient { address: "0xA".into(), percent: None, amount: Some(dec!(50)) },
            Recipient { address: "0xB".into(), percent: None, amount: None },
        ];
        let err = calculate_fixed_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn fixed_split_allows_one_unit_tolerance() {
        // amounts sum to 99.999999, total is 100 — within 1 unit tolerance
        let r = make_fixed_recipients(&[("0xA", dec!(59.999999)), ("0xB", dec!(40.000000))]);
        // This should fail since diff is 0.000001 which equals one_unit
        // The check is > one_unit, so exactly one_unit should pass
        let result = calculate_fixed_split(dec!(100), dec!(0), &r);
        assert!(result.is_ok());
    }

    #[test]
    fn fixed_split_normalizes_residual_to_match_total_exactly() {
        let r = make_fixed_recipients(&[("0xA", dec!(59.999999)), ("0xB", dec!(40.000000))]);
        let result = calculate_fixed_split(dec!(100), dec!(0), &r).unwrap();

        assert_eq!(result.shares[0].amount, dec!(59.999999));
        assert_eq!(result.shares[1].amount, dec!(40.000001));
        assert_eq!(result.total_distributed, dec!(100.000000));
    }

    #[test]
    fn fixed_split_rejects_negative_platform_fee_pct() {
        let r = make_fixed_recipients(&[("0xA", dec!(60)), ("0xB", dec!(40))]);
        let err = calculate_fixed_split(dec!(100), dec!(-0.1), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn fixed_split_rejects_platform_fee_pct_above_100() {
        let r = make_fixed_recipients(&[("0xA", dec!(60)), ("0xB", dec!(40))]);
        let err = calculate_fixed_split(dec!(100), dec!(100.1), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn fixed_split_rejects_negative_amount() {
        let r = make_fixed_recipients(&[("0xA", dec!(120)), ("0xB", dec!(-20))]);
        let err = calculate_fixed_split(dec!(100), dec!(0), &r).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }
}
