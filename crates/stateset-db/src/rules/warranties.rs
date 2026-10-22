//! Warranty rules shared by both backends.

use chrono::{DateTime, Duration, TimeZone, Utc};

/// Latest instant a horizon is clamped to. It keeps a four-digit RFC 3339
/// year, so SQLite's text comparison still orders it correctly (chrono's own
/// maximum renders as `+262143-...`, which sorts before every real date).
fn latest() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(9999, 12, 31, 23, 59, 59).single().unwrap_or(DateTime::<Utc>::MAX_UTC)
}

/// Earliest instant a horizon is clamped to, for the same reason.
fn earliest() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(1, 1, 1, 0, 0, 0).single().unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// The latest end date `WarrantyFilter::expiring_within_days` accepts: `now`
/// plus `days`, clamped to years 1..=9999 instead of overflowing. A negative
/// count yields a horizon in the past, so nothing matches.
pub(crate) fn expiry_horizon(now: DateTime<Utc>, days: i32) -> DateTime<Utc> {
    now.checked_add_signed(Duration::days(i64::from(days)))
        .unwrap_or(if days < 0 { earliest() } else { latest() })
        .clamp(earliest(), latest())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizon_adds_days_and_clamps_to_four_digit_years() {
        let now = Utc::now();
        assert_eq!(expiry_horizon(now, 30), now + Duration::days(30));
        assert_eq!(expiry_horizon(now, i32::MAX), latest());
        assert_eq!(expiry_horizon(now, i32::MIN), earliest());
        assert!(expiry_horizon(now, i32::MAX).to_rfc3339().starts_with("9999-"));
    }
}
