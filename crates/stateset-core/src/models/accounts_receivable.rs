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
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Enums
// ============================================================================

/// AR aging bucket classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgingBucket {
    #[default]
    Current,
    Days1To30,
    Days31To60,
    Days61To90,
    DaysOver90,
}

impl fmt::Display for AgingBucket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgingBucket::Current => write!(f, "current"),
            AgingBucket::Days1To30 => write!(f, "1_30"),
            AgingBucket::Days31To60 => write!(f, "31_60"),
            AgingBucket::Days61To90 => write!(f, "61_90"),
            AgingBucket::DaysOver90 => write!(f, "over_90"),
        }
    }
}

impl FromStr for AgingBucket {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "current" => Ok(AgingBucket::Current),
            "1_30" | "days_1_to_30" => Ok(AgingBucket::Days1To30),
            "31_60" | "days_31_to_60" => Ok(AgingBucket::Days31To60),
            "61_90" | "days_61_to_90" => Ok(AgingBucket::Days61To90),
            "over_90" | "days_over_90" => Ok(AgingBucket::DaysOver90),
            _ => Err(format!("Unknown aging bucket: {}", s)),
        }
    }
}

/// Collection status for an invoice
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CollectionStatus {
    #[default]
    None,
    Reminder1Sent,
    Reminder2Sent,
    Reminder3Sent,
    InCollections,
    SentToAgency,
    WrittenOff,
    PromiseToPay,
    PaymentPlan,
}

impl fmt::Display for CollectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CollectionStatus::None => write!(f, "none"),
            CollectionStatus::Reminder1Sent => write!(f, "reminder_1_sent"),
            CollectionStatus::Reminder2Sent => write!(f, "reminder_2_sent"),
            CollectionStatus::Reminder3Sent => write!(f, "reminder_3_sent"),
            CollectionStatus::InCollections => write!(f, "in_collections"),
            CollectionStatus::SentToAgency => write!(f, "sent_to_agency"),
            CollectionStatus::WrittenOff => write!(f, "written_off"),
            CollectionStatus::PromiseToPay => write!(f, "promise_to_pay"),
            CollectionStatus::PaymentPlan => write!(f, "payment_plan"),
        }
    }
}

impl FromStr for CollectionStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" => Ok(CollectionStatus::None),
            "reminder_1_sent" => Ok(CollectionStatus::Reminder1Sent),
            "reminder_2_sent" => Ok(CollectionStatus::Reminder2Sent),
            "reminder_3_sent" => Ok(CollectionStatus::Reminder3Sent),
            "in_collections" => Ok(CollectionStatus::InCollections),
            "sent_to_agency" => Ok(CollectionStatus::SentToAgency),
            "written_off" => Ok(CollectionStatus::WrittenOff),
            "promise_to_pay" => Ok(CollectionStatus::PromiseToPay),
            "payment_plan" => Ok(CollectionStatus::PaymentPlan),
            _ => Err(format!("Unknown collection status: {}", s)),
        }
    }
}

/// Dunning letter template type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DunningLetterType {
    #[default]
    Reminder1,
    Reminder2,
    Reminder3,
    DemandLetter,
    CollectionNotice,
}

impl fmt::Display for DunningLetterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DunningLetterType::Reminder1 => write!(f, "reminder_1"),
            DunningLetterType::Reminder2 => write!(f, "reminder_2"),
            DunningLetterType::Reminder3 => write!(f, "reminder_3"),
            DunningLetterType::DemandLetter => write!(f, "demand_letter"),
            DunningLetterType::CollectionNotice => write!(f, "collection_notice"),
        }
    }
}

impl FromStr for DunningLetterType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "reminder_1" => Ok(DunningLetterType::Reminder1),
            "reminder_2" => Ok(DunningLetterType::Reminder2),
            "reminder_3" => Ok(DunningLetterType::Reminder3),
            "demand_letter" => Ok(DunningLetterType::DemandLetter),
            "collection_notice" => Ok(DunningLetterType::CollectionNotice),
            _ => Err(format!("Unknown dunning letter type: {}", s)),
        }
    }
}

