//! Price schedule domain models
//!
//! A price schedule is a time-bounded set of product price overrides — e.g. a
//! promotional window or seasonal price list. Each schedule carries an optional
//! active window (`starts_at`/`ends_at`); a schedule is effective at an instant
//! only when active and within that window. Per-product prices live in entries.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, PriceScheduleId, ProductId};

/// A time-bounded set of product price overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceSchedule {
    /// Unique price schedule ID.
    pub id: PriceScheduleId,
    /// Display name (e.g. "Black Friday 2026").
    pub name: String,
    /// Optional short code.
    pub code: Option<String>,
    /// Currency for entry prices.
    pub currency: CurrencyCode,
    /// Window start (inclusive); `None` means no lower bound.
    pub starts_at: Option<DateTime<Utc>>,
    /// Window end (inclusive); `None` means no upper bound.
    pub ends_at: Option<DateTime<Utc>>,
    /// Whether the schedule is enabled.
    pub is_active: bool,
    /// Priority used to break ties when multiple schedules match
    /// (higher wins).
    pub priority: i32,
    /// When the schedule was created.
    pub created_at: DateTime<Utc>,
    /// When the schedule was last updated.
    pub updated_at: DateTime<Utc>,
}

impl PriceSchedule {
    /// Whether the schedule is effective at the given instant.
    #[must_use]
    pub fn is_effective_at(&self, now: DateTime<Utc>) -> bool {
        if !self.is_active {
            return false;
        }
        let after_start = self.starts_at.is_none_or(|s| now >= s);
        let before_end = self.ends_at.is_none_or(|e| now <= e);
        after_start && before_end
    }
}

/// A per-product fixed price within a schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceScheduleEntry {
    /// Owning schedule.
    pub price_schedule_id: PriceScheduleId,
    /// Product the price applies to.
    pub product_id: ProductId,
    /// Scheduled price for this product.
    pub price: Decimal,
    /// When the entry was created.
    pub created_at: DateTime<Utc>,
    /// When the entry was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a price schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePriceSchedule {
    /// Display name.
    pub name: String,
    /// Optional short code.
    pub code: Option<String>,
    /// Currency (defaults to account base currency when omitted).
    pub currency: Option<CurrencyCode>,
    /// Window start.
    pub starts_at: Option<DateTime<Utc>>,
    /// Window end.
    pub ends_at: Option<DateTime<Utc>>,
    /// Priority (default 0).
    #[serde(default)]
    pub priority: i32,
}

/// Input for updating a price schedule (partial).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePriceSchedule {
    /// Updated name.
    pub name: Option<String>,
    /// Updated code.
    pub code: Option<String>,
    /// Updated window start.
    pub starts_at: Option<DateTime<Utc>>,
    /// Updated window end.
    pub ends_at: Option<DateTime<Utc>>,
    /// Updated active flag.
    pub is_active: Option<bool>,
    /// Updated priority.
    pub priority: Option<i32>,
}

/// Filter for listing price schedules.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PriceScheduleFilter {
    /// Filter by active flag.
    pub is_active: Option<bool>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};
    use rust_decimal_macros::dec;

    fn at(y: i32, m: u32, d: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
    }

    fn make(
        starts_at: Option<DateTime<Utc>>,
        ends_at: Option<DateTime<Utc>>,
        is_active: bool,
    ) -> PriceSchedule {
        PriceSchedule {
            id: PriceScheduleId::new(),
            name: "Sale".into(),
            code: None,
            currency: CurrencyCode::USD,
            starts_at,
            ends_at,
            is_active,
            priority: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn effective_within_window() {
        let s = make(Some(at(2026, 6, 1)), Some(at(2026, 6, 30)), true);
        assert!(s.is_effective_at(at(2026, 6, 15)));
        assert!(!s.is_effective_at(at(2026, 5, 31)));
        assert!(!s.is_effective_at(at(2026, 7, 1)));
    }

    #[test]
    fn boundaries_inclusive() {
        let start = at(2026, 6, 1);
        let end = at(2026, 6, 30);
        let s = make(Some(start), Some(end), true);
        assert!(s.is_effective_at(start));
        assert!(s.is_effective_at(end));
        assert!(!s.is_effective_at(end + Duration::seconds(1)));
    }

    #[test]
    fn inactive_never_effective() {
        let s = make(None, None, false);
        assert!(!s.is_effective_at(at(2026, 6, 15)));
    }

    #[test]
    fn open_ended_windows() {
        // no bounds → always effective when active
        assert!(make(None, None, true).is_effective_at(at(2026, 1, 1)));
        // only start bound
        let s = make(Some(at(2026, 6, 1)), None, true);
        assert!(s.is_effective_at(at(2030, 1, 1)));
        assert!(!s.is_effective_at(at(2025, 1, 1)));
    }

    #[test]
    fn entry_holds_price() {
        let entry = PriceScheduleEntry {
            price_schedule_id: PriceScheduleId::new(),
            product_id: ProductId::new(),
            price: dec!(19.99),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(entry.price, dec!(19.99));
    }
}
