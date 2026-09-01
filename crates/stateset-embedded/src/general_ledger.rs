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

use chrono::{Months, NaiveDate};
use rust_decimal::Decimal;
use stateset_core::{
    AccountStatus, AutoPostingConfig, BalanceSheet, BatchResult, CloseMonthOptions,
    CloseMonthReport, CloseMonthStepReport, CloseMonthStepStatus, CommerceError,
    CreateAutoPostingConfig, CreateGlAccount, CreateGlPeriod, CreateJournalEntry, Currency,
    CurrencyCode, DepreciationEntryStatus, FixedAssetFilter, FixedAssetStatus, GlAccount,
    GlAccountFilter, GlPeriod, GlPeriodFilter, IncomeStatement, JournalEntry, JournalEntryFilter,
    JournalEntryLine, Result, RevaluationResult, RevenueContractFilter, RevenueContractStatus,
    RevenueEntryStatus, TrialBalance, UpdateGlAccount,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;
use uuid::Uuid;

#[cfg(feature = "events")]
use crate::events::EventSystem;
#[cfg(feature = "events")]
use chrono::Utc;
#[cfg(feature = "events")]
use stateset_core::CommerceEvent;

/// General Ledger interface.
pub struct GeneralLedger {
    db: Arc<dyn Database>,
    #[cfg(feature = "events")]
    event_system: Arc<EventSystem>,
}

impl std::fmt::Debug for GeneralLedger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GeneralLedger").finish_non_exhaustive()
    }
}

impl GeneralLedger {
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
    // Chart of Accounts
    // ========================================================================

