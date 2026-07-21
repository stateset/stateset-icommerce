//! Revenue recognition domain models (ASC 606 style)
//!
//! Supports:
//! - Revenue contracts linked to orders and invoices
//! - Performance obligations with allocated transaction price
//! - Recognition methods (point in time, ratable over time, milestone)
//! - Revenue schedules with deferred/recognized entries and exact plugs

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::CurrencyCode;
use strum::{Display, EnumString};
use uuid::Uuid;

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};

/// Monetary scale (2 decimal places) used for schedule rounding.
const MONEY_SCALE: u32 = 2;

// ============================================================================
// Enums
// ============================================================================

/// Method by which revenue for a performance obligation is recognized
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecognitionMethod {
    /// Recognize the full amount when control transfers (e.g. delivery).
    PointInTime,
    /// Recognize ratably across the service period.
    RatableOverTime {
        /// First month of the recognition period (inclusive).
        start: NaiveDate,
        /// Last month of the recognition period (inclusive).
        end: NaiveDate,
    },
    /// Recognize as contract milestones are achieved.
    Milestone,
}

/// Revenue contract lifecycle status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RevenueContractStatus {
    /// Contract is being drafted; obligations may still change.
    #[default]
    Draft,
    /// Contract is active; revenue is being deferred and recognized.
    Active,
    /// All obligations satisfied and all revenue recognized.
    Completed,
    /// Contract was cancelled before completion.
    Cancelled,
}

impl RevenueContractStatus {
    /// Returns true if a transition from `self` to `next` is allowed.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::Active | Self::Cancelled)
                | (Self::Active, Self::Completed | Self::Cancelled)
        )
    }

    /// Returns true if this status is terminal (no further transitions).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

/// Status of a single revenue schedule entry
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RevenueEntryStatus {
    /// Revenue is deferred; not yet recognized in the income statement.
    #[default]
    Deferred,
    /// Revenue has been recognized.
    Recognized,
}

impl RevenueEntryStatus {
    /// Returns true if a transition from `self` to `next` is allowed.
    ///
    /// Recognition is one-way: `Deferred -> Recognized` only.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!((self, next), (Self::Deferred, Self::Recognized))
    }
}

// ============================================================================
// Core Structs
// ============================================================================

/// A revenue contract grouping performance obligations (ASC 606 step 1)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueContract {
    /// Unique identifier for this contract.
    pub id: Uuid,
    /// Human-readable contract reference number.
    pub contract_number: String,
    /// Customer party to the contract.
    pub customer_id: Uuid,
    /// Originating order, if any.
    pub order_id: Option<Uuid>,
    /// Related invoice, if any.
    pub invoice_id: Option<Uuid>,
    /// Total transaction price for the contract (ASC 606 step 3).
    pub transaction_price: Decimal,
    /// Currency of all monetary amounts on this contract.
    pub currency: CurrencyCode,
    /// Current lifecycle status.
    pub status: RevenueContractStatus,
    /// Date the contract takes effect.
    pub effective_date: NaiveDate,
    /// Performance obligations under this contract (ASC 606 step 2).
    pub obligations: Vec<PerformanceObligation>,
    /// Timestamp of record creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

impl RevenueContract {
    /// Sum of allocated amounts across all obligations.
    #[must_use]
    pub fn total_allocated(&self) -> Decimal {
        self.obligations.iter().map(|o| o.allocated_amount).sum()
    }

    /// Sum of recognized revenue across all obligations.
    #[must_use]
    pub fn total_recognized(&self) -> Decimal {
        self.obligations.iter().map(|o| o.recognized_amount).sum()
    }

    /// Remaining deferred revenue: transaction price less recognized.
    #[must_use]
    pub fn deferred_balance(&self) -> Decimal {
        self.transaction_price - self.total_recognized()
    }

    /// Returns true when every obligation is fully recognized.
    #[must_use]
    pub fn is_fully_recognized(&self) -> bool {
        self.obligations.iter().all(PerformanceObligation::is_fully_recognized)
    }
}

impl Validate for RevenueContract {
    /// Validate a revenue contract.
    ///
    /// Requires a positive transaction price, at least one obligation, and
    /// obligation allocated amounts that sum exactly to the transaction price
    /// (ASC 606 step 4).
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .required("contract_number", &self.contract_number)
            .uuid_not_nil("customer_id", self.customer_id)
            .positive("transaction_price", self.transaction_price)
            .non_empty_list("obligations", &self.obligations)
            .check(
                "obligations",
                self.total_allocated() == self.transaction_price,
                "allocated amounts must sum exactly to transaction_price",
            )
            .build()?;