/// Write-off reason code
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl fmt::Display for WriteOffReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteOffReason::Uncollectible => write!(f, "uncollectible"),
            WriteOffReason::Bankruptcy => write!(f, "bankruptcy"),
            WriteOffReason::CustomerDispute => write!(f, "customer_dispute"),
            WriteOffReason::SmallBalance => write!(f, "small_balance"),
            WriteOffReason::AccountClosed => write!(f, "account_closed"),
            WriteOffReason::Deceased => write!(f, "deceased"),
            WriteOffReason::Other => write!(f, "other"),
        }
    }
}

impl FromStr for WriteOffReason {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "uncollectible" => Ok(WriteOffReason::Uncollectible),
            "bankruptcy" => Ok(WriteOffReason::Bankruptcy),
            "customer_dispute" => Ok(WriteOffReason::CustomerDispute),
            "small_balance" => Ok(WriteOffReason::SmallBalance),
            "account_closed" => Ok(WriteOffReason::AccountClosed),
            "deceased" => Ok(WriteOffReason::Deceased),
            "other" => Ok(WriteOffReason::Other),
            _ => Err(format!("Unknown write-off reason: {}", s)),
        }
    }
}

/// Credit memo reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl fmt::Display for CreditMemoReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreditMemoReason::ReturnedGoods => write!(f, "returned_goods"),
            CreditMemoReason::PricingError => write!(f, "pricing_error"),
            CreditMemoReason::Overpayment => write!(f, "overpayment"),
            CreditMemoReason::Damaged => write!(f, "damaged"),
            CreditMemoReason::ServiceCredit => write!(f, "service_credit"),
            CreditMemoReason::GoodwillAdjustment => write!(f, "goodwill_adjustment"),
            CreditMemoReason::Other => write!(f, "other"),
        }
    }
}

impl FromStr for CreditMemoReason {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "returned_goods" => Ok(CreditMemoReason::ReturnedGoods),
            "pricing_error" => Ok(CreditMemoReason::PricingError),
            "overpayment" => Ok(CreditMemoReason::Overpayment),
            "damaged" => Ok(CreditMemoReason::Damaged),
            "service_credit" => Ok(CreditMemoReason::ServiceCredit),
            "goodwill_adjustment" => Ok(CreditMemoReason::GoodwillAdjustment),
            "other" => Ok(CreditMemoReason::Other),
            _ => Err(format!("Unknown credit memo reason: {}", s)),
        }
    }
}

/// Credit memo status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreditMemoStatus {
    #[default]
    Open,
    PartiallyApplied,
    FullyApplied,
    Voided,
}

impl fmt::Display for CreditMemoStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CreditMemoStatus::Open => write!(f, "open"),
            CreditMemoStatus::PartiallyApplied => write!(f, "partially_applied"),
            CreditMemoStatus::FullyApplied => write!(f, "fully_applied"),
            CreditMemoStatus::Voided => write!(f, "voided"),
        }
    }
}

impl FromStr for CreditMemoStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(CreditMemoStatus::Open),
            "partially_applied" => Ok(CreditMemoStatus::PartiallyApplied),
            "fully_applied" => Ok(CreditMemoStatus::FullyApplied),
            "voided" => Ok(CreditMemoStatus::Voided),
            _ => Err(format!("Unknown credit memo status: {}", s)),
        }
    }
}

