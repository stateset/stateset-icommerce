//! Finance accessors: AP, cost accounting, credit, backorders, AR, general ledger.

use super::*;

/// Async accounts payable operations.
pub struct AsyncAccountsPayable {
    db: Arc<PostgresDatabase>,
}

impl AsyncAccountsPayable {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_bill(&self, input: CreateBill) -> Result<Bill> {
        self.db.accounts_payable().create_bill_async(input).await
    }

    pub async fn get_bill(&self, id: Uuid) -> Result<Option<Bill>> {
        self.db.accounts_payable().get_bill_async(id).await
    }

    pub async fn get_bill_by_number(&self, number: &str) -> Result<Option<Bill>> {
        self.db.accounts_payable().get_bill_by_number_async(number).await
    }

    pub async fn update_bill(&self, id: Uuid, input: UpdateBill) -> Result<Bill> {
        self.db.accounts_payable().update_bill_async(id, input).await
    }

    pub async fn list_bills(&self, filter: BillFilter) -> Result<Vec<Bill>> {
        self.db.accounts_payable().list_bills_async(filter).await
    }

    pub async fn delete_bill(&self, id: Uuid) -> Result<()> {
        self.db.accounts_payable().delete_bill_async(id).await
    }

    pub async fn approve_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().approve_bill_async(id).await
    }

    pub async fn cancel_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().cancel_bill_async(id).await
    }

    pub async fn dispute_bill(&self, id: Uuid) -> Result<Bill> {
        self.db.accounts_payable().dispute_bill_async(id).await
    }

    pub async fn get_bill_items(&self, bill_id: Uuid) -> Result<Vec<BillItem>> {
        self.db.accounts_payable().get_bill_items_async(bill_id).await
    }

    pub async fn add_bill_item(&self, bill_id: Uuid, item: CreateBillItem) -> Result<BillItem> {
        self.db.accounts_payable().add_bill_item_async(bill_id, item).await
    }

    pub async fn remove_bill_item(&self, item_id: Uuid) -> Result<()> {
        self.db.accounts_payable().remove_bill_item_async(item_id).await
    }

    pub async fn count_bills(&self, filter: BillFilter) -> Result<u64> {
        self.db.accounts_payable().count_bills_async(filter).await
    }

    pub async fn get_overdue_bills(&self) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_overdue_bills_async().await
    }

    pub async fn get_bills_due_soon(&self, days: i32) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_bills_due_soon_async(days).await
    }

    pub async fn create_payment(&self, input: CreateBillPayment) -> Result<BillPayment> {
        self.db.accounts_payable().create_payment_async(input).await
    }

    pub async fn get_payment(&self, id: Uuid) -> Result<Option<BillPayment>> {
        self.db.accounts_payable().get_payment_async(id).await
    }

    pub async fn get_payment_by_number(&self, number: &str) -> Result<Option<BillPayment>> {
        self.db.accounts_payable().get_payment_by_number_async(number).await
    }

    pub async fn list_payments(&self, filter: BillPaymentFilter) -> Result<Vec<BillPayment>> {
        self.db.accounts_payable().list_payments_async(filter).await
    }

    pub async fn void_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.db.accounts_payable().void_payment_async(id).await
    }

    pub async fn clear_payment(&self, id: Uuid) -> Result<BillPayment> {
        self.db.accounts_payable().clear_payment_async(id).await
    }

    pub async fn get_payment_allocations(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<PaymentAllocation>> {
        self.db.accounts_payable().get_payment_allocations_async(payment_id).await
    }

    pub async fn get_payments_for_bill(&self, bill_id: Uuid) -> Result<Vec<BillPayment>> {
        self.db.accounts_payable().get_payments_for_bill_async(bill_id).await
    }

    pub async fn count_payments(&self, filter: BillPaymentFilter) -> Result<u64> {
        self.db.accounts_payable().count_payments_async(filter).await
    }

    pub async fn create_payment_run(&self, input: CreatePaymentRun) -> Result<PaymentRun> {
        self.db.accounts_payable().create_payment_run_async(input).await
    }

    pub async fn get_payment_run(&self, id: Uuid) -> Result<Option<PaymentRun>> {
        self.db.accounts_payable().get_payment_run_async(id).await
    }

    pub async fn list_payment_runs(&self, filter: PaymentRunFilter) -> Result<Vec<PaymentRun>> {
        self.db.accounts_payable().list_payment_runs_async(filter).await
    }

    pub async fn approve_payment_run(&self, id: Uuid, approved_by: &str) -> Result<PaymentRun> {
        self.db.accounts_payable().approve_payment_run_async(id, approved_by).await
    }

    pub async fn process_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.db.accounts_payable().process_payment_run_async(id).await
    }

    pub async fn cancel_payment_run(&self, id: Uuid) -> Result<PaymentRun> {
        self.db.accounts_payable().cancel_payment_run_async(id).await
    }

    pub async fn get_payment_run_bills(&self, run_id: Uuid) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_payment_run_bills_async(run_id).await
    }

    pub async fn get_aging_summary(&self) -> Result<ApAgingSummary> {
        self.db.accounts_payable().get_aging_summary_async().await
    }

    pub async fn get_supplier_summary(
        &self,
        supplier_id: Uuid,
    ) -> Result<Option<SupplierApSummary>> {
        self.db.accounts_payable().get_supplier_summary_async(supplier_id).await
    }

    pub async fn get_total_outstanding(&self) -> Result<Decimal> {
        self.db.accounts_payable().get_total_outstanding_async().await
    }

    pub async fn create_bills_batch(&self, inputs: Vec<CreateBill>) -> Result<BatchResult<Bill>> {
        self.db.accounts_payable().create_bills_batch_async(inputs).await
    }

    pub async fn get_bills_batch(&self, ids: Vec<Uuid>) -> Result<Vec<Bill>> {
        self.db.accounts_payable().get_bills_batch_async(ids).await
    }
}

