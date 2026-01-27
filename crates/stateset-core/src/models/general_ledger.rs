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
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Account Type Enums
// ============================================================================

/// GL Account type (follows standard accounting)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Asset,
    Liability,
    Equity,
    Revenue,
    Expense,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountType::Asset => write!(f, "asset"),
            AccountType::Liability => write!(f, "liability"),
            AccountType::Equity => write!(f, "equity"),
            AccountType::Revenue => write!(f, "revenue"),
            AccountType::Expense => write!(f, "expense"),
        }
    }
}

impl FromStr for AccountType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "asset" => Ok(AccountType::Asset),
            "liability" => Ok(AccountType::Liability),
            "equity" => Ok(AccountType::Equity),
            "revenue" => Ok(AccountType::Revenue),
            "expense" => Ok(AccountType::Expense),
            _ => Err(format!("Unknown account type: {}", s)),
        }
    }
}

impl AccountType {
    /// Returns the normal balance side for this account type
    pub fn normal_balance(&self) -> BalanceSide {
        match self {
            AccountType::Asset | AccountType::Expense => BalanceSide::Debit,
            AccountType::Liability | AccountType::Equity | AccountType::Revenue => BalanceSide::Credit,
        }
    }

    /// Returns true if this account type appears on the Balance Sheet
    pub fn is_balance_sheet(&self) -> bool {
        matches!(self, AccountType::Asset | AccountType::Liability | AccountType::Equity)
    }

    /// Returns true if this account type appears on the Income Statement
    pub fn is_income_statement(&self) -> bool {
        matches!(self, AccountType::Revenue | AccountType::Expense)
    }
}

/// Balance side (debit or credit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BalanceSide {
    #[default]
    Debit,
    Credit,
}

impl fmt::Display for BalanceSide {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BalanceSide::Debit => write!(f, "debit"),
            BalanceSide::Credit => write!(f, "credit"),
        }
    }
}

impl FromStr for BalanceSide {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "debit" => Ok(BalanceSide::Debit),
            "credit" => Ok(BalanceSide::Credit),
            _ => Err(format!("Unknown balance side: {}", s)),
        }
    }
}

/// Account sub-types for more granular classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountSubType {
    // Assets
    Cash,
    AccountsReceivable,
    Inventory,
    PrepaidExpense,
    FixedAsset,
    AccumulatedDepreciation,
    OtherCurrentAsset,
    OtherNonCurrentAsset,
    // Liabilities
    AccountsPayable,
    AccruedLiabilities,
    UnearnedRevenue,
    ShortTermDebt,
    LongTermDebt,
    OtherCurrentLiability,
    OtherNonCurrentLiability,
    // Equity
    CommonStock,
    RetainedEarnings,
    OtherEquity,
    // Revenue
    SalesRevenue,
    ServiceRevenue,
    OtherRevenue,
    // Expense
    CostOfGoodsSold,
    OperatingExpense,
    Payroll,
    RentExpense,
    UtilitiesExpense,
    DepreciationExpense,
    InterestExpense,
    TaxExpense,
    OtherExpense,
}

