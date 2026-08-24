//! Finance repositories: AP, AR, GL, invoices, credit, cost accounting, vendor credits, fixed assets, and revenue recognition.

use super::*;

/// Invoice repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait InvoiceRepository: Send + Sync {
    /// Create a new invoice
    fn create(&self, input: CreateInvoice) -> Result<Invoice>;

    /// Get invoice by ID
    fn get(&self, id: InvoiceId) -> Result<Option<Invoice>>;

    /// Get invoice by invoice number
    fn get_by_number(&self, invoice_number: &str) -> Result<Option<Invoice>>;

    /// Update an invoice
    fn update(&self, id: InvoiceId, input: UpdateInvoice) -> Result<Invoice>;

    /// List invoices with filter
    fn list(&self, filter: InvoiceFilter) -> Result<Vec<Invoice>>;

    /// Get invoices for a customer
    fn for_customer(&self, customer_id: CustomerId) -> Result<Vec<Invoice>>;

    /// Get invoices for an order
    fn for_order(&self, order_id: OrderId) -> Result<Vec<Invoice>>;

    /// Delete an invoice (only if draft)
    fn delete(&self, id: InvoiceId) -> Result<()>;

    // Status transitions
    /// Send invoice to customer
    fn send(&self, id: InvoiceId) -> Result<Invoice>;

    /// Mark invoice as viewed
    fn mark_viewed(&self, id: InvoiceId) -> Result<Invoice>;

    /// Record a payment on the invoice
    fn record_payment(&self, id: InvoiceId, payment: RecordInvoicePayment) -> Result<Invoice>;

    /// Void an invoice
    fn void(&self, id: InvoiceId) -> Result<Invoice>;

    /// Write off an invoice as uncollectible
    fn write_off(&self, id: InvoiceId) -> Result<Invoice>;

    /// Mark invoice as disputed
    fn dispute(&self, id: InvoiceId) -> Result<Invoice>;

    // Item operations
    /// Add item to invoice
    fn add_item(&self, invoice_id: InvoiceId, item: CreateInvoiceItem) -> Result<InvoiceItem>;

    /// Update an invoice item
    fn update_item(&self, item_id: Uuid, item: CreateInvoiceItem) -> Result<InvoiceItem>;

    /// Remove item from invoice
    fn remove_item(&self, item_id: Uuid) -> Result<()>;

    /// Get items for invoice
    fn get_items(&self, invoice_id: InvoiceId) -> Result<Vec<InvoiceItem>>;

    /// Recalculate invoice totals
    fn recalculate(&self, id: InvoiceId) -> Result<Invoice>;

    /// Get overdue invoices
    fn get_overdue(&self) -> Result<Vec<Invoice>>;

    /// Count invoices matching filter
    fn count(&self, filter: InvoiceFilter) -> Result<u64>;

    // === Batch Operations ===

    /// Create multiple invoices - partial success allowed
    fn create_batch(&self, inputs: Vec<CreateInvoice>) -> Result<BatchResult<Invoice>>;

    /// Create multiple invoices - atomic (all-or-nothing)
    fn create_batch_atomic(&self, inputs: Vec<CreateInvoice>) -> Result<Vec<Invoice>>;

    /// Update multiple invoices - partial success allowed
    fn update_batch(
        &self,
        updates: Vec<(InvoiceId, UpdateInvoice)>,
    ) -> Result<BatchResult<Invoice>>;

    /// Update multiple invoices - atomic (all-or-nothing)
    fn update_batch_atomic(&self, updates: Vec<(InvoiceId, UpdateInvoice)>)
    -> Result<Vec<Invoice>>;

    /// Delete multiple invoices - partial success allowed
    fn delete_batch(&self, ids: Vec<InvoiceId>) -> Result<BatchResult<Uuid>>;

    /// Delete multiple invoices - atomic (all-or-nothing)
    fn delete_batch_atomic(&self, ids: Vec<InvoiceId>) -> Result<()>;

    /// Get multiple invoices by ID
    fn get_batch(&self, ids: Vec<InvoiceId>) -> Result<Vec<Invoice>>;
}