// ============================================================================
// Async Cost Accounting
// ============================================================================

/// Async cost accounting operations.
pub struct AsyncCostAccounting {
    db: Arc<PostgresDatabase>,
}

impl AsyncCostAccounting {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn get_item_cost(&self, sku: &str) -> Result<Option<ItemCost>> {
        self.db.cost_accounting().get_item_cost_async(sku).await
    }

    pub async fn set_item_cost(&self, input: SetItemCost) -> Result<ItemCost> {
        self.db.cost_accounting().set_item_cost_async(input).await
    }

    pub async fn list_item_costs(&self, filter: ItemCostFilter) -> Result<Vec<ItemCost>> {
        self.db.cost_accounting().list_item_costs_async(filter).await
    }

    pub async fn update_average_cost(
        &self,
        sku: &str,
        quantity: Decimal,
        unit_cost: Decimal,
    ) -> Result<ItemCost> {
        self.db.cost_accounting().update_average_cost_async(sku, quantity, unit_cost).await
    }

    pub async fn update_last_cost(&self, sku: &str, unit_cost: Decimal) -> Result<ItemCost> {
        self.db.cost_accounting().update_last_cost_async(sku, unit_cost).await
    }

    pub async fn create_cost_layer(&self, input: CreateCostLayer) -> Result<CostLayer> {
        self.db.cost_accounting().create_cost_layer_async(input).await
    }

    pub async fn get_cost_layer(&self, id: Uuid) -> Result<Option<CostLayer>> {
        self.db.cost_accounting().get_cost_layer_async(id).await
    }

    pub async fn list_cost_layers(&self, filter: CostLayerFilter) -> Result<Vec<CostLayer>> {
        self.db.cost_accounting().list_cost_layers_async(filter).await
    }

