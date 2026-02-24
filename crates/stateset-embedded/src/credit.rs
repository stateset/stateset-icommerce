//! Credit Management operations
//!
//! Comprehensive credit management supporting:
//! - Customer credit limits and accounts
//! - Credit checks for orders
//! - Credit holds and releases
//! - Credit applications and approvals
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateCreditAccount, CustomerId};
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a credit account for a customer
//! let account = commerce.credit().create_credit_account(CreateCreditAccount {
//!     customer_id: CustomerId::new(),
//!     credit_limit: dec!(10000.00),
//!     payment_terms: Some("Net 30".into()),
//!     ..Default::default()
//! })?;
//!
//! println!("Credit limit: ${}", account.credit_limit);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    CreateCreditAccount, CreditAccount, CreditAccountFilter, CreditAgingBucket, CreditApplication,
    CreditApplicationFilter, CreditCheckResult, CreditHold, CreditHoldFilter, CreditId,
    CreditTransaction, CreditTransactionFilter, CustomerCreditSummary, CustomerId, OrderId,
    PlaceCreditHold, RecordCreditTransaction, ReleaseCreditHold, Result, ReviewCreditApplication,
    SubmitCreditApplication, UpdateCreditAccount,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// Credit Management interface.
pub struct Credit {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for Credit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credit").finish_non_exhaustive()
    }
}

impl Credit {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Credit Account Operations
    // ========================================================================

