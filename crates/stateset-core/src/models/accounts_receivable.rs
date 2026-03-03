//! Accounts Receivable domain models
//!
//! Extends the Invoice module with AR-specific features:
//! - Aging analysis and tracking
//! - Collections management (dunning)
//! - Write-offs and bad debt
//! - Credit memos and payment application
//! - Customer AR summaries and statements

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Enums
// ============================================================================

/// AR aging bucket classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AgingBucket {
    #[default]
    Current,
    #[strum(to_string = "1_30", serialize = "days_1_to_30")]
    Days1To30,
    #[strum(to_string = "31_60", serialize = "days_31_to_60")]
    Days31To60,
    #[strum(to_string = "61_90", serialize = "days_61_to_90")]
    Days61To90,
    #[strum(to_string = "over_90", serialize = "days_over_90")]
    DaysOver90,
}

/// Collection status for an invoice
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CollectionStatus {
    #[default]
    None,
    #[strum(serialize = "reminder_1_sent")]
    Reminder1Sent,
    #[strum(serialize = "reminder_2_sent")]
    Reminder2Sent,
    #[strum(serialize = "reminder_3_sent")]
    Reminder3Sent,
    InCollections,
    SentToAgency,
    WrittenOff,
    PromiseToPay,
    PaymentPlan,
}

/// Dunning letter template type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DunningLetterType {
    #[default]
    #[strum(serialize = "reminder_1")]
    Reminder1,
    #[strum(serialize = "reminder_2")]
    Reminder2,
    #[strum(serialize = "reminder_3")]
    Reminder3,
    DemandLetter,
    CollectionNotice,
}

/// Write-off reason code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum WriteOffReason {
    #[default]
    Uncollectible,
    Bankruptcy,
    CustomerDispute,
    SmallBalance,
    AccountClosed,
    Deceased,
    Other,
}

/// Credit memo reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditMemoReason {
    #[default]
    ReturnedGoods,
    PricingError,
    Overpayment,
    Damaged,
    ServiceCredit,
    GoodwillAdjustment,
    Other,
}

/// Credit memo status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditMemoStatus {
    #[default]
    Open,
    PartiallyApplied,
    FullyApplied,
    Voided,
}

/// Collection activity type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CollectionActivityType {
    #[default]
    DunningLetterSent,
    PhoneCall,
    Email,
    InPersonVisit,
    PromiseToPay,
    PaymentPlanCreated,
    SentToCollections,
    WriteOffApproved,
    DisputeLogged,
    DisputeResolved,
    Note,
}

/// Statement transaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StatementTransactionType {
    Invoice,
    Payment,
    CreditMemo,
    WriteOff,
    Adjustment,
}

// ============================================================================
// Core Structs
// ============================================================================

/// AR aging summary across all customers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArAgingSummary {
    pub current: Decimal,
    pub days_1_30: Decimal,
    pub days_31_60: Decimal,
    pub days_61_90: Decimal,
    pub days_over_90: Decimal,
    pub total: Decimal,
    pub as_of_date: DateTime<Utc>,
}

impl ArAgingSummary {
    /// Create an empty aging summary as of now
    pub fn new() -> Self {
        Self {
            current: Decimal::ZERO,
            days_1_30: Decimal::ZERO,
            days_31_60: Decimal::ZERO,
            days_61_90: Decimal::ZERO,
            days_over_90: Decimal::ZERO,
            total: Decimal::ZERO,
            as_of_date: Utc::now(),
        }
    }

    /// Returns the total overdue amount (everything except current)
    pub fn total_overdue(&self) -> Decimal {
        self.days_1_30 + self.days_31_60 + self.days_61_90 + self.days_over_90
    }

    /// Returns percentage of total that is overdue
    pub fn overdue_percentage(&self) -> Decimal {
        if self.total == Decimal::ZERO {
            Decimal::ZERO
        } else {
            (self.total_overdue() / self.total) * Decimal::from(100)
        }
    }
}

impl Default for ArAgingSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// AR aging by customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerArAging {
    pub customer_id: Uuid,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
    pub current: Decimal,
    pub days_1_30: Decimal,
    pub days_31_60: Decimal,
    pub days_61_90: Decimal,
    pub days_over_90: Decimal,
    pub total_outstanding: Decimal,
    pub invoice_count: i32,
    pub oldest_invoice_date: Option<DateTime<Utc>>,
    pub last_payment_date: Option<DateTime<Utc>>,
}

