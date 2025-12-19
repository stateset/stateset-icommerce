//! Manufacturing domain models
//!
//! This module contains models for manufacturing operations including:
//! - Bill of Materials (BOM) - defines product composition
//! - Work Orders - tracks manufacturing jobs
//! - Work Order Tasks - individual steps in manufacturing
//! - Work Order Materials - material consumption tracking

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// =============================================================================
// Bill of Materials (BOM)
// =============================================================================

/// Bill of Materials - defines what components make up a product
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BillOfMaterials {
    pub id: Uuid,
    /// Unique BOM number (e.g., "BOM-2024-001")
    pub bom_number: String,
    /// Product this BOM is for
    pub product_id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Description of the BOM
    pub description: Option<String>,
    /// Revision identifier (e.g., "A", "B", "1.0")
    pub revision: String,
    /// Lifecycle status
    pub status: BomStatus,
    /// Components in this BOM
    pub components: Vec<BomComponent>,
    /// Who created this BOM
    pub created_by: Option<Uuid>,
    /// Who last updated this BOM
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BillOfMaterials {
    /// Generate a BOM number based on timestamp
    pub fn generate_bom_number() -> String {
        let now = Utc::now();
        format!("BOM-{}", now.format("%Y%m%d%H%M%S"))
    }

    /// Calculate total component count
    pub fn total_component_count(&self) -> usize {
        self.components.len()
    }

    /// Calculate total material cost (sum of component quantities * unit costs if available)
    pub fn total_quantity(&self) -> Decimal {
        self.components.iter().map(|c| c.quantity).sum()
    }
}

/// BOM lifecycle status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BomStatus {
    /// Draft - being edited, not ready for production
    #[default]
    Draft,
    /// Active - approved for use in production
    Active,
    /// Obsolete - no longer used
    Obsolete,
}

impl std::fmt::Display for BomStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BomStatus::Draft => write!(f, "draft"),
            BomStatus::Active => write!(f, "active"),
            BomStatus::Obsolete => write!(f, "obsolete"),
        }
    }
}

/// A component in a Bill of Materials
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BomComponent {
    pub id: Uuid,
    /// The BOM this component belongs to
    pub bom_id: Uuid,
    /// The component product (for sub-assemblies)
    pub component_product_id: Option<Uuid>,
    /// Component SKU
    pub component_sku: Option<String>,
    /// Component name
    pub name: String,
    /// Quantity required per unit of finished product
    pub quantity: Decimal,
    /// Unit of measure (e.g., "each", "kg", "m")
    pub unit_of_measure: String,
    /// Position/reference designator (e.g., "R1", "C5")
    pub position: Option<String>,
    /// Notes about this component
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new BOM
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateBom {
    pub product_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub revision: Option<String>,
    pub components: Option<Vec<CreateBomComponent>>,
    pub created_by: Option<Uuid>,
}

/// Input for creating a BOM component
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateBomComponent {
    pub component_product_id: Option<Uuid>,
    pub component_sku: Option<String>,
    pub name: String,
    pub quantity: Decimal,
    pub unit_of_measure: Option<String>,
    pub position: Option<String>,
    pub notes: Option<String>,
}

/// Input for updating a BOM
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateBom {
    pub name: Option<String>,
    pub description: Option<String>,
    pub revision: Option<String>,
    pub status: Option<BomStatus>,
    pub updated_by: Option<Uuid>,
}

/// Filter for listing BOMs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BomFilter {
    pub product_id: Option<Uuid>,
    pub status: Option<BomStatus>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// =============================================================================
// Work Orders
// =============================================================================

/// Work Order - a manufacturing job to build products
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkOrder {
    pub id: Uuid,
    /// Unique work order number (e.g., "WO-2024-001")
    pub work_order_number: String,
    /// Product being manufactured
    pub product_id: Uuid,
    /// BOM to use (optional - can manufacture without BOM)
    pub bom_id: Option<Uuid>,
    /// Work center or production line
    pub work_center_id: Option<String>,
    /// User assigned to this work order
    pub assigned_to: Option<Uuid>,
    /// Current status
    pub status: WorkOrderStatus,
    /// Priority level
    pub priority: WorkOrderPriority,
    /// Quantity to build
    pub quantity_to_build: Decimal,
    /// Quantity completed so far
    pub quantity_completed: Decimal,
    /// Scheduled start date/time
    pub scheduled_start: Option<DateTime<Utc>>,
    /// Scheduled end date/time
    pub scheduled_end: Option<DateTime<Utc>>,
    /// Actual start date/time
    pub actual_start: Option<DateTime<Utc>>,
    /// Actual end date/time
    pub actual_end: Option<DateTime<Utc>>,
    /// Tasks within this work order
    pub tasks: Vec<WorkOrderTask>,
    /// Materials reserved/consumed
    pub materials: Vec<WorkOrderMaterial>,
    /// Notes
    pub notes: Option<String>,
    /// Version for optimistic locking
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkOrder {
    /// Generate a work order number based on timestamp
    pub fn generate_work_order_number() -> String {
        let now = Utc::now();
        format!("WO-{}", now.format("%Y%m%d%H%M%S"))
    }

    /// Check if work order can be started
    pub fn can_start(&self) -> bool {
        matches!(self.status, WorkOrderStatus::Planned | WorkOrderStatus::OnHold)
    }

    /// Check if work order can be completed
    pub fn can_complete(&self) -> bool {
        matches!(self.status, WorkOrderStatus::InProgress)
    }

    /// Calculate completion percentage
    pub fn completion_percentage(&self) -> Decimal {
        if self.quantity_to_build.is_zero() {
            Decimal::ZERO
        } else {
            (self.quantity_completed / self.quantity_to_build) * Decimal::from(100)
        }
    }

    /// Check if work order is overdue
    pub fn is_overdue(&self) -> bool {
        if let Some(scheduled_end) = self.scheduled_end {
            if !matches!(self.status, WorkOrderStatus::Completed | WorkOrderStatus::Cancelled) {
                return Utc::now() > scheduled_end;
            }
        }
        false
    }
}

/// Work order status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkOrderStatus {
    /// Planned but not started
    #[default]
    Planned,
    /// Currently in progress
    InProgress,
    /// Completed successfully
    Completed,
    /// Partially completed (some quantity done)
    PartiallyCompleted,
    /// Cancelled
    Cancelled,
    /// On hold (temporarily stopped)
    OnHold,
}

impl std::fmt::Display for WorkOrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkOrderStatus::Planned => write!(f, "planned"),
            WorkOrderStatus::InProgress => write!(f, "in_progress"),
            WorkOrderStatus::Completed => write!(f, "completed"),
            WorkOrderStatus::PartiallyCompleted => write!(f, "partially_completed"),
            WorkOrderStatus::Cancelled => write!(f, "cancelled"),
            WorkOrderStatus::OnHold => write!(f, "on_hold"),
        }
    }
}

