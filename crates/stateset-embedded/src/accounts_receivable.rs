//! Accounts Receivable operations
//!
//! Comprehensive accounts receivable management supporting:
//! - AR aging reports and analysis
//! - Collection activity tracking
//! - Credit memos and applications
//! - Write-offs
//! - Payment applications
//! - Customer statements
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::Commerce;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Get AR aging summary
//! let aging = commerce.accounts_receivable().get_aging_summary()?;
//! println!("Total outstanding: ${}", aging.total);
//! println!("Current: ${}", aging.current);
//! println!("30+ days: ${}", aging.days_1_30);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    ApplyCreditMemo, ApplyPaymentToInvoices, ArAgingFilter, ArAgingSummary, ArPaymentApplication,
    CollectionActivity, CollectionActivityFilter, CollectionStatus, CreateCollectionActivity,
    CreateCreditMemo, CreateWriteOff, CreditMemo, CreditMemoFilter, CustomerArAging,
    CustomerArSummary, CustomerStatement, DunningLetterType, GenerateStatementRequest, Invoice,
    Result, WriteOff, WriteOffFilter,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Accounts Receivable interface.
pub struct AccountsReceivable {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for AccountsReceivable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountsReceivable").finish_non_exhaustive()
    }
}

impl AccountsReceivable {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Aging Reports
    // ========================================================================

