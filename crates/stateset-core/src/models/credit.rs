//! Credit Management domain models
//!
//! Models for customer credit limits, credit holds, and credit applications.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CreditId, CustomerId, OrderId};
use std::str::FromStr;
use uuid::Uuid;

// ============================================================================
// Core Credit Types
// ============================================================================

/// Customer credit account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditAccount {
    pub id: CreditId,
    pub customer_id: CustomerId,
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
    pub customer_id: CustomerId,
    pub order_id: Option<OrderId>,
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
    pub customer_id: CustomerId,
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
    pub customer_id: CustomerId,
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
    pub customer_id: CustomerId,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditAccountStatus {
    #[default]
    Active,
    Suspended,
    OnHold,
    Closed,
    PendingReview,
}

impl FromStr for CreditAccountStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "on_hold" | "onhold" => Ok(Self::OnHold),
            "closed" => Ok(Self::Closed),
            "pending_review" | "pendingreview" => Ok(Self::PendingReview),
            _ => Err(format!("Unknown credit account status: {}", s)),
        }
    }
}

/// Risk rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl FromStr for RiskRating {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(format!("Unknown risk rating: {}", s)),
        }
    }
}

/// Credit hold type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::OverLimit => write!(f, "over_limit"),
            Self::PastDue => write!(f, "past_due"),
            Self::Manual => write!(f, "manual"),
            Self::NewCustomer => write!(f, "new_customer"),
            Self::HighRisk => write!(f, "high_risk"),
        }
    }
}

impl FromStr for CreditHoldType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "over_limit" | "overlimit" => Ok(Self::OverLimit),
            "past_due" | "pastdue" => Ok(Self::PastDue),
            "manual" => Ok(Self::Manual),
            "new_customer" | "newcustomer" => Ok(Self::NewCustomer),
            "high_risk" | "highrisk" => Ok(Self::HighRisk),
            _ => Err(format!("Unknown credit hold type: {}", s)),
        }
    }
}

/// Credit hold status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditHoldStatus {
    #[default]
    Active,
    Released,
    Expired,
}

impl std::fmt::Display for CreditHoldStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Released => write!(f, "released"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

impl FromStr for CreditHoldStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "released" => Ok(Self::Released),
            "expired" => Ok(Self::Expired),
            _ => Err(format!("Unknown credit hold status: {}", s)),
        }
    }
}

/// Credit application status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::Pending => write!(f, "pending"),
            Self::UnderReview => write!(f, "under_review"),
            Self::Approved => write!(f, "approved"),
            Self::Denied => write!(f, "denied"),
            Self::MoreInfoNeeded => write!(f, "more_info_needed"),
            Self::Withdrawn => write!(f, "withdrawn"),
        }
    }
}

impl FromStr for CreditApplicationStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "under_review" | "underreview" => Ok(Self::UnderReview),
            "approved" => Ok(Self::Approved),
            "denied" | "rejected" => Ok(Self::Denied),
            "more_info_needed" | "moreinfoneeded" | "info_needed" => Ok(Self::MoreInfoNeeded),
            "withdrawn" => Ok(Self::Withdrawn),
            _ => Err(format!("Unknown credit application status: {}", s)),
        }
    }
}

/// Credit transaction type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
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
            Self::Charge => write!(f, "charge"),
            Self::Payment => write!(f, "payment"),
            Self::CreditMemo => write!(f, "credit_memo"),
            Self::Adjustment => write!(f, "adjustment"),
            Self::WriteOff => write!(f, "write_off"),
            Self::LimitChange => write!(f, "limit_change"),
        }
    }
}

impl FromStr for CreditTransactionType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "charge" => Ok(Self::Charge),
            "payment" => Ok(Self::Payment),
            "credit_memo" | "creditmemo" => Ok(Self::CreditMemo),
            "adjustment" => Ok(Self::Adjustment),
            "write_off" | "writeoff" => Ok(Self::WriteOff),
            "limit_change" | "limitchange" => Ok(Self::LimitChange),
            _ => Err(format!("Unknown credit transaction type: {}", s)),
        }
    }
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a credit account.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateCreditAccount {
    pub customer_id: CustomerId,
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
    pub customer_id: CustomerId,
    pub order_id: Option<OrderId>,
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
    pub customer_id: CustomerId,
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
    pub customer_id: CustomerId,
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
    pub customer_id: Option<CustomerId>,
    pub status: Option<CreditAccountStatus>,
    pub risk_rating: Option<RiskRating>,
    pub over_limit: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing credit holds.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditHoldFilter {
    pub customer_id: Option<CustomerId>,
    pub order_id: Option<OrderId>,
    pub hold_type: Option<CreditHoldType>,
    pub status: Option<CreditHoldStatus>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing credit applications.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditApplicationFilter {
    pub customer_id: Option<CustomerId>,
    pub status: Option<CreditApplicationStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing credit transactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreditTransactionFilter {
    pub customer_id: Option<CustomerId>,
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
    pub customer_id: CustomerId,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credit_account_status_from_str() {
        assert_eq!(CreditAccountStatus::from_str("active").unwrap(), CreditAccountStatus::Active);
        assert_eq!(CreditAccountStatus::from_str("OnHold").unwrap(), CreditAccountStatus::OnHold);
        assert!(CreditAccountStatus::from_str("nope").is_err());
    }

    #[test]
    fn test_risk_rating_from_str() {
        assert_eq!(RiskRating::from_str("low").unwrap(), RiskRating::Low);
        assert_eq!(RiskRating::from_str("CRITICAL").unwrap(), RiskRating::Critical);
        assert!(RiskRating::from_str("nope").is_err());
    }

    #[test]
    fn test_credit_hold_type_from_str() {
        assert_eq!(CreditHoldType::from_str("overlimit").unwrap(), CreditHoldType::OverLimit);
        assert_eq!(CreditHoldType::from_str("past_due").unwrap(), CreditHoldType::PastDue);
        assert!(CreditHoldType::from_str("nope").is_err());
    }

    #[test]
    fn test_credit_hold_status_from_str() {
        assert_eq!(CreditHoldStatus::from_str("released").unwrap(), CreditHoldStatus::Released);
        assert!(CreditHoldStatus::from_str("nope").is_err());
    }

    #[test]
    fn test_credit_application_status_from_str() {
        assert_eq!(
            CreditApplicationStatus::from_str("under_review").unwrap(),
            CreditApplicationStatus::UnderReview
        );
        assert_eq!(
            CreditApplicationStatus::from_str("info_needed").unwrap(),
            CreditApplicationStatus::MoreInfoNeeded
        );
        assert!(CreditApplicationStatus::from_str("nope").is_err());
    }

    #[test]
    fn test_credit_transaction_type_from_str() {
        assert_eq!(
            CreditTransactionType::from_str("creditmemo").unwrap(),
            CreditTransactionType::CreditMemo
        );
        assert_eq!(
            CreditTransactionType::from_str("limit_change").unwrap(),
            CreditTransactionType::LimitChange
        );
        assert!(CreditTransactionType::from_str("nope").is_err());
    }
}
