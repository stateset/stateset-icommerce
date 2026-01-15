//! General Ledger operations
//!
//! Comprehensive general ledger supporting:
//! - Chart of accounts management
//! - Journal entries with double-entry bookkeeping
//! - Period management (open, close, lock)
//! - Auto-posting from commerce transactions
//! - Financial reports (Trial Balance, Balance Sheet, Income Statement)
//!
//! # Example
//!
//! ```rust,ignore
//! use stateset_embedded::{Commerce, CreateGlAccount, AccountType};
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Initialize standard chart of accounts
//! commerce.general_ledger().initialize_chart_of_accounts()?;
//!
//! // Create a custom account
//! let account = commerce.general_ledger().create_account(CreateGlAccount {
//!     account_number: "6100".into(),
//!     name: "Marketing Expense".into(),
//!     account_type: AccountType::Expense,
//!     ..Default::default()
//! })?;
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use chrono::NaiveDate;
use rust_decimal::Decimal;
use stateset_core::{
    AutoPostingConfig, BalanceSheet, BatchResult, CreateAutoPostingConfig,
    CreateGlAccount, CreateGlPeriod, CreateJournalEntry, GeneralLedgerRepository,
    GlAccount, GlAccountFilter, GlPeriod, GlPeriodFilter, IncomeStatement,
    JournalEntry, JournalEntryFilter, JournalEntryLine, Result, TrialBalance,
    UpdateGlAccount,
};
use stateset_db::Database;
use std::sync::Arc;
use uuid::Uuid;

/// General Ledger interface.
pub struct GeneralLedger {
    db: Arc<dyn Database>,
}

impl GeneralLedger {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    // ========================================================================
    // Chart of Accounts
    // ========================================================================

    /// Create a new GL account.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateGlAccount, AccountType, AccountSubType};
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let account = commerce.general_ledger().create_account(CreateGlAccount {
    ///     account_number: "1000".into(),
    ///     name: "Cash".into(),
    ///     description: Some("Operating cash account".into()),
    ///     account_type: AccountType::Asset,
    ///     account_sub_type: Some(AccountSubType::Cash),
    ///     is_posting: Some(true),
    ///     currency: Some("USD".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_account(&self, input: CreateGlAccount) -> Result<GlAccount> {
        self.db.general_ledger().create_account(input)
    }

    /// Get a GL account by ID.
    pub fn get_account(&self, id: Uuid) -> Result<Option<GlAccount>> {
        self.db.general_ledger().get_account(id)
    }

    /// Get a GL account by account number.
    pub fn get_account_by_number(&self, account_number: &str) -> Result<Option<GlAccount>> {
        self.db.general_ledger().get_account_by_number(account_number)
    }

    /// Update a GL account.
    pub fn update_account(&self, id: Uuid, input: UpdateGlAccount) -> Result<GlAccount> {
        self.db.general_ledger().update_account(id, input)
    }

    /// List GL accounts with filtering.
    pub fn list_accounts(&self, filter: GlAccountFilter) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().list_accounts(filter)
    }

