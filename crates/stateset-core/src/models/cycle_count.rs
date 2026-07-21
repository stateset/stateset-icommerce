//! Cycle count domain models
//!
//! A cycle count is a scheduled physical inventory count of a warehouse (or a
//! single location within it). Counts move through a small state machine
//! (`draft` → `in_progress` → `completed` / `cancelled`); completing a count
//! computes per-line variances and applies matching inventory adjustments.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

use crate::{CommerceError, Result};

// ============================================================================
// Status
// ============================================================================

/// Status of a cycle count
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CycleCountStatus {
    /// Created but counting has not started
    #[default]
    Draft,
    /// Counting is underway
    InProgress,
    /// Count finished; variances applied
    Completed,
    /// Count abandoned; no adjustments applied
    Cancelled,
}

impl CycleCountStatus {
    /// Returns true if a transition from `self` to `next` is allowed.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::InProgress | Self::Cancelled)
                | (Self::InProgress, Self::Completed | Self::Cancelled)
        )
    }

    /// Returns true if this status is terminal (no further transitions).
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }
}

// ============================================================================
// Cycle count + lines
// ============================================================================

/// A cycle count header
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CycleCount {
    pub id: Uuid,
    pub warehouse_id: i32,
    /// Optional single location scope; `None` counts across the warehouse.
    pub location_id: Option<i32>,
    pub status: CycleCountStatus,
    pub scheduled_date: Option<DateTime<Utc>>,
    pub counted_by: Option<String>,
    pub lines: Vec<CycleCountLine>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// A single SKU line on a cycle count
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CycleCountLine {
    pub id: Uuid,
    pub cycle_count_id: Uuid,
    pub sku: String,
    pub lot_id: Option<Uuid>,
    /// System quantity expected at count time.
    pub expected_quantity: Decimal,
    /// Physically counted quantity (recorded while in progress).
    pub counted_quantity: Option<Decimal>,
    /// `counted_quantity - expected_quantity`, set once counted.
    pub variance: Option<Decimal>,
}

// ============================================================================
// DTOs
// ============================================================================

/// Input line for creating a cycle count
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CreateCycleCountLine {
    pub sku: String,
    pub lot_id: Option<Uuid>,
    pub expected_quantity: Decimal,
}

/// Input for creating a cycle count
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct CreateCycleCount {
    pub warehouse_id: i32,
    pub location_id: Option<i32>,
    pub scheduled_date: Option<DateTime<Utc>>,
    pub counted_by: Option<String>,
    pub lines: Vec<CreateCycleCountLine>,
}

impl CreateCycleCount {
    /// Validate the input before persisting.
    ///
    /// # Errors
    /// Returns [`CommerceError::ValidationError`] when the warehouse id is not
    /// positive, no lines are supplied, a line has an empty SKU, or a line has
    /// a negative expected quantity.
    pub fn validate(&self) -> Result<()> {
        if self.warehouse_id <= 0 {
            return Err(CommerceError::ValidationError(
                "cycle count warehouse_id must be positive".into(),
            ));
        }
        if self.lines.is_empty() {
            return Err(CommerceError::ValidationError(
                "cycle count requires at least one line".into(),
            ));
        }
        for (i, line) in self.lines.iter().enumerate() {
            if line.sku.trim().is_empty() {
                return Err(CommerceError::ValidationError(format!(
                    "cycle count line {i} has an empty sku"
                )));
            }
            if line.expected_quantity < Decimal::ZERO {
                return Err(CommerceError::ValidationError(format!(
                    "cycle count line {i} has a negative expected_quantity"
                )));
            }
        }
        Ok(())
    }
}

/// Input for updating a draft cycle count
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct UpdateCycleCount {
    pub scheduled_date: Option<DateTime<Utc>>,
    pub counted_by: Option<String>,
}

/// A recorded physical count for one line
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RecordCycleCountLine {
    pub sku: String,
    pub lot_id: Option<Uuid>,
    pub counted_quantity: Decimal,
}

impl RecordCycleCountLine {
    /// Validate a recorded count.
    ///
    /// # Errors
    /// Returns [`CommerceError::ValidationError`] on an empty SKU or negative
    /// counted quantity.
    pub fn validate(&self) -> Result<()> {
        if self.sku.trim().is_empty() {
            return Err(CommerceError::ValidationError("recorded count has an empty sku".into()));
        }
        if self.counted_quantity < Decimal::ZERO {
            return Err(CommerceError::ValidationError(
                "counted_quantity cannot be negative".into(),
            ));
        }
        Ok(())
    }
}

/// Filter for listing cycle counts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CycleCountFilter {
    pub warehouse_id: Option<i32>,
    pub location_id: Option<i32>,
    pub status: Option<CycleCountStatus>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `created_at` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    #[test]
    fn status_transitions() {
        use CycleCountStatus as S;
        assert!(S::Draft.can_transition_to(S::InProgress));
        assert!(S::Draft.can_transition_to(S::Cancelled));
        assert!(S::InProgress.can_transition_to(S::Completed));
        assert!(S::InProgress.can_transition_to(S::Cancelled));
        // Invalid transitions
        assert!(!S::Draft.can_transition_to(S::Completed));
        assert!(!S::Draft.can_transition_to(S::Draft));
        assert!(!S::InProgress.can_transition_to(S::Draft));
        assert!(!S::Completed.can_transition_to(S::Cancelled));
        assert!(!S::Completed.can_transition_to(S::InProgress));
        assert!(!S::Cancelled.can_transition_to(S::InProgress));
        assert!(S::Completed.is_terminal());
        assert!(S::Cancelled.is_terminal());
        assert!(!S::Draft.is_terminal());
        assert!(!S::InProgress.is_terminal());
    }

    #[test]
    fn status_display_from_str_round_trip() {
        for s in [
            CycleCountStatus::Draft,
            CycleCountStatus::InProgress,
            CycleCountStatus::Completed,
            CycleCountStatus::Cancelled,
        ] {
            assert_eq!(CycleCountStatus::from_str(&s.to_string()), Ok(s));
        }
        assert_eq!(CycleCountStatus::InProgress.to_string(), "in_progress");
        assert_eq!(CycleCountStatus::from_str("IN_PROGRESS"), Ok(CycleCountStatus::InProgress));
        assert!(CycleCountStatus::from_str("counting").is_err());
    }

    #[test]
    fn create_validate_rejects_bad_input() {
        let ok = CreateCycleCount {
            warehouse_id: 1,
            lines: vec![CreateCycleCountLine {
                sku: "SKU-1".into(),
                lot_id: None,
                expected_quantity: dec!(5),
            }],
            ..Default::default()
        };
        assert!(ok.validate().is_ok());

        let mut bad = ok.clone();
        bad.warehouse_id = 0;
        assert!(bad.validate().is_err());

        let mut bad = ok.clone();
        bad.lines.clear();
        assert!(bad.validate().is_err());

        let mut bad = ok.clone();
        bad.lines[0].sku = "  ".into();
        assert!(bad.validate().is_err());

        let mut bad = ok;
        bad.lines[0].expected_quantity = dec!(-1);
        assert!(bad.validate().is_err());
    }

    #[test]
    fn record_line_validate() {
        let ok =
            RecordCycleCountLine { sku: "SKU-1".into(), lot_id: None, counted_quantity: dec!(3) };
        assert!(ok.validate().is_ok());
        let mut bad = ok.clone();
        bad.sku = String::new();
        assert!(bad.validate().is_err());
        let mut bad = ok;
        bad.counted_quantity = dec!(-0.5);
        assert!(bad.validate().is_err());
    }
}
