//! Receiving and goods receipt operations
//!
//! Comprehensive receiving management supporting:
//! - ASN (Advanced Shipping Notice) processing
//! - Goods receipt from purchase orders
//! - Quality inspection integration
//! - Put-away task management
//!
//! # Example
//!
//! ```ignore
//! use stateset_embedded::{Commerce, CreateReceipt, CreateReceiptItem, ReceiptType};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a receipt from an ASN
//! let receipt = commerce.receiving().create_receipt(CreateReceipt {
//!     receipt_type: ReceiptType::PurchaseOrder,
//!     warehouse_id: 1,
//!     items: vec![CreateReceiptItem {
//!         sku: "PROD-001".into(),
//!         expected_quantity: dec!(100),
//!         ..Default::default()
//!     }],
//!     ..Default::default()
//! })?;
//!
//! println!("Created receipt {}", receipt.receipt_number);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use stateset_core::{
    BatchResult, CompletePutAway, CreatePutAway, CreateReceipt, PutAway, PutAwayFilter, Receipt,
    ReceiptFilter, ReceiptItem, ReceiveItems, Result, UpdateReceipt,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Receiving and goods receipt management interface.
pub struct Receiving {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Receiving {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiving").finish_non_exhaustive()
    }
}

impl Receiving {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Receipt Operations
    // ========================================================================