impl fmt::Display for AccountSubType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountSubType::Cash => write!(f, "cash"),
            AccountSubType::AccountsReceivable => write!(f, "accounts_receivable"),
            AccountSubType::Inventory => write!(f, "inventory"),
            AccountSubType::PrepaidExpense => write!(f, "prepaid_expense"),
            AccountSubType::FixedAsset => write!(f, "fixed_asset"),
            AccountSubType::AccumulatedDepreciation => write!(f, "accumulated_depreciation"),
            AccountSubType::OtherCurrentAsset => write!(f, "other_current_asset"),
            AccountSubType::OtherNonCurrentAsset => write!(f, "other_non_current_asset"),
            AccountSubType::AccountsPayable => write!(f, "accounts_payable"),
            AccountSubType::AccruedLiabilities => write!(f, "accrued_liabilities"),
            AccountSubType::UnearnedRevenue => write!(f, "unearned_revenue"),
            AccountSubType::ShortTermDebt => write!(f, "short_term_debt"),
            AccountSubType::LongTermDebt => write!(f, "long_term_debt"),
            AccountSubType::OtherCurrentLiability => write!(f, "other_current_liability"),
            AccountSubType::OtherNonCurrentLiability => write!(f, "other_non_current_liability"),
            AccountSubType::CommonStock => write!(f, "common_stock"),
            AccountSubType::RetainedEarnings => write!(f, "retained_earnings"),
            AccountSubType::OtherEquity => write!(f, "other_equity"),
            AccountSubType::SalesRevenue => write!(f, "sales_revenue"),
            AccountSubType::ServiceRevenue => write!(f, "service_revenue"),
            AccountSubType::OtherRevenue => write!(f, "other_revenue"),
            AccountSubType::CostOfGoodsSold => write!(f, "cost_of_goods_sold"),
            AccountSubType::OperatingExpense => write!(f, "operating_expense"),
            AccountSubType::Payroll => write!(f, "payroll"),
            AccountSubType::RentExpense => write!(f, "rent_expense"),
            AccountSubType::UtilitiesExpense => write!(f, "utilities_expense"),
            AccountSubType::DepreciationExpense => write!(f, "depreciation_expense"),
            AccountSubType::InterestExpense => write!(f, "interest_expense"),
            AccountSubType::TaxExpense => write!(f, "tax_expense"),
            AccountSubType::OtherExpense => write!(f, "other_expense"),
        }
    }
}

impl FromStr for AccountSubType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cash" => Ok(AccountSubType::Cash),
            "accounts_receivable" => Ok(AccountSubType::AccountsReceivable),
            "inventory" => Ok(AccountSubType::Inventory),
            "prepaid_expense" => Ok(AccountSubType::PrepaidExpense),
            "fixed_asset" => Ok(AccountSubType::FixedAsset),
            "accumulated_depreciation" => Ok(AccountSubType::AccumulatedDepreciation),
            "other_current_asset" => Ok(AccountSubType::OtherCurrentAsset),
            "other_non_current_asset" => Ok(AccountSubType::OtherNonCurrentAsset),
            "accounts_payable" => Ok(AccountSubType::AccountsPayable),
            "accrued_liabilities" => Ok(AccountSubType::AccruedLiabilities),
            "unearned_revenue" => Ok(AccountSubType::UnearnedRevenue),
            "short_term_debt" => Ok(AccountSubType::ShortTermDebt),
            "long_term_debt" => Ok(AccountSubType::LongTermDebt),
            "other_current_liability" => Ok(AccountSubType::OtherCurrentLiability),
            "other_non_current_liability" => Ok(AccountSubType::OtherNonCurrentLiability),
            "common_stock" => Ok(AccountSubType::CommonStock),
            "retained_earnings" => Ok(AccountSubType::RetainedEarnings),
            "other_equity" => Ok(AccountSubType::OtherEquity),
            "sales_revenue" => Ok(AccountSubType::SalesRevenue),
            "service_revenue" => Ok(AccountSubType::ServiceRevenue),
            "other_revenue" => Ok(AccountSubType::OtherRevenue),
            "cost_of_goods_sold" => Ok(AccountSubType::CostOfGoodsSold),
            "operating_expense" => Ok(AccountSubType::OperatingExpense),
            "payroll" => Ok(AccountSubType::Payroll),
            "rent_expense" => Ok(AccountSubType::RentExpense),
            "utilities_expense" => Ok(AccountSubType::UtilitiesExpense),
            "depreciation_expense" => Ok(AccountSubType::DepreciationExpense),
            "interest_expense" => Ok(AccountSubType::InterestExpense),
            "tax_expense" => Ok(AccountSubType::TaxExpense),
            "other_expense" => Ok(AccountSubType::OtherExpense),
            _ => Err(format!("Unknown account sub-type: {}", s)),
        }
    }
}

/// Account status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    #[default]
    Active,
    Inactive,
    Archived,
}

impl fmt::Display for AccountStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountStatus::Active => write!(f, "active"),
            AccountStatus::Inactive => write!(f, "inactive"),
            AccountStatus::Archived => write!(f, "archived"),
        }
    }
}

impl FromStr for AccountStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(AccountStatus::Active),
            "inactive" => Ok(AccountStatus::Inactive),
            "archived" => Ok(AccountStatus::Archived),
            _ => Err(format!("Unknown account status: {}", s)),
        }
    }
}

// ============================================================================
// Period & Journal Entry Enums
// ============================================================================

