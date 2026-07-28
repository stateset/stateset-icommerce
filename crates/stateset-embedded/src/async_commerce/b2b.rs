//! B2B accessors: purchase orders and invoices.

use super::*;

/// Async purchase order operations.
pub struct AsyncPurchaseOrders {
    db: Arc<PostgresDatabase>,
}

impl AsyncPurchaseOrders {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    // Supplier operations

    /// Create a new supplier.
    pub async fn create_supplier(&self, input: CreateSupplier) -> Result<Supplier> {
        self.db.purchase_orders().create_supplier_async(input).await
    }

    /// Get supplier by ID.
    pub async fn get_supplier(&self, id: Uuid) -> Result<Option<Supplier>> {
        self.db.purchase_orders().get_supplier_async(id).await
    }

    /// Get supplier by code.
    pub async fn get_supplier_by_code(&self, code: &str) -> Result<Option<Supplier>> {
        self.db.purchase_orders().get_supplier_by_code_async(code).await
    }

    /// Update a supplier.
    pub async fn update_supplier(&self, id: Uuid, input: UpdateSupplier) -> Result<Supplier> {
        self.db.purchase_orders().update_supplier_async(id, input).await
    }

    /// List suppliers.
    pub async fn list_suppliers(&self, filter: SupplierFilter) -> Result<Vec<Supplier>> {
        self.db.purchase_orders().list_suppliers_async(filter).await
    }

    /// Delete a supplier.
    pub async fn delete_supplier(&self, id: Uuid) -> Result<()> {
        self.db.purchase_orders().delete_supplier_async(id).await
    }

    // Purchase Order operations

    /// Create a new purchase order.
    pub async fn create(&self, input: CreatePurchaseOrder) -> Result<PurchaseOrder> {
        self.db.purchase_orders().create_async(input).await
    }

    /// Get purchase order by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<PurchaseOrder>> {
        self.db.purchase_orders().get_async(id).await
    }

    /// Get purchase order by PO number.
    pub async fn get_by_number(&self, po_number: &str) -> Result<Option<PurchaseOrder>> {
        self.db.purchase_orders().get_by_number_async(po_number).await
    }

    /// Update a purchase order.
    pub async fn update(&self, id: Uuid, input: UpdatePurchaseOrder) -> Result<PurchaseOrder> {
        self.db.purchase_orders().update_async(id, input).await
    }

    /// List purchase orders.
    pub async fn list(&self, filter: PurchaseOrderFilter) -> Result<Vec<PurchaseOrder>> {
        self.db.purchase_orders().list_async(filter).await
    }

    /// Get purchase orders for a supplier.
    pub async fn for_supplier(&self, supplier_id: Uuid) -> Result<Vec<PurchaseOrder>> {
        self.db.purchase_orders().for_supplier_async(supplier_id).await
    }

    /// Delete a purchase order.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.purchase_orders().delete_async(id).await
    }

    /// Submit for approval.
    pub async fn submit(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().submit_for_approval_async(id).await
    }

    /// Approve purchase order.
    pub async fn approve(&self, id: Uuid, approved_by: &str) -> Result<PurchaseOrder> {
        self.db.purchase_orders().approve_async(id, approved_by).await
    }

    /// Send to supplier.
    pub async fn send(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().send_async(id).await
    }

    /// Mark as acknowledged by supplier.
    pub async fn acknowledge(
        &self,
        id: Uuid,
        supplier_reference: Option<&str>,
    ) -> Result<PurchaseOrder> {
        self.db.purchase_orders().acknowledge_async(id, supplier_reference).await
    }

    /// Put on hold.
    pub async fn hold(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().hold_async(id).await
    }

    /// Cancel purchase order.
    pub async fn cancel(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().cancel_async(id).await
    }

    /// Receive items.
    pub async fn receive(
        &self,
        id: Uuid,
        items: ReceivePurchaseOrderItems,
    ) -> Result<PurchaseOrder> {
        self.db.purchase_orders().receive_async(id, items).await
    }

    /// Complete purchase order.
    pub async fn complete(&self, id: Uuid) -> Result<PurchaseOrder> {
        self.db.purchase_orders().complete_async(id).await
    }

    /// Add item to purchase order.
    pub async fn add_item(
        &self,
        po_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        self.db.purchase_orders().add_item_async(po_id, item).await
    }

    /// Update a PO item.
    pub async fn update_item(
        &self,
        item_id: Uuid,
        item: CreatePurchaseOrderItem,
    ) -> Result<PurchaseOrderItem> {
        self.db.purchase_orders().update_item_async(item_id, item).await
    }

    /// Remove item from purchase order.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.purchase_orders().remove_item_async(item_id).await
    }

    /// Get items for purchase order.
    pub async fn get_items(&self, po_id: Uuid) -> Result<Vec<PurchaseOrderItem>> {
        self.db.purchase_orders().get_items_async(po_id).await
    }

    /// Count purchase orders.
    pub async fn count(&self, filter: PurchaseOrderFilter) -> Result<u64> {
        self.db.purchase_orders().count_async(filter).await
    }

    /// Count suppliers.
    pub async fn count_suppliers(&self, filter: SupplierFilter) -> Result<u64> {
        self.db.purchase_orders().count_suppliers_async(filter).await
    }
}

