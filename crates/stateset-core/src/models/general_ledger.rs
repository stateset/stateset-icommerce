//! General Ledger domain models
//!
//! Full double-entry accounting system supporting:
//! - Chart of Accounts with hierarchy
//! - Journal entries (balanced debits = credits)
//! - GL periods (open, closed, locked)
//! - Auto-posting from commerce transactions
//! - Trial balance and financial statements

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::CurrencyCode;
use std::fmt;
use std::str::FromStr;
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Account Type Enums
// ============================================================================

/// GL Account type (follows standard accounting)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountType {
    /// Cash, receivables, inventory, and other economic resources.
    Asset,
    /// Obligations owed to creditors and suppliers.
    Liability,
    /// Owner's residual interest in the business.
    Equity,
    /// Income earned from sales or services.
    Revenue,
    /// Costs incurred in earning revenue.
    Expense,
}

impl AccountType {
    /// Returns the normal balance side for this account type
    #[must_use]
    pub const fn normal_balance(&self) -> BalanceSide {
        match self {
            Self::Asset | Self::Expense => BalanceSide::Debit,
            Self::Liability | Self::Equity | Self::Revenue => BalanceSide::Credit,
        }
    }

    /// Returns true if this account type appears on the Balance Sheet
    #[must_use]
    pub const fn is_balance_sheet(&self) -> bool {
        matches!(self, Self::Asset | Self::Liability | Self::Equity)
    }

    /// Returns true if this account type appears on the Income Statement
    #[must_use]
    pub const fn is_income_statement(&self) -> bool {
        matches!(self, Self::Revenue | Self::Expense)
    }
}

/// Balance side (debit or credit)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BalanceSide {
    /// Increases asset and expense accounts; decreases liabilities, equity, and revenue.
    #[default]
    Debit,
    /// Increases liability, equity, and revenue accounts; decreases assets and expenses.
    Credit,
}

/// Account sub-types for more granular classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountSubType {
    // Assets
    /// Cash and cash equivalents.
    Cash,
    /// Amounts owed by customers for goods or services delivered.
    AccountsReceivable,
    /// Goods held for sale or used in production.
    Inventory,
    /// Expenses paid in advance not yet recognized as cost.
    PrepaidExpense,
    /// Long-lived tangible assets such as equipment and buildings.
    FixedAsset,
    /// Contra-asset reducing the carrying value of fixed assets.
    AccumulatedDepreciation,
    /// Current assets not classified elsewhere.
    OtherCurrentAsset,
    /// Non-current assets not classified elsewhere.
    OtherNonCurrentAsset,
    // Liabilities
    /// Amounts owed to suppliers for goods or services received.
    AccountsPayable,
    /// Expenses incurred but not yet paid.
    AccruedLiabilities,
    /// Customer deposits or payments for goods/services not yet delivered.
    UnearnedRevenue,
    /// Debt due within one year.
    ShortTermDebt,
    /// Debt due beyond one year.
    LongTermDebt,
    /// Current liabilities not classified elsewhere.
    OtherCurrentLiability,
    /// Non-current liabilities not classified elsewhere.
    OtherNonCurrentLiability,
    // Equity
    /// Paid-in capital from shareholders.
    CommonStock,
    /// Cumulative earnings retained in the business.
    RetainedEarnings,
    /// Equity accounts not classified elsewhere.
    OtherEquity,
    // Revenue
    /// Revenue from product sales.
    SalesRevenue,
    /// Revenue from services rendered.
    ServiceRevenue,
    /// Revenue not classified elsewhere.
    OtherRevenue,
    // Expense
    /// Direct cost of goods sold to customers.
    CostOfGoodsSold,
    /// Recurring expenses related to running the business.
    OperatingExpense,
    /// Wages, salaries, and related employee costs.
    Payroll,
    /// Costs for leasing office or warehouse space.
    RentExpense,
    /// Electricity, water, and similar utility costs.
    UtilitiesExpense,
    /// Allocation of fixed asset cost over its useful life.
    DepreciationExpense,
    /// Cost of borrowing funds.
    InterestExpense,
    /// Income tax and other tax charges.
    TaxExpense,
    /// Expenses not classified elsewhere.
    OtherExpense,
}

/// Account status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountStatus {
    /// Account accepts postings and appears in reports.
    #[default]
    Active,
    /// Account is temporarily disabled; no new postings allowed.
    Inactive,
    /// Account is permanently closed and hidden from normal views.
    Archived,
}

impl fmt::Display for AccountStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

impl FromStr for AccountStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "inactive" => Ok(Self::Inactive),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("Unknown account status: {s}")),
        }
    }
}

// ============================================================================
// Period & Journal Entry Enums
// ============================================================================