    pub async fn issue_fifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().issue_fifo_async(input).await
    }

    pub async fn issue_lifo(&self, input: IssueCostLayers) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().issue_lifo_async(input).await
    }

    pub async fn get_layers_remaining(&self, sku: &str) -> Result<Decimal> {
        self.db.cost_accounting().get_layers_remaining_async(sku).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_cost_transaction(
        &self,
        sku: &str,
        transaction_type: stateset_core::CostTransactionType,
        quantity: Decimal,
        unit_cost: Decimal,
        layer_id: Option<Uuid>,
        reference_type: Option<&str>,
        reference_id: Option<Uuid>,
        notes: Option<&str>,
    ) -> Result<CostTransaction> {
        self.db
            .cost_accounting()
            .record_cost_transaction_async(
                sku,
                transaction_type,
                quantity,
                unit_cost,
                layer_id,
                reference_type,
                reference_id,
                notes,
            )
            .await
    }

    pub async fn list_cost_transactions(
        &self,
        filter: CostTransactionFilter,
    ) -> Result<Vec<CostTransaction>> {
        self.db.cost_accounting().list_cost_transactions_async(filter).await
    }

    pub async fn record_variance(&self, input: RecordCostVariance) -> Result<CostVariance> {
        self.db.cost_accounting().record_variance_async(input).await
    }

    pub async fn list_variances(&self, filter: CostVarianceFilter) -> Result<Vec<CostVariance>> {
        self.db.cost_accounting().list_variances_async(filter).await
    }

    pub async fn get_variance_summary(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Decimal> {
        self.db.cost_accounting().get_variance_summary_async(from, to).await
    }

    pub async fn create_adjustment(&self, input: CreateCostAdjustment) -> Result<CostAdjustment> {
        self.db.cost_accounting().create_adjustment_async(input).await
    }

    pub async fn get_adjustment(&self, id: Uuid) -> Result<Option<CostAdjustment>> {
        self.db.cost_accounting().get_adjustment_async(id).await
    }

    pub async fn list_adjustments(
        &self,
        filter: CostAdjustmentFilter,
    ) -> Result<Vec<CostAdjustment>> {
        self.db.cost_accounting().list_adjustments_async(filter).await
    }

    pub async fn approve_adjustment(&self, id: Uuid, approved_by: &str) -> Result<CostAdjustment> {
        self.db.cost_accounting().approve_adjustment_async(id, approved_by).await
    }

    pub async fn apply_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.db.cost_accounting().apply_adjustment_async(id).await
    }

    pub async fn reject_adjustment(&self, id: Uuid) -> Result<CostAdjustment> {
        self.db.cost_accounting().reject_adjustment_async(id).await
    }

    pub async fn calculate_rollup(&self, sku: &str, bom_id: Option<Uuid>) -> Result<CostRollup> {
        self.db.cost_accounting().calculate_rollup_async(sku, bom_id).await
    }

    pub async fn get_rollup(&self, sku: &str) -> Result<Option<CostRollup>> {
        self.db.cost_accounting().get_rollup_async(sku).await
    }

    pub async fn get_inventory_valuation(
        &self,
        cost_method: CostMethod,
    ) -> Result<InventoryValuation> {
        self.db.cost_accounting().get_inventory_valuation_async(cost_method).await
    }

    pub async fn get_sku_cost_summary(&self, sku: &str) -> Result<Option<SkuCostSummary>> {
        self.db.cost_accounting().get_sku_cost_summary_async(sku).await
    }

    pub async fn get_total_inventory_value(&self) -> Result<Decimal> {
        self.db.cost_accounting().get_total_inventory_value_async().await
    }
}

// ============================================================================
// Async Credit
// ============================================================================

/// Async credit operations.
pub struct AsyncCredit {
    db: Arc<PostgresDatabase>,
}

impl AsyncCredit {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_credit_account(&self, input: CreateCreditAccount) -> Result<CreditAccount> {
        self.db.credit().create_credit_account_async(input).await
    }

    pub async fn get_credit_account(&self, id: Uuid) -> Result<Option<CreditAccount>> {
        self.db.credit().get_credit_account_async(id).await
    }

