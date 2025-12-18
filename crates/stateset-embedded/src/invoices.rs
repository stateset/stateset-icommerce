//! Invoice operations for billing and accounts receivable
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateInvoice, CreateInvoiceItem};
//! use rust_decimal_macros::dec;
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create an invoice
//! let invoice = commerce.invoices().create(CreateInvoice {
//!     customer_id: Uuid::new_v4(),
//!     order_id: Some(Uuid::new_v4()),
//!     billing_email: Some("customer@example.com".into()),
//!     billing_name: Some("Alice Smith".into()),
//!     billing_address: Some("123 Main St, City, ST 12345".into()),
//!     items: vec![CreateInvoiceItem {
//!         description: "Professional Services".into(),
//!         quantity: dec!(10),
//!         unit_price: dec!(150.00),
//!         ..Default::default()
//!     }],
//!     ..Default::default()
//! })?;
//!
//! // Send the invoice
//! let invoice = commerce.invoices().send(invoice.id)?;
//!
//! // Record a payment
//! commerce.invoices().record_payment(invoice.id, stateset_embedded::RecordInvoicePayment {
//!     amount: dec!(1500.00),
//!     payment_method: Some("credit_card".into()),
//!     reference: Some("PAY-12345".into()),
//!     ..Default::default()
//! })?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use crate::Database;
use stateset_core::{
    CreateInvoice, CreateInvoiceItem, Invoice, InvoiceFilter, InvoiceItem,
    RecordInvoicePayment, Result, UpdateInvoice,
};
use std::sync::Arc;
use uuid::Uuid;

/// Invoice operations for billing and accounts receivable
pub struct Invoices {
    db: Arc<dyn Database>,
}

impl Invoices {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new invoice
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateInvoice, CreateInvoiceItem, InvoiceType};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let invoice = commerce.invoices().create(CreateInvoice {
    ///     customer_id: Uuid::new_v4(),
    ///     invoice_type: Some(InvoiceType::Standard),
    ///     billing_email: Some("billing@company.com".into()),
    ///     billing_name: Some("Acme Corp".into()),
    ///     items: vec![
    ///         CreateInvoiceItem {
    ///             description: "Consulting - November".into(),
    ///             quantity: dec!(40),
    ///             unit_price: dec!(200.00),
    ///             ..Default::default()
    ///         },
    ///         CreateInvoiceItem {
    ///             description: "Software License".into(),
    ///             quantity: dec!(1),
    ///             unit_price: dec!(500.00),
    ///             ..Default::default()
    ///         },
    ///     ],
    ///     notes: Some("Payment due within 30 days".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateInvoice) -> Result<Invoice> {
        self.db.invoices().create(input)
    }

    /// Get an invoice by ID
    pub fn get(&self, id: Uuid) -> Result<Option<Invoice>> {
        self.db.invoices().get(id)
    }

    /// Get an invoice by invoice number (e.g., "INV-20231215123456")
    pub fn get_by_number(&self, invoice_number: &str) -> Result<Option<Invoice>> {
        self.db.invoices().get_by_number(invoice_number)
    }

    /// Update an invoice
    pub fn update(&self, id: Uuid, input: UpdateInvoice) -> Result<Invoice> {
        self.db.invoices().update(id, input)
    }

    /// List invoices with optional filtering
    pub fn list(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>> {
        self.db.invoices().list(filter)
    }

    /// Get all invoices for a customer
    pub fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Invoice>> {
        self.db.invoices().for_customer(customer_id)
    }

    /// Get all invoices for an order
    pub fn for_order(&self, order_id: Uuid) -> Result<Vec<Invoice>> {
        self.db.invoices().for_order(order_id)
    }

    // === Status Transitions ===

    /// Send an invoice to the customer
    pub fn send(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().send(id)
    }

    /// Mark invoice as viewed by customer
    pub fn mark_viewed(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().mark_viewed(id)
    }

    /// Void an invoice
    pub fn void(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().void(id)
    }

    /// Write off an uncollectible invoice
    pub fn write_off(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().write_off(id)
    }

    /// Mark invoice as disputed
    pub fn dispute(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().dispute(id)
    }

    // === Line Item Operations ===

    /// Add an item to an invoice
    pub fn add_item(&self, invoice_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem> {
        self.db.invoices().add_item(invoice_id, item)
    }

    /// Update a line item
    pub fn update_item(
        &self,
        item_id: Uuid,
        input: CreateInvoiceItem,
    ) -> Result<InvoiceItem> {
        self.db.invoices().update_item(item_id, input)
    }

    /// Remove an item from an invoice
    pub fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.invoices().remove_item(item_id)
    }

    /// Get items for an invoice
    pub fn get_items(&self, invoice_id: Uuid) -> Result<Vec<InvoiceItem>> {
        self.db.invoices().get_items(invoice_id)
    }

    // === Payment Operations ===

    /// Record a payment against an invoice
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, RecordInvoicePayment};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Record full payment
    /// let invoice = commerce.invoices().record_payment(Uuid::new_v4(), RecordInvoicePayment {
    ///     amount: dec!(1500.00),
    ///     payment_method: Some("wire_transfer".into()),
    ///     reference: Some("Wire ref: 123456789".into()),
    ///     notes: Some("Paid in full".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Record partial payment
    /// let invoice = commerce.invoices().record_payment(Uuid::new_v4(), RecordInvoicePayment {
    ///     amount: dec!(500.00),
    ///     payment_method: Some("check".into()),
    ///     reference: Some("Check #1234".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn record_payment(&self, id: Uuid, payment: RecordInvoicePayment) -> Result<Invoice> {
        self.db.invoices().record_payment(id, payment)
    }

    // === Queries ===

    /// Get all overdue invoices
    ///
    /// Returns invoices that are past their due date and not fully paid.
    pub fn get_overdue(&self) -> Result<Vec<Invoice>> {
        self.db.invoices().get_overdue()
    }

    /// Recalculate invoice totals
    ///
    /// Use this after modifying line items to update subtotal, tax, and total.
    pub fn recalculate(&self, id: Uuid) -> Result<Invoice> {
        self.db.invoices().recalculate(id)
    }

    /// Count invoices matching a filter
    pub fn count(&self, filter: InvoiceFilter) -> Result<u64> {
        self.db.invoices().count(filter)
    }

    /// Get the total outstanding balance for a customer
    pub fn customer_balance(&self, customer_id: Uuid) -> Result<rust_decimal::Decimal> {
        let invoices = self.for_customer(customer_id)?;
        let balance = invoices
            .iter()
            .map(|inv| inv.total - inv.amount_paid)
            .sum();
        Ok(balance)
    }
}
