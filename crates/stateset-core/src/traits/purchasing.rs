//! Purchase-order, supplier-SKU, and vendor-return repositories.

use super::*;

/// Purchase Order repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait PurchaseOrderRepository: Send + Sync {
    // Supplier operations
    /// Create a new supplier
    fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier>;

    /// Get supplier by ID
    fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>>;

    /// Get supplier by code
    fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>>;

    /// Update a supplier
    fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier>;

    /// List suppliers with filter
    fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>>;

    /// Delete supplier (deactivate)
    fn delete_supplier(&self, id: Uuid) -> Result<()>;

    // Purchase Order operations
    /// Create a new purchase order
    fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder>;

    /// Get purchase order by ID
    fn get(&self, id: PurchaseOrderId) -> Result<Option<PurchaseOrder>>;

    /// Get purchase order by PO number
    fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>>;

    /// Update a purchase order
    fn update(&self, id: PurchaseOrderId, input: UpdatePurchaseOrder) -> Result<PurchaseOrder>;

    /// List purchase orders with filter
    fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>>;

    /// Get purchase orders for a supplier
    fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>>;

    /// Delete a purchase order (only if draft)
    fn delete(&self, id: PurchaseOrderId) -> Result<()>;

    // Status transitions
    /// Submit for approval
    fn submit_for_approval(&self, id: PurchaseOrderId) -> Result<PurchaseOrder>;

    /// Approve purchase order
    fn approve(&self, id: PurchaseOrderId, approved_by: &str) -> Result<PurchaseOrder>;

    /// Send to supplier
    fn send(&self, id: PurchaseOrderId) -> Result<PurchaseOrder>;

    /// Mark as acknowledged by supplier
    fn acknowledge(
        &self,
        id: PurchaseOrderId,
        supplier_reference: Option<&str>,
    ) -> Result<PurchaseOrder>;

    /// Put on hold
    fn hold(&self, id: PurchaseOrderId) -> Result<PurchaseOrder>;

    /// Cancel purchase order
    fn cancel(&self, id: PurchaseOrderId) -> Result<PurchaseOrder>;

    /// Receive items on a purchase order
    fn receive(
        &self,
        id: PurchaseOrderId,
        items: ReceivePurchaseOrderItems,
    ) -> Result<PurchaseOrder>;

    /// Complete/close purchase order
    fn complete(&self, id: PurchaseOrderId) -> Result<PurchaseOrder>;

    // Item operations
    /// Add item to purchase order
    fn add_item(
        &self,
        po_id: PurchaseOrderId,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem>;

    /// Update a PO item
    fn update_item(
        &self,
        item_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem>;

    /// Remove item from purchase order
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items for purchase order
    fn get_items(&self, po_id: PurchaseOrderId) -> Result<Vec<PurchaseOrderItem>>;

    /// Count purchase orders matching filter
    fn count(&self, filter: PurchaseOrderFilter) -> Result<u64>;

    /// Count suppliers matching filter
    fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple purchase orders - partial success allowed
    fn create_batch(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<BatchResult<PurchaseOrder>>;

    /// Create multiple purchase orders - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreatePurchaseOrder>) -> Result<Vec<PurchaseOrder>>;

    /// Update multiple purchase orders - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(PurchaseOrderId, UpdatePurchaseOrder)>,
    ) -> Result<BatchResult<PurchaseOrder>>;

    /// Update multiple purchase orders - atomic (all-or-nothing)
    fn update_batch_atomic(
        &self,
        updates: Vec<(PurchaseOrderId, UpdatePurchaseOrder)>,
    ) -> Result<Vec<PurchaseOrder>>;

    /// Delete multiple purchase orders - partial success allowed
    fn delete_batch(&self, ids: Vec<PurchaseOrderId>) -> Result<BatchResult<PurchaseOrderId>>;

    /// Delete multiple purchase orders - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<PurchaseOrderId>) -> Result<()>;

    /// Get multiple purchase orders by ID
    fn get_batch(&self, ids: Vec<PurchaseOrderId>) -> Result<Vec<PurchaseOrder>>;
}

/// Supplier SKU repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait SupplierSkuRepository: Send + Sync {
    /// Create a new supplier SKU.
    fn create(&self, input: CreateSupplierSku) -> Result<SupplierSku>;

    /// Get a supplier SKU by ID.
    fn get(&self, id: SupplierSkuId) -> Result<Option<SupplierSku>>;

    /// Update a supplier SKU (partial).
    fn update(&self, id: SupplierSkuId, input: UpdateSupplierSku) -> Result<SupplierSku>;

    /// List supplier SKUs with filter.
    fn list(&self, filter: SupplierSkuFilter) -> Result<Vec<SupplierSku>>;

    /// Delete a supplier SKU.
    fn delete(&self, id: SupplierSkuId) -> Result<()>;

    /// Bulk upsert supplier SKUs for a single supplier, keyed by internal
    /// product. Returns the number of rows affected.
    fn bulk_upsert(&self, supplier_id: uuid::Uuid, items: Vec<BulkSupplierSkuItem>) -> Result<u64>;
}

/// Vendor return (return-to-supplier) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait VendorReturnRepository: Send + Sync {
    /// Create a new vendor return.
    fn create(&self, input: CreateVendorReturn) -> Result<VendorReturn>;

    /// Get a vendor return by ID (with line items).
    fn get(&self, id: VendorReturnId) -> Result<Option<VendorReturn>>;

    /// List vendor returns with filter.
    fn list(&self, filter: VendorReturnFilter) -> Result<Vec<VendorReturn>>;

    /// Submit a draft vendor return to the supplier.
    fn submit(&self, id: VendorReturnId) -> Result<VendorReturn>;

    /// Process a vendor return: mark processed and optionally generate a credit.
    fn process(&self, id: VendorReturnId, generate_credit: bool) -> Result<VendorReturn>;

    /// Cancel a vendor return.
    fn cancel(&self, id: VendorReturnId) -> Result<VendorReturn>;
}