    pub async fn get_credit_account_by_customer(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CreditAccount>> {
        self.db.credit().get_credit_account_by_customer_async(customer_id).await
    }

    pub async fn update_credit_account(
        &self,
        id: Uuid,
        input: UpdateCreditAccount,
    ) -> Result<CreditAccount> {
        self.db.credit().update_credit_account_async(id, input).await
    }

    pub async fn list_credit_accounts(
        &self,
        filter: CreditAccountFilter,
    ) -> Result<Vec<CreditAccount>> {
        self.db.credit().list_credit_accounts_async(filter).await
    }

    pub async fn adjust_credit_limit(
        &self,
        customer_id: Uuid,
        new_limit: Decimal,
        reason: &str,
    ) -> Result<CreditAccount> {
        self.db.credit().adjust_credit_limit_async(customer_id, new_limit, reason).await
    }

    pub async fn suspend_credit_account(
        &self,
        customer_id: Uuid,
        reason: &str,
    ) -> Result<CreditAccount> {
        self.db.credit().suspend_credit_account_async(customer_id, reason).await
    }

    pub async fn reactivate_credit_account(&self, customer_id: Uuid) -> Result<CreditAccount> {
        self.db.credit().reactivate_credit_account_async(customer_id).await
    }

    pub async fn check_credit(
        &self,
        customer_id: Uuid,
        order_amount: Decimal,
    ) -> Result<CreditCheckResult> {
        self.db.credit().check_credit_async(customer_id, order_amount).await
    }

    pub async fn reserve_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        self.db.credit().reserve_credit_async(customer_id, order_id, amount).await
    }

    pub async fn release_credit_reservation(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
    ) -> Result<CreditAccount> {
        self.db.credit().release_credit_reservation_async(customer_id, order_id).await
    }

    pub async fn charge_credit(
        &self,
        customer_id: Uuid,
        order_id: Uuid,
        amount: Decimal,
    ) -> Result<CreditAccount> {
        self.db.credit().charge_credit_async(customer_id, order_id, amount).await
    }

    pub async fn place_hold(&self, input: PlaceCreditHold) -> Result<CreditHold> {
        self.db.credit().place_hold_async(input).await
    }

    pub async fn get_hold(&self, id: Uuid) -> Result<Option<CreditHold>> {
        self.db.credit().get_hold_async(id).await
    }

    pub async fn list_holds(&self, filter: CreditHoldFilter) -> Result<Vec<CreditHold>> {
        self.db.credit().list_holds_async(filter).await
    }

    pub async fn release_hold(&self, input: ReleaseCreditHold) -> Result<CreditHold> {
        self.db.credit().release_hold_async(input).await
    }

    pub async fn get_active_holds(&self, customer_id: Uuid) -> Result<Vec<CreditHold>> {
        self.db.credit().get_active_holds_async(customer_id).await
    }

    pub async fn get_holds_for_order(&self, order_id: Uuid) -> Result<Vec<CreditHold>> {
        self.db.credit().get_holds_for_order_async(order_id).await
    }

    pub async fn submit_application(
        &self,
        input: SubmitCreditApplication,
    ) -> Result<CreditApplication> {
        self.db.credit().submit_application_async(input).await
    }

    pub async fn get_application(&self, id: Uuid) -> Result<Option<CreditApplication>> {
        self.db.credit().get_application_async(id).await
    }

    pub async fn list_applications(
        &self,
        filter: CreditApplicationFilter,
    ) -> Result<Vec<CreditApplication>> {
        self.db.credit().list_applications_async(filter).await
    }

    pub async fn review_application(
        &self,
        input: ReviewCreditApplication,
    ) -> Result<CreditApplication> {
        self.db.credit().review_application_async(input).await
    }

    pub async fn withdraw_application(&self, id: Uuid) -> Result<CreditApplication> {
        self.db.credit().withdraw_application_async(id).await
    }