/// Work order priority
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkOrderPriority {
    /// Low priority
    Low,
    /// Normal priority
    #[default]
    Normal,
    /// High priority
    High,
    /// Urgent - needs immediate attention
    Urgent,
}

impl std::fmt::Display for WorkOrderPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkOrderPriority::Low => write!(f, "low"),
            WorkOrderPriority::Normal => write!(f, "normal"),
            WorkOrderPriority::High => write!(f, "high"),
            WorkOrderPriority::Urgent => write!(f, "urgent"),
        }
    }
}

/// Input for creating a work order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkOrder {
    pub product_id: Uuid,
    pub bom_id: Option<Uuid>,
    pub work_center_id: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub priority: Option<WorkOrderPriority>,
    pub quantity_to_build: Decimal,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    /// Optionally create tasks at the same time
    pub tasks: Option<Vec<CreateWorkOrderTask>>,
}

impl Default for CreateWorkOrder {
    fn default() -> Self {
        Self {
            product_id: Uuid::nil(),
            bom_id: None,
            work_center_id: None,
            assigned_to: None,
            priority: None,
            quantity_to_build: Decimal::ONE,
            scheduled_start: None,
            scheduled_end: None,
            notes: None,
            tasks: None,
        }
    }
}

/// Input for updating a work order
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateWorkOrder {
    pub work_center_id: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub status: Option<WorkOrderStatus>,
    pub priority: Option<WorkOrderPriority>,
    pub quantity_to_build: Option<Decimal>,
    pub quantity_completed: Option<Decimal>,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub actual_start: Option<DateTime<Utc>>,
    pub actual_end: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Filter for listing work orders
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkOrderFilter {
    pub product_id: Option<Uuid>,
    pub bom_id: Option<Uuid>,
    pub status: Option<WorkOrderStatus>,
    pub priority: Option<WorkOrderPriority>,
    pub assigned_to: Option<Uuid>,
    pub work_center_id: Option<String>,
    pub overdue_only: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// =============================================================================
// Work Order Tasks
// =============================================================================

/// A task within a work order (a step in manufacturing)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkOrderTask {
    pub id: Uuid,
    /// Parent work order
    pub work_order_id: Uuid,
    /// Sequence number (order of execution)
    pub sequence: i32,
    /// Task name/description
    pub task_name: String,
    /// Task status
    pub status: TaskStatus,
    /// Estimated hours to complete
    pub estimated_hours: Option<Decimal>,
    /// Actual hours spent
    pub actual_hours: Option<Decimal>,
    /// User assigned to this task
    pub assigned_to: Option<Uuid>,
    /// When task was started
    pub started_at: Option<DateTime<Utc>>,
    /// When task was completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Notes
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkOrderTask {
    /// Check if task can be started
    pub fn can_start(&self) -> bool {
        matches!(self.status, TaskStatus::Pending)
    }

    /// Check if task can be completed
    pub fn can_complete(&self) -> bool {
        matches!(self.status, TaskStatus::InProgress)
    }

    /// Calculate efficiency (estimated vs actual hours)
    pub fn efficiency(&self) -> Option<Decimal> {
        match (self.estimated_hours, self.actual_hours) {
            (Some(est), Some(act)) if !act.is_zero() => Some((est / act) * Decimal::from(100)),
            _ => None,
        }
    }
}

/// Task status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Not started
    #[default]
    Pending,
    /// In progress
    InProgress,
    /// Completed
    Completed,
    /// Skipped
    Skipped,
    /// Cancelled
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Skipped => write!(f, "skipped"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Input for creating a work order task
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateWorkOrderTask {
    pub sequence: Option<i32>,
    pub task_name: String,
    pub estimated_hours: Option<Decimal>,
    pub assigned_to: Option<Uuid>,
    pub notes: Option<String>,
}

/// Input for updating a work order task
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateWorkOrderTask {
    pub sequence: Option<i32>,
    pub task_name: Option<String>,
    pub status: Option<TaskStatus>,
    pub estimated_hours: Option<Decimal>,
    pub actual_hours: Option<Decimal>,
    pub assigned_to: Option<Uuid>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

// =============================================================================
// Work Order Materials
// =============================================================================

/// Material tracking for a work order
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkOrderMaterial {
    pub id: Uuid,
    /// Parent work order
    pub work_order_id: Uuid,
    /// Reference to BOM component (if applicable)
    pub component_id: Option<Uuid>,
    /// Component SKU
    pub component_sku: String,
    /// Component name
    pub component_name: String,
    /// Quantity reserved from inventory
    pub reserved_quantity: Decimal,
    /// Quantity actually consumed
    pub consumed_quantity: Decimal,
    /// Link to inventory reservation
    pub inventory_reservation_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkOrderMaterial {
    /// Get remaining reserved quantity (reserved but not consumed)
    pub fn remaining_reserved(&self) -> Decimal {
        self.reserved_quantity - self.consumed_quantity
    }

    /// Check if fully consumed
    pub fn is_fully_consumed(&self) -> bool {
        self.consumed_quantity >= self.reserved_quantity
    }
}

/// Input for adding material to a work order
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddWorkOrderMaterial {
    pub component_id: Option<Uuid>,
    pub component_sku: String,
    pub component_name: String,
    pub quantity: Decimal,
}

/// Input for consuming material from a work order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumeMaterial {
    pub material_id: Uuid,
    pub quantity: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_order_completion_percentage() {
        let wo = WorkOrder {
            id: Uuid::new_v4(),
            work_order_number: "WO-001".into(),
            product_id: Uuid::new_v4(),
            bom_id: None,
            work_center_id: None,
            assigned_to: None,
            status: WorkOrderStatus::InProgress,
            priority: WorkOrderPriority::Normal,
            quantity_to_build: Decimal::from(100),
            quantity_completed: Decimal::from(25),
            scheduled_start: None,
            scheduled_end: None,
            actual_start: None,
            actual_end: None,
            tasks: vec![],
            materials: vec![],
            notes: None,
            version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(wo.completion_percentage(), Decimal::from(25));
    }

    #[test]
    fn test_bom_status_display() {
        assert_eq!(BomStatus::Draft.to_string(), "draft");
        assert_eq!(BomStatus::Active.to_string(), "active");
        assert_eq!(BomStatus::Obsolete.to_string(), "obsolete");
    }

    #[test]
    fn test_work_order_status_display() {
        assert_eq!(WorkOrderStatus::Planned.to_string(), "planned");
        assert_eq!(WorkOrderStatus::InProgress.to_string(), "in_progress");
        assert_eq!(WorkOrderStatus::Completed.to_string(), "completed");
    }
}