    /// Create a new GL account.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use stateset_embedded::{Commerce, CreateGlAccount, AccountType, AccountSubType, CurrencyCode};
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
    ///     currency: Some(CurrencyCode::USD),
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
    pub fn reverse_journal_entry(
        &self,
        id: Uuid,
        reversal_date: NaiveDate,
    ) -> Result<JournalEntry> {
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
    /// The optional `auto_post_depreciation` and `auto_post_revenue_recognition`
    /// flags (default off) additionally auto-post journal entries when
    /// fixed-asset depreciation is posted or deferred revenue is recognized.
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
    pub fn set_auto_posting_config(
        &self,
        input: CreateAutoPostingConfig,
    ) -> Result<AutoPostingConfig> {
        self.db.general_ledger().set_auto_posting_config(input)
    }

    // ========================================================================
    // Auto-Posting Operations
    // ========================================================================

    /// Auto-post a customer invoice (debit AR, credit Revenue).
    pub fn auto_post_invoice(&self, invoice_id: Uuid) -> Result<JournalEntry> {
        self.db.general_ledger().auto_post_invoice(invoice_id.into())
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
    pub fn get_income_statement(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<IncomeStatement> {
        self.db.general_ledger().get_income_statement(start_date, end_date)
    }

    /// Get the current balance of an account.
    pub fn get_account_balance(
        &self,
        account_id: Uuid,
        as_of_date: Option<NaiveDate>,
    ) -> Result<Option<Decimal>> {
        self.db.general_ledger().get_account_balance(account_id, as_of_date)
    }

    /// Get all transactions for an account.
    pub fn get_account_transactions(
        &self,
        account_id: Uuid,
        filter: JournalEntryFilter,
    ) -> Result<Vec<JournalEntryLine>> {
        self.db.general_ledger().get_account_transactions(account_id, filter)
    }

    // ========================================================================
    // Period Close
    // ========================================================================

    /// Revalue foreign-currency account balances at the as-of exchange rate.
    ///
    /// For every active posting account whose currency differs from the base
    /// currency, the outstanding foreign-currency balance is revalued at the
    /// current exchange rate and the net unrealized gain/loss is posted as a
    /// balanced adjusting journal entry against the configured FX gain/loss
    /// account.
    ///
    /// `base_currency` defaults to the store's configured base currency.
    pub fn revalue(
        &self,
        as_of_date: NaiveDate,
        base_currency: Option<Currency>,
    ) -> Result<RevaluationResult> {
        let result = self.db.general_ledger().revalue(as_of_date, base_currency)?;
        #[cfg(feature = "events")]
        if result.journal_entry.is_some() {
            self.emit(CommerceEvent::FxRevaluationPosted {
                as_of_date: result.as_of_date,
                base_currency: result.base_currency,
                total_unrealized_gain_loss: result.total_unrealized_gain_loss,
                journal_entry_id: result.journal_entry.as_ref().map(|e| e.id),
                timestamp: Utc::now(),
            });
        }
        Ok(result)
    }

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

    /// Re-close a period that was reopened for adjustments.
    ///
    /// [`run_period_close`](Self::run_period_close) refuses while a posted
    /// closing entry stands (re-closing would sweep the same revenue and
    /// expenses into retained earnings twice). This operation encodes the
    /// correct choreography as one call: void the standing closing entry
    /// (or entries), then close the period again so the fresh closing entry
    /// reflects the adjusted activity. The period must be open.
    pub fn reclose_period(&self, period_id: Uuid, closed_by: &str) -> Result<JournalEntry> {
        let standing = self.list_journal_entries(stateset_core::JournalEntryFilter {
            source_document_type: Some("period_close".into()),
            source_document_id: Some(period_id),
            status: Some(stateset_core::JournalEntryStatus::Posted),
            ..Default::default()
        })?;
        for entry in standing {
            self.void_journal_entry(entry.id)?;
        }
        self.run_period_close(period_id, closed_by)
    }

    /// Close the month: orchestrate every period-end step in order.
    ///
    /// Runs, composing existing operations:
    /// 1. Post scheduled depreciation due through period end for all
    ///    in-service assets.
    /// 2. Recognize deferred revenue through period end for active contracts.
    /// 3. FX revaluation as of period end — skipped silently when there are
    ///    no foreign-currency accounts or no FX gain/loss account is
    ///    configured (the close never fails on this step).
    /// 4. [`run_period_close`](Self::run_period_close) (closing entries +
    ///    close period).
    ///
    /// With `options.dry_run` the candidates for each step are computed via
    /// read APIs and reported without writing anything. Per-asset and
    /// per-obligation failures are collected as warnings on the step report
    /// and do not abort the close.
    pub fn close_month(
        &self,
        period_id: Uuid,
        options: CloseMonthOptions,
    ) -> Result<CloseMonthReport> {
        let period = self.get_period(period_id)?.ok_or(CommerceError::NotFound)?;
        let closed_by = options.closed_by.clone().unwrap_or_else(|| "system".to_string());

        let depreciation = if options.skip_depreciation {
            CloseMonthStepReport::skipped(None)
        } else {
            self.close_month_depreciation(&period, options.dry_run)?
        };

        let revenue_recognition = if options.skip_revenue_recognition {
            CloseMonthStepReport::skipped(None)
        } else {
            self.close_month_revenue_recognition(&period, options.dry_run)?
        };

        let fx_revaluation = if options.skip_fx_revaluation {
            CloseMonthStepReport::skipped(None)
        } else {
            self.close_month_fx_revaluation(&period, options.dry_run)?
        };

        let mut closing_entry = None;
        let period_close = if options.skip_period_close {
            CloseMonthStepReport::skipped(None)
        } else if options.dry_run {
            let statement = self.get_income_statement(period.start_date, period.end_date)?;
            let has_activity =
                !statement.total_revenue.is_zero() || !statement.total_expenses.is_zero();
            CloseMonthStepReport {
                status: CloseMonthStepStatus::DryRun,
                entry_count: u64::from(has_activity),
                total_amount: statement.total_revenue + statement.total_expenses,
                warnings: Vec::new(),
            }
        } else {
            let entry = self.run_period_close(period_id, &closed_by)?;
            let total = entry.total_debits;
            closing_entry = Some(entry);
            CloseMonthStepReport {
                status: CloseMonthStepStatus::Executed,
                entry_count: 1,
                total_amount: total,
                warnings: Vec::new(),
            }
        };

        let period_status = if options.dry_run || options.skip_period_close {
            period.status
        } else {
            self.get_period(period_id)?.map_or(period.status, |p| p.status)
        };

        let report = CloseMonthReport {
            period_id,
            period_name: period.period_name,
            dry_run: options.dry_run,
            depreciation,
            revenue_recognition,
            fx_revaluation,
            period_close,
            closing_entry,
            period_status,
        };
        #[cfg(feature = "events")]
        if !report.dry_run {
            self.emit(CommerceEvent::MonthEndCloseCompleted {
                period_id: report.period_id,
                period_name: report.period_name.clone(),
                depreciation_total: report.depreciation.total_amount,
                revenue_recognized_total: report.revenue_recognition.total_amount,
                fx_unrealized_gain_loss: report.fx_revaluation.total_amount,
                closing_entry_id: report.closing_entry.as_ref().map(|e| e.id),
                timestamp: Utc::now(),
            });
        }
        Ok(report)
    }

    /// Step 1: post scheduled depreciation due through period end.
    ///
    /// A schedule entry `n` (1-based) is due once `period.end_date` reaches
    /// `in_service_date + (n - 1) months` — i.e. closing the month the asset
    /// went into service posts its first depreciation period.
    fn close_month_depreciation(
        &self,
        period: &GlPeriod,
        dry_run: bool,
    ) -> Result<CloseMonthStepReport> {
        if !self.db.supports_capability(DatabaseCapability::FixedAssets) {
            return Ok(CloseMonthStepReport::skipped(Some(
                "fixed assets are not supported by the active backend".to_string(),
            )));
        }
        let assets = self.db.fixed_assets().list(FixedAssetFilter {
            status: Some(FixedAssetStatus::InService),
            ..Default::default()
        })?;
        let mut entry_count = 0u64;
        let mut total_amount = Decimal::ZERO;
        let mut warnings = Vec::new();
        for asset in assets {
            let Some(in_service) = asset.in_service_date else { continue };
            let Some(schedule) = self.db.fixed_assets().get_schedule(asset.id)? else {
                warnings.push(format!(
                    "asset {}: no depreciation schedule generated",
                    asset.asset_number
                ));
                continue;
            };
            let due: Vec<_> = schedule
                .entries
                .iter()
                .filter(|e| {
                    e.status == DepreciationEntryStatus::Scheduled
                        && e.period >= 1
                        && in_service
                            .checked_add_months(Months::new(e.period - 1))
                            .is_some_and(|d| d <= period.end_date)
                })
                .collect();
            if due.is_empty() {
                continue;
            }
            let amount: Decimal = due.iter().map(|e| e.amount).sum();
            if dry_run {
                entry_count += due.len() as u64;
                total_amount += amount;
                continue;
            }
            match self.db.fixed_assets().post_depreciation(asset.id, due.len() as u32) {
                Ok(_) => {
                    entry_count += due.len() as u64;
                    total_amount += amount;
                }
                Err(e) => warnings.push(format!("asset {}: {e}", asset.asset_number)),
            }
        }
        Ok(CloseMonthStepReport {
            status: if dry_run {
                CloseMonthStepStatus::DryRun
            } else {
                CloseMonthStepStatus::Executed
            },
            entry_count,
            total_amount,
            warnings,
        })
    }

    /// Step 2: recognize deferred revenue through period end for active
    /// contracts with generated schedules.
    fn close_month_revenue_recognition(
        &self,
        period: &GlPeriod,
        dry_run: bool,
    ) -> Result<CloseMonthStepReport> {
        if !self.db.supports_capability(DatabaseCapability::RevenueRecognition) {
            return Ok(CloseMonthStepReport::skipped(Some(
                "revenue recognition is not supported by the active backend".to_string(),
            )));
        }
        let contracts = self.db.revenue_recognition().list_contracts(RevenueContractFilter {
            status: Some(RevenueContractStatus::Active),
            ..Default::default()
        })?;
        let mut entry_count = 0u64;
        let mut total_amount = Decimal::ZERO;
        let mut warnings = Vec::new();
        for contract in contracts {
            for obligation in &contract.obligations {
                let Some(schedule) = self.db.revenue_recognition().get_schedule(obligation.id)?
                else {
                    continue;
                };
                let due: Vec<_> = schedule
                    .entries
                    .iter()
                    .filter(|e| {
                        e.status == RevenueEntryStatus::Deferred
                            && e.period_start <= period.end_date
                    })
                    .collect();
                if due.is_empty() {
                    continue;
                }
                let amount: Decimal = due.iter().map(|e| e.amount).sum();
                if dry_run {
                    entry_count += due.len() as u64;
                    total_amount += amount;
                    continue;
                }
                match self.db.revenue_recognition().recognize_period(obligation.id, period.end_date)
                {
                    Ok(_) => {
                        entry_count += due.len() as u64;
                        total_amount += amount;
                    }
                    Err(e) => warnings.push(format!(
                        "contract {} obligation {}: {e}",
                        contract.contract_number, obligation.id
                    )),
                }
            }
        }
        Ok(CloseMonthStepReport {
            status: if dry_run {
                CloseMonthStepStatus::DryRun
            } else {
                CloseMonthStepStatus::Executed
            },
            entry_count,
            total_amount,
            warnings,
        })
    }

    /// Step 3: FX revaluation as of period end.
    ///
    /// Skipped silently when no active posting account is held in a foreign
    /// currency. Validation failures inside the revaluation (no FX gain/loss
    /// account configured, missing exchange rate) also downgrade to a skipped
    /// step with a warning so the close never fails here.
    fn close_month_fx_revaluation(
        &self,
        period: &GlPeriod,
        dry_run: bool,
    ) -> Result<CloseMonthStepReport> {
        let base: CurrencyCode = {
            let settings = self.db.currency().get_settings()?;
            match settings.base_currency.code().parse() {
                Ok(code) => code,
                Err(_) => {
                    return Ok(CloseMonthStepReport::skipped(Some(format!(
                        "store base currency {} is not a valid ISO code",
                        settings.base_currency.code()
                    ))));
                }
            }
        };
        let foreign_accounts: Vec<GlAccount> = self
            .list_accounts(GlAccountFilter {
                status: Some(AccountStatus::Active),
                is_posting: Some(true),
                ..Default::default()
            })?
            .into_iter()
            .filter(|a| a.currency != base)
            .collect();
        if foreign_accounts.is_empty() {
            return Ok(CloseMonthStepReport::skipped(None));
        }
        if dry_run {
            return Ok(CloseMonthStepReport {
                status: CloseMonthStepStatus::DryRun,
                entry_count: foreign_accounts.len() as u64,
                total_amount: Decimal::ZERO,
                warnings: Vec::new(),
            });
        }
        match self.revalue(period.end_date, None) {
            Ok(result) => Ok(CloseMonthStepReport {
                status: CloseMonthStepStatus::Executed,
                entry_count: u64::from(result.journal_entry.is_some()),
                total_amount: result.total_unrealized_gain_loss,
                warnings: Vec::new(),
            }),
            Err(CommerceError::ValidationError(message)) => Ok(CloseMonthStepReport::skipped(
                Some(format!("fx revaluation skipped: {message}")),
            )),
            Err(e) => Err(e),
        }
    }

    // ========================================================================
    // Batch Operations
    // ========================================================================

    /// Create multiple accounts in batch.
    pub fn create_accounts_batch(
        &self,
        inputs: Vec<CreateGlAccount>,
    ) -> Result<BatchResult<GlAccount>> {
        self.db.general_ledger().create_accounts_batch(inputs)
    }

    /// Get multiple accounts by IDs.
    pub fn get_accounts_batch(&self, ids: Vec<Uuid>) -> Result<Vec<GlAccount>> {
        self.db.general_ledger().get_accounts_batch(ids)
    }
}
