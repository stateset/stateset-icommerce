//! Accounts Payable domain models
//!
//! Models for managing supplier bills, payment scheduling, and disbursements.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::CurrencyCode;
use std::str::FromStr;
use strum::{Display, EnumString};
use uuid::Uuid;

// ============================================================================
// Core AP Types
// ============================================================================

/// A bill/invoice from a supplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bill {
    pub id: Uuid,
    pub bill_number: String,
    pub supplier_id: Uuid,
    pub supplier_name: Option<String>,
    pub purchase_order_id: Option<Uuid>,
    pub status: BillStatus,
    pub bill_date: DateTime<Utc>,
    pub due_date: DateTime<Utc>,
    pub payment_terms: Option<String>,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub shipping_amount: Decimal,
    pub discount_amount: Decimal,
    pub total_amount: Decimal,
    pub amount_paid: Decimal,
    pub amount_due: Decimal,
    pub currency: CurrencyCode,
    pub reference_number: Option<String>,
    pub memo: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A line item on a bill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillItem {
    pub id: Uuid,
    pub bill_id: Uuid,
    pub line_number: i32,
    pub description: String,
    pub account_code: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub amount: Decimal,
    pub tax_rate: Option<Decimal>,
    pub tax_amount: Decimal,
    pub po_line_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// A payment made to a supplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillPayment {
    pub id: Uuid,
    pub payment_number: String,
    pub supplier_id: Uuid,
    pub payment_date: DateTime<Utc>,
    pub payment_method: PaymentMethodAP,
    pub amount: Decimal,
    pub currency: CurrencyCode,
    pub reference_number: Option<String>,
    pub bank_account: Option<String>,
    pub check_number: Option<String>,
    pub memo: Option<String>,
    pub status: PaymentStatusAP,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Allocation of a payment to specific bills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAllocation {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub bill_id: Uuid,
    pub amount: Decimal,
    pub created_at: DateTime<Utc>,
}

/// A scheduled payment batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRun {
    pub id: Uuid,
    pub run_number: String,
    pub status: PaymentRunStatus,
    pub payment_date: DateTime<Utc>,
    pub payment_method: PaymentMethodAP,
    pub total_amount: Decimal,
    pub payment_count: i32,
    pub notes: Option<String>,
    pub created_by: Option<String>,
    pub approved_by: Option<String>,
    pub approved_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Enums
// ============================================================================

/// Status of a bill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BillStatus {
    #[default]
    Draft,
    Pending,
    Approved,
    PartiallyPaid,
    Paid,
    Overdue,
    Cancelled,
    Disputed,
}

impl FromStr for BillStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "partially_paid" | "partiallypaid" => Ok(Self::PartiallyPaid),
            "paid" => Ok(Self::Paid),
            "overdue" => Ok(Self::Overdue),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            "disputed" => Ok(Self::Disputed),
            _ => Err(format!("Unknown bill status: {}", s)),
        }
    }
}

/// AP payment method.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Display, EnumString, Serialize, Deserialize, Default,
)]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentMethodAP {
    #[default]
    Check,
    Ach,
    Wire,
    #[strum(serialize = "credit_card", serialize = "creditcard")]
    CreditCard,
    Cash,
    Other,
}

/// AP payment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentStatusAP {
    #[default]
    Pending,
    Processed,
    Cleared,
    Voided,
    Failed,
}

impl FromStr for PaymentStatusAP {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "processed" => Ok(Self::Processed),
            "cleared" => Ok(Self::Cleared),
            "voided" => Ok(Self::Voided),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("Unknown payment status: {}", s)),
        }
    }
}

/// Payment run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentRunStatus {
    #[default]
    Draft,
    Pending,
    Approved,
    Processing,
    Completed,
    Cancelled,
}

impl FromStr for PaymentRunStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "processing" | "in_progress" | "inprogress" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown payment run status: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn bill_status_from_str() {
        assert_eq!(BillStatus::from_str("partiallypaid").unwrap(), BillStatus::PartiallyPaid);
        assert_eq!(BillStatus::from_str("canceled").unwrap(), BillStatus::Cancelled);
        assert!(BillStatus::from_str("unknown").is_err());
    }

    #[test]
    fn payment_method_from_str() {
        assert_eq!(PaymentMethodAP::from_str("creditcard").unwrap(), PaymentMethodAP::CreditCard);
        assert_eq!(PaymentMethodAP::from_str("other").unwrap(), PaymentMethodAP::Other);
        assert!(PaymentMethodAP::from_str("wire_transfer").is_err());
    }

    #[test]
    fn payment_status_from_str() {
        assert_eq!(PaymentStatusAP::from_str("processed").unwrap(), PaymentStatusAP::Processed);
        assert!(PaymentStatusAP::from_str("unknown").is_err());
    }

    #[test]
    fn payment_run_status_from_str() {
        assert_eq!(
            PaymentRunStatus::from_str("in_progress").unwrap(),
            PaymentRunStatus::Processing
        );
        assert_eq!(PaymentRunStatus::from_str("cancelled").unwrap(), PaymentRunStatus::Cancelled);
        assert!(PaymentRunStatus::from_str("unknown").is_err());
    }
}

