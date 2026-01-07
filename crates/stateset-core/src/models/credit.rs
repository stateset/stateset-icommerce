//! Credit Management domain models
//!
//! Models for customer credit limits, credit holds, and credit applications.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Core Credit Types
// ============================================================================

/// Customer credit account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditAccount {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub credit_limit: Decimal,
    pub available_credit: Decimal,
    pub current_balance: Decimal,
    pub hold_amount: Decimal,
    pub currency: String,
    pub status: CreditAccountStatus,
    pub payment_terms: Option<String>,
    pub risk_rating: Option<RiskRating>,
    pub last_review_date: Option<DateTime<Utc>>,
    pub next_review_date: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Credit hold on an order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditHold {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub order_id: Option<Uuid>,
    pub hold_type: CreditHoldType,
    pub hold_amount: Decimal,
    pub reason: String,
    pub status: CreditHoldStatus,
    pub placed_by: Option<String>,
    pub placed_at: DateTime<Utc>,
    pub released_by: Option<String>,
    pub released_at: Option<DateTime<Utc>>,
    pub release_notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Credit application from a customer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditApplication {
    pub id: Uuid,
    pub application_number: String,
    pub customer_id: Uuid,
    pub requested_limit: Decimal,
    pub approved_limit: Option<Decimal>,
    pub status: CreditApplicationStatus,
    pub business_name: Option<String>,
    pub tax_id: Option<String>,
    pub years_in_business: Option<i32>,
    pub annual_revenue: Option<Decimal>,
    pub bank_reference: Option<String>,
    pub trade_references: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub decision_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Credit transaction history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditTransaction {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub transaction_type: CreditTransactionType,
    pub amount: Decimal,
    pub running_balance: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Credit check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditCheckResult {
    pub customer_id: Uuid,
    pub order_amount: Decimal,
    pub credit_limit: Decimal,
    pub available_credit: Decimal,
    pub current_balance: Decimal,
    pub approved: bool,
    pub reason: Option<String>,
    pub requires_approval: bool,
    pub checked_at: DateTime<Utc>,
}

// ============================================================================
// Enums
// ============================================================================

/// Credit account status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreditAccountStatus {
    #[default]
    Active,
    Suspended,
    OnHold,
    Closed,
    PendingReview,
}

impl std::fmt::Display for CreditAccountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreditAccountStatus::Active => write!(f, "active"),
            CreditAccountStatus::Suspended => write!(f, "suspended"),
            CreditAccountStatus::OnHold => write!(f, "on_hold"),
            CreditAccountStatus::Closed => write!(f, "closed"),
            CreditAccountStatus::PendingReview => write!(f, "pending_review"),
        }
    }
}

impl FromStr for CreditAccountStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(CreditAccountStatus::Active),
            "suspended" => Ok(CreditAccountStatus::Suspended),
            "on_hold" => Ok(CreditAccountStatus::OnHold),
            "closed" => Ok(CreditAccountStatus::Closed),
            "pending_review" => Ok(CreditAccountStatus::PendingReview),
            _ => Ok(CreditAccountStatus::Active),
        }
    }
}

/// Risk rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RiskRating {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskRating {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskRating::Low => write!(f, "low"),
            RiskRating::Medium => write!(f, "medium"),
            RiskRating::High => write!(f, "high"),
            RiskRating::Critical => write!(f, "critical"),
        }
    }
}

impl FromStr for RiskRating {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(RiskRating::Low),
            "medium" => Ok(RiskRating::Medium),
            "high" => Ok(RiskRating::High),
            "critical" => Ok(RiskRating::Critical),
            _ => Ok(RiskRating::Medium),
        }
    }
}

/// Credit hold type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreditHoldType {
    #[default]
    OverLimit,
    PastDue,
    Manual,
    NewCustomer,
    HighRisk,
}

impl std::fmt::Display for CreditHoldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreditHoldType::OverLimit => write!(f, "over_limit"),
            CreditHoldType::PastDue => write!(f, "past_due"),
            CreditHoldType::Manual => write!(f, "manual"),
            CreditHoldType::NewCustomer => write!(f, "new_customer"),
            CreditHoldType::HighRisk => write!(f, "high_risk"),
        }
    }
}

impl FromStr for CreditHoldType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "over_limit" => Ok(CreditHoldType::OverLimit),
            "past_due" => Ok(CreditHoldType::PastDue),
            "manual" => Ok(CreditHoldType::Manual),
            "new_customer" => Ok(CreditHoldType::NewCustomer),
            "high_risk" => Ok(CreditHoldType::HighRisk),
            _ => Ok(CreditHoldType::Manual),
        }
    }
}

/// Credit hold status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreditHoldStatus {
    #[default]
    Active,
    Released,
    Expired,
}

impl std::fmt::Display for CreditHoldStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreditHoldStatus::Active => write!(f, "active"),
            CreditHoldStatus::Released => write!(f, "released"),
            CreditHoldStatus::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for CreditHoldStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(CreditHoldStatus::Active),
            "released" => Ok(CreditHoldStatus::Released),
            "expired" => Ok(CreditHoldStatus::Expired),
            _ => Ok(CreditHoldStatus::Active),
        }
    }
}

/// Credit application status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreditApplicationStatus {
    #[default]
    Pending,
    UnderReview,
    Approved,
    Denied,
    MoreInfoNeeded,
    Withdrawn,
}