/// Collection activity type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl fmt::Display for CollectionActivityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CollectionActivityType::DunningLetterSent => write!(f, "dunning_letter_sent"),
            CollectionActivityType::PhoneCall => write!(f, "phone_call"),
            CollectionActivityType::Email => write!(f, "email"),
            CollectionActivityType::InPersonVisit => write!(f, "in_person_visit"),
            CollectionActivityType::PromiseToPay => write!(f, "promise_to_pay"),
            CollectionActivityType::PaymentPlanCreated => write!(f, "payment_plan_created"),
            CollectionActivityType::SentToCollections => write!(f, "sent_to_collections"),
            CollectionActivityType::WriteOffApproved => write!(f, "write_off_approved"),
            CollectionActivityType::DisputeLogged => write!(f, "dispute_logged"),
            CollectionActivityType::DisputeResolved => write!(f, "dispute_resolved"),
            CollectionActivityType::Note => write!(f, "note"),
        }
    }
}

impl FromStr for CollectionActivityType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dunning_letter_sent" => Ok(CollectionActivityType::DunningLetterSent),
            "phone_call" => Ok(CollectionActivityType::PhoneCall),
            "email" => Ok(CollectionActivityType::Email),
            "in_person_visit" => Ok(CollectionActivityType::InPersonVisit),
            "promise_to_pay" => Ok(CollectionActivityType::PromiseToPay),
            "payment_plan_created" => Ok(CollectionActivityType::PaymentPlanCreated),
            "sent_to_collections" => Ok(CollectionActivityType::SentToCollections),
            "write_off_approved" => Ok(CollectionActivityType::WriteOffApproved),
            "dispute_logged" => Ok(CollectionActivityType::DisputeLogged),
            "dispute_resolved" => Ok(CollectionActivityType::DisputeResolved),
            "note" => Ok(CollectionActivityType::Note),
            _ => Err(format!("Unknown collection activity type: {}", s)),
        }
    }
}

/// Statement transaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementTransactionType {
    Invoice,
    Payment,
    CreditMemo,
    WriteOff,
    Adjustment,
}

impl fmt::Display for StatementTransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatementTransactionType::Invoice => write!(f, "invoice"),
            StatementTransactionType::Payment => write!(f, "payment"),
            StatementTransactionType::CreditMemo => write!(f, "credit_memo"),
            StatementTransactionType::WriteOff => write!(f, "write_off"),
            StatementTransactionType::Adjustment => write!(f, "adjustment"),
        }
    }
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
    pub fn is_reversed(&self) -> bool {
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
    pub fn can_apply(&self) -> bool {
        self.status != CreditMemoStatus::Voided &&
        self.status != CreditMemoStatus::FullyApplied &&
        self.unapplied_amount > Decimal::ZERO
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWriteOff {
    pub invoice_id: Uuid,
    pub amount: Decimal,
    pub reason: WriteOffReason,
    pub notes: Option<String>,
    pub approved_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCreditMemo {
    pub customer_id: Uuid,
    pub original_invoice_id: Option<Uuid>,
    pub reason: CreditMemoReason,
    pub amount: Decimal,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPaymentToInvoices {
    pub payment_id: Uuid,
    pub applications: Vec<PaymentApplicationLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentApplicationLine {
    pub invoice_id: Uuid,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCreditMemo {
    pub credit_memo_id: Uuid,
    pub invoice_id: Uuid,
    pub amount: Decimal,
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArAgingFilter {
    pub customer_id: Option<Uuid>,
    pub min_balance: Option<Decimal>,
    pub overdue_only: Option<bool>,
    pub aging_bucket: Option<AgingBucket>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

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

pub fn generate_write_off_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("WO-{}-{}", timestamp, random)
}

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
pub fn suggest_dunning_letter(bucket: AgingBucket) -> Option<DunningLetterType> {
    match bucket {
        AgingBucket::Current => None,
        AgingBucket::Days1To30 => Some(DunningLetterType::Reminder1),
        AgingBucket::Days31To60 => Some(DunningLetterType::Reminder2),
        AgingBucket::Days61To90 => Some(DunningLetterType::Reminder3),
        AgingBucket::DaysOver90 => Some(DunningLetterType::DemandLetter),
    }
}
