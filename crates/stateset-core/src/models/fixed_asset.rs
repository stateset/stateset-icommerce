//! Fixed asset register domain models
//!
//! Supports:
//! - Fixed asset records with acquisition cost, salvage value, and useful life
//! - Depreciation methods (straight-line, declining balance, units of production)
//! - Depreciation schedule generation with exact final-period plug
//! - Asset lifecycle status transitions and disposal gain/loss calculation

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::CurrencyCode;
use strum::{Display, EnumString};
use uuid::Uuid;

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder};

/// Monetary scale (2 decimal places) used for depreciation rounding.
const MONEY_SCALE: u32 = 2;

// ============================================================================
// Enums
// ============================================================================

/// Fixed asset category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FixedAssetCategory {
    /// Land held by the business (typically not depreciated).
    Land,
    /// Buildings and structural improvements.
    Building,
    /// Machinery and production equipment.
    Machinery,
    /// Office or warehouse equipment.
    Equipment,
    /// Cars, trucks, and other vehicles.
    Vehicle,
    /// Office furniture and fixtures.
    FurnitureAndFixtures,
    /// Computer hardware.
    ComputerHardware,
    /// Purchased or capitalized software.
    Software,
    /// Improvements made to leased property.
    LeaseholdImprovement,
    /// Assets not classified elsewhere.
    Other,
}

/// Depreciation method for a fixed asset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DepreciationMethod {
    /// Equal expense in every period over the useful life.
    StraightLine,
    /// Accelerated method applying a fixed rate to declining book value.
    DecliningBalance {
        /// Periodic rate applied to opening book value (e.g. `0.2` for 20%).
        rate: Decimal,
    },
    /// Expense proportional to units produced or consumed.
    UnitsOfProduction,
}

/// Fixed asset lifecycle status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FixedAssetStatus {
    /// Asset record is being prepared; not yet in service.
    #[default]
    Draft,
    /// Asset is in service and depreciating.
    InService,
    /// Accumulated depreciation has reached the depreciable base.
    FullyDepreciated,
    /// Asset has been sold or otherwise disposed of.
    Disposed,
    /// Asset has been written off (no proceeds).
    WrittenOff,
}

impl FixedAssetStatus {
    /// Returns true if a transition from `self` to `next` is allowed.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::InService)
                | (Self::InService, Self::FullyDepreciated)
                | (Self::InService | Self::FullyDepreciated, Self::Disposed | Self::WrittenOff)
        )
    }

    /// Returns true if this status is terminal (no further transitions).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Disposed | Self::WrittenOff)
    }
}

/// Depreciation entry status
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DepreciationEntryStatus {
    /// Entry has been scheduled but not yet posted to the GL.
    #[default]
    Scheduled,
    /// Entry has been posted to the general ledger.
    Posted,
}

// ============================================================================
// Core Structs
// ============================================================================

/// A fixed asset register entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixedAsset {
    /// Unique identifier for this asset.
    pub id: Uuid,
    /// Human-readable asset reference number (e.g. `"FA-20240101-ABCD1234"`).
    pub asset_number: String,
    /// Asset name.
    pub name: String,
    /// Optional description of the asset.
    pub description: Option<String>,
    /// Asset category.
    pub category: FixedAssetCategory,
    /// Date the asset was acquired.
    pub acquisition_date: NaiveDate,
    /// Original acquisition cost.
    pub acquisition_cost: Decimal,
    /// Estimated residual value at end of useful life.
    pub salvage_value: Decimal,
    /// Useful life in months.
    pub useful_life_months: u32,
    /// Depreciation method applied to this asset.
    pub depreciation_method: DepreciationMethod,
    /// Current lifecycle status.
    pub status: FixedAssetStatus,
    /// Date the asset was placed in service.
    pub in_service_date: Option<NaiveDate>,
    /// Physical location reference (e.g. warehouse or site id).
    pub location_id: Option<Uuid>,
    /// GL asset account this asset is capitalized to.
    pub asset_account_id: Option<Uuid>,
    /// GL accumulated depreciation (contra-asset) account.
    pub accumulated_depreciation_account_id: Option<Uuid>,
    /// GL depreciation expense account.
    pub depreciation_expense_account_id: Option<Uuid>,
    /// Depreciation accumulated to date.
    pub accumulated_depreciation: Decimal,
    /// Currency of all monetary amounts on this asset.
    pub currency: CurrencyCode,
    /// Disposal details, populated when the asset is disposed.
    pub disposal: Option<AssetDisposal>,
    /// Timestamp of record creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