    pub async fn record_transaction(
        &self,
        input: RecordCreditTransaction,
    ) -> Result<CreditTransaction> {
        self.db.credit().record_transaction_async(input).await
    }

    pub async fn list_transactions(
        &self,
        filter: CreditTransactionFilter,
    ) -> Result<Vec<CreditTransaction>> {
        self.db.credit().list_transactions_async(filter).await
    }

    pub async fn apply_payment(
        &self,
        customer_id: Uuid,
        amount: Decimal,
        reference_id: Option<Uuid>,
    ) -> Result<CreditAccount> {
        self.db.credit().apply_payment_async(customer_id, amount, reference_id).await
    }

    pub async fn get_customer_summary(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CustomerCreditSummary>> {
        self.db.credit().get_customer_summary_async(customer_id).await
    }

    pub async fn get_aging_report(&self) -> Result<Vec<(Uuid, CreditAgingBucket)>> {
        Ok(self
            .db
            .credit()
            .get_aging_report_async()
            .await?
            .into_iter()
            .map(|(customer_id, bucket)| (customer_id.into_uuid(), bucket))
            .collect())
    }

    pub async fn get_over_limit_customers(&self) -> Result<Vec<CreditAccount>> {
        self.db.credit().get_over_limit_customers_async().await
    }
}

// ============================================================================
// Async Backorder
// ============================================================================

/// Async backorder operations.
pub struct AsyncBackorder {
    db: Arc<PostgresDatabase>,
}

impl AsyncBackorder {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_backorder(&self, input: CreateBackorder) -> Result<Backorder> {
        self.db.backorder().create_backorder_async(input).await
    }

    pub async fn get_backorder(&self, id: Uuid) -> Result<Option<Backorder>> {
        self.db.backorder().get_backorder_async(id).await
    }

    pub async fn get_backorder_by_number(&self, number: &str) -> Result<Option<Backorder>> {
        self.db.backorder().get_backorder_by_number_async(number).await
    }

    pub async fn update_backorder(&self, id: Uuid, input: UpdateBackorder) -> Result<Backorder> {
        self.db.backorder().update_backorder_async(id, input).await
    }

    pub async fn list_backorders(&self, filter: BackorderFilter) -> Result<Vec<Backorder>> {
        self.db.backorder().list_backorders_async(filter).await
    }

    pub async fn cancel_backorder(&self, id: Uuid) -> Result<Backorder> {
        self.db.backorder().cancel_backorder_async(id).await
    }

    pub async fn get_backorders_for_order(&self, order_id: Uuid) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_order_async(order_id).await
    }

    pub async fn get_backorders_for_customer(&self, customer_id: Uuid) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_customer_async(customer_id).await
    }

    pub async fn get_backorders_for_sku(&self, sku: &str) -> Result<Vec<Backorder>> {
        self.db.backorder().get_backorders_for_sku_async(sku).await
    }

    pub async fn fulfill_backorder(&self, input: FulfillBackorder) -> Result<Backorder> {
        self.db.backorder().fulfill_backorder_async(input).await
    }

    pub async fn get_fulfillment_history(
        &self,
        backorder_id: Uuid,
    ) -> Result<Vec<BackorderFulfillment>> {
        self.db.backorder().get_fulfillment_history_async(backorder_id).await
    }

    pub async fn allocate_backorder(
        &self,
        input: AllocateBackorder,
    ) -> Result<BackorderAllocation> {
        self.db.backorder().allocate_backorder_async(input).await
    }

    pub async fn get_allocations(&self, backorder_id: Uuid) -> Result<Vec<BackorderAllocation>> {
        self.db.backorder().get_allocations_async(backorder_id).await
    }

    pub async fn release_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.db.backorder().release_allocation_async(allocation_id).await
    }

    pub async fn confirm_allocation(&self, allocation_id: Uuid) -> Result<BackorderAllocation> {
        self.db.backorder().confirm_allocation_async(allocation_id).await
    }

    pub async fn expire_allocations(&self) -> Result<u32> {
        self.db.backorder().expire_allocations_async().await
    }

    pub async fn auto_allocate_inventory(&self, sku: &str) -> Result<Vec<BackorderAllocation>> {
        self.db.backorder().auto_allocate_inventory_async(sku).await
    }

    pub async fn get_summary(&self) -> Result<BackorderSummary> {
        self.db.backorder().get_summary_async().await
    }

    pub async fn get_sku_summary(&self, sku: &str) -> Result<Option<SkuBackorderSummary>> {
        self.db.backorder().get_sku_summary_async(sku).await
    }

    pub async fn get_overdue_backorders(&self) -> Result<Vec<Backorder>> {
        self.db.backorder().get_overdue_backorders_async().await
    }

    pub async fn count_pending(&self) -> Result<u64> {
        self.db.backorder().count_pending_async().await
    }
}