    /// Get the full account hierarchy.
    pub fn get_account_hierarchy(&self) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().get_account_hierarchy()
    }

    /// Delete a GL account (only if no transactions).
    pub fn delete_account(&self, id: Uuid) -> Result<()> {
        self.db.general_ledger().delete_account(id)
    }

    /// Initialize the standard chart of accounts.
    ///
    /// Creates a default set of accounts for Assets, Liabilities, Equity,
    /// Revenue, and Expenses.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let accounts = commerce.general_ledger().initialize_chart_of_accounts()?;
    /// println!("Created {} standard accounts", accounts.len());
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn initialize_chart_of_accounts(&self) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().initialize_chart_of_accounts()
    }

    // ========================================================================
    // Accounting Periods
    // ========================================================================

    /// Create a new accounting period.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateGlPeriod};
    /// use chrono::NaiveDate;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let period = commerce.general_ledger().create_period(CreateGlPeriod {
    ///     period_name: "January 2025".into(),
    ///     fiscal_year: 2025,
    ///     period_number: 1,
    ///     start_date: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    ///     end_date: NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_period(&self, input: CreateGlPeriod) -> Result<GlPeriod> {
        self.db.general_ledger().create_period(input)
    }

    /// Get a period by ID.
    pub fn get_period(&self, id: Uuid) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_period(id)
    }

    /// Get the current open period.
    pub fn get_current_period(&self) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_current_period()
    }

    /// Get the period for a specific date.
    pub fn get_period_for_date(&self, date: NaiveDate) -> Result<Option<GlPeriod>> {
        self.db.general_ledger().get_period_for_date(date)
    }

    /// List periods with filtering.
    pub fn list_periods(&self, filter: GlPeriodFilter) -> Result<Vec<GlPeriod>> {
        self.db.general_ledger().list_periods(filter)
    }

    /// Open a period (transition from future to open).
    pub fn open_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.db.general_ledger().open_period(id)
    }

    /// Close a period (no more postings allowed except adjustments).
    pub fn close_period(&self, id: Uuid, closed_by: &str) -> Result<GlPeriod> {
        self.db.general_ledger().close_period(id, closed_by)
    }

    /// Lock a period (permanently closed, no changes allowed).
    pub fn lock_period(&self, id: Uuid, locked_by: &str) -> Result<GlPeriod> {
        self.db.general_ledger().lock_period(id, locked_by)
    }

    /// Reopen a closed period (for adjustments).
    pub fn reopen_period(&self, id: Uuid) -> Result<GlPeriod> {
        self.db.general_ledger().reopen_period(id)
    }

    // ========================================================================
    // Journal Entries
    // ========================================================================

    /// Create a journal entry.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateJournalEntry, CreateJournalEntryLine};
    /// use rust_decimal_macros::dec;
    /// use chrono::NaiveDate;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Debit Cash, Credit Sales Revenue
    /// let entry = commerce.general_ledger().create_journal_entry(CreateJournalEntry {
    ///     entry_date: NaiveDate::from_ymd_opt(2025, 1, 15).unwrap(),
    ///     description: "Cash sale".into(),
    ///     lines: vec![
    ///         CreateJournalEntryLine::debit(cash_account_id, dec!(100.00), Some("Cash received".into())),
    ///         CreateJournalEntryLine::credit(sales_account_id, dec!(100.00), Some("Sales revenue".into())),
    ///     ],
    ///     auto_post: Some(true),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create_journal_entry(&self, input: CreateJournalEntry) -> Result<JournalEntry> {
        self.db.general_ledger().create_journal_entry(input)
    }

    /// Get a journal entry by ID.
    pub fn get_journal_entry(&self, id: Uuid) -> Result<Option<JournalEntry>> {
        self.db.general_ledger().get_journal_entry(id)
    }

    /// Get a journal entry by entry number.
    pub fn get_journal_entry_by_number(&self, number: &str) -> Result<Option<JournalEntry>> {
        self.db.general_ledger().get_journal_entry_by_number(number)
    }

    /// List journal entries with filtering.
    pub fn list_journal_entries(&self, filter: JournalEntryFilter) -> Result<Vec<JournalEntry>> {
        self.db.general_ledger().list_journal_entries(filter)
    }

    /// Post a journal entry (update account balances).
    pub fn post_journal_entry(&self, id: Uuid, posted_by: &str) -> Result<JournalEntry> {
        self.db.general_ledger().post_journal_entry(id, posted_by)
    }

    /// Void a posted journal entry.
    pub fn void_journal_entry(&self, id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().void_journal_entry(id)
    }

    /// Reverse a journal entry (create an offsetting entry).
    pub fn reverse_journal_entry(&self, id: Uuid, reversal_date: NaiveDate) -> Result<JournalEntry> {
        self.db.general_ledger().reverse_journal_entry(id, reversal_date)
    }

    /// Get journal entry lines for an entry.
    pub fn get_journal_entry_lines(&self, journal_entry_id: Uuid) -> Result<Vec<JournalEntryLine>> {
        self.db.general_ledger().get_journal_entry_lines(journal_entry_id)
    }

    // ========================================================================
    // Auto-Posting Configuration
    // ========================================================================

    /// Get the current auto-posting configuration.
    pub fn get_auto_posting_config(&self) -> Result<Option<AutoPostingConfig>> {
        self.db.general_ledger().get_auto_posting_config()
    }

    /// Set up auto-posting configuration.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateAutoPostingConfig};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// // Set up automatic GL postings for commerce transactions
    /// commerce.general_ledger().set_auto_posting_config(CreateAutoPostingConfig {
    ///     config_name: "Default".into(),
    ///     cash_account_id: cash_id,
    ///     accounts_receivable_account_id: ar_id,
    ///     inventory_account_id: inv_id,
    ///     accounts_payable_account_id: ap_id,
    ///     sales_revenue_account_id: revenue_id,
    ///     cogs_account_id: cogs_id,
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn set_auto_posting_config(&self, input: CreateAutoPostingConfig) -> Result<AutoPostingConfig> {
        self.db.general_ledger().set_auto_posting_config(input)
    }

    // ========================================================================
    // Auto-Posting Operations
    // ========================================================================

    /// Auto-post a customer invoice (debit AR, credit Revenue).
    pub fn auto_post_invoice(&self, invoice_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_invoice(invoice_id)
    }

    /// Auto-post a payment received (debit Cash, credit AR).
    pub fn auto_post_payment_received(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_payment_received(payment_id)
    }

    /// Auto-post a supplier bill (debit Inventory/Expense, credit AP).
    pub fn auto_post_bill(&self, bill_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_bill(bill_id)
    }

    /// Auto-post a bill payment (debit AP, credit Cash).
    pub fn auto_post_bill_payment(&self, payment_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_bill_payment(payment_id)
    }

    /// Auto-post inventory cost (COGS on sale).
    pub fn auto_post_inventory_cost(&self, cost_transaction_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_inventory_cost(cost_transaction_id)
    }

    /// Auto-post a write-off (debit Bad Debt Expense, credit AR).
    pub fn auto_post_write_off(&self, write_off_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_write_off(write_off_id)
    }

    // ========================================================================
    // Financial Reports
    // ========================================================================

    /// Generate a trial balance.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use chrono::NaiveDate;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let trial_balance = commerce.general_ledger().get_trial_balance(
    ///     NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()
    /// )?;
    ///
    /// println!("Trial Balance as of {}", trial_balance.as_of_date);
    /// println!("Total Debits: ${}", trial_balance.total_debits);
    /// println!("Total Credits: ${}", trial_balance.total_credits);
    /// println!("Is Balanced: {}", trial_balance.is_balanced);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_trial_balance(&self, as_of_date: NaiveDate) -> Result<TrialBalance> {
        self.db.general_ledger().get_trial_balance(as_of_date)
    }

    /// Generate a balance sheet.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use chrono::NaiveDate;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let balance_sheet = commerce.general_ledger().get_balance_sheet(
    ///     NaiveDate::from_ymd_opt(2025, 1, 31).unwrap()
    /// )?;
    ///
    /// println!("Balance Sheet as of {}", balance_sheet.as_of_date);
    /// println!("Total Assets: ${}", balance_sheet.total_assets);
    /// println!("Total Liabilities: ${}", balance_sheet.total_liabilities);
    /// println!("Total Equity: ${}", balance_sheet.total_equity);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_balance_sheet(&self, as_of_date: NaiveDate) -> Result<BalanceSheet> {
        self.db.general_ledger().get_balance_sheet(as_of_date)
    }

    /// Generate an income statement.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::Commerce;
    /// use chrono::NaiveDate;
    ///
    /// let commerce = Commerce::new(":memory:")?;
    ///
    /// let income_statement = commerce.general_ledger().get_income_statement(
    ///     NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    ///     NaiveDate::from_ymd_opt(2025, 1, 31).unwrap(),
    /// )?;
    ///
    /// println!("Income Statement {} to {}", income_statement.period_start, income_statement.period_end);
    /// println!("Total Revenue: ${}", income_statement.total_revenue);
    /// println!("Total Expenses: ${}", income_statement.total_expenses);
    /// println!("Net Income: ${}", income_statement.net_income);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn get_income_statement(&self, start_date: NaiveDate, end_date: NaiveDate) -> Result<IncomeStatement> {
        self.db.general_ledger().get_income_statement(start_date, end_date)
    }

    /// Get the current balance of an account.
    pub fn get_account_balance(&self, account_id: Uuid, as_of_date: Option<NaiveDate>) -> Result<Decimal> {
        self.db.general_ledger().get_account_balance(account_id, as_of_date)
    }

    /// Get all transactions for an account.
    pub fn get_account_transactions(&self, account_id: Uuid, filter: JournalEntryFilter) -> Result<Vec<JournalEntryLine>> {
        self.db.general_ledger().get_account_transactions(account_id, filter)
    }

    // ========================================================================
    // Period Close
    // ========================================================================

    /// Run period close process (generate closing entries, close period).
    ///
    /// This will:
    /// 1. Generate income statement for the period
    /// 2. Create closing entries to zero out revenue/expense accounts
    /// 3. Transfer net income to retained earnings
    /// 4. Close the period
    pub fn run_period_close(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        self.db.general_ledger().run_period_close(period_id, closed_by)
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple accounts in batch.
    pub fn create_accounts_batch(&self, inputs: Vec<CreateGlAccount>) -> Result<BatchResult<GlAccount>> {
        self.db.general_ledger().create_accounts_batch(inputs)
    }

    /// Get multiple accounts by IDs.
    pub fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().get_accounts_batch(ids)
    }
}