/// GL Period status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PeriodStatus {
    #[default]
    Future,
    Open,
    Closed,
    Locked,
}

impl fmt::Display for PeriodStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PeriodStatus::Future => write!(f, "future"),
            PeriodStatus::Open => write!(f, "open"),
            PeriodStatus::Closed => write!(f, "closed"),
            PeriodStatus::Locked => write!(f, "locked"),
        }
    }
}

impl FromStr for PeriodStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "future" => Ok(PeriodStatus::Future),
            "open" => Ok(PeriodStatus::Open),
            "closed" => Ok(PeriodStatus::Closed),
            "locked" => Ok(PeriodStatus::Locked),
            _ => Err(format!("Unknown period status: {}", s)),
        }
    }
}

/// Journal entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JournalEntryType {
    #[default]
    Standard,
    Adjusting,
    Closing,
    Reversing,
    Opening,
}

impl fmt::Display for JournalEntryType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalEntryType::Standard => write!(f, "standard"),
            JournalEntryType::Adjusting => write!(f, "adjusting"),
            JournalEntryType::Closing => write!(f, "closing"),
            JournalEntryType::Reversing => write!(f, "reversing"),
            JournalEntryType::Opening => write!(f, "opening"),
        }
    }
}

impl FromStr for JournalEntryType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(JournalEntryType::Standard),
            "adjusting" => Ok(JournalEntryType::Adjusting),
            "closing" => Ok(JournalEntryType::Closing),
            "reversing" => Ok(JournalEntryType::Reversing),
            "opening" => Ok(JournalEntryType::Opening),
            _ => Err(format!("Unknown journal entry type: {}", s)),
        }
    }
}

/// Journal entry source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JournalEntrySource {
    #[default]
    Manual,
    AutoInvoice,
    AutoPayment,
    AutoBill,
    AutoBillPayment,
    AutoInventory,
    AutoWriteOff,
    SystemClosing,
    Import,
}

impl fmt::Display for JournalEntrySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalEntrySource::Manual => write!(f, "manual"),
            JournalEntrySource::AutoInvoice => write!(f, "auto_invoice"),
            JournalEntrySource::AutoPayment => write!(f, "auto_payment"),
            JournalEntrySource::AutoBill => write!(f, "auto_bill"),
            JournalEntrySource::AutoBillPayment => write!(f, "auto_bill_payment"),
            JournalEntrySource::AutoInventory => write!(f, "auto_inventory"),
            JournalEntrySource::AutoWriteOff => write!(f, "auto_write_off"),
            JournalEntrySource::SystemClosing => write!(f, "system_closing"),
            JournalEntrySource::Import => write!(f, "import"),
        }
    }
}

impl FromStr for JournalEntrySource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "manual" => Ok(JournalEntrySource::Manual),
            "auto_invoice" => Ok(JournalEntrySource::AutoInvoice),
            "auto_payment" => Ok(JournalEntrySource::AutoPayment),
            "auto_bill" => Ok(JournalEntrySource::AutoBill),
            "auto_bill_payment" => Ok(JournalEntrySource::AutoBillPayment),
            "auto_inventory" => Ok(JournalEntrySource::AutoInventory),
            "auto_write_off" => Ok(JournalEntrySource::AutoWriteOff),
            "system_closing" => Ok(JournalEntrySource::SystemClosing),
            "import" => Ok(JournalEntrySource::Import),
            _ => Err(format!("Unknown journal entry source: {}", s)),
        }
    }
}

/// Journal entry status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JournalEntryStatus {
    #[default]
    Draft,
    Pending,
    Posted,
    Voided,
    Reversed,
}

impl fmt::Display for JournalEntryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalEntryStatus::Draft => write!(f, "draft"),
            JournalEntryStatus::Pending => write!(f, "pending"),
            JournalEntryStatus::Posted => write!(f, "posted"),
            JournalEntryStatus::Voided => write!(f, "voided"),
            JournalEntryStatus::Reversed => write!(f, "reversed"),
        }
    }
}

impl FromStr for JournalEntryStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(JournalEntryStatus::Draft),
            "pending" => Ok(JournalEntryStatus::Pending),
            "posted" => Ok(JournalEntryStatus::Posted),
            "voided" => Ok(JournalEntryStatus::Voided),
            "reversed" => Ok(JournalEntryStatus::Reversed),
            _ => Err(format!("Unknown journal entry status: {}", s)),
        }
    }
}