        for obligation in &self.obligations {
            obligation.validate()?;
        }

        Ok(())
    }
}

/// A distinct promise to transfer goods or services (ASC 606 step 2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceObligation {
    /// Unique identifier for this obligation.
    pub id: Uuid,
    /// Parent revenue contract.
    pub contract_id: Uuid,
    /// Description of the promised good or service.
    pub description: String,
    /// Standalone selling price used for allocation.
    pub standalone_selling_price: Option<Decimal>,
    /// Portion of the transaction price allocated to this obligation.
    pub allocated_amount: Decimal,
    /// Method by which the allocated amount is recognized (ASC 606 step 5).
    pub recognition_method: RecognitionMethod,
    /// Revenue recognized to date for this obligation.
    pub recognized_amount: Decimal,
    /// Timestamp of record creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

impl PerformanceObligation {
    /// Deferred (not yet recognized) portion of the allocated amount.
    #[must_use]
    pub fn deferred_amount(&self) -> Decimal {
        self.allocated_amount - self.recognized_amount
    }

    /// Returns true when the full allocated amount has been recognized.
    #[must_use]
    pub fn is_fully_recognized(&self) -> bool {
        self.recognized_amount >= self.allocated_amount
    }
}

impl Validate for PerformanceObligation {
    /// Validate a performance obligation.
    ///
    /// Requires a description, a positive allocated amount, a recognized
    /// amount within `[0, allocated_amount]`, and for ratable recognition a
    /// period whose start does not fall after its end.
    fn validate(&self) -> Result<()> {
        let builder = ValidationBuilder::new()
            .required("description", &self.description)
            .positive("allocated_amount", self.allocated_amount)
            .non_negative("recognized_amount", self.recognized_amount)
            .check(
                "recognized_amount",
                self.recognized_amount <= self.allocated_amount,
                "must not exceed allocated_amount",
            );

        let builder = match self.recognition_method {
            RecognitionMethod::RatableOverTime { start, end } => {
                builder.check("recognition_method", start <= end, "start must not be after end")
            }
            _ => builder,
        };

        builder.build()
    }
}

/// A single period's scheduled revenue for an obligation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueScheduleEntry {
    /// 1-based period number within the schedule.
    pub period: u32,
    /// First day of the month this entry belongs to.
    pub period_start: NaiveDate,
    /// Revenue amount for this period.
    pub amount: Decimal,
    /// Recognition status of this entry.
    pub status: RevenueEntryStatus,
}

/// A full recognition schedule for a performance obligation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevenueSchedule {
    /// Obligation this schedule belongs to.
    pub obligation_id: Uuid,
    /// Method used to generate the schedule.
    pub method: RecognitionMethod,
    /// Per-period entries in order.
    pub entries: Vec<RevenueScheduleEntry>,
    /// Total scheduled revenue (equals the obligation's allocated amount).
    pub total_amount: Decimal,
}

impl RevenueSchedule {
    /// Sum of entries already recognized.
    #[must_use]
    pub fn recognized_total(&self) -> Decimal {
        self.entries
            .iter()
            .filter(|e| e.status == RevenueEntryStatus::Recognized)
            .map(|e| e.amount)
            .sum()
    }

    /// Sum of entries still deferred.
    #[must_use]
    pub fn deferred_total(&self) -> Decimal {
        self.entries
            .iter()
            .filter(|e| e.status == RevenueEntryStatus::Deferred)
            .map(|e| e.amount)
            .sum()
    }
}

// ============================================================================
// Schedule Generation
// ============================================================================

/// Number of calendar months from `start`'s month through `end`'s month, inclusive.
fn months_inclusive(start: NaiveDate, end: NaiveDate) -> u32 {
    use chrono::Datelike;
    let months = (i64::from(end.year()) * 12 + i64::from(end.month0()))
        - (i64::from(start.year()) * 12 + i64::from(start.month0()))
        + 1;
    u32::try_from(months.max(0)).unwrap_or(0)
}

/// First day of the month `offset` months after `start`'s month.
fn month_start_offset(start: NaiveDate, offset: u32) -> NaiveDate {
    use chrono::Datelike;
    let total = i64::from(start.year()) * 12 + i64::from(start.month0()) + i64::from(offset);
    let year = i32::try_from(total.div_euclid(12)).unwrap_or(start.year());
    let month = u32::try_from(total.rem_euclid(12)).unwrap_or(0) + 1;
    NaiveDate::from_ymd_opt(year, month, 1).unwrap_or(start)
}

