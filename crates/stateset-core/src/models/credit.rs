//! Credit Management domain models
//!
//! Models for customer credit limits, credit holds, and credit applications.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CreditId, CurrencyCode, CustomerId, OrderId};
use strum::{Display, EnumString};
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
    pub currency: CurrencyCode,
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditAccountStatus {
    #[default]
    Active,
    Suspended,
    #[strum(serialize = "on_hold", serialize = "onhold")]
    OnHold,
    Closed,
    #[strum(serialize = "pending_review", serialize = "pendingreview")]
    PendingReview,
}

/// Risk rating.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RiskRating {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

/// Credit hold type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditHoldType {
    #[default]
    #[strum(serialize = "over_limit", serialize = "overlimit")]
    OverLimit,
    #[strum(serialize = "past_due", serialize = "pastdue")]
    PastDue,
    Manual,
    #[strum(serialize = "new_customer", serialize = "newcustomer")]
    NewCustomer,
    #[strum(serialize = "high_risk", serialize = "highrisk")]
    HighRisk,
}

/// Credit hold status.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditHoldStatus {
    #[default]
    Active,
    Released,
    Expired,
}

/// Credit application status.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditApplicationStatus {
    #[default]
    Pending,
    #[strum(serialize = "under_review", serialize = "underreview")]
    UnderReview,
    Approved,
    #[strum(serialize = "denied", serialize = "rejected")]
    Denied,
    #[strum(
        serialize = "more_info_needed",
        serialize = "moreinfoneeded",
        serialize = "info_needed"
    )]
    MoreInfoNeeded,
    Withdrawn,
}

/// Credit transaction type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CreditTransactionType {
    #[default]
    Charge,
    Payment,
    #[strum(serialize = "credit_memo", serialize = "creditmemo")]
    CreditMemo,
    Adjustment,
    #[strum(serialize = "write_off", serialize = "writeoff")]
    WriteOff,
    #[strum(serialize = "limit_change", serialize = "limitchange")]
    LimitChange,
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a credit account.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateCreditAccount {
    pub customer_id: CustomerId,
    pub credit_limit: Decimal,
    pub currency: Option<CurrencyCode>,
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
#[must_use] 
pub fn generate_credit_application_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("CAPP-{timestamp}-{random}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