// ============================================================================
// Input Types
// ============================================================================

/// Input for creating a bill.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateBill {
    pub bill_number: Option<String>,
    pub supplier_id: Uuid,
    pub purchase_order_id: Option<Uuid>,
    pub bill_date: Option<DateTime<Utc>>,
    pub due_date: DateTime<Utc>,
    pub payment_terms: Option<String>,
    pub currency: Option<CurrencyCode>,
    pub reference_number: Option<String>,
    pub memo: Option<String>,
    pub items: Vec<CreateBillItem>,
}

/// Input for creating a bill item.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateBillItem {
    pub description: String,
    pub account_code: Option<String>,
    pub quantity: Decimal,
    pub unit_price: Decimal,
    pub tax_rate: Option<Decimal>,
    pub po_line_id: Option<Uuid>,
}

/// Input for updating a bill.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateBill {
    pub due_date: Option<DateTime<Utc>>,
    pub payment_terms: Option<String>,
    pub reference_number: Option<String>,
    pub memo: Option<String>,
}

/// Input for creating a payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBillPayment {
    pub supplier_id: Uuid,
    pub payment_date: Option<DateTime<Utc>>,
    pub payment_method: PaymentMethodAP,
    pub amount: Decimal,
    pub currency: Option<CurrencyCode>,
    pub reference_number: Option<String>,
    pub bank_account: Option<String>,
    pub check_number: Option<String>,
    pub memo: Option<String>,
    pub allocations: Vec<PaymentAllocationInput>,
}

/// Payment allocation input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAllocationInput {
    pub bill_id: Uuid,
    pub amount: Decimal,
}

/// Input for creating a payment run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreatePaymentRun {
    pub payment_date: DateTime<Utc>,
    pub payment_method: PaymentMethodAP,
    pub bill_ids: Vec<Uuid>,
    pub notes: Option<String>,
    pub created_by: Option<String>,
}

/// Input for paying a bill directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayBill {
    pub amount: Decimal,
    pub payment_method: PaymentMethodAP,
    pub reference_number: Option<String>,
    pub memo: Option<String>,
    pub payment_date: Option<DateTime<Utc>>,
}

impl Default for PayBill {
    fn default() -> Self {
        Self {
            amount: Decimal::ZERO,
            payment_method: PaymentMethodAP::default(),
            reference_number: None,
            memo: None,
            payment_date: None,
        }
    }
}

// ============================================================================
// Filter Types
// ============================================================================

/// Filter for listing bills.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BillFilter {
    pub supplier_id: Option<Uuid>,
    pub status: Option<BillStatus>,
    pub purchase_order_id: Option<Uuid>,
    pub overdue_only: Option<bool>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub min_amount: Option<Decimal>,
    pub max_amount: Option<Decimal>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for listing payments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BillPaymentFilter {
    pub supplier_id: Option<Uuid>,
    pub status: Option<PaymentStatusAP>,
    pub payment_method: Option<PaymentMethodAP>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Filter for payment runs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PaymentRunFilter {
    pub status: Option<PaymentRunStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

// ============================================================================
// Summary Types
// ============================================================================

/// AP aging summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApAgingSummary {
    pub current: Decimal,
    pub days_1_30: Decimal,
    pub days_31_60: Decimal,
    pub days_61_90: Decimal,
    pub days_over_90: Decimal,
    pub total: Decimal,
}

/// AP summary by supplier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplierApSummary {
    pub supplier_id: Uuid,
    pub supplier_name: Option<String>,
    pub total_outstanding: Decimal,
    pub total_overdue: Decimal,
    pub bill_count: i32,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a bill number.
pub fn generate_bill_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("BILL-{}-{}", timestamp, random)
}

/// Generate a payment number.
pub fn generate_ap_payment_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("APMT-{}-{}", timestamp, random)
}

/// Generate a payment run number.
pub fn generate_payment_run_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..4].to_uppercase();
    format!("RUN-{}-{}", timestamp, random)
}