impl FixedAsset {
    /// Depreciable base: acquisition cost less salvage value.
    #[must_use]
    pub fn depreciable_base(&self) -> Decimal {
        self.acquisition_cost - self.salvage_value
    }

    /// Current net book value: cost less accumulated depreciation.
    #[must_use]
    pub fn book_value(&self) -> Decimal {
        self.acquisition_cost - self.accumulated_depreciation
    }

    /// Returns true if accumulated depreciation has reached the depreciable base.
    #[must_use]
    pub fn is_fully_depreciated(&self) -> bool {
        self.accumulated_depreciation >= self.depreciable_base()
    }

    /// Gain (positive) or loss (negative) on disposal for the given proceeds.
    #[must_use]
    pub fn disposal_gain_loss(&self, proceeds: Decimal) -> Decimal {
        proceeds - self.book_value()
    }
}

/// Disposal details for a fixed asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetDisposal {
    /// Date of disposal.
    pub disposal_date: NaiveDate,
    /// Proceeds received on disposal (zero for a write-off).
    pub proceeds: Decimal,
    /// Net book value at time of disposal.
    pub book_value_at_disposal: Decimal,
    /// Gain (positive) or loss (negative): proceeds less book value.
    pub gain_loss: Decimal,
    /// Optional narrative for the disposal.
    pub notes: Option<String>,
}

impl AssetDisposal {
    /// Build a disposal record, computing the gain/loss from proceeds and book value.
    #[must_use]
    pub fn new(
        disposal_date: NaiveDate,
        proceeds: Decimal,
        book_value_at_disposal: Decimal,
        notes: Option<String>,
    ) -> Self {
        Self {
            disposal_date,
            proceeds,
            book_value_at_disposal,
            gain_loss: proceeds - book_value_at_disposal,
            notes,
        }
    }
}

/// A single period's depreciation entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepreciationEntry {
    /// 1-based period number within the schedule.
    pub period: u32,
    /// Depreciation expense for this period.
    pub amount: Decimal,
    /// Accumulated depreciation through this period.
    pub accumulated: Decimal,
    /// Net book value at the end of this period.
    pub book_value: Decimal,
    /// Posting status of this entry.
    pub status: DepreciationEntryStatus,
}

/// A full depreciation schedule for a fixed asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepreciationSchedule {
    /// Asset this schedule belongs to.
    pub asset_id: Uuid,
    /// Method used to generate the schedule.
    pub method: DepreciationMethod,
    /// Per-period entries in order.
    pub entries: Vec<DepreciationEntry>,
    /// Total scheduled depreciation (equals cost less salvage).
    pub total_depreciation: Decimal,
}

// ============================================================================
// Schedule Generation
// ============================================================================

/// Generate a monthly depreciation schedule for the given method.
///
/// Amounts are rounded to 2 decimal places; the final period is plugged so
/// that accumulated depreciation equals exactly `cost - salvage`.
///
/// Returns an empty vector when `useful_life_months` is zero or the
/// depreciable base is not positive. `UnitsOfProduction` cannot be scheduled
/// by time alone and returns an empty vector.
#[must_use]
pub fn generate_depreciation_schedule(
    method: DepreciationMethod,
    cost: Decimal,
    salvage: Decimal,
    useful_life_months: u32,
) -> Vec<DepreciationEntry> {
    let base = cost - salvage;
    if useful_life_months == 0 || base <= Decimal::ZERO {
        return Vec::new();
    }

    let mut entries = Vec::with_capacity(useful_life_months as usize);
    let mut accumulated = Decimal::ZERO;

    for period in 1..=useful_life_months {
        let remaining = base - accumulated;
        let raw = if period == useful_life_months {
            // Final-period plug: force accumulated == cost - salvage exactly.
            remaining
        } else {
            match method {
                DepreciationMethod::StraightLine => {
                    (base / Decimal::from(useful_life_months)).round_dp(MONEY_SCALE)
                }
                DepreciationMethod::DecliningBalance { rate } => {
                    // Rate applied to opening book value; capped below so the
                    // asset never depreciates past the salvage value.
                    ((cost - accumulated) * rate).round_dp(MONEY_SCALE)
                }
                DepreciationMethod::UnitsOfProduction => return Vec::new(),
            }
        };
        let amount = raw.min(remaining).max(Decimal::ZERO).round_dp(MONEY_SCALE);
        accumulated += amount;
        entries.push(DepreciationEntry {
            period,
            amount,
            accumulated,
            book_value: cost - accumulated,
            status: DepreciationEntryStatus::Scheduled,
        });
    }

    entries
}