// ============================================================================
// Async Accounts Receivable
// ============================================================================

/// Async accounts receivable operations.
pub struct AsyncAccountsReceivable {
    db: Arc<PostgresDatabase>,
}

impl AsyncAccountsReceivable {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn get_aging_summary(&self) -> Result<ArAgingSummary> {
        self.db.accounts_receivable().get_aging_summary_async().await
    }

    pub async fn get_customer_aging(&self, customer_id: Uuid) -> Result<Option<CustomerArAging>> {
        self.db.accounts_receivable().get_customer_aging_async(customer_id).await
    }

    pub async fn get_aging_report(&self, filter: ArAgingFilter) -> Result<Vec<CustomerArAging>> {
        self.db.accounts_receivable().get_aging_report_async(filter).await
    }

    pub async fn log_collection_activity(
        &self,
        input: CreateCollectionActivity,
    ) -> Result<CollectionActivity> {
        self.db.accounts_receivable().log_collection_activity_async(input).await
    }

    pub async fn list_collection_activities(
        &self,
        filter: CollectionActivityFilter,
    ) -> Result<Vec<CollectionActivity>> {
        self.db.accounts_receivable().list_collection_activities_async(filter).await
    }

    pub async fn update_collection_status(
        &self,
        invoice_id: Uuid,
        status: CollectionStatus,
    ) -> Result<()> {
        self.db.accounts_receivable().update_collection_status_async(invoice_id, status).await
    }

    pub async fn get_invoices_due_for_dunning(&self) -> Result<Vec<Invoice>> {
        self.db.accounts_receivable().get_invoices_due_for_dunning_async().await
    }

    pub async fn send_dunning_letter(
        &self,
        invoice_id: Uuid,
        letter_type: DunningLetterType,
        sent_by: Option<&str>,
    ) -> Result<CollectionActivity> {
        self.db
            .accounts_receivable()
            .send_dunning_letter_async(invoice_id, letter_type, sent_by)
            .await
    }

    pub async fn create_write_off(&self, input: CreateWriteOff) -> Result<WriteOff> {
        self.db.accounts_receivable().create_write_off_async(input).await
    }

    pub async fn get_write_off(&self, id: Uuid) -> Result<Option<WriteOff>> {
        self.db.accounts_receivable().get_write_off_async(id).await
    }

    pub async fn list_write_offs(&self, filter: WriteOffFilter) -> Result<Vec<WriteOff>> {
        self.db.accounts_receivable().list_write_offs_async(filter).await
    }

    pub async fn reverse_write_off(&self, id: Uuid) -> Result<WriteOff> {
        self.db.accounts_receivable().reverse_write_off_async(id).await
    }

    pub async fn create_credit_memo(&self, input: CreateCreditMemo) -> Result<CreditMemo> {
        self.db.accounts_receivable().create_credit_memo_async(input).await
    }

    pub async fn get_credit_memo(&self, id: Uuid) -> Result<Option<CreditMemo>> {
        self.db.accounts_receivable().get_credit_memo_async(id).await
    }

    pub async fn get_credit_memo_by_number(&self, number: &str) -> Result<Option<CreditMemo>> {
        self.db.accounts_receivable().get_credit_memo_by_number_async(number).await
    }