/// GL Period status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PeriodStatus {
    /// Period has not yet started; posting is not allowed.
    #[default]
    Future,
    /// Period is active and accepts journal entry postings.
    Open,
    /// Period has ended; no further postings permitted without re-opening.
    Closed,
    /// Period is permanently sealed; cannot be re-opened.
    Locked,
}

impl fmt::Display for PeriodStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Future => write!(f, "future"),
            Self::Open => write!(f, "open"),
            Self::Closed => write!(f, "closed"),
            Self::Locked => write!(f, "locked"),
        }
    }
}

impl FromStr for PeriodStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "future" => Ok(Self::Future),
            "open" => Ok(Self::Open),
            "closed" => Ok(Self::Closed),
            "locked" => Ok(Self::Locked),
            _ => Err(format!("Unknown period status: {s}")),
        }
    }
}

/// Journal entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JournalEntryType {
    /// Routine transaction entry.
    #[default]
    Standard,
    /// End-of-period accrual or deferral entry.
    Adjusting,
    /// Entry to close temporary accounts at period end.
    Closing,
    /// Auto-generated entry that reverses a prior adjusting entry.
    Reversing,
    /// Entry to establish opening balances for a new period or entity.
    Opening,
}

impl fmt::Display for JournalEntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::Adjusting => write!(f, "adjusting"),
            Self::Closing => write!(f, "closing"),
            Self::Reversing => write!(f, "reversing"),
            Self::Opening => write!(f, "opening"),
        }
    }
}

impl FromStr for JournalEntryType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "adjusting" => Ok(Self::Adjusting),
            "closing" => Ok(Self::Closing),
            "reversing" => Ok(Self::Reversing),
            "opening" => Ok(Self::Opening),
            _ => Err(format!("Unknown journal entry type: {s}")),
        }
    }
}

/// Journal entry source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JournalEntrySource {
    /// Entry created manually by a user.
    #[default]
    Manual,
    /// Auto-generated when a customer invoice is posted.
    AutoInvoice,
    /// Auto-generated when a customer payment is received.
    AutoPayment,
    /// Auto-generated when a supplier bill is approved.
    AutoBill,
    /// Auto-generated when a supplier bill payment is made.
    AutoBillPayment,
    /// Auto-generated from an inventory transaction.
    AutoInventory,
    /// Auto-generated when an AR balance is written off.
    AutoWriteOff,
    /// Generated automatically during period-close processing.
    SystemClosing,
    /// Imported from an external system or file.
    Import,
}

impl fmt::Display for JournalEntrySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual => write!(f, "manual"),
            Self::AutoInvoice => write!(f, "auto_invoice"),
            Self::AutoPayment => write!(f, "auto_payment"),
            Self::AutoBill => write!(f, "auto_bill"),
            Self::AutoBillPayment => write!(f, "auto_bill_payment"),
            Self::AutoInventory => write!(f, "auto_inventory"),
            Self::AutoWriteOff => write!(f, "auto_write_off"),
            Self::SystemClosing => write!(f, "system_closing"),
            Self::Import => write!(f, "import"),
        }
    }
}

impl FromStr for JournalEntrySource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" => Ok(Self::Manual),
            "auto_invoice" => Ok(Self::AutoInvoice),
            "auto_payment" => Ok(Self::AutoPayment),
            "auto_bill" => Ok(Self::AutoBill),
            "auto_bill_payment" => Ok(Self::AutoBillPayment),
            "auto_inventory" => Ok(Self::AutoInventory),
            "auto_write_off" => Ok(Self::AutoWriteOff),
            "system_closing" => Ok(Self::SystemClosing),
            "import" => Ok(Self::Import),
            _ => Err(format!("Unknown journal entry source: {s}")),
        }
    }
}

/// Journal entry status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum JournalEntryStatus {
    /// Entry is being prepared; not yet submitted for posting.
    #[default]
    Draft,
    /// Entry is awaiting approval before posting.
    Pending,
    /// Entry has been posted and affects account balances.
    Posted,
    /// Entry has been cancelled; has no effect on balances.
    Voided,
    /// Entry has been offset by a reversing entry.
    Reversed,
}

impl fmt::Display for JournalEntryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Pending => write!(f, "pending"),
            Self::Posted => write!(f, "posted"),
            Self::Voided => write!(f, "voided"),
            Self::Reversed => write!(f, "reversed"),
        }
    }
}

impl FromStr for JournalEntryStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "pending" => Ok(Self::Pending),
            "posted" => Ok(Self::Posted),
            "voided" => Ok(Self::Voided),
            "reversed" => Ok(Self::Reversed),
            _ => Err(format!("Unknown journal entry status: {s}")),
        }
    }
}

// ============================================================================
// Core GL Structs
// ============================================================================