impl CustomerArAging {
    /// Returns the total overdue amount
    pub fn total_overdue(&self) -> Decimal {
        self.days_1_30 + self.days_31_60 + self.days_61_90 + self.days_over_90
    }

    /// Returns the worst aging bucket with a balance
    pub fn worst_aging_bucket(&self) -> AgingBucket {
        if self.days_over_90 > Decimal::ZERO {
            AgingBucket::DaysOver90
        } else if self.days_61_90 > Decimal::ZERO {
            AgingBucket::Days61To90
        } else if self.days_31_60 > Decimal::ZERO {
            AgingBucket::Days31To60
        } else if self.days_1_30 > Decimal::ZERO {
            AgingBucket::Days1To30
        } else {
            AgingBucket::Current
        }
    }
}

/// Collection activity record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionActivity {
    pub id: Uuid,
    pub invoice_id: Uuid,
    pub customer_id: Uuid,
    pub activity_type: CollectionActivityType,
    pub activity_date: DateTime<Utc>,
    pub dunning_letter_type: Option<DunningLetterType>,
    pub notes: Option<String>,
    pub contact_method: Option<String>,
    pub contact_result: Option<String>,
    pub promise_to_pay_date: Option<DateTime<Utc>>,
    pub promise_to_pay_amount: Option<Decimal>,
    pub performed_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Write-off record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteOff {
    pub id: Uuid,
    pub write_off_number: String,
    pub invoice_id: Uuid,
    pub customer_id: Uuid,
    pub amount: Decimal,
    pub reason: WriteOffReason,
    pub notes: Option<String>,
    pub write_off_date: DateTime<Utc>,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub reversed_at: Option<DateTime<Utc>>,
    pub gl_journal_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl WriteOff {
    /// Returns true if the write-off has been reversed
    pub const fn is_reversed(&self) -> bool {
        self.reversed_at.is_some()
    }
}

/// Credit memo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditMemo {
    pub id: Uuid,
    pub credit_memo_number: String,
    pub customer_id: Uuid,
    pub original_invoice_id: Option<Uuid>,
    pub reason: CreditMemoReason,
    pub amount: Decimal,
    pub applied_amount: Decimal,
    pub unapplied_amount: Decimal,
    pub status: CreditMemoStatus,
    pub notes: Option<String>,
    pub issue_date: DateTime<Utc>,
    pub gl_journal_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CreditMemo {
    /// Returns true if the credit memo can be applied to invoices
    pub fn can_apply(&self) -> bool {
        self.status != CreditMemoStatus::Voided
            && self.status != CreditMemoStatus::FullyApplied
            && self.unapplied_amount > Decimal::ZERO
    }
}

/// Credit memo application to invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditMemoApplication {
    pub id: Uuid,
    pub credit_memo_id: Uuid,
    pub invoice_id: Uuid,
    pub applied_amount: Decimal,
    pub applied_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// AR payment application (maps payments to invoices)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArPaymentApplication {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub invoice_id: Uuid,
    pub applied_amount: Decimal,
    pub applied_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Customer AR summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerArSummary {
    pub customer_id: Uuid,
    pub customer_name: Option<String>,
    pub total_outstanding: Decimal,
    pub total_overdue: Decimal,
    pub credit_limit: Option<Decimal>,
    pub available_credit: Option<Decimal>,
    pub unapplied_credits: Decimal,
    pub unapplied_payments: Decimal,
    pub average_days_to_pay: Option<i32>,
    pub oldest_open_invoice: Option<DateTime<Utc>>,
    pub last_activity_date: Option<DateTime<Utc>>,
    pub collection_status: CollectionStatus,
}

/// Customer statement line item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementLineItem {
    pub date: DateTime<Utc>,
    pub transaction_type: StatementTransactionType,
    pub reference_number: String,
    pub description: String,
    pub debit: Option<Decimal>,
    pub credit: Option<Decimal>,
    pub balance: Decimal,
}