    pub async fn list_credit_memos(&self, filter: CreditMemoFilter) -> Result<Vec<CreditMemo>> {
        self.db.accounts_receivable().list_credit_memos_async(filter).await
    }

    pub async fn apply_credit_memo(&self, input: ApplyCreditMemo) -> Result<CreditMemo> {
        self.db.accounts_receivable().apply_credit_memo_async(input).await
    }

    pub async fn void_credit_memo(&self, id: Uuid) -> Result<CreditMemo> {
        self.db.accounts_receivable().void_credit_memo_async(id).await
    }

    pub async fn get_unapplied_credits(&self, customer_id: Uuid) -> Result<Vec<CreditMemo>> {
        self.db.accounts_receivable().get_unapplied_credits_async(customer_id).await
    }

    pub async fn apply_payment_to_invoices(
        &self,
        input: ApplyPaymentToInvoices,
    ) -> Result<Vec<ArPaymentApplication>> {
        self.db.accounts_receivable().apply_payment_to_invoices_async(input).await
    }

    pub async fn get_payment_applications(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<ArPaymentApplication>> {
        self.db.accounts_receivable().get_payment_applications_async(payment_id).await
    }

    pub async fn unapply_payment(&self, application_id: Uuid) -> Result<()> {
        self.db.accounts_receivable().unapply_payment_async(application_id).await
    }

    pub async fn get_customer_summary(
        &self,
        customer_id: Uuid,
    ) -> Result<Option<CustomerArSummary>> {
        self.db.accounts_receivable().get_customer_summary_async(customer_id).await
    }

    pub async fn generate_statement(
        &self,
        request: GenerateStatementRequest,
    ) -> Result<CustomerStatement> {
        self.db.accounts_receivable().generate_statement_async(request).await
    }

    pub async fn get_total_outstanding(&self) -> Result<Decimal> {
        self.db.accounts_receivable().get_total_outstanding_async().await
    }

    pub async fn get_dso(&self, days: i32) -> Result<Decimal> {
        self.db.accounts_receivable().get_dso_async(days).await
    }

    pub async fn get_average_days_to_pay(&self, customer_id: Uuid) -> Result<Option<i32>> {
        self.db.accounts_receivable().get_average_days_to_pay_async(customer_id).await
    }

    pub async fn get_customers_batch(&self, ids: Vec<Uuid>) -> Result<Vec<CustomerArSummary>> {
        self.db.accounts_receivable().get_customers_batch_async(ids).await
    }
}

// ============================================================================
// Async General Ledger
// ============================================================================

/// Async general ledger operations.
pub struct AsyncGeneralLedger {
    db: Arc<PostgresDatabase>,
}

impl AsyncGeneralLedger {
    pub(crate) const fn new(db: Arc<PostgresDatabase>) -> Self {
        Self { db }
    }

    pub async fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount> {
        self.db.general_ledger().create_account_async(input).await
    }

    pub async fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>> {
        self.db.general_ledger().get_account_async(id).await
    }

    pub async fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>> {
        self.db.general_ledger().get_account_by_number_async(account_number).await
    }

    pub async fn update_account(&self, id: Uuid, input: UpdateGlAccount) -> Result<GlAccount> {
        self.db.general_ledger().update_account_async(id, input).await
    }

    pub async fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().list_accounts_async(filter).await
    }