// ============================================================================
// Async Invoices
// ============================================================================

/// Async invoice operations.
pub struct AsyncInvoices {
    db: Arc<PostgresDatabase>,
}

impl AsyncInvoices {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    /// Create a new invoice.
    pub async fn create(&self, input: CreateInvoice) -> Result<Invoice> {
        self.db.invoices().create_async(input).await
    }

    /// Get invoice by ID.
    pub async fn get(&self, id: Uuid) -> Result<Option<Invoice>> {
        self.db.invoices().get_async(id).await
    }

    /// Get invoice by invoice number.
    pub async fn get_by_number(&self, invoice_number: &str) -> Result<Option<Invoice>> {
        self.db.invoices().get_by_number_async(invoice_number).await
    }

    /// Update an invoice.
    pub async fn update(&self, id: Uuid, input: UpdateInvoice) -> Result<Invoice> {
        self.db.invoices().update_async(id, input).await
    }

    /// List invoices.
    pub async fn list(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>> {
        self.db.invoices().list_async(filter).await
    }

    /// Get invoices for a customer.
    pub async fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Invoice>> {
        self.db.invoices().for_customer_async(customer_id).await
    }

    /// Get invoices for an order.
    pub async fn for_order(&self, order_id: Uuid) -> Result<Vec<Invoice>> {
        self.db.invoices().for_order_async(order_id).await
    }

    /// Delete an invoice.
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        self.db.invoices().delete_async(id).await
    }

    /// Send invoice to customer.
    pub async fn send(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().send_async(id).await
    }

    /// Mark invoice as viewed.
    pub async fn mark_viewed(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().mark_viewed_async(id).await
    }

    /// Record a payment on the invoice.
    pub async fn record_payment(&self, id: Uuid, payment: RecordInvoicePayment) -> Result<Invoice> {
        self.db.invoices().record_payment_async(id, payment).await
    }

    /// Void an invoice.
    pub async fn void(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().void_async(id).await
    }

    /// Write off an invoice.
    pub async fn write_off(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().write_off_async(id).await
    }

    /// Mark invoice as disputed.
    pub async fn dispute(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().dispute_async(id).await
    }

    /// Add item to invoice.
    pub async fn add_item(&self, invoice_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        self.db.invoices().add_item_async(invoice_id, item).await
    }

    /// Update an invoice item.
    pub async fn update_item(&self, item_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        self.db.invoices().update_item_async(item_id, item).await
    }

    /// Remove item from invoice.
    pub async fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.invoices().remove_item_async(item_id).await
    }

    /// Get items for invoice.
    pub async fn get_items(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        self.db.invoices().get_items_async(invoice_id).await
    }

    /// Recalculate invoice totals.
    pub async fn recalculate(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().recalculate_invoice_async(id).await
    }

    /// Get overdue invoices.
    pub async fn get_overdue(&self) -> Result<Vec<Invoice>> {
        self.db.invoices().get_overdue_async().await
    }

    /// Count invoices.
    pub async fn count(&self, filter: InvoiceFilter) -> Result<u64> {
        self.db.invoices().count_async(filter).await
    }
}

// ============================================================================
// Async Carts
// ============================================================================