// ============================================================================
// Core GL Structs
// ============================================================================

/// A GL Account (Chart of Accounts entry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlAccount {
    pub id: Uuid,
    pub account_number: String,
    pub name: String,
    pub description: Option<String>,
    pub account_type: AccountType,
    pub account_sub_type: Option<AccountSubType>,
    pub parent_account_id: Option<Uuid>,
    pub is_header: bool,
    pub is_posting: bool,
    pub normal_balance: BalanceSide,
    pub currency: String,
    pub status: AccountStatus,
    pub current_balance: Decimal,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl GlAccount {
    /// Returns true if this account can accept postings
    pub fn can_post(&self) -> bool {
        self.is_posting && self.status == AccountStatus::Active
    }

    /// Calculates the balance effect of a debit/credit
    pub fn balance_effect(&self, debit: Decimal, credit: Decimal) -> Decimal {
        match self.normal_balance {
            BalanceSide::Debit => debit - credit,
            BalanceSide::Credit => credit - debit,
        }
    }
}

/// GL Period (accounting period)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlPeriod {
    pub id: Uuid,
    pub period_name: String,
    pub fiscal_year: i32,
    pub period_number: i32,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: PeriodStatus,
    pub closed_at: Option<DateTime<Utc>>,
    pub closed_by: Option<String>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl GlPeriod {
    /// Returns true if the period allows posting
    pub fn can_post(&self) -> bool {
        self.status == PeriodStatus::Open
    }

    /// Returns true if a date falls within this period
    pub fn contains_date(&self, date: NaiveDate) -> bool {
        date >= self.start_date && date <= self.end_date
    }
}

/// Journal Entry header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: Uuid,
    pub entry_number: String,
    pub entry_date: NaiveDate,
    pub period_id: Uuid,
    pub entry_type: JournalEntryType,
    pub source: JournalEntrySource,
    pub source_document_type: Option<String>,
    pub source_document_id: Option<Uuid>,
    pub description: String,
    pub total_debits: Decimal,
    pub total_credits: Decimal,
    pub is_balanced: bool,
    pub status: JournalEntryStatus,
    pub posted_at: Option<DateTime<Utc>>,
    pub posted_by: Option<String>,
    pub reversed_entry_id: Option<Uuid>,
    pub reversing_entry_id: Option<Uuid>,
    pub lines: Vec<JournalEntryLine>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl JournalEntry {
    /// Returns true if debits equal credits
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
        self.status == JournalEntryStatus::Draft
            && self.is_balanced
            && !self.lines.is_empty()
    }

    /// Returns true if entry can be voided
    pub fn can_void(&self) -> bool {
        self.status == JournalEntryStatus::Posted
    }
}

/// Journal Entry line (detail)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntryLine {
    pub id: Uuid,
    pub journal_entry_id: Uuid,
    pub line_number: i32,
    pub account_id: Uuid,
    pub account_number: Option<String>,
    pub account_name: Option<String>,
    pub description: Option<String>,
    pub debit_amount: Decimal,
    pub credit_amount: Decimal,
    pub currency: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl JournalEntryLine {
    /// Returns true if line has only debit or only credit
    pub fn is_valid(&self) -> bool {
        (self.debit_amount > Decimal::ZERO && self.credit_amount == Decimal::ZERO)
            || (self.debit_amount == Decimal::ZERO && self.credit_amount > Decimal::ZERO)
    }

    /// Returns the net amount (positive for debit, negative for credit)
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
    pub currency: Option<String>,
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
    pub fn debit(account_id: Uuid, amount: Decimal, description: Option<String>) -> Self {
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
    pub fn credit(account_id: Uuid, amount: Decimal, description: Option<String>) -> Self {
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
// Helper Functions
// ============================================================================

/// Generate a journal entry number using a timestamp
pub fn generate_journal_entry_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    format!("JE-{}", timestamp)
}

/// Generate a period name in YYYY-MM format
pub fn generate_period_name(year: i32, month: i32) -> String {
    format!("{}-{:02}", year, month)
}

/// Create a default Chart of Accounts
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