/// Generate a revenue recognition schedule for the given method and amount.
///
/// - `PointInTime` produces a single entry dated `recognition_date`.
/// - `RatableOverTime` splits the amount evenly across each calendar month
///   from `start` through `end` (inclusive), rounded to 2 decimal places,
///   with the final period plugged so the entries sum exactly to `amount`.
/// - `Milestone` cannot be scheduled by time alone and returns an empty vector.
///
/// Returns an empty vector when `amount` is not positive or when a ratable
/// period is inverted (`start` after `end`).
#[must_use]
pub fn generate_revenue_schedule(
    method: RecognitionMethod,
    amount: Decimal,
    recognition_date: NaiveDate,
) -> Vec<RevenueScheduleEntry> {
    if amount <= Decimal::ZERO {
        return Vec::new();
    }

    match method {
        RecognitionMethod::PointInTime => vec![RevenueScheduleEntry {
            period: 1,
            period_start: recognition_date,
            amount,
            status: RevenueEntryStatus::Deferred,
        }],
        RecognitionMethod::RatableOverTime { start, end } => {
            if start > end {
                return Vec::new();
            }
            let months = months_inclusive(start, end);
            let per_period = (amount / Decimal::from(months)).round_dp(MONEY_SCALE);
            let mut entries = Vec::with_capacity(months as usize);
            let mut accumulated = Decimal::ZERO;
            for period in 1..=months {
                let entry_amount = if period == months {
                    // Final-period plug so entries sum exactly to `amount`.
                    amount - accumulated
                } else {
                    // Never over-recognize ahead of the plug: when rounding
                    // rounds up, uncapped periods would leave a negative
                    // final entry (a spurious revenue reversal).
                    per_period.min(amount - accumulated)
                };
                accumulated += entry_amount;
                entries.push(RevenueScheduleEntry {
                    period,
                    period_start: month_start_offset(start, period - 1),
                    amount: entry_amount,
                    status: RevenueEntryStatus::Deferred,
                });
            }
            entries
        }
        RecognitionMethod::Milestone => Vec::new(),
    }
}

// ============================================================================
// Input & Filter Types
// ============================================================================

/// Input for a performance obligation on a new contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePerformanceObligation {
    pub description: String,
    pub standalone_selling_price: Option<Decimal>,
    pub allocated_amount: Decimal,
    pub recognition_method: RecognitionMethod,
}

impl Validate for CreatePerformanceObligation {
    /// Validate a performance obligation create request.
    fn validate(&self) -> Result<()> {
        let builder = ValidationBuilder::new()
            .required("description", &self.description)
            .positive("allocated_amount", self.allocated_amount);

        let builder = match self.recognition_method {
            RecognitionMethod::RatableOverTime { start, end } => {
                builder.check("recognition_method", start <= end, "start must not be after end")
            }
            _ => builder,
        };

        builder.build()
    }
}

/// Input for creating a revenue contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRevenueContract {
    pub contract_number: Option<String>,
    pub customer_id: Uuid,
    pub order_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub transaction_price: Decimal,
    pub currency: Option<CurrencyCode>,
    pub effective_date: NaiveDate,
    pub obligations: Vec<CreatePerformanceObligation>,
}

impl Validate for CreateRevenueContract {
    /// Validate a revenue contract create request.
    ///
    /// Requires a non-nil customer, positive transaction price, at least one
    /// obligation, and obligation allocations that sum exactly to the
    /// transaction price.
    fn validate(&self) -> Result<()> {
        let allocated: Decimal = self.obligations.iter().map(|o| o.allocated_amount).sum();
        ValidationBuilder::new()
            .uuid_not_nil("customer_id", self.customer_id)
            .positive("transaction_price", self.transaction_price)
            .non_empty_list("obligations", &self.obligations)
            .check(
                "obligations",
                allocated == self.transaction_price,
                "allocated amounts must sum exactly to transaction_price",
            )
            .build()?;

        for obligation in &self.obligations {
            obligation.validate()?;
        }

        Ok(())
    }
}

/// Input for updating a revenue contract
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateRevenueContract {
    pub order_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub status: Option<RevenueContractStatus>,
    pub effective_date: Option<NaiveDate>,
}

/// Filter for listing revenue contracts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RevenueContractFilter {
    pub customer_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub status: Option<RevenueContractStatus>,
    pub effective_from: Option<NaiveDate>,
    pub effective_to: Option<NaiveDate>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Generate a revenue contract number using a timestamp