/// Customer statement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerStatement {
    pub customer_id: Uuid,
    pub customer_name: String,
    pub customer_email: Option<String>,
    pub billing_address: Option<String>,
    pub statement_date: DateTime<Utc>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub opening_balance: Decimal,
    pub total_invoices: Decimal,
    pub total_payments: Decimal,
    pub total_credits: Decimal,
    pub closing_balance: Decimal,
    pub aging: CustomerArAging,
    pub line_items: Vec<StatementLineItem>,
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for logging a collection activity
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateCollectionActivity {
    pub invoice_id: Uuid,
    pub activity_type: CollectionActivityType,
    pub dunning_letter_type: Option<DunningLetterType>,
    pub notes: Option<String>,
    pub contact_method: Option<String>,
    pub contact_result: Option<String>,
    pub promise_to_pay_date: Option<DateTime<Utc>>,
    pub promise_to_pay_amount: Option<Decimal>,
    pub performed_by: Option<String>,
}

/// Input for creating a write-off
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWriteOff {
    pub invoice_id: Uuid,
    pub amount: Decimal,
    pub reason: WriteOffReason,
    pub notes: Option<String>,
    pub approved_by: Option<String>,
}

/// Input for creating a credit memo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCreditMemo {
    pub customer_id: Uuid,
    pub original_invoice_id: Option<Uuid>,
    pub reason: CreditMemoReason,
    pub amount: Decimal,
    pub notes: Option<String>,
}

/// Input for applying a payment across invoices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPaymentToInvoices {
    pub payment_id: Uuid,
    pub applications: Vec<PaymentApplicationLine>,
}

/// Payment allocation to a single invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentApplicationLine {
    pub invoice_id: Uuid,
    pub amount: Decimal,
}

/// Input for applying a credit memo to an invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCreditMemo {
    pub credit_memo_id: Uuid,
    pub invoice_id: Uuid,
    pub amount: Decimal,
}

/// Request to generate a customer statement
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GenerateStatementRequest {
    pub customer_id: Uuid,
    pub period_start: Option<DateTime<Utc>>,
    pub period_end: Option<DateTime<Utc>>,
    pub include_paid_invoices: Option<bool>,
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for AR aging queries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArAgingFilter {
    pub customer_id: Option<Uuid>,
    pub min_balance: Option<Decimal>,
    pub overdue_only: Option<bool>,
    pub aging_bucket: Option<AgingBucket>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for collection activity queries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CollectionActivityFilter {
    pub invoice_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub activity_type: Option<CollectionActivityType>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for write-off queries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WriteOffFilter {
    pub customer_id: Option<Uuid>,
    pub invoice_id: Option<Uuid>,
    pub reason: Option<WriteOffReason>,
    pub include_reversed: Option<bool>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for credit memo queries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditMemoFilter {
    pub customer_id: Option<Uuid>,
    pub status: Option<CreditMemoStatus>,
    pub reason: Option<CreditMemoReason>,
    pub has_unapplied: Option<bool>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a write-off reference number
pub fn generate_write_off_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("WO-{}-{}", timestamp, random)
}

/// Generate a credit memo reference number
pub fn generate_credit_memo_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("CM-{}-{}", timestamp, random)
}

/// Calculate the aging bucket for a given due date
pub fn calculate_aging_bucket(due_date: DateTime<Utc>) -> AgingBucket {
    let now = Utc::now();
    let days_overdue = (now - due_date).num_days();

    if days_overdue <= 0 {
        AgingBucket::Current
    } else if days_overdue <= 30 {
        AgingBucket::Days1To30
    } else if days_overdue <= 60 {
        AgingBucket::Days31To60
    } else if days_overdue <= 90 {
        AgingBucket::Days61To90
    } else {
        AgingBucket::DaysOver90
    }
}

/// Get the suggested dunning letter type based on aging
pub const fn suggest_dunning_letter(bucket: AgingBucket) -> Option<DunningLetterType> {
    match bucket {
        AgingBucket::Current => None,
        AgingBucket::Days1To30 => Some(DunningLetterType::Reminder1),
        AgingBucket::Days31To60 => Some(DunningLetterType::Reminder2),
        AgingBucket::Days61To90 => Some(DunningLetterType::Reminder3),
        AgingBucket::DaysOver90 => Some(DunningLetterType::DemandLetter),
    }
}