// ============================================================================
// Accounts Payable Repository
// ============================================================================

/// Accounts Payable repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait AccountsPayableRepository: Send + Sync {
    // Bill operations
    /// Create a new bill
    fn create_bill(&self, input: CreateBill) -> Result<Bill>;

    /// Get bill by ID
    fn get_bill(&self, id: Uuid) -> Result<Option<Bill>>;

    /// Get bill by number
    fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>>;

    /// Update a bill
    fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill>;

    /// List bills with filter
    fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>>;

    /// Delete a bill (only if draft)
    fn delete_bill(&self, id: Uuid) -> Result<()>;

    /// Approve a bill
    fn approve_bill(&self, id: Uuid) -> Result<Bill>;

    /// Cancel a bill
    fn cancel_bill(&self, id: Uuid) -> Result<Bill>;

    /// Mark bill as disputed
    fn dispute_bill(&self, id: Uuid) -> Result<Bill>;

    /// Get bill items
    fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>>;

    /// Add item to bill
    fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem>;

    /// Remove item from bill
    fn remove_bill_item(&self, item_id: Uuid) -> Result<()>;

    /// Count bills
    fn count_bills(&self, filter: BillFilter) -> Result<u64>;

    /// Get overdue bills
    fn get_overdue_bills(&self) -> Result<Vec<Bill>>;

    /// Get bills due soon (within days)
    fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>>;

    // Payment operations
    /// Create a payment
    fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment>;

    /// Get payment by ID
    fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>>;

    /// Get payment by number
    fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>>;

    /// List payments with filter
    fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>>;

    /// Void a payment
    fn void_payment(&self, id: Uuid) -> Result<BillPayment>;

    /// Mark payment as cleared
    fn clear_payment(&self, id: Uuid) -> Result<BillPayment>;

    /// Get payment allocations
    fn get_payment_allocations(&self, payment_id: Uuid) -> Result<Vec<PaymentAllocation>>;

    /// Get payments for bill
    fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>>;

    /// Count payments
    fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64>;

    // Payment run operations
    /// Create a payment run
    fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun>;

    /// Get payment run by ID
    fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>>;

    /// List payment runs with filter
    fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>>;

    /// Approve payment run
    fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun>;

    /// Process payment run
    fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun>;

    /// Cancel payment run
    fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun>;

    /// Get bills in payment run
    fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>>;

    // Analytics
    /// Get AP aging summary
    fn get_aging_summary(&self) -> Result<ApAgingSummary>;

    /// Get AP summary by supplier (None if supplier is not found)
    fn get_supplier_summary(&self, supplier_id: Uuid) -> Result<Option<SupplierApSummary>>;

    /// Get total AP outstanding
    fn get_total_outstanding(&self) -> Result<rust_decimal::Decimal>;

    // Batch operations
    /// Create multiple bills
    fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>>;

    /// Get multiple bills by ID
    fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>>;
}