impl std::fmt::Display for CreditApplicationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreditApplicationStatus::Pending => write!(f, "pending"),
            CreditApplicationStatus::UnderReview => write!(f, "under_review"),
            CreditApplicationStatus::Approved => write!(f, "approved"),
            CreditApplicationStatus::Denied => write!(f, "denied"),
            CreditApplicationStatus::MoreInfoNeeded => write!(f, "more_info_needed"),
            CreditApplicationStatus::Withdrawn => write!(f, "withdrawn"),
        }
    }
}

impl FromStr for CreditApplicationStatus {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(CreditApplicationStatus::Pending),
            "under_review" => Ok(CreditApplicationStatus::UnderReview),
            "approved" => Ok(CreditApplicationStatus::Approved),
            "denied" => Ok(CreditApplicationStatus::Denied),
            "more_info_needed" => Ok(CreditApplicationStatus::MoreInfoNeeded),
            "withdrawn" => Ok(CreditApplicationStatus::Withdrawn),
            _ => Ok(CreditApplicationStatus::Pending),
        }
    }
}

/// Credit transaction type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CreditTransactionType {
    #[default]
    Charge,
    Payment,
    CreditMemo,
    Adjustment,
    WriteOff,
    LimitChange,
}

impl std::fmt::Display for CreditTransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreditTransactionType::Charge => write!(f, "charge"),
            CreditTransactionType::Payment => write!(f, "payment"),
            CreditTransactionType::CreditMemo => write!(f, "credit_memo"),
            CreditTransactionType::Adjustment => write!(f, "adjustment"),
            CreditTransactionType::WriteOff => write!(f, "write_off"),
            CreditTransactionType::LimitChange => write!(f, "limit_change"),
        }
    }
}

impl FromStr for CreditTransactionType {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "charge" => Ok(CreditTransactionType::Charge),
            "payment" => Ok(CreditTransactionType::Payment),
            "credit_memo" => Ok(CreditTransactionType::CreditMemo),
            "adjustment" => Ok(CreditTransactionType::Adjustment),
            "write_off" => Ok(CreditTransactionType::WriteOff),
            "limit_change" => Ok(CreditTransactionType::LimitChange),
            _ => Ok(CreditTransactionType::Charge),
        }
    }
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a credit account.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateCreditAccount {
    pub customer_id: Uuid,
    pub credit_limit: Decimal,
    pub currency: Option<String>,
    pub payment_terms: Option<String>,
    pub risk_rating: Option<RiskRating>,
    pub notes: Option<String>,
}

/// Input for updating a credit account.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCreditAccount {
    pub credit_limit: Option<Decimal>,
    pub status: Option<CreditAccountStatus>,
    pub payment_terms: Option<String>,
    pub risk_rating: Option<RiskRating>,
    pub notes: Option<String>,
}

/// Input for placing a credit hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceCreditHold {
    pub customer_id: Uuid,
    pub order_id: Option<Uuid>,
    pub hold_type: CreditHoldType,
    pub hold_amount: Decimal,
    pub reason: String,
    pub placed_by: Option<String>,
}

/// Input for releasing a credit hold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseCreditHold {
    pub hold_id: Uuid,
    pub released_by: Option<String>,
    pub release_notes: Option<String>,
}

/// Input for submitting a credit application.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubmitCreditApplication {
    pub customer_id: Uuid,
    pub requested_limit: Decimal,
    pub business_name: Option<String>,
    pub tax_id: Option<String>,
    pub years_in_business: Option<i32>,
    pub annual_revenue: Option<Decimal>,
    pub bank_reference: Option<String>,
    pub trade_references: Option<String>,
}

/// Input for reviewing a credit application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCreditApplication {
    pub application_id: Uuid,
    pub approved_limit: Option<Decimal>,
    pub status: CreditApplicationStatus,
    pub reviewed_by: String,
    pub decision_notes: Option<String>,
}

/// Input for recording a credit transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordCreditTransaction {
    pub customer_id: Uuid,
    pub transaction_type: CreditTransactionType,
    pub amount: Decimal,
    pub reference_type: Option<String>,
    pub reference_id: Option<Uuid>,
    pub notes: Option<String>,
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for listing credit accounts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditAccountFilter {
    pub customer_id: Option<Uuid>,
    pub status: Option<CreditAccountStatus>,
    pub risk_rating: Option<RiskRating>,
    pub over_limit: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing credit holds.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditHoldFilter {
    pub customer_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub hold_type: Option<CreditHoldType>,
    pub status: Option<CreditHoldStatus>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing credit applications.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditApplicationFilter {
    pub customer_id: Option<Uuid>,
    pub status: Option<CreditApplicationStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing credit transactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditTransactionFilter {
    pub customer_id: Option<Uuid>,
    pub transaction_type: Option<CreditTransactionType>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Summary Types
// ============================================================================

/// Credit aging bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditAgingBucket {
    pub current: Decimal,
    pub days_1_30: Decimal,
    pub days_31_60: Decimal,
    pub days_61_90: Decimal,
    pub days_over_90: Decimal,
    pub total: Decimal,
}

/// Customer credit summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerCreditSummary {
    pub customer_id: Uuid,
    pub credit_limit: Decimal,
    pub current_balance: Decimal,
    pub available_credit: Decimal,
    pub oldest_due_date: Option<DateTime<Utc>>,
    pub days_past_due: i32,
    pub hold_count: i32,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a credit application number.
pub fn generate_credit_application_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("CAPP-{}-{}", timestamp, random)
}