// ============================================================================
// Input & Filter Types
// ============================================================================

/// Input for creating a fixed asset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFixedAsset {
    pub asset_number: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub category: FixedAssetCategory,
    pub acquisition_date: NaiveDate,
    pub acquisition_cost: Decimal,
    pub salvage_value: Decimal,
    pub useful_life_months: u32,
    pub depreciation_method: DepreciationMethod,
    pub in_service_date: Option<NaiveDate>,
    pub location_id: Option<Uuid>,
    pub asset_account_id: Option<Uuid>,
    pub accumulated_depreciation_account_id: Option<Uuid>,
    pub depreciation_expense_account_id: Option<Uuid>,
    pub currency: Option<CurrencyCode>,
}

impl Validate for CreateFixedAsset {
    /// Validate a fixed asset create request.
    ///
    /// Requires a non-empty name, positive acquisition cost, non-negative
    /// salvage value not exceeding cost, a positive useful life, and a
    /// declining-balance rate strictly between 0 and 1 when applicable.
    fn validate(&self) -> Result<()> {
        let builder = ValidationBuilder::new()
            .required("name", &self.name)
            .positive("acquisition_cost", self.acquisition_cost)
            .non_negative("salvage_value", self.salvage_value)
            .check(
                "salvage_value",
                self.salvage_value <= self.acquisition_cost,
                "must not exceed acquisition_cost",
            )
            .check("useful_life_months", self.useful_life_months > 0, "must be positive");

        let builder = match self.depreciation_method {
            DepreciationMethod::DecliningBalance { rate } => builder.check(
                "depreciation_method.rate",
                rate > Decimal::ZERO && rate < Decimal::ONE,
                "must be between 0 and 1 exclusive",
            ),
            _ => builder,
        };

        builder.build()
    }
}

/// Input for updating a fixed asset
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateFixedAsset {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category: Option<FixedAssetCategory>,
    pub salvage_value: Option<Decimal>,
    pub useful_life_months: Option<u32>,
    pub in_service_date: Option<NaiveDate>,
    pub location_id: Option<Uuid>,
    pub asset_account_id: Option<Uuid>,
    pub accumulated_depreciation_account_id: Option<Uuid>,
    pub depreciation_expense_account_id: Option<Uuid>,
}

impl Validate for UpdateFixedAsset {
    /// Validate a fixed asset update request.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .required_if_present("name", self.name.as_deref())
            .non_negative("salvage_value", self.salvage_value.unwrap_or(Decimal::ZERO))
            .check(
                "useful_life_months",
                self.useful_life_months.is_none_or(|m| m > 0),
                "must be positive",
            )
            .build()
    }
}

/// Filter for listing fixed assets
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FixedAssetFilter {
    pub category: Option<FixedAssetCategory>,
    pub status: Option<FixedAssetStatus>,
    pub location_id: Option<Uuid>,
    pub acquired_from: Option<NaiveDate>,
    pub acquired_to: Option<NaiveDate>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `created_at` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
}