/// A GL Account (Chart of Accounts entry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlAccount {
    /// Unique identifier for this account.
    pub id: Uuid,
    /// Structured account code (e.g. `"1010"`).
    pub account_number: String,
    /// Human-readable account name.
    pub name: String,
    /// Optional description of the account's purpose.
    pub description: Option<String>,
    /// Top-level classification (Asset, Liability, Equity, Revenue, Expense).
    pub account_type: AccountType,
    /// Finer-grained classification within the account type.
    pub account_sub_type: Option<AccountSubType>,
    /// Parent account for hierarchy grouping; `None` if top-level.
    pub parent_account_id: Option<Uuid>,
    /// If `true`, this is a summary header; postings go to child accounts.
    pub is_header: bool,
    /// If `true`, journal entry lines may be posted directly to this account.
    pub is_posting: bool,
    /// Expected side (Debit/Credit) that increases this account.
    pub normal_balance: BalanceSide,
    /// Currency in which this account is maintained.
    pub currency: CurrencyCode,
    /// Lifecycle status of the account.
    pub status: AccountStatus,
    /// Running balance as of the last posting.
    pub current_balance: Decimal,
    /// Timestamp of account creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

impl GlAccount {
    /// Returns true if this account can accept postings
    #[must_use]
    pub fn can_post(&self) -> bool {
        self.is_posting && self.status == AccountStatus::Active
    }

    /// Calculates the balance effect of a debit/credit
    #[must_use]
    pub fn balance_effect(&self, debit: Decimal, credit: Decimal) -> Decimal {
        // Keep behavior consistent with account type, even if persisted normal_balance drifts.
        match self.account_type.normal_balance() {
            BalanceSide::Debit => debit - credit,
            BalanceSide::Credit => credit - debit,
        }
    }
}

/// GL Period (accounting period)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlPeriod {
    /// Unique identifier for this period.
    pub id: Uuid,
    /// Display name, typically in `YYYY-MM` format.
    pub period_name: String,
    /// Fiscal year this period belongs to.
    pub fiscal_year: i32,
    /// Sequential number within the fiscal year (1–12 for monthly).
    pub period_number: i32,
    /// First date of the period (inclusive).
    pub start_date: NaiveDate,
    /// Last date of the period (inclusive).
    pub end_date: NaiveDate,
    /// Current lifecycle status of the period.
    pub status: PeriodStatus,
    /// Timestamp when the period was closed.
    pub closed_at: Option<DateTime<Utc>>,
    /// User who closed the period.
    pub closed_by: Option<String>,
    /// Timestamp when the period was permanently locked.
    pub locked_at: Option<DateTime<Utc>>,
    /// User who locked the period.
    pub locked_by: Option<String>,
    /// Timestamp of period creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

impl GlPeriod {
    /// Returns true if the period allows posting
    #[must_use]
    pub fn can_post(&self) -> bool {
        self.status == PeriodStatus::Open
    }

    /// Returns true if a date falls within this period
    #[must_use]
    pub fn contains_date(&self, date: NaiveDate) -> bool {
        date >= self.start_date && date <= self.end_date
    }
}

/// Journal Entry header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Unique identifier for this journal entry.
    pub id: Uuid,
    /// Human-readable entry reference number (e.g. `"JE-20240101-ABCD1234"`).
    pub entry_number: String,
    /// Date the transaction occurred or is being recorded.
    pub entry_date: NaiveDate,
    /// Accounting period this entry belongs to.
    pub period_id: Uuid,
    /// Classification of the entry (standard, adjusting, closing, etc.).
    pub entry_type: JournalEntryType,
    /// System or process that created the entry.
    pub source: JournalEntrySource,
    /// Entity type of the originating document (e.g. `"invoice"`).
    pub source_document_type: Option<String>,
    /// Identifier of the originating document.
    pub source_document_id: Option<Uuid>,
    /// Narrative description of the transaction.
    pub description: String,
    /// Sum of all debit line amounts.
    pub total_debits: Decimal,
    /// Sum of all credit line amounts.
    pub total_credits: Decimal,
    /// `true` when `total_debits == total_credits`.
    pub is_balanced: bool,
    /// Current lifecycle status.
    pub status: JournalEntryStatus,
    /// Timestamp when the entry was posted to the ledger.
    pub posted_at: Option<DateTime<Utc>>,
    /// User who posted the entry.
    pub posted_by: Option<String>,
    /// Entry that this one reverses, if applicable.
    pub reversed_entry_id: Option<Uuid>,
    /// Entry created to reverse this one, if applicable.
    pub reversing_entry_id: Option<Uuid>,
    /// Individual debit/credit lines that make up this entry.
    pub lines: Vec<JournalEntryLine>,
    /// Timestamp of entry creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
    pub updated_at: DateTime<Utc>,
}

impl JournalEntry {
    /// Returns true if debits equal credits
    #[must_use]
    pub fn is_balanced(&self) -> bool {
        self.total_debits == self.total_credits
    }

