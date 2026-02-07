//! Backorder Management operations
//!
//! Comprehensive backorder management supporting:
//! - Backorder creation when inventory is unavailable
//! - Priority-based fulfillment
//! - Allocation and auto-allocation
//! - Status tracking and reporting
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateBackorder, BackorderPriority};
//! use rust_decimal_macros::dec;
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a backorder when inventory is unavailable
//! let backorder = commerce.backorder().create_backorder(CreateBackorder {
//!     order_id: Uuid::new_v4(),
//!     customer_id: Uuid::new_v4(),
//!     sku: "WIDGET-001".into(),
//!     quantity: dec!(50),
//!     priority: Some(BackorderPriority::High),
//!     ..Default::default()
//! })?;
//!
//! println!("Created backorder {}", backorder.backorder_number);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    AllocateBackorder, Backorder, BackorderAllocation, BackorderFilter, BackorderFulfillment,
    BackorderSummary, CreateBackorder, FulfillBackorder, Result, SkuBackorderSummary,
    UpdateBackorder,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Backorder Management interface.
pub struct Backorders {
    db: Arc<dyn Database>,
}

impl Backorders {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Backorder Operations
    // ========================================================================

    /// Create a backorder.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateBackorder, BackorderPriority};
    /// use rust_decimal_macros::dec;
    /// use chrono::{Utc, Duration};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let backorder = commerce.backorder().create_backorder(CreateBackorder {
    ///     order_id: Uuid::new_v4(),
    ///     order_line_id: Some(Uuid::new_v4()),
    ///     customer_id: Uuid::new_v4(),
    ///     sku: "GADGET-002".into(),
    ///     quantity: dec!(25),
    ///     priority: Some(BackorderPriority::Critical),
    ///     expected_date: Some(Utc::now() + Duration::days(7)),
    ///     notes: Some("Rush order - customer requested expedite".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder> {
        self.db.backorder().create_backorder(input)
    }

    /// Get a backorder by ID.
    pub fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>> {
        self.db.backorder().get_backorder(id)
    }

    /// Get a backorder by backorder number.
    pub fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>> {
        self.db.backorder().get_backorder_by_number(number)
    }

    /// Update a backorder.
    pub fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder> {
        self.db.backorder().update_backorder(id, input)
    }

    /// List backorders with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, BackorderFilter, BackorderStatus, BackorderPriority};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all critical pending backorders
    /// let backorders = commerce.backorder().list_backorders(BackorderFilter {
    ///     status: Some(BackorderStatus::Pending),
    ///     priority: Some(BackorderPriority::Critical),
    ///     limit: Some(50),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        self.db.backorder().list_backorders(filter)
    }

    /// Cancel a backorder.
    pub fn cancel_backorder(&self, id: Uuid) -> Result<Backorder> {
        self.db.backorder().cancel_backorder(id)
    }

    /// Get all backorders for an order.
    pub fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_order(order_id)
    }

    /// Get all backorders for a customer.
    pub fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_customer(customer_id)
    }

    /// Get all backorders for a SKU.
    pub fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_sku(sku)
    }

    // ========================================================================
    // Fulfillment Operations
    // ========================================================================

    /// Fulfill a backorder (partial or complete).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, FulfillBackorder, FulfillmentSourceType};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Fulfill from received inventory
    /// let backorder = commerce.backorder().fulfill_backorder(FulfillBackorder {
    ///     backorder_id: Uuid::new_v4(),
    ///     quantity: dec!(25),
    ///     source_type: FulfillmentSourceType::PurchaseOrder,
    ///     source_id: Some(Uuid::new_v4()), // PO receipt ID
    ///     notes: Some("Fulfilled from PO-123".into()),
    ///     fulfilled_by: Some("warehouse_user".into()),
    /// })?;
    ///
    /// println!("Remaining: {}", backorder.quantity_remaining);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder> {
        self.db.backorder().fulfill_backorder(input)
    }

    /// Get fulfillment history for a backorder.
    pub fn get_fulfillment_history(&self, backorder_id: Uuid) -> Result<Vec<BackorderFulfillment>> {
        self.db.backorder().get_fulfillment_history(backorder_id)
    }

    // ========================================================================
    // Allocation Operations
    // ========================================================================

    /// Allocate inventory to a backorder.
    ///
    /// Reserves inventory for the backorder until it can be fulfilled.
    pub fn allocate_backorder(&self, input: AllocateBackorder) -> Result<BackorderAllocation> {
        self.db.backorder().allocate_backorder(input)
    }

    /// Get allocations for a backorder.
    pub fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>> {
        self.db.backorder().get_allocations(backorder_id)
    }

    /// Release an allocation.
    pub fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.db.backorder().release_allocation(allocation_id)
    }

    /// Confirm an allocation.
    pub fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.db.backorder().confirm_allocation(allocation_id)
    }

    /// Expire old allocations past their expiration date.
    pub fn expire_allocations(&self) -> Result<u32> {
        self.db.backorder().expire_allocations()
    }

    /// Automatically allocate available inventory to pending backorders.
    ///
    /// Allocates in priority order (critical first, then by oldest date).
    pub fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>> {
        self.db.backorder().auto_allocate_inventory(sku)
    }

    // ========================================================================
    // Analytics
    // ========================================================================

    /// Get overall backorder summary.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let summary = commerce.backorder().get_summary()?;
    /// println!("Total backorders: {}", summary.total_backorders);
    /// println!("Critical: {}", summary.critical_count);
    /// println!("Overdue: {}", summary.overdue_count);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_summary(&self) -> Result<BackorderSummary> {
        self.db.backorder().get_summary()
    }

    /// Get backorder summary for a specific SKU.
    pub fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        self.db.backorder().get_sku_summary(sku)
    }

    /// Get overdue backorders.
    pub fn get_overdue_backorders(&self) -> Result<Vec<Backorder>> {
        self.db.backorder().get_overdue_backorders()
    }

    /// Count pending backorders.
    pub fn count_pending(&self) -> Result<u64> {
        self.db.backorder().count_pending()
    }
}