    /// Create a new receipt (ASN/Goods Receipt).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateReceipt, CreateReceiptItem, ReceiptType};
    /// use rust_decimal_macros::dec;
    /// use chrono::{Utc, Duration};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let receipt = commerce.receiving().create_receipt(CreateReceipt {
    ///     receipt_type: ReceiptType::PurchaseOrder,
    ///     warehouse_id: 1,
    ///     carrier: Some("UPS".into()),
    ///     tracking_number: Some("1Z999AA10123456784".into()),
    ///     expected_date: Some(Utc::now() + Duration::days(3)),
    ///     items: vec![
    ///         CreateReceiptItem {
    ///             sku: "WIDGET-001".into(),
    ///             description: Some("Standard Widget".into()),
    ///             expected_quantity: dec!(50),
    ///             unit_cost: Some(dec!(10.00)),
    ///             ..Default::default()
    ///         },
    ///         CreateReceiptItem {
    ///             sku: "GADGET-002".into(),
    ///             description: Some("Premium Gadget".into()),
    ///             expected_quantity: dec!(25),
    ///             unit_cost: Some(dec!(25.00)),
    ///             ..Default::default()
    ///         },
    ///     ],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_receipt(&self, input: CreateReceipt) -> Result<Receipt> {
        self.db.receiving().create_receipt(input)
    }

    /// Get a receipt by ID.
    pub fn get_receipt(&self, id: Uuid) -> Result<Option<Receipt>> {
        self.db.receiving().get_receipt(id)
    }

    /// Get a receipt by receipt number.
    pub fn get_receipt_by_number(&self, number: &str) -> Result<Option<Receipt>> {
        self.db.receiving().get_receipt_by_number(number)
    }

    /// Update receipt details (carrier, tracking, expected date).
    pub fn update_receipt(&self, id: Uuid, input: UpdateReceipt) -> Result<Receipt> {
        self.db.receiving().update_receipt(id, input)
    }

    /// List receipts with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ReceiptFilter, ReceiptStatus};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all in-progress receipts for a warehouse
    /// let receipts = commerce.receiving().list_receipts(ReceiptFilter {
    ///     warehouse_id: Some(1),
    ///     status: Some(ReceiptStatus::InProgress),
    ///     limit: Some(50),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list_receipts(&self, filter: ReceiptFilter) -> Result<Vec<Receipt>> {
        self.db.receiving().list_receipts(filter)
    }

    /// Delete a receipt (only if still in 'expected' status).
    pub fn delete_receipt(&self, id: Uuid) -> Result<()> {
        self.db.receiving().delete_receipt(id)
    }

    /// Start receiving goods (transition from 'expected' to '`in_progress`').
    ///
    /// Call this when goods arrive and receiving process begins.
    pub fn start_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().start_receiving(id)
    }

    /// Receive items against a receipt.
    ///
    /// Updates receipt item quantities with actual received amounts.
    /// Can be called multiple times for partial receipts.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ReceiveItems, ReceiveItemLine};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Receive items with lot tracking
    /// let receipt = commerce.receiving().receive_items(ReceiveItems {
    ///     receipt_id: Uuid::new_v4(), // receipt ID
    ///     items: vec![ReceiveItemLine {
    ///         receipt_item_id: Uuid::new_v4(), // receipt item ID
    ///         quantity_received: dec!(48),
    ///         quantity_rejected: Some(dec!(2)),
    ///         rejection_reason: Some("Damaged in transit".into()),
    ///         lot_number: Some("LOT-2025-001".into()),
    ///         serial_numbers: None,
    ///         expiration_date: None,
    ///         notes: None,
    ///     }],
    ///     receiving_location_id: Some(1),
    ///     received_by: Some("warehouse_user".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn receive_items(&self, input: ReceiveItems) -> Result<Receipt> {
        self.db.receiving().receive_items(input)
    }

    /// Complete receiving (all items received).
    ///
    /// Transitions receipt to 'received' status.
    pub fn complete_receiving(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().complete_receiving(id)
    }

    /// Cancel a receipt.
    pub fn cancel_receipt(&self, id: Uuid) -> Result<Receipt> {
        self.db.receiving().cancel_receipt(id)
    }

    /// Get all line items for a receipt.
    pub fn get_receipt_items(&self, receipt_id: Uuid) -> Result<Vec<ReceiptItem>> {
        self.db.receiving().get_receipt_items(receipt_id)
    }

    /// Count receipts matching the filter.
    pub fn count_receipts(&self, filter: ReceiptFilter) -> Result<u64> {
        self.db.receiving().count_receipts(filter)
    }

    // ========================================================================
    // Put-Away Operations
    // ========================================================================

    /// Create a put-away task for received items.
    ///
    /// Put-away tasks direct warehouse workers to move received goods
    /// from the receiving area to storage locations.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePutAway};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let put_away = commerce.receiving().create_put_away(CreatePutAway {
    ///     receipt_id: Uuid::new_v4(),
    ///     receipt_item_id: Uuid::new_v4(),
    ///     sku: "WIDGET-001".into(),
    ///     from_location_id: Some(1), // Receiving dock
    ///     to_location_id: 5,         // Storage location A-01-02
    ///     quantity: dec!(50),
    ///     lot_id: None,
    ///     assigned_to: Some("forklift_operator".into()),
    ///     notes: None,
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_put_away(&self, input: CreatePutAway) -> Result<PutAway> {
        self.db.receiving().create_put_away(input)
    }

    /// Get a put-away task by ID.
    pub fn get_put_away(&self, id: Uuid) -> Result<Option<PutAway>> {
        self.db.receiving().get_put_away(id)
    }

    /// List put-away tasks with optional filtering.
    pub fn list_put_aways(&self, filter: PutAwayFilter) -> Result<Vec<PutAway>> {
        self.db.receiving().list_put_aways(filter)
    }

    /// Assign a put-away task to a user.
    pub fn assign_put_away(&self, id: Uuid, assigned_to: &str) -> Result<PutAway> {
        self.db.receiving().assign_put_away(id, assigned_to)
    }

    /// Start a put-away task.
    pub fn start_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.db.receiving().start_put_away(id)
    }

    /// Complete a put-away task.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CompletePutAway};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Complete put-away (optionally with different actual location)
    /// let put_away = commerce.receiving().complete_put_away(CompletePutAway {
    ///     put_away_id: Uuid::new_v4(),
    ///     actual_location_id: Some(6), // Different from planned if needed
    ///     completed_by: Some("forklift_operator".into()),
    ///     notes: Some("Placed in alternate bin due to space".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn complete_put_away(&self, input: CompletePutAway) -> Result<PutAway> {
        self.db.receiving().complete_put_away(input)
    }

    /// Cancel a put-away task.
    pub fn cancel_put_away(&self, id: Uuid) -> Result<PutAway> {
        self.db.receiving().cancel_put_away(id)
    }

    /// Get pending put-away tasks for a receipt.
    pub fn get_pending_put_aways(&self, receipt_id: Uuid) -> Result<Vec<PutAway>> {
        self.db.receiving().get_pending_put_aways(receipt_id)
    }

    /// Count put-away tasks matching the filter.
    pub fn count_put_aways(&self, filter: PutAwayFilter) -> Result<u64> {
        self.db.receiving().count_put_aways(filter)
    }

    // ========================================================================
    // Integration Operations
    // ========================================================================

    /// Create a receipt directly from a purchase order.
    ///
    /// Copies PO line items to create expected receipt items.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let receipt = commerce.receiving().create_receipt_from_po(
    ///     Uuid::new_v4(), // PO ID
    ///     1,              // Warehouse ID
    /// )?;
    ///
    /// println!("Created receipt {} from PO", receipt.receipt_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_receipt_from_po(&self, po_id: Uuid, warehouse_id: i32) -> Result<Receipt> {
        self.db.receiving().create_receipt_from_po(po_id, warehouse_id)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple receipts in a batch.
    pub fn create_receipts_batch(
        &self,
        inputs: Vec<CreateReceipt>,
    ) -> Result<BatchResult<Receipt>> {
        self.db.receiving().create_receipts_batch(inputs)
    }

    /// Get multiple receipts by ID.
    pub fn get_receipts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Receipt>> {
        self.db.receiving().get_receipts_batch(ids)
    }
}