    /// Recalculates totals from lines
    pub fn recalculate_totals(&mut self) {
        self.total_debits = self.lines.iter().map(|l| l.debit_amount).sum();
        self.total_credits = self.lines.iter().map(|l| l.credit_amount).sum();
        self.is_balanced = self.total_debits == self.total_credits;
    }

    /// Returns true if entry can be posted
    pub fn can_post(&self) -> bool {
        if self.status != JournalEntryStatus::Draft || self.lines.is_empty() {
            return false;
        }

        if !self.lines.iter().all(JournalEntryLine::is_valid) {
            return false;
        }

        let calculated_debits: Decimal = self.lines.iter().map(|l| l.debit_amount).sum();
        let calculated_credits: Decimal = self.lines.iter().map(|l| l.credit_amount).sum();
        self.total_debits == calculated_debits
            && self.total_credits == calculated_credits
            && calculated_debits == calculated_credits
    }

    /// Returns true if entry can be voided
    #[must_use]
    pub fn can_void(&self) -> bool {
        self.status == JournalEntryStatus::Posted
    }
}

/// Journal Entry line (detail)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntryLine {
    /// Unique identifier for this line.
    pub id: Uuid,
    /// Parent journal entry.
    pub journal_entry_id: Uuid,
    /// Sequence number within the entry (1-based).
    pub line_number: i32,
    /// GL account being debited or credited.
    pub account_id: Uuid,
    /// Denormalized account number for reporting convenience.
    pub account_number: Option<String>,
    /// Denormalized account name for reporting convenience.
    pub account_name: Option<String>,
    /// Optional line-level narrative.
    pub description: Option<String>,
    /// Debit amount; exactly one of `debit_amount` or `credit_amount` must be non-zero.
    pub debit_amount: Decimal,
    /// Credit amount; exactly one of `debit_amount` or `credit_amount` must be non-zero.
    pub credit_amount: Decimal,
    /// Currency of the amounts on this line.
    pub currency: CurrencyCode,
    /// Entity type of a related sub-ledger record (e.g. `"invoice_line"`).
    pub reference_type: Option<String>,
    /// Identifier of the related sub-ledger record.
    pub reference_id: Option<Uuid>,
    /// Timestamp of line creation.
    pub created_at: DateTime<Utc>,
}

impl JournalEntryLine {
    /// Returns true if line has only debit or only credit
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (self.debit_amount > Decimal::ZERO && self.credit_amount == Decimal::ZERO)
            || (self.debit_amount == Decimal::ZERO && self.credit_amount > Decimal::ZERO)
    }

    /// Returns the net amount (positive for debit, negative for credit)
    #[must_use]
    pub fn net_amount(&self) -> Decimal {
        self.debit_amount - self.credit_amount
    }
}

/// Auto-posting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPostingConfig {
    pub id: Uuid,
    pub config_name: String,
    pub cash_account_id: Uuid,
    pub accounts_receivable_account_id: Uuid,
    pub inventory_account_id: Uuid,
    pub accounts_payable_account_id: Uuid,
    pub unearned_revenue_account_id: Option<Uuid>,
    pub sales_revenue_account_id: Uuid,
    pub shipping_revenue_account_id: Option<Uuid>,
    pub cogs_account_id: Uuid,
    pub bad_debt_expense_account_id: Option<Uuid>,
    /// Account receiving unrealized FX gains/losses posted by period-end revaluation.
    #[serde(default)]
    pub fx_gain_loss_account_id: Option<Uuid>,
    /// Auto-post a journal entry when fixed-asset depreciation is posted.
    #[serde(default)]
    pub auto_post_depreciation: bool,
    /// Auto-post a journal entry when deferred revenue is recognized.
    #[serde(default)]
    pub auto_post_revenue_recognition: bool,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Account balance for a period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub id: Uuid,
    pub account_id: Uuid,
    pub period_id: Uuid,
    pub opening_balance: Decimal,
    pub total_debits: Decimal,
    pub total_credits: Decimal,
    pub closing_balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Financial Reports
// ============================================================================

/// Trial Balance line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalanceLine {
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub account_type: AccountType,
    pub debit_balance: Decimal,
    pub credit_balance: Decimal,
}

/// Trial Balance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialBalance {
    pub as_of_date: NaiveDate,
    pub period_id: Option<Uuid>,
    pub total_debits: Decimal,
    pub total_credits: Decimal,
    pub is_balanced: bool,
    pub lines: Vec<TrialBalanceLine>,
}

impl TrialBalance {
    /// Returns true if debits equal credits
    #[must_use]
    pub fn is_balanced(&self) -> bool {
        self.total_debits == self.total_credits
    }
}

/// Balance Sheet line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheetLine {
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub account_sub_type: Option<AccountSubType>,
    pub balance: Decimal,
    pub indent_level: i32,
    pub is_total: bool,
}