/// Cost Accounting repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait CostAccountingRepository: Send + Sync {
    // Item cost operations
    /// Get item cost by SKU
    fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>>;

    /// Set/update item cost
    fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost>;

    /// List item costs
    fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>>;

    /// Update average cost (called when receiving inventory)
    fn update_average_cost(
        &self,
        sku: &str,
        quantity: rust_decimal::Decimal,
        unit_cost: rust_decimal::Decimal,
    ) -> Result<ItemCost>;

    /// Update last cost
    fn update_last_cost(&self, sku: &str, unit_cost: rust_decimal::Decimal) -> Result<ItemCost>;

    // Cost layer operations (for FIFO/LIFO)
    /// Create a cost layer
    fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer>;

    /// Get cost layer by ID
    fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>>;

    /// List cost layers
    fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>>;

    /// Issue from cost layers (FIFO)
    fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>>;

    /// Issue from cost layers (LIFO)
    fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>>;

    /// Get remaining quantity in layers for SKU
    fn get_layers_remaining(&self, sku: &str) -> Result<rust_decimal::Decimal>;

    // Cost transaction operations
    /// Record a cost transaction
    #[allow(clippy::too_many_arguments)]
    fn record_cost_transaction(
        &self,
        sku: &str,
        transaction_type: CostTransactionType,
        quantity: rust_decimal::Decimal,
        unit_cost: rust_decimal::Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction>;

    /// List cost transactions
    fn list_cost_transactions(&self, filter: CostTransactionFilter)
    -> Result<Vec<CostTransaction>>;

    // Cost variance operations
    /// Record a cost variance
    fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance>;

    /// List cost variances
    fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>>;

    /// Get variance summary for period
    fn get_variance_summary(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<rust_decimal::Decimal>;

    // Cost adjustment operations
    /// Create a cost adjustment
    fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment>;

    /// Get adjustment by ID
    fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>>;

    /// List adjustments
    fn list_adjustments(&self, filter: CostAdjustmentFilter) -> Result<Vec<CostAdjustment>>;

    /// Approve adjustment
    fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment>;

    /// Apply adjustment (update item cost)
    fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment>;

    /// Reject adjustment
    fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment>;

    // Rollup operations
    /// Calculate cost rollup for manufactured item
    fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup>;

    /// Get latest rollup for SKU
    fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>>;

    // Valuation operations
    /// Get inventory valuation
    fn get_inventory_valuation(&self, cost_method: CostMethod) -> Result<InventoryValuation>;

    /// Get SKU cost summary
    fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>>;

    /// Get total inventory value
    fn get_total_inventory_value(&self) -> Result<rust_decimal::Decimal>;
}

/// Credit Management repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait CreditRepository: Send + Sync {
    // Credit account operations
    /// Create a credit account for a customer
    fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount>;

    /// Get credit account by ID
    fn get_credit_account(&self, id: CreditId) -> Result<Option<CreditAccount>>;

    /// Get credit account by customer ID
    fn get_credit_account_by_customer(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CreditAccount>>;

    /// Update credit account
    fn update_credit_account(
        &self,
        id: CreditId,
        input: UpdateCreditAccount,
    ) -> Result<CreditAccount>;

    /// List credit accounts
    fn list_credit_accounts(&self, filter: CreditAccountFilter) -> Result<Vec<CreditAccount>>;

    /// Adjust credit limit
    fn adjust_credit_limit(
        &self,
        customer_id: CustomerId,
        new_limit: rust_decimal::Decimal,
        reason: &str,
    ) -> Result<CreditAccount>;

    /// Suspend credit account
    fn suspend_credit_account(
        &self,
        customer_id: CustomerId,
        reason: &str,
    ) -> Result<CreditAccount>;

    /// Reactivate credit account
    fn reactivate_credit_account(&self, customer_id: CustomerId) -> Result<CreditAccount>;

    // Credit check operations
    /// Check credit for an order
    fn check_credit(
        &self,
        customer_id: CustomerId,
        order_amount: rust_decimal::Decimal,
    ) -> Result<CreditCheckResult>;

    /// Reserve credit for an order
    fn reserve_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: rust_decimal::Decimal,
    ) -> Result<CreditAccount>;

    /// Release credit reservation
    fn release_credit_reservation(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
    ) -> Result<CreditAccount>;

    /// Charge credit (convert reservation to balance)
    fn charge_credit(
        &self,
        customer_id: CustomerId,
        order_id: OrderId,
        amount: rust_decimal::Decimal,
    ) -> Result<CreditAccount>;

    // Credit hold operations
    /// Place a credit hold
    fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold>;

    /// Get credit hold by ID
    fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>>;

    /// List credit holds
    fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>>;

    /// Release a credit hold
    fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold>;

    /// Get active holds for customer
    fn get_active_holds(&self, customer_id: CustomerId) -> Result<Vec<CreditHold>>;

    /// Get active holds for order
    fn get_holds_for_order(&self, order_id: OrderId) -> Result<Vec<CreditHold>>;

    // Credit application operations
    /// Submit a credit application
    fn submit_application(&self, input: SubmitCreditApplication) -> Result<CreditApplication>;

    /// Get credit application by ID
    fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>>;

    /// List credit applications
    fn list_applications(&self, filter: CreditApplicationFilter) -> Result<Vec<CreditApplication>>;

    /// Review credit application
    fn review_application(&self, input: ReviewCreditApplication) -> Result<CreditApplication>;

    /// Withdraw credit application
    fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication>;

    // Transaction operations
    /// Record a credit transaction
    fn record_transaction(&self, input: RecordCreditTransaction) -> Result<CreditTransaction>;

    /// List credit transactions
    fn list_transactions(&self, filter: CreditTransactionFilter) -> Result<Vec<CreditTransaction>>;

    /// Apply payment to balance
    fn apply_payment(
        &self,
        customer_id: CustomerId,
        amount: rust_decimal::Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount>;

    // Analytics
    /// Get customer credit summary
    fn get_customer_summary(
        &self,
        customer_id: CustomerId,
    ) -> Result<Option<CustomerCreditSummary>>;

    /// Get credit aging buckets
    fn get_aging_report(&self) -> Result<Vec<(CustomerId, CreditAgingBucket)>>;

    /// Get customers over credit limit
    fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>>;
}

// ============================================================================
// Accounts Receivable Repository
// ============================================================================

/// Accounts Receivable repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait AccountsReceivableRepository: Send + Sync {
    // Aging reports
    /// Get AR aging summary across all customers
    fn get_aging_summary(&self) -> Result<ArAgingSummary>;

    /// Get aging by customer (None if customer is not found)
    fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>>;

    /// Get all customers with aging (AR aging report)
    fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>>;

    // Collection management
    /// Log collection activity
    fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity>;

    /// Get collection activities
    fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>>;

    /// Update invoice collection status
    fn update_collection_status(
        &self,
        invoice_id: InvoiceId,
        status: CollectionStatus,
    ) -> Result<()>;

    /// Get invoices due for dunning (based on aging)
    fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>>;

    /// Send dunning letter (records activity, updates status)
    fn send_dunning_letter(
        &self,
        invoice_id: InvoiceId,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity>;

    // Write-offs
    /// Create a write-off
    fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff>;

    /// Get write-off by ID
    fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>>;

    /// List write-offs
    fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>>;

    /// Reverse a write-off
    fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff>;

    // Credit memos
    /// Create a credit memo
    fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo>;

    /// Get credit memo by ID
    fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>>;

    /// Get credit memo by number
    fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>>;

    /// List credit memos
    fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>>;

    /// Apply credit memo to invoice
    fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo>;

    /// Void credit memo
    fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo>;

    /// Get unapplied credit memos for customer
    fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>>;

    // Payment application
    /// Apply payment to invoices
    fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>>;

    /// Get payment applications
    fn get_payment_applications(&self, payment_id: Uuid) -> Result<Vec<ArPaymentApplication>>;

    /// Unapply payment from invoice
    fn unapply_payment(&self, application_id: Uuid) -> Result<()>;

    // Customer summaries and statements
    /// Get customer AR summary (None if customer is not found)
    fn get_customer_summary(&self, customer_id: Uuid) -> Result<Option<CustomerArSummary>>;

    /// Generate customer statement
    fn generate_statement(&self, request: GenerateStatementRequest) -> Result<CustomerStatement>;

    // Analytics
    /// Get total AR outstanding
    fn get_total_outstanding(&self) -> Result<rust_decimal::Decimal>;

    /// Get Days Sales Outstanding (DSO)
    fn get_dso(&self, days: i32) -> Result<rust_decimal::Decimal>;

    /// Get average days to pay by customer
    fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>>;

    // Batch operations
    fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>>;
}

// ============================================================================
// General Ledger Repository
// ============================================================================

/// General Ledger repository trait
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait GeneralLedgerRepository: Send + Sync {
    // Chart of Accounts
    /// Create a GL account
    fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount>;

    /// Get account by ID
    fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>>;

    /// Get account by account number
    fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>>;

    /// Update account
    fn update_account(&self, id: Uuid, input: UpdateGlAccount) -> Result<GlAccount>;

    /// List accounts (Chart of Accounts)
    fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>>;

    /// Get account hierarchy (parent-child)
    fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>>;

    /// Delete account (only if no transactions)
    fn delete_account(&self, id: Uuid) -> Result<()>;

    /// Initialize default Chart of Accounts
    fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>>;

    // GL Periods
    /// Create a GL period
    fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod>;

    /// Get period by ID
    fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>>;

    /// Get current open period
    fn get_current_period(&self) -> Result<Option<GlPeriod>>;

    /// Get period for a date
    fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>>;

    /// List periods
    fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>>;

    /// Open a period
    fn open_period(&self, id: Uuid) -> Result<GlPeriod>;

    /// Close a period
    fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod>;

    /// Lock a period (prevents any changes)
    fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod>;

    /// Reopen a closed period (not locked)
    fn reopen_period(&self, id: Uuid) -> Result<GlPeriod>;

    // Journal Entries
    /// Create a journal entry
    fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry>;

    /// Get journal entry by ID
    fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>>;

    /// Get journal entry by number
    fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>>;

    /// List journal entries
    fn list_journal_entries(&self, filter: JournalEntryFilter) -> Result<Vec<JournalEntry>>;

    /// Post a journal entry (update account balances)
    fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry>;

    /// Void a journal entry
    fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry>;

    /// Reverse a journal entry (creates reversing entry)
    fn reverse_journal_entry(&self, id: Uuid, reversal_date: NaiveDate) -> Result<JournalEntry>;

    /// Get journal entry lines
    fn get_journal_entry_lines(&self, journal_entry_id: Uuid) -> Result<Vec<JournalEntryLine>>;

    // Auto-posting
    /// Get active auto-posting config
    fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>>;

    /// Create/update auto-posting config
    fn set_auto_posting_config(&self, input: CreateAutoPostingConfig) -> Result<AutoPostingConfig>;

    /// Auto-post invoice creation (DR AR / CR Revenue)
    fn auto_post_invoice(&self, invoice_id: InvoiceId) -> Result<JournalEntry>;

    /// Auto-post payment received (DR Cash / CR AR)
    fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post bill creation (DR Expense / CR AP)
    fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post bill payment (DR AP / CR Cash)
    fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post inventory cost transaction (DR/CR Inventory/COGS)
    fn auto_post_inventory_cost(&self, cost_transaction_id: Uuid) -> Result<JournalEntry>;

    /// Auto-post write-off (DR Bad Debt / CR AR)
    fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry>;

    // Financial Reports
    /// Generate trial balance
    fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance>;

    /// Generate balance sheet
    fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet>;

    /// Generate income statement
    fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement>;

    /// Get account balance (None if account is not found)
    fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<rust_decimal::Decimal>>;

    /// Get account transaction history
    fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>>;

    // Period close process
    /// Run period close (creates closing entries)
    fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry>;

    // FX revaluation
    /// Revalue foreign-currency account balances at the as-of exchange rate,
    /// posting the net unrealized gain/loss as a balanced adjusting entry.
    ///
    /// `base_currency` defaults to the store's configured base currency.
    fn revalue(
        &self,
        as_of_date: NaiveDate,
        base_currency: Option<Currency>,
    ) -> Result<RevaluationResult>;

    // Batch operations
    fn create_accounts_batch(&self, inputs: Vec<CreateGlAccount>)
    -> Result<BatchResult<GlAccount>>;
    fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>>;
}

/// Vendor credit repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait VendorCreditRepository: Send + Sync {
    /// Create a new vendor credit.
    fn create(&self, input: CreateVendorCredit) -> Result<VendorCredit>;

    /// Get a vendor credit by ID.
    fn get(&self, id: VendorCreditId) -> Result<Option<VendorCredit>>;

    /// List vendor credits with filter.
    fn list(&self, filter: VendorCreditFilter) -> Result<Vec<VendorCredit>>;

    /// Apply a vendor credit against a bill or payment obligation. Decrements
    /// the remaining balance and records an application.
    fn apply(&self, id: VendorCreditId, input: ApplyVendorCredit) -> Result<VendorCredit>;

    /// List applications for a vendor credit.
    fn list_applications(&self, id: VendorCreditId) -> Result<Vec<VendorCreditApplication>>;

    /// Reverse a previously-recorded application, restoring the balance.
    fn reverse_application(
        &self,
        id: VendorCreditId,
        application_id: VendorCreditApplicationId,
    ) -> Result<VendorCredit>;

    /// Cancel a vendor credit (only when it has no active applications).
    fn cancel(&self, id: VendorCreditId) -> Result<VendorCredit>;
}