    /// Get overall AR aging summary.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let aging = commerce.accounts_receivable().get_aging_summary()?;
    /// println!("Current: ${}", aging.current);
    /// println!("1-30 days: ${}", aging.days_1_30);
    /// println!("31-60 days: ${}", aging.days_31_60);
    /// println!("61-90 days: ${}", aging.days_61_90);
    /// println!("90+ days: ${}", aging.days_over_90);
    /// println!("Total: ${}", aging.total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_aging_summary(&self) -> Result<ArAgingSummary> {
        self.db.accounts_receivable().get_aging_summary()
    }

    /// Get AR aging for a specific customer.
    pub fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>> {
        self.db.accounts_receivable().get_customer_aging(customer_id)
    }

    /// Get detailed AR aging report with filtering.
    pub fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>> {
        self.db.accounts_receivable().get_aging_report(filter)
    }

    // ========================================================================
    // Collection Activities
    // ========================================================================

    /// Log a collection activity (call, email, dunning letter, etc.).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateCollectionActivity, CollectionActivityType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let activity = commerce.accounts_receivable().log_collection_activity(
    ///     CreateCollectionActivity {
    ///         invoice_id: Uuid::new_v4(),
    ///         activity_type: CollectionActivityType::PhoneCall,
    ///         notes: Some("Spoke with customer, promised to pay by Friday".into()),
    ///         contact_method: Some("Phone".into()),
    ///         contact_result: Some("Promise to pay".into()),
    ///         performed_by: Some("John Collector".into()),
    ///         ..Default::default()
    ///     }
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity> {
        self.db.accounts_receivable().log_collection_activity(input)
    }

    /// List collection activities with filtering.
    pub fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>> {
        self.db.accounts_receivable().list_collection_activities(filter)
    }

    /// Update the collection status of an invoice.
    pub fn update_collection_status(
        &self,
        invoice_id: Uuid,
        status: CollectionStatus,
    ) -> Result<()> {
        self.db.accounts_receivable().update_collection_status(invoice_id.into(), status)
    }

    // ========================================================================
    // Dunning
    // ========================================================================

    /// Get invoices that are due for dunning letters.
    pub fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>> {
        self.db.accounts_receivable().get_invoices_due_for_dunning()
    }

    /// Send a dunning letter and log the activity.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, DunningLetterType};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let activity = commerce.accounts_receivable().send_dunning_letter(
    ///     Uuid::new_v4(), // invoice_id
    ///     DunningLetterType::Reminder1,
    ///     Some("ar_system"),
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn send_dunning_letter(
        &self,
        invoice_id: Uuid,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity> {
        self.db.accounts_receivable().send_dunning_letter(invoice_id.into(), letter_type, sent_by)
    }

    // ========================================================================
    // Write-offs
    // ========================================================================

    /// Create a write-off for an uncollectable invoice.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateWriteOff, WriteOffReason};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let write_off = commerce.accounts_receivable().create_write_off(CreateWriteOff {
    ///     invoice_id: Uuid::new_v4(),
    ///     amount: dec!(500.00),
    ///     reason: WriteOffReason::Uncollectable,
    ///     notes: Some("Customer bankruptcy".into()),
    ///     approved_by: Some("CFO".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        self.db.accounts_receivable().create_write_off(input)
    }

    /// Get a write-off by ID.
    pub fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>> {
        self.db.accounts_receivable().get_write_off(id)
    }

    /// List write-offs with filtering.
    pub fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>> {
        self.db.accounts_receivable().list_write_offs(filter)
    }

    /// Reverse a write-off (restore invoice to collections).
    pub fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff> {
        self.db.accounts_receivable().reverse_write_off(id)
    }

    // ========================================================================
    // Credit Memos
    // ========================================================================

    /// Create a credit memo for a customer.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateCreditMemo, CreditMemoReason};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let memo = commerce.accounts_receivable().create_credit_memo(CreateCreditMemo {
    ///     customer_id: Uuid::new_v4(),
    ///     original_invoice_id: Some(Uuid::new_v4()),
    ///     reason: CreditMemoReason::ReturnCredit,
    ///     amount: dec!(150.00),
    ///     notes: Some("Credit for returned merchandise".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        self.db.accounts_receivable().create_credit_memo(input)
    }

    /// Get a credit memo by ID.
    pub fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>> {
        self.db.accounts_receivable().get_credit_memo(id)
    }

    /// Get a credit memo by number.
    pub fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>> {
        self.db.accounts_receivable().get_credit_memo_by_number(number)
    }

    /// List credit memos with filtering.
    pub fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>> {
        self.db.accounts_receivable().list_credit_memos(filter)
    }

    /// Apply a credit memo to an invoice.
    pub fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        self.db.accounts_receivable().apply_credit_memo(input)
    }

    /// Void a credit memo (only if not yet applied).
    pub fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        self.db.accounts_receivable().void_credit_memo(id)
    }

    /// Get unapplied credit memos for a customer.
    pub fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        self.db.accounts_receivable().get_unapplied_credits(customer_id)
    }

    // ========================================================================
    // Payment Applications
    // ========================================================================

    /// Apply a payment to one or more invoices.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, ApplyPaymentToInvoices, InvoicePaymentApplication};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let applications = commerce.accounts_receivable().apply_payment_to_invoices(
    ///     ApplyPaymentToInvoices {
    ///         payment_id: Uuid::new_v4(),
    ///         applications: vec![
    ///             InvoicePaymentApplication {
    ///                 invoice_id: Uuid::new_v4(),
    ///                 amount: dec!(500.00),
    ///             },
    ///             InvoicePaymentApplication {
    ///                 invoice_id: Uuid::new_v4(),
    ///                 amount: dec!(250.00),
    ///             },
    ///         ],
    ///     }
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>> {
        self.db.accounts_receivable().apply_payment_to_invoices(input)
    }

    /// Get all payment applications for a payment.
    pub fn get_payment_applications(&self, payment_id: Uuid) -> Result<Vec<ArPaymentApplication>> {
        self.db.accounts_receivable().get_payment_applications(payment_id)
    }

    /// Unapply a payment application (remove application, restore invoice balance).
    pub fn unapply_payment(&self, application_id: Uuid) -> Result<()> {
        self.db.accounts_receivable().unapply_payment(application_id)
    }

    // ========================================================================
    // Customer Summary & Statements
    // ========================================================================

    /// Get AR summary for a customer.
    pub fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerArSummary>> {
        self.db.accounts_receivable().get_customer_summary(customer_id)
    }

    /// Generate a customer statement.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, GenerateStatementRequest};
    /// use chrono::Utc;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let statement = commerce.accounts_receivable().generate_statement(
    ///     GenerateStatementRequest {
    ///         customer_id: Uuid::new_v4(),
    ///         period_start: None, // defaults to 30 days ago
    ///         period_end: None,   // defaults to now
    ///         include_paid_invoices: Some(false),
    ///     }
    /// )?;
    ///
    /// println!("Statement for: {}", statement.customer_name);
    /// println!("Closing balance: ${}", statement.closing_balance);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn generate_statement(
        &self,
        request: GenerateStatementRequest,
    ) -> Result<CustomerStatement> {
        self.db.accounts_receivable().generate_statement(request)
    }

    // ========================================================================
    // Analytics
    // ========================================================================

    /// Get total outstanding receivables.
    pub fn get_total_outstanding(&self) -> Result<Decimal> {
        self.db.accounts_receivable().get_total_outstanding()
    }

    /// Calculate Days Sales Outstanding (DSO).
    pub fn get_dso(&self, days: i32) -> Result<Decimal> {
        self.db.accounts_receivable().get_dso(days)
    }

    /// Get average days to pay for a customer.
    pub fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>> {
        self.db.accounts_receivable().get_average_days_to_pay(customer_id)
    }

    /// Get AR summary for multiple customers.
    pub fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>> {
        self.db.accounts_receivable().get_customers_batch(ids)
    }
}