/// Balance Sheet report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceSheet {
    pub as_of_date: NaiveDate,
    pub total_assets: Decimal,
    pub total_liabilities: Decimal,
    pub total_equity: Decimal,
    pub assets: Vec<BalanceSheetLine>,
    pub liabilities: Vec<BalanceSheetLine>,
    pub equity: Vec<BalanceSheetLine>,
}

impl BalanceSheet {
    /// Returns true if assets equal liabilities plus equity
    #[must_use]
    pub fn is_balanced(&self) -> bool {
        self.total_assets == self.total_liabilities + self.total_equity
    }
}

/// Income Statement line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatementLine {
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub account_sub_type: Option<AccountSubType>,
    pub amount: Decimal,
    pub indent_level: i32,
    pub is_total: bool,
}

/// Income Statement report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomeStatement {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_revenue: Decimal,
    pub total_expenses: Decimal,
    pub net_income: Decimal,
    pub revenue_lines: Vec<IncomeStatementLine>,
    pub expense_lines: Vec<IncomeStatementLine>,
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a general ledger account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGlAccount {
    pub account_number: String,
    pub name: String,
    pub description: Option<String>,
    pub account_type: AccountType,
    pub account_sub_type: Option<AccountSubType>,
    pub parent_account_id: Option<Uuid>,
    pub is_header: Option<bool>,
    pub is_posting: Option<bool>,
    pub currency: Option<CurrencyCode>,
}

/// Input for updating a general ledger account
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateGlAccount {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parent_account_id: Option<Uuid>,
    pub status: Option<AccountStatus>,
}

/// Input for creating a fiscal period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGlPeriod {
    pub period_name: String,
    pub fiscal_year: i32,
    pub period_number: i32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

/// Input for creating a journal entry with lines
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJournalEntry {
    pub entry_date: NaiveDate,
    pub entry_type: Option<JournalEntryType>,
    pub description: String,
    pub lines: Vec<CreateJournalEntryLine>,
    pub source_document_type: Option<String>,
    pub source_document_id: Option<Uuid>,
    pub auto_post: Option<bool>,
}

/// Input for a journal entry line
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateJournalEntryLine {
    pub account_id: Uuid,
    pub description: Option<String>,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
}

impl CreateJournalEntryLine {
    /// Create a debit line for an account
    #[must_use]
    pub const fn debit(account_id: Uuid, amount: Decimal, description: Option<String>) -> Self {
        Self {
            account_id,
            description,
            debit_amount: amount,
            credit_amount: Decimal::ZERO,
            reference_type: None,
            reference_id: None,
        }
    }

    /// Create a credit line for an account
    #[must_use]
    pub const fn credit(account_id: Uuid, amount: Decimal, description: Option<String>) -> Self {
        Self {
            account_id,
            description,
            debit_amount: Decimal::ZERO,
            credit_amount: amount,
            reference_type: None,
            reference_id: None,
        }
    }
}