/// Fixed asset register repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait FixedAssetRepository: Send + Sync {
    /// Create a new fixed asset (draft, or in-service when an in-service date is supplied).
    fn create(&self, input: CreateFixedAsset) -> Result<FixedAsset>;

    /// Get a fixed asset by ID.
    fn get(&self, id: uuid::Uuid) -> Result<Option<FixedAsset>>;

    /// List fixed assets with filter.
    fn list(&self, filter: FixedAssetFilter) -> Result<Vec<FixedAsset>>;

    /// Update a fixed asset (partial). Terminal assets cannot be updated.
    fn update(&self, id: uuid::Uuid, input: UpdateFixedAsset) -> Result<FixedAsset>;

    /// Place a draft asset in service on the given date.
    fn place_in_service(&self, id: uuid::Uuid, date: chrono::NaiveDate) -> Result<FixedAsset>;

    /// Dispose of an asset for the given proceeds, recording gain/loss.
    fn dispose(
        &self,
        id: uuid::Uuid,
        date: chrono::NaiveDate,
        proceeds: rust_decimal::Decimal,
        notes: Option<String>,
    ) -> Result<FixedAsset>;

    /// Write off an asset (disposal with zero proceeds).
    fn write_off(
        &self,
        id: uuid::Uuid,
        date: chrono::NaiveDate,
        notes: Option<String>,
    ) -> Result<FixedAsset>;

    /// Generate and persist the depreciation schedule for an asset,
    /// replacing any previously scheduled (unposted) entries.
    fn generate_schedule(&self, id: uuid::Uuid) -> Result<DepreciationSchedule>;

    /// Get the persisted depreciation schedule for an asset, if generated.
    fn get_schedule(&self, id: uuid::Uuid) -> Result<Option<DepreciationSchedule>>;

    /// Post the next `periods` scheduled depreciation entries, advancing
    /// accumulated depreciation (and status when fully depreciated).
    fn post_depreciation(&self, id: uuid::Uuid, periods: u32) -> Result<FixedAsset>;
}