#[must_use]
pub fn generate_revenue_contract_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let suffix = Uuid::new_v4().simple().to_string();
    format!("RC-{}-{}", timestamp, &suffix[..8])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn obligation(allocated: Decimal, recognized: Decimal) -> PerformanceObligation {
        PerformanceObligation {
            id: Uuid::new_v4(),
            contract_id: Uuid::new_v4(),
            description: "Annual support".into(),
            standalone_selling_price: Some(allocated),
            allocated_amount: allocated,
            recognition_method: RecognitionMethod::RatableOverTime {
                start: date(2026, 1, 1),
                end: date(2026, 12, 31),
            },
            recognized_amount: recognized,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn contract(price: Decimal, obligations: Vec<PerformanceObligation>) -> RevenueContract {
        RevenueContract {
            id: Uuid::new_v4(),
            contract_number: "RC-1".into(),
            customer_id: Uuid::new_v4(),
            order_id: None,
            invoice_id: None,
            transaction_price: price,
            currency: CurrencyCode::default(),
            status: RevenueContractStatus::Active,
            effective_date: date(2026, 1, 1),
            obligations,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn ratable_schedule_sums_exactly_with_final_plug() {
        let entries = generate_revenue_schedule(
            RecognitionMethod::RatableOverTime { start: date(2026, 1, 15), end: date(2026, 12, 1) },
            dec!(1000),
            date(2026, 1, 15),
        );
        assert_eq!(entries.len(), 12);
        let total: Decimal = entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(1000));
        // 1000/12 = 83.333... -> 83.33 for the first 11, plug 83.37 last.
        assert_eq!(entries[0].amount, dec!(83.33));
        assert_eq!(entries[11].amount, dec!(83.37));
        assert!(entries.iter().all(|e| e.status == RevenueEntryStatus::Deferred));
    }

    #[test]
    fn ratable_schedule_period_dates_are_month_starts() {
        let entries = generate_revenue_schedule(
            RecognitionMethod::RatableOverTime { start: date(2026, 11, 20), end: date(2027, 2, 5) },
            dec!(400),
            date(2026, 11, 20),
        );
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].period_start, date(2026, 11, 1));
        assert_eq!(entries[1].period_start, date(2026, 12, 1));
        assert_eq!(entries[2].period_start, date(2027, 1, 1));
        assert_eq!(entries[3].period_start, date(2027, 2, 1));
        let total: Decimal = entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(400));
    }

    #[test]
    fn single_month_ratable_schedule() {
        let entries = generate_revenue_schedule(
            RecognitionMethod::RatableOverTime { start: date(2026, 3, 1), end: date(2026, 3, 31) },
            dec!(99.99),
            date(2026, 3, 1),
        );
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].amount, dec!(99.99));
    }

    #[test]
    fn point_in_time_schedule_is_single_entry() {
        let entries =
            generate_revenue_schedule(RecognitionMethod::PointInTime, dec!(250), date(2026, 6, 15));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].amount, dec!(250));
        assert_eq!(entries[0].period_start, date(2026, 6, 15));
    }

    #[test]
    fn empty_schedule_for_invalid_inputs() {
        assert!(
            generate_revenue_schedule(RecognitionMethod::PointInTime, dec!(0), date(2026, 1, 1))
                .is_empty()
        );
        assert!(
            generate_revenue_schedule(
                RecognitionMethod::RatableOverTime {
                    start: date(2026, 6, 1),
                    end: date(2026, 1, 1),
                },
                dec!(100),
                date(2026, 1, 1)
            )
            .is_empty()
        );
        assert!(
            generate_revenue_schedule(RecognitionMethod::Milestone, dec!(100), date(2026, 1, 1))
                .is_empty()
        );
    }

    #[test]
    fn contract_status_transitions() {
        use RevenueContractStatus as S;
        assert!(S::Draft.can_transition_to(S::Active));
        assert!(S::Draft.can_transition_to(S::Cancelled));
        assert!(S::Active.can_transition_to(S::Completed));
        assert!(S::Active.can_transition_to(S::Cancelled));
        assert!(!S::Draft.can_transition_to(S::Completed));
        assert!(!S::Completed.can_transition_to(S::Active));
        assert!(!S::Cancelled.can_transition_to(S::Active));
        assert!(!S::Active.can_transition_to(S::Draft));
        assert!(S::Completed.is_terminal());
        assert!(S::Cancelled.is_terminal());
        assert!(!S::Active.is_terminal());
    }

    #[test]
    fn entry_status_transition_is_one_way() {
        use RevenueEntryStatus as S;
        assert!(S::Deferred.can_transition_to(S::Recognized));
        assert!(!S::Recognized.can_transition_to(S::Deferred));
        assert!(!S::Deferred.can_transition_to(S::Deferred));
    }

    #[test]
    fn contract_totals_and_deferred_balance() {
        let c = contract(
            dec!(1200),
            vec![obligation(dec!(700), dec!(700)), obligation(dec!(500), dec!(100))],
        );
        assert_eq!(c.total_allocated(), dec!(1200));
        assert_eq!(c.total_recognized(), dec!(800));
        assert_eq!(c.deferred_balance(), dec!(400));
        assert!(!c.is_fully_recognized());

        let done = contract(dec!(1200), vec![obligation(dec!(1200), dec!(1200))]);
        assert!(done.is_fully_recognized());
        assert_eq!(done.deferred_balance(), dec!(0));
    }

    #[test]
    fn contract_validate_requires_allocation_to_sum_to_price() {
        let ok = contract(
            dec!(1000),
            vec![obligation(dec!(600), dec!(0)), obligation(dec!(400), dec!(0))],
        );
        assert!(ok.validate().is_ok());

        let bad = contract(
            dec!(1000),
            vec![obligation(dec!(600), dec!(0)), obligation(dec!(300), dec!(0))],
        );
        assert!(bad.validate().is_err());
    }

    #[test]
    fn obligation_validate_bounds() {
        let ok = obligation(dec!(500), dec!(250));
        assert!(ok.validate().is_ok());

        let mut over = obligation(dec!(500), dec!(600));
        assert!(over.validate().is_err());
        over.recognized_amount = dec!(-1);
        assert!(over.validate().is_err());

        let mut inverted = obligation(dec!(500), dec!(0));
        inverted.recognition_method =
            RecognitionMethod::RatableOverTime { start: date(2026, 6, 1), end: date(2026, 1, 1) };
        assert!(inverted.validate().is_err());

        let o = obligation(dec!(500), dec!(200));
        assert_eq!(o.deferred_amount(), dec!(300));
        assert!(!o.is_fully_recognized());
    }

    #[test]
    fn create_contract_validation() {
        let ok = CreateRevenueContract {
            contract_number: None,
            customer_id: Uuid::new_v4(),
            order_id: None,
            invoice_id: None,
            transaction_price: dec!(100),
            currency: None,
            effective_date: date(2026, 1, 1),
            obligations: vec![CreatePerformanceObligation {
                description: "License".into(),
                standalone_selling_price: None,
                allocated_amount: dec!(100),
                recognition_method: RecognitionMethod::PointInTime,
            }],
        };
        assert!(ok.validate().is_ok());

        let mut bad = ok.clone();
        bad.obligations[0].allocated_amount = dec!(90);
        assert!(bad.validate().is_err());

        let mut empty = ok.clone();
        empty.obligations.clear();
        assert!(empty.validate().is_err());

        let mut nil = ok;
        nil.customer_id = Uuid::nil();
        assert!(nil.validate().is_err());
    }

    #[test]
    fn schedule_recognized_and_deferred_totals() {
        let mut entries = generate_revenue_schedule(
            RecognitionMethod::RatableOverTime { start: date(2026, 1, 1), end: date(2026, 3, 31) },
            dec!(300),
            date(2026, 1, 1),
        );
        entries[0].status = RevenueEntryStatus::Recognized;
        let schedule = RevenueSchedule {
            obligation_id: Uuid::new_v4(),
            method: RecognitionMethod::RatableOverTime {
                start: date(2026, 1, 1),
                end: date(2026, 3, 31),
            },
            entries,
            total_amount: dec!(300),
        };
        assert_eq!(schedule.recognized_total(), dec!(100));
        assert_eq!(schedule.deferred_total(), dec!(200));
    }

    #[test]
    fn recognition_method_serde_round_trip() {
        let m =
            RecognitionMethod::RatableOverTime { start: date(2026, 1, 1), end: date(2026, 12, 31) };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("ratable_over_time"));
        let back: RecognitionMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert_eq!(RevenueEntryStatus::Deferred.to_string(), "deferred");
        assert_eq!(
            "recognized".parse::<RevenueEntryStatus>().unwrap(),
            RevenueEntryStatus::Recognized
        );
    }

    #[test]
    fn generates_contract_number_with_prefix() {
        let n = generate_revenue_contract_number();
        assert!(n.starts_with("RC-"));
        assert_ne!(n, generate_revenue_contract_number());
    }
}