/// Configuration for automatic postings between accounts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAutoPostingConfig {
    pub config_name: String,
    pub cash_account_id: Uuid,
    pub accounts_receivable_account_id: Uuid,
    pub inventory_account_id: Uuid,
    pub accounts_payable_account_id: Uuid,
    pub unearned_revenue_account_id: Option<Uuid>,
    pub sales_revenue_account_id: Uuid,
    pub shipping_revenue_account_id: Option<Uuid>,
    pub cogs_account_id: Uuid,
    pub bad_debt_expense_account_id: Option<Uuid>,
    /// Account receiving unrealized FX gains/losses posted by period-end revaluation.
    #[serde(default)]
    pub fx_gain_loss_account_id: Option<Uuid>,
    /// Auto-post a journal entry when fixed-asset depreciation is posted (default off).
    #[serde(default)]
    pub auto_post_depreciation: bool,
    /// Auto-post a journal entry when deferred revenue is recognized (default off).
    #[serde(default)]
    pub auto_post_revenue_recognition: bool,
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for listing general ledger accounts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlAccountFilter {
    pub account_type: Option<AccountType>,
    pub account_sub_type: Option<AccountSubType>,
    pub parent_account_id: Option<Uuid>,
    pub status: Option<AccountStatus>,
    pub is_posting: Option<bool>,
    pub is_header: Option<bool>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing fiscal periods
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlPeriodFilter {
    pub fiscal_year: Option<i32>,
    pub status: Option<PeriodStatus>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing journal entries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JournalEntryFilter {
    pub period_id: Option<Uuid>,
    pub entry_type: Option<JournalEntryType>,
    pub source: Option<JournalEntrySource>,
    pub status: Option<JournalEntryStatus>,
    pub account_id: Option<Uuid>,
    pub from_date: Option<NaiveDate>,
    pub to_date: Option<NaiveDate>,
    pub source_document_type: Option<String>,
    pub source_document_id: Option<Uuid>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// FX Revaluation
// ============================================================================

/// `reference_type` stamped on journal entry lines created by FX revaluation.
///
/// Lines carrying this marker are base-currency adjustments, so they are
/// excluded when deriving an account's outstanding foreign-currency balance
/// for subsequent revaluations.
pub const FX_REVALUATION_REFERENCE: &str = "fx_revaluation";

/// Per-account result of a period-end FX revaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevaluationLine {
    /// Account being revalued.
    pub account_id: Uuid,
    /// Denormalized account number.
    pub account_number: String,
    /// Denormalized account name.
    pub account_name: String,
    /// Currency the account is maintained in (differs from base currency).
    pub currency: CurrencyCode,
    /// Side (Debit/Credit) that increases this account.
    pub normal_balance: BalanceSide,
    /// Outstanding balance in the account's own currency (normal-balance
    /// terms), derived from posted lines excluding prior FX adjustments.
    pub foreign_balance: Decimal,
    /// Value currently carried on the books (base-currency terms).
    pub carrying_value: Decimal,
    /// Exchange rate used: 1 unit of account currency = `rate` base units.
    pub rate: Decimal,
    /// `foreign_balance * rate`, rounded to base-currency precision.
    pub revalued_value: Decimal,
    /// `revalued_value - carrying_value` in normal-balance terms.
    pub adjustment: Decimal,
    /// Unrealized FX gain (positive) or loss (negative) in base currency.
    pub unrealized_gain_loss: Decimal,
}

/// Result of a period-end FX revaluation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevaluationResult {
    /// Date the revaluation is effective (journal entry date).
    pub as_of_date: NaiveDate,
    /// Base (functional) currency balances were revalued into.
    pub base_currency: CurrencyCode,
    /// Sum of `unrealized_gain_loss` across all lines.
    pub total_unrealized_gain_loss: Decimal,
    /// Per-account revaluation detail (includes zero-adjustment accounts).
    pub lines: Vec<RevaluationLine>,
    /// Balanced, posted adjusting entry for the net delta; `None` when every
    /// evaluated account required no adjustment.
    pub journal_entry: Option<JournalEntry>,
}

/// Compute the unrealized FX gain/loss for one foreign-currency account.
///
/// `foreign_balance` is the account's outstanding balance in its own currency
/// (normal-balance terms, excluding prior revaluation adjustments); `rate`
/// converts one unit of the account currency into the base currency;
/// `base_decimal_places` is the base currency's precision.
#[must_use]
pub fn compute_revaluation_line(
    account: &GlAccount,
    foreign_balance: Decimal,
    rate: Decimal,
    base_decimal_places: u32,
) -> RevaluationLine {
    let normal_balance = account.account_type.normal_balance();
    let revalued_value = (foreign_balance * rate).round_dp(base_decimal_places);
    let adjustment = revalued_value - account.current_balance;
    // Growing a debit-normal account (asset) is a gain; growing a
    // credit-normal account (liability) is a loss.
    let unrealized_gain_loss = match normal_balance {
        BalanceSide::Credit => -adjustment,
        _ => adjustment,
    };
    RevaluationLine {
        account_id: account.id,
        account_number: account.account_number.clone(),
        account_name: account.name.clone(),
        currency: account.currency,
        normal_balance,
        foreign_balance,
        carrying_value: account.current_balance,
        rate,
        revalued_value,
        adjustment,
        unrealized_gain_loss,
    }
}

/// Build balanced journal entry lines for a set of revaluation adjustments.
///
/// Each non-zero adjustment posts on the side that moves the account toward
/// its revalued carrying amount; the net offset posts to `fx_account_id`
/// (credit for a net gain, debit for a net loss). Returns an empty vector
/// when no account requires adjustment.
#[must_use]
pub fn build_revaluation_journal_lines(
    lines: &[RevaluationLine],
    fx_account_id: Uuid,
) -> Vec<CreateJournalEntryLine> {
    let mut entry_lines = Vec::new();
    // Positive => the FX account must be credited (net gain).
    let mut fx_net = Decimal::ZERO;

    for line in lines {
        if line.adjustment.is_zero() {
            continue;
        }
        let amount = line.adjustment.abs();
        let increase = line.adjustment > Decimal::ZERO;
        let debit_side = match line.normal_balance {
            BalanceSide::Credit => !increase,
            _ => increase,
        };
        let (debit_amount, credit_amount) = if debit_side {
            fx_net += amount;
            (amount, Decimal::ZERO)
        } else {
            fx_net -= amount;
            (Decimal::ZERO, amount)
        };
        entry_lines.push(CreateJournalEntryLine {
            account_id: line.account_id,
            description: Some(format!(
                "FX revaluation of {} ({})",
                line.account_number, line.currency
            )),
            debit_amount,
            credit_amount,
            reference_type: Some(FX_REVALUATION_REFERENCE.to_string()),
            reference_id: Some(line.account_id),
        });
    }

    if !fx_net.is_zero() {
        let amount = fx_net.abs();
        let (debit_amount, credit_amount) =
            if fx_net > Decimal::ZERO { (Decimal::ZERO, amount) } else { (amount, Decimal::ZERO) };
        entry_lines.push(CreateJournalEntryLine {
            account_id: fx_account_id,
            description: Some("Unrealized FX gain/loss".to_string()),
            debit_amount,
            credit_amount,
            reference_type: Some(FX_REVALUATION_REFERENCE.to_string()),
            reference_id: None,
        });
    }

    entry_lines
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a journal entry number using a timestamp
#[must_use]
pub fn generate_journal_entry_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S%3f").to_string();
    let suffix = Uuid::new_v4().simple().to_string();
    format!("JE-{}-{}", timestamp, &suffix[..8])
}

/// Generate a period name in YYYY-MM format
#[must_use]
pub fn generate_period_name(year: i32, month: i32) -> String {
    format!("{year}-{month:02}")
}

/// Create a default Chart of Accounts
#[must_use]
pub fn create_default_chart_of_accounts() -> Vec<CreateGlAccount> {
    vec![
        // Assets (1xxx)
        CreateGlAccount {
            account_number: "1000".into(),
            name: "Assets".into(),
            description: Some("All asset accounts".into()),
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(true),
            is_posting: Some(false),
            currency: None,
        },
        CreateGlAccount {
            account_number: "1010".into(),
            name: "Cash".into(),
            description: Some("Cash and cash equivalents".into()),
            account_type: AccountType::Asset,
            account_sub_type: Some(AccountSubType::Cash),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
        CreateGlAccount {
            account_number: "1100".into(),
            name: "Accounts Receivable".into(),
            description: Some("Customer receivables".into()),
            account_type: AccountType::Asset,
            account_sub_type: Some(AccountSubType::AccountsReceivable),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
        CreateGlAccount {
            account_number: "1200".into(),
            name: "Inventory".into(),
            description: Some("Merchandise inventory".into()),
            account_type: AccountType::Asset,
            account_sub_type: Some(AccountSubType::Inventory),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
        // Liabilities (2xxx)
        CreateGlAccount {
            account_number: "2000".into(),
            name: "Liabilities".into(),
            description: Some("All liability accounts".into()),
            account_type: AccountType::Liability,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(true),
            is_posting: Some(false),
            currency: None,
        },
        CreateGlAccount {
            account_number: "2010".into(),
            name: "Accounts Payable".into(),
            description: Some("Supplier payables".into()),
            account_type: AccountType::Liability,
            account_sub_type: Some(AccountSubType::AccountsPayable),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
        // Equity (3xxx)
        CreateGlAccount {
            account_number: "3000".into(),
            name: "Equity".into(),
            description: Some("Owner's equity accounts".into()),
            account_type: AccountType::Equity,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(true),
            is_posting: Some(false),
            currency: None,
        },
        CreateGlAccount {
            account_number: "3100".into(),
            name: "Retained Earnings".into(),
            description: Some("Accumulated profits".into()),
            account_type: AccountType::Equity,
            account_sub_type: Some(AccountSubType::RetainedEarnings),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
        // Revenue (4xxx)
        CreateGlAccount {
            account_number: "4000".into(),
            name: "Revenue".into(),
            description: Some("All revenue accounts".into()),
            account_type: AccountType::Revenue,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(true),
            is_posting: Some(false),
            currency: None,
        },
        CreateGlAccount {
            account_number: "4010".into(),
            name: "Sales Revenue".into(),
            description: Some("Product sales".into()),
            account_type: AccountType::Revenue,
            account_sub_type: Some(AccountSubType::SalesRevenue),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
        // Expenses (5xxx)
        CreateGlAccount {
            account_number: "5000".into(),
            name: "Expenses".into(),
            description: Some("All expense accounts".into()),
            account_type: AccountType::Expense,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(true),
            is_posting: Some(false),
            currency: None,
        },
        CreateGlAccount {
            account_number: "5010".into(),
            name: "Cost of Goods Sold".into(),
            description: Some("Direct cost of products sold".into()),
            account_type: AccountType::Expense,
            account_sub_type: Some(AccountSubType::CostOfGoodsSold),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
        CreateGlAccount {
            account_number: "5900".into(),
            name: "Bad Debt Expense".into(),
            description: Some("Uncollectible accounts written off".into()),
            account_type: AccountType::Expense,
            account_sub_type: Some(AccountSubType::OtherExpense),
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn foreign_account(account_type: AccountType, carrying: Decimal) -> GlAccount {
        let now = Utc::now();
        GlAccount {
            id: Uuid::new_v4(),
            account_number: "1015".into(),
            name: "EUR Cash".into(),
            description: None,
            account_type,
            account_sub_type: None,
            parent_account_id: None,
            is_header: false,
            is_posting: true,
            normal_balance: account_type.normal_balance(),
            currency: CurrencyCode::EUR,
            status: AccountStatus::Active,
            current_balance: carrying,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn revaluation_gain_on_debit_normal_account() {
        // Booked 1000 EUR at 1.00; rate is now 1.10 => 100 unrealized gain.
        let account = foreign_account(AccountType::Asset, dec!(1000));
        let line = compute_revaluation_line(&account, dec!(1000), dec!(1.10), 2);
        assert_eq!(line.revalued_value, dec!(1100.00));
        assert_eq!(line.adjustment, dec!(100.00));
        assert_eq!(line.unrealized_gain_loss, dec!(100.00));
    }

    #[test]
    fn revaluation_loss_on_debit_normal_account() {
        let account = foreign_account(AccountType::Asset, dec!(1000));
        let line = compute_revaluation_line(&account, dec!(1000), dec!(0.85), 2);
        assert_eq!(line.adjustment, dec!(-150.00));
        assert_eq!(line.unrealized_gain_loss, dec!(-150.00));
    }

    #[test]
    fn revaluation_on_credit_normal_account_flips_sign() {
        // A payable growing in base terms is a loss.
        let account = foreign_account(AccountType::Liability, dec!(500));
        let line = compute_revaluation_line(&account, dec!(500), dec!(1.20), 2);
        assert_eq!(line.adjustment, dec!(100.00));
        assert_eq!(line.unrealized_gain_loss, dec!(-100.00));
    }

    #[test]
    fn revaluation_noop_when_rate_unchanged() {
        let account = foreign_account(AccountType::Asset, dec!(1000));
        let line = compute_revaluation_line(&account, dec!(1000), dec!(1), 2);
        assert!(line.adjustment.is_zero());
        assert!(line.unrealized_gain_loss.is_zero());
        assert!(build_revaluation_journal_lines(&[line], Uuid::new_v4()).is_empty());
    }

    #[test]
    fn revaluation_rounds_to_base_precision() {
        let account = foreign_account(AccountType::Asset, dec!(0));
        let line = compute_revaluation_line(&account, dec!(100.333), dec!(1.005), 2);
        assert_eq!(line.revalued_value, dec!(100.83));
    }

    #[test]
    fn revaluation_journal_lines_are_balanced() {
        let asset = foreign_account(AccountType::Asset, dec!(1000));
        let liability = foreign_account(AccountType::Liability, dec!(500));
        let fx_account = Uuid::new_v4();

        let lines = vec![
            compute_revaluation_line(&asset, dec!(1000), dec!(1.10), 2), // +100 gain
            compute_revaluation_line(&liability, dec!(500), dec!(1.20), 2), // -100 loss
        ];
        let je_lines = build_revaluation_journal_lines(&lines, fx_account);

        // Asset debit 100, liability credit 100 — nets to zero, so no FX line.
        assert_eq!(je_lines.len(), 2);
        let debits: Decimal = je_lines.iter().map(|l| l.debit_amount).sum();
        let credits: Decimal = je_lines.iter().map(|l| l.credit_amount).sum();
        assert_eq!(debits, credits);
        assert!(je_lines.iter().all(|l| l.reference_type.as_deref() == Some("fx_revaluation")));
    }

    #[test]
    fn revaluation_journal_lines_offset_net_gain_to_fx_account() {
        let asset = foreign_account(AccountType::Asset, dec!(1000));
        let fx_account = Uuid::new_v4();
        let lines = vec![compute_revaluation_line(&asset, dec!(1000), dec!(1.10), 2)];
        let je_lines = build_revaluation_journal_lines(&lines, fx_account);

        assert_eq!(je_lines.len(), 2);
        assert_eq!(je_lines[0].debit_amount, dec!(100.00));
        assert_eq!(je_lines[1].account_id, fx_account);
        assert_eq!(je_lines[1].credit_amount, dec!(100.00));
    }

    #[test]
    fn revaluation_journal_lines_offset_net_loss_to_fx_account() {
        let liability = foreign_account(AccountType::Liability, dec!(500));
        let fx_account = Uuid::new_v4();
        let lines = vec![compute_revaluation_line(&liability, dec!(500), dec!(1.20), 2)];
        let je_lines = build_revaluation_journal_lines(&lines, fx_account);

        assert_eq!(je_lines.len(), 2);
        assert_eq!(je_lines[0].credit_amount, dec!(100.00));
        assert_eq!(je_lines[1].account_id, fx_account);
        assert_eq!(je_lines[1].debit_amount, dec!(100.00));
    }
}
