//! Month-end close orchestration report types
//!
//! The "close the month" operation runs, in order: scheduled fixed-asset
//! depreciation, revenue recognition through period end, FX revaluation as of
//! period end, and the period close itself (closing entries + close period).
//! These types describe the options controlling the run and the per-step
//! report it returns. Per-item failures (e.g. a single asset that cannot post)
//! are collected as warnings and never abort the close.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use super::general_ledger::{JournalEntry, PeriodStatus};

/// Options controlling a month-end close run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CloseMonthOptions {
    /// Compute what WOULD happen (counts and amounts per step) without
    /// writing anything.
    pub dry_run: bool,
    /// Skip posting scheduled fixed-asset depreciation.
    pub skip_depreciation: bool,
    /// Skip recognizing deferred revenue through period end.
    pub skip_revenue_recognition: bool,
    /// Skip FX revaluation of foreign-currency accounts.
    pub skip_fx_revaluation: bool,
    /// Skip the final period close (closing entries + close period).
    pub skip_period_close: bool,
    /// Actor recorded as the closer. Defaults to `system`.
    pub closed_by: Option<String>,
}

/// Outcome of one step in a month-end close run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CloseMonthStepStatus {
    /// The step ran and wrote its entries.
    Executed,
    /// The step was skipped (by flag, missing capability, or nothing to do
    /// that could be attempted, e.g. no FX account configured).
    Skipped,
    /// Candidates were computed but nothing was written (dry run).
    DryRun,
}

impl fmt::Display for CloseMonthStepStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Executed => write!(f, "executed"),
            Self::Skipped => write!(f, "skipped"),
            Self::DryRun => write!(f, "dry_run"),
        }
    }
}

/// Per-step detail of a month-end close run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseMonthStepReport {
    /// Whether the step executed, was skipped, or ran in dry-run mode.
    pub status: CloseMonthStepStatus,
    /// Number of entries posted (or that would be posted in a dry run):
    /// depreciation schedule entries, revenue schedule entries, revaluation
    /// journal entries, or closing entries.
    pub entry_count: u64,
    /// Total amount across those entries (depreciation posted, revenue
    /// recognized, net unrealized FX gain/loss, or closing entry debits).
    pub total_amount: Decimal,
    /// Per-item failures and notes that did not abort the close.
    pub warnings: Vec<String>,
}

impl CloseMonthStepReport {
    /// A step that was skipped, optionally with a note explaining why.
    #[must_use]
    pub fn skipped(warning: Option<String>) -> Self {
        Self {
            status: CloseMonthStepStatus::Skipped,
            entry_count: 0,
            total_amount: Decimal::ZERO,
            warnings: warning.into_iter().collect(),
        }
    }
}

/// Report returned by the month-end close orchestration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseMonthReport {
    /// Period that was closed (or evaluated in dry-run mode).
    pub period_id: Uuid,
    /// Denormalized period name.
    pub period_name: String,
    /// Whether this was a dry run (nothing was written).
    pub dry_run: bool,
    /// Step 1: scheduled fixed-asset depreciation due through period end.
    pub depreciation: CloseMonthStepReport,
    /// Step 2: deferred revenue recognized through period end.
    pub revenue_recognition: CloseMonthStepReport,
    /// Step 3: FX revaluation of foreign-currency accounts as of period end.
    pub fx_revaluation: CloseMonthStepReport,
    /// Step 4: closing entries + close period.
    pub period_close: CloseMonthStepReport,
    /// The posted closing entry; `None` for dry runs, skipped closes, or a
    /// period with nothing to close.
    pub closing_entry: Option<JournalEntry>,
    /// Period status after the run (`Closed` after a real close).
    pub period_status: PeriodStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&CloseMonthStepStatus::DryRun).expect("serialize"),
            "\"dry_run\""
        );
        assert_eq!(CloseMonthStepStatus::Executed.to_string(), "executed");
    }

    #[test]
    fn skipped_helper_collects_warning() {
        let step = CloseMonthStepReport::skipped(Some("why".into()));
        assert_eq!(step.status, CloseMonthStepStatus::Skipped);
        assert_eq!(step.entry_count, 0);
        assert!(step.total_amount.is_zero());
        assert_eq!(step.warnings, vec!["why".to_string()]);
        assert!(CloseMonthStepReport::skipped(None).warnings.is_empty());
    }

    #[test]
    fn options_default_is_full_wet_run() {
        let options = CloseMonthOptions::default();
        assert!(!options.dry_run);
        assert!(!options.skip_depreciation);
        assert!(!options.skip_revenue_recognition);
        assert!(!options.skip_fx_revaluation);
        assert!(!options.skip_period_close);
        assert!(options.closed_by.is_none());
    }
}
