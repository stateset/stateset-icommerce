//! Accounts Payable domain models
//!
//! Models for managing supplier bills, payment scheduling, and disbursements.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
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
    pub currency: String,
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
    pub currency: String,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for BillStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BillStatus::Draft => write!(f, "draft"),
            BillStatus::Pending => write!(f, "pending"),
            BillStatus::Approved => write!(f, "approved"),
            BillStatus::PartiallyPaid => write!(f, "partially_paid"),
            BillStatus::Paid => write!(f, "paid"),
            BillStatus::Overdue => write!(f, "overdue"),
            BillStatus::Cancelled => write!(f, "cancelled"),
            BillStatus::Disputed => write!(f, "disputed"),
        }
    }
}

impl FromStr for BillStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(BillStatus::Draft),
            "pending" => Ok(BillStatus::Pending),
            "approved" => Ok(BillStatus::Approved),
            "partially_paid" | "partiallypaid" => Ok(BillStatus::PartiallyPaid),
            "paid" => Ok(BillStatus::Paid),
            "overdue" => Ok(BillStatus::Overdue),
            "cancelled" | "canceled" => Ok(BillStatus::Cancelled),
            "disputed" => Ok(BillStatus::Disputed),
            _ => Err(format!("Unknown bill status: {}", s)),
        }
    }
}

/// AP payment method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethodAP {
    #[default]
    Check,
    Ach,
    Wire,
    CreditCard,
    Cash,
    Other,
}

impl std::fmt::Display for PaymentMethodAP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentMethodAP::Check => write!(f, "check"),
            PaymentMethodAP::Ach => write!(f, "ach"),
            PaymentMethodAP::Wire => write!(f, "wire"),
            PaymentMethodAP::CreditCard => write!(f, "credit_card"),
            PaymentMethodAP::Cash => write!(f, "cash"),
            PaymentMethodAP::Other => write!(f, "other"),
        }
    }
}

impl FromStr for PaymentMethodAP {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "check" => Ok(PaymentMethodAP::Check),
            "ach" => Ok(PaymentMethodAP::Ach),
            "wire" => Ok(PaymentMethodAP::Wire),
            "credit_card" | "creditcard" => Ok(PaymentMethodAP::CreditCard),
            "cash" => Ok(PaymentMethodAP::Cash),
            "other" => Ok(PaymentMethodAP::Other),
            _ => Err(format!("Unknown payment method: {}", s)),
        }
    }
}

/// AP payment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatusAP {
    #[default]
    Pending,
    Processed,
    Cleared,
    Voided,
    Failed,
}

impl std::fmt::Display for PaymentStatusAP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentStatusAP::Pending => write!(f, "pending"),
            PaymentStatusAP::Processed => write!(f, "processed"),
            PaymentStatusAP::Cleared => write!(f, "cleared"),
            PaymentStatusAP::Voided => write!(f, "voided"),
            PaymentStatusAP::Failed => write!(f, "failed"),
        }
    }
}

impl FromStr for PaymentStatusAP {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(PaymentStatusAP::Pending),
            "processed" => Ok(PaymentStatusAP::Processed),
            "cleared" => Ok(PaymentStatusAP::Cleared),
            "voided" => Ok(PaymentStatusAP::Voided),
            "failed" => Ok(PaymentStatusAP::Failed),
            _ => Err(format!("Unknown payment status: {}", s)),
        }
    }
}

/// Payment run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaymentRunStatus {
    #[default]
    Draft,
    Pending,
    Approved,
    Processing,
    Completed,
    Cancelled,
}

impl std::fmt::Display for PaymentRunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentRunStatus::Draft => write!(f, "draft"),
            PaymentRunStatus::Pending => write!(f, "pending"),
            PaymentRunStatus::Approved => write!(f, "approved"),
            PaymentRunStatus::Processing => write!(f, "processing"),
            PaymentRunStatus::Completed => write!(f, "completed"),
            PaymentRunStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for PaymentRunStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "draft" => Ok(PaymentRunStatus::Draft),
            "pending" => Ok(PaymentRunStatus::Pending),
            "approved" => Ok(PaymentRunStatus::Approved),
            "processing" | "in_progress" | "inprogress" => Ok(PaymentRunStatus::Processing),
            "completed" => Ok(PaymentRunStatus::Completed),
            "cancelled" | "canceled" => Ok(PaymentRunStatus::Cancelled),
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
        assert_eq!(PaymentRunStatus::from_str("in_progress").unwrap(), PaymentRunStatus::Processing);
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
    pub currency: Option<String>,
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
    pub currency: Option<String>,
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
