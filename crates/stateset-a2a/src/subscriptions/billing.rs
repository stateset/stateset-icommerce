//! Billing interval calculations.
//!
//! Computes the next billing date from a given start date and interval.

use chrono::{DateTime, Months, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

/// Supported billing intervals for recurring subscriptions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BillingInterval {
    /// Every 7 days.
    Weekly,
    /// Every 14 days.
    Biweekly,
    /// Every calendar month.
    Monthly,
    /// Every 3 calendar months.
    Quarterly,
    /// Every calendar year.
    Annual,
}

impl std::fmt::Display for BillingInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Weekly => write!(f, "weekly"),
            Self::Biweekly => write!(f, "biweekly"),
            Self::Monthly => write!(f, "monthly"),
            Self::Quarterly => write!(f, "quarterly"),
            Self::Annual => write!(f, "annual"),
        }
    }
}

impl BillingInterval {
    /// Parse a billing interval from a string.
    ///
    /// # Errors
    ///
    /// Returns [`A2AError::InvalidBillingInterval`] if the string is not recognized.
    pub fn from_str_checked(s: &str) -> A2AResult<Self> {
        match s {
            "weekly" => Ok(Self::Weekly),
            "biweekly" => Ok(Self::Biweekly),
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "annual" => Ok(Self::Annual),
            other => Err(A2AError::InvalidBillingInterval(other.to_string())),
        }
    }

    /// All valid billing interval values.
    pub const ALL: &'static [BillingInterval] = &[
        Self::Weekly,
        Self::Biweekly,
        Self::Monthly,
        Self::Quarterly,
        Self::Annual,
    ];
}

/// Compute the next billing date from a given start date and interval.
///
/// For month-based intervals (monthly, quarterly), uses chrono's
/// [`checked_add_months`](DateTime::checked_add_months) to handle
/// month-end edge cases correctly.
///
/// # Errors
///
/// Returns [`A2AError::Internal`] if date arithmetic overflows (should not happen
/// in practice).
///
/// # Example
///
/// ```
/// use chrono::{Datelike, TimeZone, Utc};
/// use stateset_a2a::subscriptions::{BillingInterval, compute_next_billing_date};
///
/// let start = Utc.with_ymd_and_hms(2026, 1, 15, 0, 0, 0).unwrap();
/// let next = compute_next_billing_date(start, BillingInterval::Monthly).unwrap();
/// assert_eq!(next.month(), 2);
/// assert_eq!(next.day(), 15);
/// ```
pub fn compute_next_billing_date(
    from: DateTime<Utc>,
    interval: BillingInterval,
) -> A2AResult<DateTime<Utc>> {
    let result = match interval {
        BillingInterval::Weekly => from + chrono::Duration::weeks(1),
        BillingInterval::Biweekly => from + chrono::Duration::weeks(2),
        BillingInterval::Monthly => from
            .checked_add_months(Months::new(1))
            .ok_or_else(|| A2AError::Internal("date overflow adding 1 month".into()))?,
        BillingInterval::Quarterly => from
            .checked_add_months(Months::new(3))
            .ok_or_else(|| A2AError::Internal("date overflow adding 3 months".into()))?,
        BillingInterval::Annual => from
            .checked_add_months(Months::new(12))
            .ok_or_else(|| A2AError::Internal("date overflow adding 12 months".into()))?,
    };
    Ok(result)
}