    pub async fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().get_account_hierarchy_async().await
    }

    pub async fn delete_account(&self, id: Uuid) -> Result<()> {
        self.db.general_ledger().delete_account_async(id).await
    }

    pub async fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().initialize_chart_of_accounts_async().await
    }

    pub async fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod> {
        self.db.general_ledger().create_period_async(input).await
    }

    pub async fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_period_async(id).await
    }

    pub async fn get_current_period(&self) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_current_period_async().await
    }

    pub async fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_period_for_date_async(date).await
    }

    pub async fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>> {
        self.db.general_ledger().list_periods_async(filter).await
    }

    pub async fn open_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.db.general_ledger().open_period_async(id).await
    }

    pub async fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        self.db.general_ledger().close_period_async(id, closed_by).await
    }

    pub async fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        self.db.general_ledger().lock_period_async(id, locked_by).await
    }

    pub async fn reopen_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.db.general_ledger().reopen_period_async(id).await
    }

    pub async fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry> {
        self.db.general_ledger().create_journal_entry_async(input).await
    }

    pub async fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        self.db.general_ledger().get_journal_entry_async(id).await
    }

    pub async fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>> {
        self.db.general_ledger().get_journal_entry_by_number_async(number).await
    }

    pub async fn list_journal_entries(
        &self,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntry>> {
        self.db.general_ledger().list_journal_entries_async(filter).await
    }

    pub async fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry> {
        self.db.general_ledger().post_journal_entry_async(id, posted_by).await
    }

    pub async fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().void_journal_entry_async(id).await
    }

    pub async fn reverse_journal_entry(
        &self,
        id: Uuid,
        reversal_date: NaiveDate,
    ) -> Result<JournalEntry> {
        self.db.general_ledger().reverse_journal_entry_async(id, reversal_date).await
    }

    pub async fn get_journal_entry_lines(
        &self,
        journal_entry_id: Uuid,
    ) -> Result<Vec<JournalEntryLine>> {
        self.db.general_ledger().get_journal_entry_lines_async(journal_entry_id).await
    }

    pub async fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>> {
        self.db.general_ledger().get_auto_posting_config_async().await
    }

    pub async fn set_auto_posting_config(
        &self,
        input: CreateAutoPostingConfig,
    ) -> Result<AutoPostingConfig> {
        self.db.general_ledger().set_auto_posting_config_async(input).await
    }

    pub async fn auto_post_invoice(&self, invoice_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_invoice_async(invoice_id).await
    }

    pub async fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_payment_received_async(payment_id).await
    }

    pub async fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_bill_async(bill_id).await
    }

    pub async fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_bill_payment_async(payment_id).await
    }

    pub async fn auto_post_inventory_cost(
        &self,
        cost_transaction_id: Uuid,
    ) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_inventory_cost_async(cost_transaction_id).await
    }

    pub async fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_write_off_async(write_off_id).await
    }

    pub async fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        self.db.general_ledger().get_trial_balance_async(as_of_date).await
    }

    pub async fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet> {
        self.db.general_ledger().get_balance_sheet_async(as_of_date).await
    }

    pub async fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement> {
        self.db.general_ledger().get_income_statement_async(start_date, end_date).await
    }

    pub async fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<Decimal>> {
        self.db.general_ledger().get_account_balance_async(account_id, as_of_date).await
    }

    pub async fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>> {
        self.db.general_ledger().get_account_transactions_async(account_id, filter).await
    }

    pub async fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        self.db.general_ledger().run_period_close_async(period_id, closed_by).await
    }

    /// Re-close a period that was reopened for adjustments: void the standing
    /// closing entry (or entries), then run the close again. The period must
    /// be open. Mirrors the sync facade.
    pub async fn reclose_period(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        let standing = self
            .list_journal_entries(stateset_core::JournalEntryFilter {
                source_document_type: Some("period_close".into()),
                source_document_id: Some(period_id),
                status: Some(stateset_core::JournalEntryStatus::Posted),
                ..Default::default()
            })
            .await?;
        for entry in standing {
            self.void_journal_entry(entry.id).await?;
        }
        self.run_period_close(period_id, closed_by).await
    }

    pub async fn create_accounts_batch(
        &self,
        inputs: Vec<CreateGlAccount>,
    ) -> Result<BatchResult<GlAccount>> {
        self.db.general_ledger().create_accounts_batch_async(inputs).await
    }

    pub async fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().get_accounts_batch_async(ids).await
    }
}

// ============================================================================
// Async X402
// ============================================================================