    /// Create a credit account for a customer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCreditAccount, CustomerId, RiskRating};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let account = commerce.credit().create_credit_account(CreateCreditAccount {
    ///     customer_id: CustomerId::new(),
    ///     credit_limit: dec!(25000.00),
    ///     payment_terms: Some("Net 45".into()),
    ///     risk_rating: Some(RiskRating::Low),
    ///     notes: Some("Established customer since 2020".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount> {
        self.db.credit().create_credit_account(input)
    }

    /// Get a credit account by ID.
    pub fn get_credit_account(&self, id: CreditId) -> Result<Option<CreditAccount>> {
        self.db.credit().get_credit_account(id)
    }

    /// Get a credit account by customer ID.
    pub fn get_credit_account_by_customer(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CreditAccount>> {
        self.db.credit().get_credit_account_by_customer(customer_id)
    }

    /// Update a credit account.
    pub fn update_credit_account(
        &self,
        id: CreditId,
        input: UpdateCreditAccount,
    ) -> Result<CreditAccount> {
        self.db.credit().update_credit_account(id, input)
    }

    /// List credit accounts with optional filtering.
    pub fn list_credit_accounts(&self, filter: CreditAccountFilter) -> Result<Vec<CreditAccount>> {
        self.db.credit().list_credit_accounts(filter)
    }

    /// Adjust a customer's credit limit.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CustomerId};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let account = commerce.credit().adjust_credit_limit(
    ///     CustomerId::new(),
    ///     dec!(50000.00),
    ///     "Annual review - increased based on payment history"
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn adjust_credit_limit(
        &self,
        customer_id: CustomerId,
        new_limit: Decimal,
        reason: &str,
    ) -> Result<CreditAccount> {
        self.db.credit().adjust_credit_limit(customer_id, new_limit, reason)
    }

    /// Suspend a credit account.
    pub fn suspend_credit_account(
        &self,
        customer_id: CustomerId,
        reason: &str,
    ) -> Result<CreditAccount> {
        self.db.credit().suspend_credit_account(customer_id, reason)
    }

    /// Reactivate a suspended credit account.
    pub fn reactivate_credit_account(&self, customer_id: CustomerId) -> Result<CreditAccount> {
        self.db.credit().reactivate_credit_account(customer_id)
    }

    // ========================================================================
    // Credit Check Operations
    // ========================================================================

    /// Check credit availability for an order.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CustomerId};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let result = commerce.credit().check_credit(
    ///     CustomerId::new(), // customer ID
    ///     dec!(5000.00),     // order amount
    /// )?;
    ///
    /// if result.approved {
    ///     println!("Credit approved");
    /// } else {
    ///     println!("Credit denied: {:?}", result.reason);
    ///     if result.requires_approval {
    ///         println!("Can be approved by credit manager");
    ///     }
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn check_credit(
        &self,
        customer_id: CustomerId,
        order_amount: Decimal,
    ) -> Result<CreditCheckResult> {
        self.db.credit().check_credit(customer_id, order_amount)
    }

    /// Reserve credit for an order.
    ///
    /// Reduces available credit until the order is invoiced or cancelled.
    pub fn reserve_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        self.db.credit().reserve_credit(customer_id, order_id, amount)
    }

    /// Release a credit reservation (e.g., order cancelled).
    pub fn release_credit_reservation(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
    ) -> Result<CreditAccount> {
        self.db.credit().release_credit_reservation(customer_id, order_id)
    }

    /// Charge credit (convert reservation to balance when order is invoiced).
    pub fn charge_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        self.db.credit().charge_credit(customer_id, order_id, amount)
    }

    // ========================================================================
    // Credit Hold Operations
    // ========================================================================

    /// Place a credit hold on a customer or order.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, PlaceCreditHold, CreditHoldType, CustomerId, OrderId};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let hold = commerce.credit().place_hold(PlaceCreditHold {
    ///     customer_id: CustomerId::new(),
    ///     order_id: Some(OrderId::new()),
    ///     hold_type: CreditHoldType::OverLimit,
    ///     hold_amount: dec!(2500.00),
    ///     reason: "Order exceeds available credit".into(),
    ///     placed_by: Some("credit_system".into()),
    /// })?;
    ///
    /// println!("Hold placed: {:?}", hold.id);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold> {
        self.db.credit().place_hold(input)
    }

    /// Get a credit hold by ID.
    pub fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>> {
        self.db.credit().get_hold(id)
    }

    /// List credit holds with optional filtering.
    pub fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>> {
        self.db.credit().list_holds(filter)
    }

    /// Release a credit hold.
    pub fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold> {
        self.db.credit().release_hold(input)
    }

    /// Get all active holds for a customer.
    pub fn get_active_holds(&self, customer_id: CustomerId) -> Result<Vec<CreditHold>> {
        self.db.credit().get_active_holds(customer_id)
    }

    /// Get all holds for an order.
    pub fn get_holds_for_order(&self, order_id: OrderId) -> Result<Vec<CreditHold>> {
        self.db.credit().get_holds_for_order(order_id)
    }

    // ========================================================================
    // Credit Application Operations
    // ========================================================================

    /// Submit a credit application.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CustomerId, SubmitCreditApplication};
    /// use rust_decimal_macros::dec;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let app = commerce.credit().submit_application(SubmitCreditApplication {
    ///     customer_id: CustomerId::new(),
    ///     requested_limit: dec!(50000.00),
    ///     business_name: Some("Acme Corp".into()),
    ///     years_in_business: Some(10),
    ///     annual_revenue: Some(dec!(5000000.00)),
    ///     bank_reference: Some("First National Bank".into()),
    ///     ..Default::default()
    /// })?;
    ///
    /// println!("Application {} submitted", app.application_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn submit_application(&self, input: SubmitCreditApplication) -> Result<CreditApplication> {
        self.db.credit().submit_application(input)
    }

    /// Get a credit application by ID.
    pub fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>> {
        self.db.credit().get_application(id)
    }

    /// List credit applications with optional filtering.
    pub fn list_applications(
        &self,
        filter: CreditApplicationFilter,
    ) -> Result<Vec<CreditApplication>> {
        self.db.credit().list_applications(filter)
    }

    /// Review and approve/deny a credit application.
    pub fn review_application(&self, input: ReviewCreditApplication) -> Result<CreditApplication> {
        self.db.credit().review_application(input)
    }

    /// Withdraw a credit application.
    pub fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication> {
        self.db.credit().withdraw_application(id)
    }

    // ========================================================================
    // Transaction Operations
    // ========================================================================

    /// Record a credit transaction.
    pub fn record_transaction(&self, input: RecordCreditTransaction) -> Result<CreditTransaction> {
        self.db.credit().record_transaction(input)
    }

    /// List credit transactions with optional filtering.
    pub fn list_transactions(
        &self,
        filter: CreditTransactionFilter,
    ) -> Result<Vec<CreditTransaction>> {
        self.db.credit().list_transactions(filter)
    }

    /// Apply a payment to reduce customer balance.
    pub fn apply_payment(
        &self,
        customer_id: CustomerId,
        amount: Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount> {
        self.db.credit().apply_payment(customer_id, amount, reference_id)
    }

    // ========================================================================
    // Analytics
    // ========================================================================

    /// Get credit summary for a customer.
    pub fn get_customer_summary(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CustomerCreditSummary>> {
        self.db.credit().get_customer_summary(customer_id)
    }

    /// Get credit aging report.
    pub fn get_aging_report(&self) -> Result<Vec<(CustomerId, CreditAgingBucket)>> {
        self.db.credit().get_aging_report()
    }

    /// Get all customers over their credit limit.
    pub fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>> {
        self.db.credit().get_over_limit_customers()
    }
}
