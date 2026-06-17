//! Production batch domain models
//!
//! A production batch groups one or more manufacturing work orders so they can
//! be scheduled, tracked, and reported on together. Its status is normally
//! derived from the aggregate state of its linked work orders.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::ProductionBatchId;
use strum::{Display, EnumString};
use uuid::Uuid;

/// Lifecycle status of a production batch.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum ProductionBatchStatus {
    /// Created, work not yet started.
    #[default]
    Planned,
    /// At least one work order is in progress.
    InProgress,
    /// All work orders complete.
    Completed,
    /// Batch was cancelled.
    Cancelled,
}

/// A batch grouping multiple work orders for coordinated production.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionBatch {
    /// Unique batch ID.
    pub id: ProductionBatchId,
    /// Human-readable batch name / number.
    pub name: String,
    /// Lifecycle status.
    pub status: ProductionBatchStatus,
    /// Optional supplier/vendor reference (e.g. for subcontracted batches).
    pub vendor_id: Option<Uuid>,
    /// Linked work order IDs.
    pub work_order_ids: Vec<Uuid>,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Scheduled start.
    pub scheduled_start: Option<DateTime<Utc>>,
    /// Scheduled completion.
    pub scheduled_end: Option<DateTime<Utc>>,
    /// When the batch was created.
    pub created_at: DateTime<Utc>,
    /// When the batch was last updated.
    pub updated_at: DateTime<Utc>,
}

impl ProductionBatch {
    /// Number of work orders linked to the batch.
    #[must_use]
    pub fn work_order_count(&self) -> usize {
        self.work_order_ids.len()
    }

    /// Whether the batch is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self.status, ProductionBatchStatus::Completed | ProductionBatchStatus::Cancelled)
    }

    /// Derive batch status from the aggregate state of its work orders.
    ///
    /// `total` is the work-order count, `completed` the number finished, and
    /// `any_in_progress` whether any has started. A cancelled batch stays
    /// cancelled.
    #[must_use]
    pub fn derive_status(
        &self,
        total: usize,
        completed: usize,
        any_in_progress: bool,
    ) -> ProductionBatchStatus {
        if self.status == ProductionBatchStatus::Cancelled {
            return ProductionBatchStatus::Cancelled;
        }
        if total > 0 && completed >= total {
            ProductionBatchStatus::Completed
        } else if any_in_progress || completed > 0 {
            ProductionBatchStatus::InProgress
        } else {
            ProductionBatchStatus::Planned
        }
    }
}

/// Input for creating a production batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProductionBatch {
    /// Batch name.
    pub name: String,
    /// Optional vendor reference.
    pub vendor_id: Option<Uuid>,
    /// Work orders to link at creation.
    #[serde(default)]
    pub work_order_ids: Vec<Uuid>,
    /// Notes.
    pub notes: Option<String>,
    /// Scheduled start.
    pub scheduled_start: Option<DateTime<Utc>>,
    /// Scheduled completion.
    pub scheduled_end: Option<DateTime<Utc>>,
}

/// Input for updating a production batch (partial).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateProductionBatch {
    /// Updated name.
    pub name: Option<String>,
    /// Updated vendor.
    pub vendor_id: Option<Uuid>,
    /// Updated status.
    pub status: Option<ProductionBatchStatus>,
    /// Updated notes.
    pub notes: Option<String>,
    /// Updated scheduled start.
    pub scheduled_start: Option<DateTime<Utc>>,
    /// Updated scheduled completion.
    pub scheduled_end: Option<DateTime<Utc>>,
}

/// Filter for listing production batches.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductionBatchFilter {
    /// Filter by status.
    pub status: Option<ProductionBatchStatus>,
    /// Filter by vendor.
    pub vendor_id: Option<Uuid>,
    /// Maximum results.
    pub limit: Option<u32>,
    /// Offset for pagination.
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_batch(status: ProductionBatchStatus, work_orders: Vec<Uuid>) -> ProductionBatch {
        ProductionBatch {
            id: ProductionBatchId::new(),
            name: "Batch 1".to_string(),
            status,
            vendor_id: None,
            work_order_ids: work_orders,
            notes: None,
            scheduled_start: None,
            scheduled_end: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn work_order_count_reflects_links() {
        let batch = make_batch(ProductionBatchStatus::Planned, vec![Uuid::nil(), Uuid::nil()]);
        assert_eq!(batch.work_order_count(), 2);
    }

    #[test]
    fn derive_completed_when_all_done() {
        let batch = make_batch(ProductionBatchStatus::InProgress, vec![]);
        assert_eq!(batch.derive_status(3, 3, false), ProductionBatchStatus::Completed);
    }

    #[test]
    fn derive_in_progress_when_some_active() {
        let batch = make_batch(ProductionBatchStatus::Planned, vec![]);
        assert_eq!(batch.derive_status(3, 1, true), ProductionBatchStatus::InProgress);
    }

    #[test]
    fn derive_planned_when_nothing_started() {
        let batch = make_batch(ProductionBatchStatus::Planned, vec![]);
        assert_eq!(batch.derive_status(3, 0, false), ProductionBatchStatus::Planned);
    }

    #[test]
    fn derive_status_cancelled_sticky() {
        let batch = make_batch(ProductionBatchStatus::Cancelled, vec![]);
        assert_eq!(batch.derive_status(3, 3, false), ProductionBatchStatus::Cancelled);
    }

    #[test]
    fn derive_status_empty_batch_is_planned() {
        let batch = make_batch(ProductionBatchStatus::Planned, vec![]);
        assert_eq!(batch.derive_status(0, 0, false), ProductionBatchStatus::Planned);
    }

    #[test]
    fn terminal_states() {
        assert!(make_batch(ProductionBatchStatus::Completed, vec![]).is_terminal());
        assert!(make_batch(ProductionBatchStatus::Cancelled, vec![]).is_terminal());
        assert!(!make_batch(ProductionBatchStatus::InProgress, vec![]).is_terminal());
    }

    #[test]
    fn status_roundtrip() {
        for s in [
            ProductionBatchStatus::Planned,
            ProductionBatchStatus::InProgress,
            ProductionBatchStatus::Completed,
            ProductionBatchStatus::Cancelled,
        ] {
            let parsed: ProductionBatchStatus = s.to_string().parse().unwrap();
            assert_eq!(parsed, s);
        }
    }
}
