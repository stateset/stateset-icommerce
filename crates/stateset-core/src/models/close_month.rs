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
    /// Refuse to write anything when any step reports warnings.
    ///
    /// With this set, a wet run first evaluates the close as a dry run: if
    /// that evaluation reports warnings the call returns
    /// [`crate::CommerceError::ValidationError`] before writing anything.
    /// Warnings that can only surface while posting (e.g. a post that fails
    /// after candidates were computed) abort the run before the final
    /// period close instead. Defaults to `false` (historical lenient mode:
    /// warnings are collected on the report and the close proceeds).
    #[serde(default)]
    pub fail_on_warnings: bool,
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
    /// Number of warnings that record a per-item *failure* (an asset that
    /// could not post, an obligation that could not be recognized), as
    /// opposed to informational skip notes (unsupported backend, nothing to
    /// do). Skipped steps and dry-run candidate computations report `0`
    /// unless an item actually failed.
    #[serde(default)]
    pub failed_item_count: u64,
}

impl CloseMonthStepReport {
    /// A step that was skipped, optionally with a note explaining why.
    ///
    /// Skip notes are informational, never item failures, so the failed
    /// count is always `0`.
    #[must_use]
    pub fn skipped(warning: Option<String>) -> Self {
        Self {
            status: CloseMonthStepStatus::Skipped,
            entry_count: 0,
            total_amount: Decimal::ZERO,
            warnings: warning.into_iter().collect(),
            failed_item_count: 0,
        }
    }

    /// Number of warning strings on this step.
    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }

    /// Whether any per-item failure was recorded on this step.
    #[must_use]
    pub const fn has_failures(&self) -> bool {
        self.failed_item_count > 0
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

impl CloseMonthReport {
    /// Total warning strings across all four steps.
    #[must_use]
    pub fn total_warnings(&self) -> usize {
        self.depreciation.warning_count()
            + self.revenue_recognition.warning_count()
            + self.fx_revaluation.warning_count()
            + self.period_close.warning_count()
    }

    /// Total per-item failures across all four steps.
    #[must_use]
    pub const fn total_failures(&self) -> u64 {
        self.depreciation.failed_item_count
            + self.revenue_recognition.failed_item_count
            + self.fx_revaluation.failed_item_count
            + self.period_close.failed_item_count
    }

    /// Whether any step reported any warning.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.total_warnings() > 0
    }
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
        assert!(!options.fail_on_warnings);
        assert!(options.closed_by.is_none());
    }

    #[test]
    fn skipped_steps_report_no_failures() {
        let step = CloseMonthStepReport::skipped(Some("why".into()));
        assert_eq!(step.warning_count(), 1);
        assert!(!step.has_failures());
        assert_eq!(step.failed_item_count, 0);
    }

    #[test]
    fn report_totals_aggregate_steps() {
        fn step(warnings: usize, failed: u64) -> CloseMonthStepReport {
            CloseMonthStepReport {
                status: CloseMonthStepStatus::Executed,
                entry_count: 0,
                total_amount: Decimal::ZERO,
                warnings: vec!["w".to_string(); warnings],
                failed_item_count: failed,
            }
        }
        let report = CloseMonthReport {
            period_id: Uuid::nil(),
            period_name: "p".into(),
            dry_run: true,
            depreciation: step(2, 1),
            revenue_recognition: step(0, 0),
            fx_revaluation: step(1, 0),
            period_close: step(0, 0),
            closing_entry: None,
            period_status: PeriodStatus::Open,
        };
        assert_eq!(report.total_warnings(), 3);
        assert_eq!(report.total_failures(), 1);
        assert!(report.has_warnings());
    }

    #[test]
    fn old_json_without_new_fields_still_deserializes() {
        // Back-compat: reports/options persisted before `failed_item_count`
        // and `fail_on_warnings` existed must keep parsing via serde defaults.
        let step: CloseMonthStepReport = serde_json::from_value(serde_json::json!({
            "status": "executed",
            "entry_count": 1,
            "total_amount": "100.00",
            "warnings": [],
        }))
        .expect("step json");
        assert_eq!(step.failed_item_count, 0);
        let options: CloseMonthOptions = serde_json::from_value(serde_json::json!({
            "dry_run": false,
            "skip_depreciation": false,
            "skip_revenue_recognition": false,
            "skip_fx_revaluation": false,
            "skip_period_close": false,
            "closed_by": null,
        }))
        .expect("options json");
        assert!(!options.fail_on_warnings);
    }
}
