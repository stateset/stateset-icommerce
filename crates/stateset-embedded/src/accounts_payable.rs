//! Accounts Payable operations
//!
//! Comprehensive AP management supporting:
//! - Supplier bill entry and tracking
//! - Payment scheduling and processing
//! - Payment run (batch payment) management
//! - Aging analysis and reports
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateBill, CreateBillItem, PaymentMethodAP};
//! use rust_decimal_macros::dec;
//! use chrono::{Utc, Duration};
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a bill from a supplier
//! let bill = commerce.accounts_payable().create_bill(CreateBill {
//!     supplier_id: Uuid::new_v4(),
//!     due_date: Utc::now() + Duration::days(30),
//!     items: vec![CreateBillItem {
//!         description: "Office supplies".into(),
//!         quantity: dec!(1),
//!         unit_price: dec!(150.00),
//!         ..Default::default()
//!     }],
//!     ..Default::default()
//! })?;
//!
//! println!("Created bill {}", bill.bill_number);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use rust_decimal::Decimal;
use stateset_core::{
    ApAgingSummary, BatchResult, Bill, BillFilter, BillItem, BillPayment, BillPaymentFilter,
    BillStatus, CreateBill, CreateBillItem, CreateBillPayment, CreatePaymentRun, PaymentAllocation,
    PaymentRun, PaymentRunFilter, Result, SupplierApSummary, UpdateBill,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use chrono::Utc;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// Accounts Payable management interface.
pub struct AccountsPayable {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl std::fmt::Debug for AccountsPayable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AccountsPayable").finish_non_exhaustive()
    }
}

impl AccountsPayable {
    #[cfg(feature = "events")]
    pub(crate) fn new(db: Arc<dyn Database>, event_system: Arc<EventSystem>) -> Self {
        Self { db, event_system }
    }

    #[cfg(not(feature = "events"))]
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    #[cfg(feature = "events")]
    fn emit(&self, event: CommerceEvent) {
        self.event_system.emit(event);
    }

    // ========================================================================
    // Bill Operations
    // ========================================================================

