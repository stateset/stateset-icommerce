//! Purchase Order operations for supplier procurement management
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateSupplier, CreatePurchaseOrder, CreatePurchaseOrderItem};
//! use rust_decimal_macros::dec;
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a supplier
//! let supplier = commerce.purchase_orders().create_supplier(CreateSupplier {
//!     name: "Acme Supplies".into(),
//!     email: Some("orders@acme.com".into()),
//!     ..Default::default()
//! })?;
//!
//! // Create a purchase order
//! let po = commerce.purchase_orders().create(CreatePurchaseOrder {
//!     supplier_id: supplier.id,
//!     items: vec![CreatePurchaseOrderItem {
//!         sku: "PART-001".into(),
//!         name: "Widget Part A".into(),
//!         quantity: dec!(100),
//!         unit_cost: dec!(5.99),
//!         ..Default::default()
//!     }],
//!     ..Default::default()
//! })?;
//!
//! // Submit for approval
//! let po = commerce.purchase_orders().submit(po.id)?;
//!
//! // Approve and send to supplier
//! let po = commerce.purchase_orders().approve(po.id, "admin")?;
//! let po = commerce.purchase_orders().send(po.id)?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use crate::Database;
use stateset_core::{
    CreatePurchaseOrder, CreatePurchaseOrderItem, CreateSupplier, PurchaseOrder,
    PurchaseOrderFilter, PurchaseOrderItem, ReceivePurchaseOrderItems,
    Result, Supplier, SupplierFilter, UpdatePurchaseOrder, UpdateSupplier,
};
use std::sync::Arc;
use uuid::Uuid;

/// Purchase Order operations for supplier procurement
pub struct PurchaseOrders {
    db: Arc<dyn Database>,
}

impl PurchaseOrders {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // === Supplier Operations ===

    /// Create a new supplier
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateSupplier, PaymentTerms};
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let supplier = commerce.purchase_orders().create_supplier(CreateSupplier {
    ///     name: "Quality Parts Inc".into(),
    ///     supplier_code: Some("QPI-001".into()),
    ///     email: Some("orders@qualityparts.com".into()),
    ///     phone: Some("555-0100".into()),
    ///     payment_terms: Some(PaymentTerms::Net30),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier> {
        self.db.purchase_orders().create_supplier(input)
    }

    /// Get a supplier by ID
    pub fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>> {
        self.db.purchase_orders().get_supplier(id)
    }

    /// Get a supplier by code
    pub fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>> {
        self.db.purchase_orders().get_supplier_by_code(code)
    }

    /// Update a supplier
    pub fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier> {
        self.db.purchase_orders().update_supplier(id, input)
    }

    /// List suppliers with optional filtering
    pub fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        self.db.purchase_orders().list_suppliers(filter)
    }

    /// Delete (deactivate) a supplier
    pub fn delete_supplier(&self, id: Uuid) -> Result<()> {
        self.db.purchase_orders().delete_supplier(id)
    }

    // === Purchase Order Operations ===

    /// Create a new purchase order
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreatePurchaseOrder, CreatePurchaseOrderItem};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let po = commerce.purchase_orders().create(CreatePurchaseOrder {
    ///     supplier_id: Uuid::new_v4(),
    ///     ship_to_address: Some("123 Warehouse Blvd, City, ST 12345".into()),
    ///     items: vec![
    ///         CreatePurchaseOrderItem {
    ///             sku: "RAW-001".into(),
    ///             name: "Raw Material A".into(),
    ///             quantity: dec!(500),
    ///             unit_cost: dec!(2.50),
    ///             ..Default::default()
    ///         },
    ///         CreatePurchaseOrderItem {
    ///             sku: "RAW-002".into(),
    ///             name: "Raw Material B".into(),
    ///             quantity: dec!(200),
    ///             unit_cost: dec!(7.25),
    ///             ..Default::default()
    ///         },
    ///     ],
    ///     notes: Some("Rush order - needed for production".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        self.db.purchase_orders().create(input)
    }

    /// Get a purchase order by ID
    pub fn get(&self, id: Uuid) -> Result<Option<PurchaseOrder>> {
        self.db.purchase_orders().get(id)
    }

    /// Get a purchase order by PO number (e.g., "PO-20231215123456")
    pub fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        self.db.purchase_orders().get_by_number(po_number)
    }

    /// Update a purchase order
    pub fn update(&self, id: Uuid, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        self.db.purchase_orders().update(id, input)
    }

    /// List purchase orders with optional filtering
    pub fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        self.db.purchase_orders().list(filter)
    }

    /// Get all purchase orders for a supplier
    pub fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        self.db.purchase_orders().for_supplier(supplier_id)
    }

    // === Status Transitions ===

    /// Submit a draft PO for approval
    pub fn submit(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().submit_for_approval(id)
    }

    /// Approve a purchase order
    ///
    /// # Arguments
    ///
    /// * `id` - Purchase order ID
    /// * `approved_by` - Name/ID of the approver
    pub fn approve(&self, id: Uuid, approved_by: &str) -> Result<PurchaseOrder> {
        self.db.purchase_orders().approve(id, approved_by)
    }

    /// Send a purchase order to the supplier
    pub fn send(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().send(id)
    }

    /// Acknowledge supplier receipt of the PO
    ///
    /// # Arguments
    ///
    /// * `id` - Purchase order ID
    /// * `supplier_reference` - Optional reference number from supplier
    pub fn acknowledge(&self, id: Uuid, supplier_reference: Option<&str>) -> Result<PurchaseOrder> {
        self.db.purchase_orders().acknowledge(id, supplier_reference)
    }

    /// Put a purchase order on hold
    pub fn hold(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().hold(id)
    }

    /// Complete the purchase order (fully received)
    pub fn complete(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().complete(id)
    }

    /// Cancel a purchase order
    pub fn cancel(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().cancel(id)
    }

    // === Line Item Operations ===

    /// Add an item to a purchase order
    pub fn add_item(&self, po_id: Uuid, item: CreatePurchaseOrderItem) -> Result<PurchaseOrderItem> {
        self.db.purchase_orders().add_item(po_id, item)
    }

    /// Update a line item
    pub fn update_item(
        &self,
        item_id: Uuid,
        input: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        self.db.purchase_orders().update_item(item_id, input)
    }

    /// Remove an item from a purchase order
    pub fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.purchase_orders().remove_item(item_id)
    }

    /// Get items for a purchase order
    pub fn get_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        self.db.purchase_orders().get_items(po_id)
    }

    /// Receive items from a purchase order
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, ReceivePurchaseOrderItems};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Receive items (can be partial)
    /// commerce.purchase_orders().receive(Uuid::new_v4(), ReceivePurchaseOrderItems {
    ///     items: vec![],  // Add items here
    ///     notes: Some("Warehouse receiving".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn receive(&self, po_id: Uuid, items: ReceivePurchaseOrderItems) -> Result<PurchaseOrder> {
        self.db.purchase_orders().receive(po_id, items)
    }

    /// Count purchase orders matching a filter
    pub fn count(&self, filter: PurchaseOrderFilter) -> Result<u64> {
        self.db.purchase_orders().count(filter)
    }

    /// Count suppliers matching a filter
    pub fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64> {
        self.db.purchase_orders().count_suppliers(filter)
    }
}