/// Compute the trial end date given a start date and number of trial days.
///
/// # Example
///
/// ```
/// use chrono::{Datelike, TimeZone, Utc};
/// use stateset_a2a::subscriptions::billing::compute_trial_end;
///
/// let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
/// let end = compute_trial_end(start, 14);
/// assert_eq!(end.day(), 15);
/// ```
#[must_use]
pub fn compute_trial_end(from: DateTime<Utc>, trial_days: u32) -> DateTime<Utc> {
    from + chrono::Duration::days(i64::from(trial_days))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, TimeZone};

    fn dt(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    #[test]
    fn weekly_adds_7_days() {
        let start = dt(2026, 1, 1);
        let next = compute_next_billing_date(start, BillingInterval::Weekly).unwrap();
        assert_eq!(next.day(), 8);
        assert_eq!(next.month(), 1);
    }

    #[test]
    fn biweekly_adds_14_days() {
        let start = dt(2026, 1, 1);
        let next = compute_next_billing_date(start, BillingInterval::Biweekly).unwrap();
        assert_eq!(next.day(), 15);
        assert_eq!(next.month(), 1);
    }

    #[test]
    fn monthly_adds_one_month() {
        let start = dt(2026, 1, 15);
        let next = compute_next_billing_date(start, BillingInterval::Monthly).unwrap();
        assert_eq!(next.month(), 2);
        assert_eq!(next.day(), 15);
    }

    #[test]
    fn monthly_end_of_month_jan_to_feb() {
        // Jan 31 -> Feb 28 (2026 is not a leap year)
        let start = dt(2026, 1, 31);
        let next = compute_next_billing_date(start, BillingInterval::Monthly).unwrap();
        assert_eq!(next.month(), 2);
        assert_eq!(next.day(), 28);
    }

    #[test]
    fn quarterly_adds_three_months() {
        let start = dt(2026, 1, 15);
        let next = compute_next_billing_date(start, BillingInterval::Quarterly).unwrap();
        assert_eq!(next.month(), 4);
        assert_eq!(next.day(), 15);
    }

    #[test]
    fn annual_adds_one_year() {
        let start = dt(2026, 3, 10);
        let next = compute_next_billing_date(start, BillingInterval::Annual).unwrap();
        assert_eq!(next.year(), 2027);
        assert_eq!(next.month(), 3);
        assert_eq!(next.day(), 10);
    }

    #[test]
    fn annual_leap_year_feb_29() {
        // 2028 is a leap year; Feb 29 + 1 year = Feb 28 2029
        let start = dt(2028, 2, 29);
        let next = compute_next_billing_date(start, BillingInterval::Annual).unwrap();
        assert_eq!(next.year(), 2029);
        assert_eq!(next.month(), 2);
        assert_eq!(next.day(), 28);
    }

    #[test]
    fn weekly_crosses_month_boundary() {
        let start = dt(2026, 1, 28);
        let next = compute_next_billing_date(start, BillingInterval::Weekly).unwrap();
        assert_eq!(next.month(), 2);
        assert_eq!(next.day(), 4);
    }

    #[test]
    fn quarterly_crosses_year_boundary() {
        let start = dt(2026, 11, 15);
        let next = compute_next_billing_date(start, BillingInterval::Quarterly).unwrap();
        assert_eq!(next.year(), 2027);
        assert_eq!(next.month(), 2);
    }

    #[test]
    fn billing_interval_from_str_valid() {
        assert_eq!(
            BillingInterval::from_str_checked("weekly").unwrap(),
            BillingInterval::Weekly
        );
        assert_eq!(
            BillingInterval::from_str_checked("monthly").unwrap(),
            BillingInterval::Monthly
        );
        assert_eq!(
            BillingInterval::from_str_checked("annual").unwrap(),
            BillingInterval::Annual
        );
    }

    #[test]
    fn billing_interval_from_str_invalid() {
        let err = BillingInterval::from_str_checked("daily").unwrap_err();
        assert!(matches!(err, A2AError::InvalidBillingInterval(_)));
    }

    #[test]
    fn billing_interval_display() {
        assert_eq!(BillingInterval::Weekly.to_string(), "weekly");
        assert_eq!(BillingInterval::Biweekly.to_string(), "biweekly");
        assert_eq!(BillingInterval::Monthly.to_string(), "monthly");
        assert_eq!(BillingInterval::Quarterly.to_string(), "quarterly");
        assert_eq!(BillingInterval::Annual.to_string(), "annual");
    }

    #[test]
    fn billing_interval_all_has_five_entries() {
        assert_eq!(BillingInterval::ALL.len(), 5);
    }

    #[test]
    fn trial_end_14_days() {
        let start = dt(2026, 1, 1);
        let end = compute_trial_end(start, 14);
        assert_eq!(end.day(), 15);
        assert_eq!(end.month(), 1);
    }

    #[test]
    fn trial_end_zero_days() {
        let start = dt(2026, 1, 1);
        let end = compute_trial_end(start, 0);
        assert_eq!(end, start);
    }

    #[test]
    fn trial_end_crosses_month() {
        let start = dt(2026, 1, 25);
        let end = compute_trial_end(start, 14);
        assert_eq!(end.month(), 2);
        assert_eq!(end.day(), 8);
    }
}