    /// Create a new bill (supplier invoice).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateBill, CreateBillItem};
    /// use rust_decimal_macros::dec;
    /// use chrono::{Utc, Duration};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let bill = commerce.accounts_payable().create_bill(CreateBill {
    ///     supplier_id: Uuid::new_v4(),
    ///     due_date: Utc::now() + Duration::days(30),
    ///     payment_terms: Some("Net 30".into()),
    ///     reference_number: Some("INV-12345".into()),
    ///     items: vec![
    ///         CreateBillItem {
    ///             description: "Raw materials".into(),
    ///             quantity: dec!(100),
    ///             unit_price: dec!(10.00),
    ///             account_code: Some("5010".into()),
    ///             ..Default::default()
    ///         },
    ///         CreateBillItem {
    ///             description: "Shipping".into(),
    ///             quantity: dec!(1),
    ///             unit_price: dec!(50.00),
    ///             account_code: Some("5020".into()),
    ///             ..Default::default()
    ///         },
    ///     ],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_bill(&self, input: CreateBill) -> Result<Bill> {
        self.db.accounts_payable().create_bill(input)
    }

    /// Get a bill by ID.
    pub fn get_bill(&self, id: Uuid) -> Result<Option<Bill>> {
        self.db.accounts_payable().get_bill(id)
    }

    /// Get a bill by bill number.
    pub fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>> {
        self.db.accounts_payable().get_bill_by_number(number)
    }

    /// Update a bill.
    pub fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill> {
        self.db.accounts_payable().update_bill(id, input)
    }

    /// List bills with optional filtering.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, BillFilter, BillStatus};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get all overdue bills for a supplier
    /// let bills = commerce.accounts_payable().list_bills(BillFilter {
    ///     supplier_id: Some(Uuid::new_v4()),
    ///     overdue_only: Some(true),
    ///     limit: Some(50),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>> {
        self.db.accounts_payable().list_bills(filter)
    }

    /// Delete a bill (only if draft).
    pub fn delete_bill(&self, id: Uuid) -> Result<()> {
        self.db.accounts_payable().delete_bill(id)
    }

    /// Approve a bill for payment.
    ///
    /// Transitions bill from draft/pending to approved status.
    pub fn approve_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().approve_bill(id)
    }

    /// Cancel a bill.
    pub fn cancel_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().cancel_bill(id)
    }

    /// Mark a bill as disputed.
    pub fn dispute_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().dispute_bill(id)
    }

    /// Get all line items for a bill.
    pub fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>> {
        self.db.accounts_payable().get_bill_items(bill_id)
    }

    /// Add an item to a bill and recalculate its totals.
    ///
    /// Only a `Draft` or `Pending` bill may be edited; adding an item to a
    /// bill in any other status (approved, paid, ...) returns
    /// [`CommerceError::Conflict`](crate::CommerceError::Conflict) naming the
    /// status, since item edits would change totals a payment may already
    /// depend on.
    pub fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem> {
        self.db.accounts_payable().add_bill_item(bill_id, item)
    }

    /// Remove an item from a bill and recalculate its totals.
    ///
    /// Only a `Draft` or `Pending` bill may be edited; removing an item from a
    /// bill in any other status (approved, paid, ...) returns
    /// [`CommerceError::Conflict`](crate::CommerceError::Conflict) naming the
    /// status, since item edits would change totals a payment may already
    /// depend on.
    pub fn remove_bill_item(&self, item_id: Uuid) -> Result<()> {
        self.db.accounts_payable().remove_bill_item(item_id)
    }

    /// Count bills matching the filter.
    pub fn count_bills(&self, filter: BillFilter) -> Result<u64> {
        self.db.accounts_payable().count_bills(filter)
    }

    /// Get all overdue bills.
    ///
    /// Returns bills past their due date that haven't been paid.
    pub fn get_overdue_bills(&self) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_overdue_bills()
    }

    /// Get bills due soon (within specified days).
    ///
    /// The window compares calendar dates and is inclusive: a bill due exactly
    /// `days` from today is included.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Get bills due in the next 7 days
    /// let bills = commerce.accounts_payable().get_bills_due_soon(7)?;
    /// for bill in bills {
    ///     println!("Bill {} due on {}: ${}", bill.bill_number, bill.due_date, bill.amount_due);
    /// }
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_bills_due_soon(days)
    }

    // ========================================================================
    // Three-Way Match
    // ========================================================================

    /// Perform a three-way match (purchase order vs receipts vs bill) for a bill.
    ///
    /// Loads the bill's linked purchase order lines and every non-cancelled
    /// receipt recorded against that PO, then compares ordered quantity/cost,
    /// received quantity, and billed quantity/cost line by line. The result is
    /// computed on read and never persisted.
    ///
    /// `tolerance_percent` is a relative tolerance (e.g. `dec!(5)` allows 5%
    /// deviation); `None` means exact matching.
    ///
    /// Returns [`stateset_core::MatchStatus::NotRequired`] when the bill has no
    /// purchase order, and an error if the bill does not exist.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    /// let result = commerce.accounts_payable().three_way_match(Uuid::new_v4(), Some(dec!(5)))?;
    /// println!("match status: {:?}", result.match_status);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn three_way_match(
        &self,
        bill_id: Uuid,
        tolerance_percent: Option<Decimal>,
    ) -> Result<stateset_core::ThreeWayMatchResult> {
        let bill = self
            .db
            .accounts_payable()
            .get_bill(bill_id)?
            .ok_or(stateset_core::CommerceError::NotFound)?;

        let Some(po_id) = bill.purchase_order_id else {
            return Ok(stateset_core::ThreeWayMatchResult::not_required());
        };

        let bill_lines = self.db.accounts_payable().get_bill_items(bill_id)?;
        let po_items = self.db.purchase_orders().get_items(po_id.into())?;

        let receipts = self.db.receiving().list_receipts(stateset_core::ReceiptFilter {
            reference_id: Some(po_id),
            ..Default::default()
        })?;
        let mut receipt_items = Vec::new();
        for receipt in receipts {
            if receipt.status == stateset_core::ReceiptStatus::Cancelled {
                continue;
            }
            receipt_items.extend(self.db.receiving().get_receipt_items(receipt.id)?);
        }

        let result = stateset_core::perform_three_way_match(
            &po_items,
            &receipt_items,
            &bill_lines,
            tolerance_percent.unwrap_or(Decimal::ZERO),
        );
        #[cfg(feature = "events")]
        if let stateset_core::MatchStatus::Variance { variance_line_count } = result.match_status {
            self.emit(CommerceEvent::ThreeWayMatchVarianceDetected {
                bill_id,
                purchase_order_id: po_id,
                variance_line_count,
                tolerance_percent: result.tolerance_percent,
                timestamp: Utc::now(),
            });
        }
        Ok(result)
    }

    // ========================================================================
    // Payment Operations
    // ========================================================================

    /// Create a payment to a supplier.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateBillPayment, PaymentMethodAP, PaymentAllocationInput};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let payment = commerce.accounts_payable().create_payment(CreateBillPayment {
    ///     supplier_id: Uuid::new_v4(),
    ///     payment_method: PaymentMethodAP::Check,
    ///     amount: dec!(1000.00),
    ///     check_number: Some("10234".into()),
    ///     allocations: vec![
    ///         PaymentAllocationInput {
    ///             bill_id: Uuid::new_v4(), // bill ID
    ///             amount: dec!(500.00),
    ///         },
    ///         PaymentAllocationInput {
    ///             bill_id: Uuid::new_v4(), // another bill ID
    ///             amount: dec!(500.00),
    ///         },
    ///     ],
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment> {
        self.db.accounts_payable().create_payment(input)
    }

    /// Get a payment by ID.
    pub fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>> {
        self.db.accounts_payable().get_payment(id)
    }

    /// Get a payment by payment number.
    pub fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>> {
        self.db.accounts_payable().get_payment_by_number(number)
    }

    /// List payments with optional filtering.
    pub fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>> {
        self.db.accounts_payable().list_payments(filter)
    }

    /// Void a payment.
    ///
    /// Reverses the effect of the payment on associated bills.
    pub fn void_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.db.accounts_payable().void_payment(id)
    }

    /// Mark a payment as cleared (e.g., check cleared the bank).
    pub fn clear_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.db.accounts_payable().clear_payment(id)
    }

    /// Get allocations for a payment.
    pub fn get_payment_allocations(&self, payment_id: Uuid) -> Result<Vec<PaymentAllocation>> {
        self.db.accounts_payable().get_payment_allocations(payment_id)
    }

    /// Get all payments for a specific bill.
    pub fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>> {
        self.db.accounts_payable().get_payments_for_bill(bill_id)
    }

    /// Count payments matching the filter.
    pub fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64> {
        self.db.accounts_payable().count_payments(filter)
    }

    /// Pay a bill directly with a single payment.
    ///
    /// Convenience method that creates a payment and allocates it fully to the specified bill.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, PayBill, PaymentMethodAP};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Pay a bill
    /// let payment = commerce.accounts_payable().pay_bill(
    ///     Uuid::new_v4(), // bill_id
    ///     stateset_core::PayBill {
    ///         amount: dec!(500.00),
    ///         payment_method: PaymentMethodAP::Check,
    ///         ..Default::default()
    ///     },
    /// )?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn pay_bill(&self, bill_id: Uuid, input: stateset_core::PayBill) -> Result<Bill> {
        // Get the bill to find the supplier
        let bill = self
            .db
            .accounts_payable()
            .get_bill(bill_id)?
            .ok_or(stateset_core::CommerceError::NotFound)?;

        if input.amount <= Decimal::ZERO {
            return Err(stateset_core::CommerceError::ValidationError(
                "Payment amount must be greater than zero".to_string(),
            ));
        }

        if !matches!(
            bill.status,
            BillStatus::Approved | BillStatus::PartiallyPaid | BillStatus::Overdue
        ) {
            return Err(stateset_core::CommerceError::ValidationError(
                "Bill is not in a payable status".to_string(),
            ));
        }

        if input.amount > bill.amount_due {
            return Err(stateset_core::CommerceError::ValidationError(
                "Payment amount exceeds bill amount due".to_string(),
            ));
        }

        let mut fallback_bill = bill.clone();
        fallback_bill.amount_paid += input.amount;
        fallback_bill.amount_due -= input.amount;
        fallback_bill.status = if fallback_bill.amount_due <= Decimal::ZERO {
            BillStatus::Paid
        } else {
            BillStatus::PartiallyPaid
        };

        // Create a payment for this bill
        let payment_input = CreateBillPayment {
            supplier_id: bill.supplier_id,
            payment_date: input.payment_date,
            payment_method: input.payment_method,
            amount: input.amount,
            currency: Some(bill.currency),
            reference_number: input.reference_number,
            bank_account: None,
            check_number: None,
            memo: input.memo,
            allocations: vec![stateset_core::PaymentAllocationInput {
                bill_id,
                amount: input.amount,
            }],
        };

        self.db.accounts_payable().create_payment(payment_input)?;

        // Return the updated bill; fallback to a deterministic in-memory update if re-read fails.
        Ok(self.db.accounts_payable().get_bill(bill_id)?.unwrap_or(fallback_bill))
    }

    // ========================================================================
    // Payment Run Operations
    // ========================================================================

    /// Create a payment run (batch payment).
    ///
    /// Groups multiple bills together for a scheduled payment batch. The run is
    /// created in `Draft` status; its `total_amount` is the sum of the bills'
    /// outstanding balances and `payment_count` is the number of bills.
    ///
    /// Validation (the whole run is created atomically or not at all):
    /// - `bill_ids` must be non-empty and free of duplicates;
    /// - every bill must exist, be in a payable status
    ///   (approved/partially-paid/overdue), and have a positive amount due;
    /// - a bill already included in another active run
    ///   (draft/pending/approved/processing) is rejected.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreatePaymentRun, PaymentMethodAP};
    /// use chrono::{Utc, Duration};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let run = commerce.accounts_payable().create_payment_run(CreatePaymentRun {
    ///     payment_date: Utc::now() + Duration::days(7),
    ///     payment_method: PaymentMethodAP::Ach,
    ///     bill_ids: vec![
    ///         Uuid::new_v4(), // approved bill 1
    ///         Uuid::new_v4(), // approved bill 2
    ///     ],
    ///     created_by: Some("finance_user".into()),
    ///     notes: Some("Weekly ACH run".into()),
    /// })?;
    ///
    /// println!("Created payment run {}", run.run_number);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        self.db.accounts_payable().create_payment_run(input)
    }

    /// Get a payment run by ID.
    pub fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>> {
        self.db.accounts_payable().get_payment_run(id)
    }

    /// List payment runs with optional filtering.
    pub fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>> {
        self.db.accounts_payable().list_payment_runs(filter)
    }

    /// Approve a payment run.
    ///
    /// Only a `Draft` or `Pending` run can be approved; approving a cancelled,
    /// completed, or processing run returns [`CommerceError::Conflict`].
    /// Approval is required before processing.
    ///
    /// [`CommerceError::Conflict`]: crate::CommerceError::Conflict
    pub fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun> {
        self.db.accounts_payable().approve_payment_run(id, approved_by)
    }

    /// Process a payment run, disbursing the batch.
    ///
    /// Only an `Approved` run can be processed; anything else (draft, pending,
    /// already completed, cancelled) returns [`CommerceError::Conflict`], so a
    /// run cannot be disbursed twice.
    ///
    /// In one atomic transaction this marks the run `Completed` and, for each
    /// bill in the run, creates a real payment for the bill's current
    /// outstanding balance (an `ap_payments` row in `Pending` status using the
    /// run's payment method and date, plus its allocation) and updates the
    /// bill's `amount_paid`/`amount_due`/status exactly like
    /// [`create_payment`](Self::create_payment). A bill that was fully paid or
    /// became unpayable between run creation and processing is skipped rather
    /// than double-paid: the run's `total_amount` and `payment_count` are
    /// adjusted to what was actually disbursed and its `notes` record the
    /// skipped bills. On any failure everything rolls back — including the
    /// status change — leaving the run `Approved` and retry-safe.
    ///
    /// [`CommerceError::Conflict`]: crate::CommerceError::Conflict
    pub fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.db.accounts_payable().process_payment_run(id)
    }

    /// Cancel a payment run.
    ///
    /// Only a run that has not started disbursing (`Draft`, `Pending`, or
    /// `Approved`) can be cancelled; cancelling a processing, completed, or
    /// already-cancelled run returns [`CommerceError::Conflict`]. Cancelling a
    /// run frees its bills to be included in a new run.
    ///
    /// [`CommerceError::Conflict`]: crate::CommerceError::Conflict
    pub fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.db.accounts_payable().cancel_payment_run(id)
    }

    /// Get bills included in a payment run.
    pub fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_payment_run_bills(run_id)
    }

    // ========================================================================
    // Analytics & Reports
    // ========================================================================

    /// Get AP aging summary.
    ///
    /// Returns outstanding amounts bucketed by age (current, 1-30, 31-60, etc.).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let aging = commerce.accounts_payable().get_aging_summary()?;
    /// println!("AP Aging Summary:");
    /// println!("  Current: ${}", aging.current);
    /// println!("  1-30 days: ${}", aging.days_1_30);
    /// println!("  31-60 days: ${}", aging.days_31_60);
    /// println!("  61-90 days: ${}", aging.days_61_90);
    /// println!("  Over 90 days: ${}", aging.days_over_90);
    /// println!("  Total: ${}", aging.total);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_aging_summary(&self) -> Result<ApAgingSummary> {
        self.db.accounts_payable().get_aging_summary()
    }

    /// Get AP summary for a specific supplier.
    pub fn get_supplier_summary(&self, supplier_id: Uuid) -> Result<Option<SupplierApSummary>> {
        self.db.accounts_payable().get_supplier_summary(supplier_id)
    }

    /// Get total AP outstanding across all suppliers.
    pub fn get_total_outstanding(&self) -> Result<Decimal> {
        self.db.accounts_payable().get_total_outstanding()
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple bills in a batch.
    pub fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>> {
        self.db.accounts_payable().create_bills_batch(inputs)
    }

    /// Get multiple bills by ID.
    pub fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_bills_batch(ids)
    }
}