/// Generate a fixed asset number using a timestamp
#[must_use]
pub fn generate_asset_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let suffix = Uuid::new_v4().simple().to_string();
    format!("FA-{}-{}", timestamp, &suffix[..8])
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn create_input() -> CreateFixedAsset {
        CreateFixedAsset {
            asset_number: None,
            name: "Forklift".into(),
            description: None,
            category: FixedAssetCategory::Machinery,
            acquisition_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            acquisition_cost: dec!(10000),
            salvage_value: dec!(1000),
            useful_life_months: 36,
            depreciation_method: DepreciationMethod::StraightLine,
            in_service_date: None,
            location_id: None,
            asset_account_id: None,
            accumulated_depreciation_account_id: None,
            depreciation_expense_account_id: None,
            currency: None,
        }
    }

    fn asset() -> FixedAsset {
        FixedAsset {
            id: Uuid::new_v4(),
            asset_number: "FA-1".into(),
            name: "Forklift".into(),
            description: None,
            category: FixedAssetCategory::Machinery,
            acquisition_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            acquisition_cost: dec!(10000),
            salvage_value: dec!(1000),
            useful_life_months: 36,
            depreciation_method: DepreciationMethod::StraightLine,
            status: FixedAssetStatus::InService,
            in_service_date: NaiveDate::from_ymd_opt(2026, 1, 1),
            location_id: None,
            asset_account_id: None,
            accumulated_depreciation_account_id: None,
            depreciation_expense_account_id: None,
            accumulated_depreciation: dec!(3000),
            currency: CurrencyCode::default(),
            disposal: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn straight_line_schedule_sums_to_depreciable_base() {
        let entries = generate_depreciation_schedule(
            DepreciationMethod::StraightLine,
            dec!(10000),
            dec!(1000),
            36,
        );
        assert_eq!(entries.len(), 36);
        let total: Decimal = entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(9000));
        let last = entries.last().unwrap();
        assert_eq!(last.accumulated, dec!(9000));
        assert_eq!(last.book_value, dec!(1000));
    }

    #[test]
    fn straight_line_final_period_plug_handles_rounding() {
        // 1000 / 7 = 142.857... -> rounds to 142.86; final period plugs the drift.
        let entries = generate_depreciation_schedule(
            DepreciationMethod::StraightLine,
            dec!(1000),
            dec!(0),
            7,
        );
        let total: Decimal = entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(1000));
        assert_eq!(entries[0].amount, dec!(142.86));
        assert_ne!(entries[6].amount, entries[0].amount);
        assert_eq!(entries.last().unwrap().accumulated, dec!(1000));
    }

    #[test]
    fn declining_balance_schedule_sums_exactly() {
        let entries = generate_depreciation_schedule(
            DepreciationMethod::DecliningBalance { rate: dec!(0.2) },
            dec!(10000),
            dec!(500),
            12,
        );
        assert_eq!(entries.len(), 12);
        let total: Decimal = entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(9500));
        assert_eq!(entries.last().unwrap().book_value, dec!(500));
        // First period is 20% of cost.
        assert_eq!(entries[0].amount, dec!(2000));
        // Second period is 20% of remaining book value.
        assert_eq!(entries[1].amount, dec!(1600));
    }

    #[test]
    fn declining_balance_never_depreciates_below_salvage() {
        let entries = generate_depreciation_schedule(
            DepreciationMethod::DecliningBalance { rate: dec!(0.5) },
            dec!(1000),
            dec!(800),
            6,
        );
        let total: Decimal = entries.iter().map(|e| e.amount).sum();
        assert_eq!(total, dec!(200));
        assert!(entries.iter().all(|e| e.book_value >= dec!(800)));
    }

    #[test]
    fn schedule_amounts_are_monotone_accumulated_and_two_dp() {
        let entries = generate_depreciation_schedule(
            DepreciationMethod::StraightLine,
            dec!(999.99),
            dec!(0.33),
            13,
        );
        let mut prev = Decimal::ZERO;
        for e in &entries {
            assert!(e.accumulated >= prev);
            assert!(e.amount.scale() <= 2);
            prev = e.accumulated;
        }
        assert_eq!(prev, dec!(999.66));
    }

    #[test]
    fn empty_schedule_for_zero_life_or_nonpositive_base() {
        assert!(
            generate_depreciation_schedule(
                DepreciationMethod::StraightLine,
                dec!(1000),
                dec!(0),
                0
            )
            .is_empty()
        );
        assert!(
            generate_depreciation_schedule(
                DepreciationMethod::StraightLine,
                dec!(1000),
                dec!(1000),
                12
            )
            .is_empty()
        );
        assert!(
            generate_depreciation_schedule(
                DepreciationMethod::UnitsOfProduction,
                dec!(1000),
                dec!(0),
                12
            )
            .is_empty()
        );
    }

    #[test]
    fn status_transitions() {
        use FixedAssetStatus as S;
        assert!(S::Draft.can_transition_to(S::InService));
        assert!(S::InService.can_transition_to(S::FullyDepreciated));
        assert!(S::InService.can_transition_to(S::Disposed));
        assert!(S::InService.can_transition_to(S::WrittenOff));
        assert!(S::FullyDepreciated.can_transition_to(S::Disposed));
        assert!(S::FullyDepreciated.can_transition_to(S::WrittenOff));
        // Invalid transitions
        assert!(!S::Draft.can_transition_to(S::Disposed));
        assert!(!S::Draft.can_transition_to(S::FullyDepreciated));
        assert!(!S::Disposed.can_transition_to(S::InService));
        assert!(!S::WrittenOff.can_transition_to(S::InService));
        assert!(!S::InService.can_transition_to(S::Draft));
        assert!(S::Disposed.is_terminal());
        assert!(S::WrittenOff.is_terminal());
        assert!(!S::InService.is_terminal());
    }

    #[test]
    fn book_value_and_disposal_gain_loss() {
        let a = asset();
        assert_eq!(a.depreciable_base(), dec!(9000));
        assert_eq!(a.book_value(), dec!(7000));
        assert!(!a.is_fully_depreciated());
        assert_eq!(a.disposal_gain_loss(dec!(7500)), dec!(500));
        assert_eq!(a.disposal_gain_loss(dec!(6000)), dec!(-1000));

        let d = AssetDisposal::new(
            NaiveDate::from_ymd_opt(2027, 6, 30).unwrap(),
            dec!(6500),
            a.book_value(),
            None,
        );
        assert_eq!(d.gain_loss, dec!(-500));
    }

    #[test]
    fn fully_depreciated_detection() {
        let mut a = asset();
        a.accumulated_depreciation = dec!(9000);
        assert!(a.is_fully_depreciated());
        assert_eq!(a.book_value(), dec!(1000));
    }

    #[test]
    fn create_validation_accepts_valid_input() {
        assert!(create_input().validate().is_ok());
    }

    #[test]
    fn create_validation_rejects_bad_input() {
        let mut input = create_input();
        input.name = String::new();
        assert!(input.validate().is_err());

        let mut input = create_input();
        input.acquisition_cost = dec!(0);
        assert!(input.validate().is_err());

        let mut input = create_input();
        input.salvage_value = dec!(20000);
        assert!(input.validate().is_err());

        let mut input = create_input();
        input.useful_life_months = 0;
        assert!(input.validate().is_err());

        let mut input = create_input();
        input.depreciation_method = DepreciationMethod::DecliningBalance { rate: dec!(1.5) };
        assert!(input.validate().is_err());
    }

    #[test]
    fn update_validation() {
        assert!(UpdateFixedAsset::default().validate().is_ok());
        let bad = UpdateFixedAsset { useful_life_months: Some(0), ..Default::default() };
        assert!(bad.validate().is_err());
        let bad = UpdateFixedAsset { name: Some(String::new()), ..Default::default() };
        assert!(bad.validate().is_err());
    }

    #[test]
    fn enum_serde_round_trip() {
        let m = DepreciationMethod::DecliningBalance { rate: dec!(0.25) };
        let json = serde_json::to_string(&m).unwrap();
        assert!(json.contains("declining_balance"));
        let back: DepreciationMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(back, m);
        assert_eq!(FixedAssetStatus::InService.to_string(), "in_service");
        assert_eq!("in_service".parse::<FixedAssetStatus>().unwrap(), FixedAssetStatus::InService);
    }

    #[test]
    fn generates_asset_number_with_prefix() {
        let n = generate_asset_number();
        assert!(n.starts_with("FA-"));
        assert_ne!(n, generate_asset_number());
    }
}