/// Revenue recognition (ASC 606) repository trait.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait RevenueRecognitionRepository: Send + Sync {
    /// Create a new revenue contract with its performance obligations.
    fn create_contract(&self, input: CreateRevenueContract) -> Result<RevenueContract>;

    /// Get a revenue contract by ID (with obligations).
    fn get_contract(&self, id: uuid::Uuid) -> Result<Option<RevenueContract>>;

    /// List revenue contracts with filter.
    fn list_contracts(&self, filter: RevenueContractFilter) -> Result<Vec<RevenueContract>>;

    /// Update a revenue contract (partial); status changes are transition-guarded.
    fn update_contract(
        &self,
        id: uuid::Uuid,
        input: UpdateRevenueContract,
    ) -> Result<RevenueContract>;

    /// List the performance obligations under a contract.
    fn list_obligations(&self, contract_id: uuid::Uuid) -> Result<Vec<PerformanceObligation>>;

    /// Generate and persist the recognition schedule for an obligation,
    /// replacing any previously deferred (unrecognized) entries.
    fn generate_schedule(&self, obligation_id: uuid::Uuid) -> Result<RevenueSchedule>;

    /// Get the persisted recognition schedule for an obligation, if generated.
    fn get_schedule(&self, obligation_id: uuid::Uuid) -> Result<Option<RevenueSchedule>>;

    /// Recognize all deferred entries with a period start on or before
    /// `through`, advancing the obligation's recognized amount (and the
    /// contract status when fully recognized).
    fn recognize_period(
        &self,
        obligation_id: uuid::Uuid,
        through: chrono::NaiveDate,
    ) -> Result<RevenueSchedule>;
}
