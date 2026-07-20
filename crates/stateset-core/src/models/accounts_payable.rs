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
    /// Unique identifier for this bill.
    pub id: Uuid,
    /// System-generated or supplier-provided bill reference (e.g. `"BILL-20240101-ABC123"`).
    pub bill_number: String,
    /// Supplier who issued this bill.
    pub supplier_id: Uuid,
    /// Denormalized supplier name for display.
    pub supplier_name: Option<String>,
    /// Purchase order this bill fulfils, if any.
    pub purchase_order_id: Option<Uuid>,
    /// Current payment lifecycle status.
    pub status: BillStatus,
    /// Date the supplier issued the bill.
    pub bill_date: DateTime<Utc>,
    /// Date by which payment must be made.
    pub due_date: DateTime<Utc>,
    /// Payment terms string (e.g. `"Net 30"`).
    pub payment_terms: Option<String>,
    /// Sum of line-item amounts before tax and adjustments.
    pub subtotal: Decimal,
    /// Total tax charged on the bill.
    pub tax_amount: Decimal,
    /// Freight or shipping charges billed separately.
    pub shipping_amount: Decimal,
    /// Any negotiated discount applied to the bill.
    pub discount_amount: Decimal,
    /// Final amount owed: `subtotal + tax + shipping - discount`.
    pub total_amount: Decimal,
    /// Amount already paid against this bill.
    pub amount_paid: Decimal,
    /// Remaining balance: `total_amount - amount_paid`.
    pub amount_due: Decimal,
    /// Currency of all monetary amounts.
    pub currency: CurrencyCode,
    /// Supplier's own invoice or reference number.
    pub reference_number: Option<String>,
    /// Free-text notes for internal use.
    pub memo: Option<String>,
    /// Timestamp of record creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
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
    /// Unique identifier for this payment.
    pub id: Uuid,
    /// System-generated payment reference (e.g. `"APMT-20240101-ABC123"`).
    pub payment_number: String,
    /// Supplier receiving the payment.
    pub supplier_id: Uuid,
    /// Date the payment was or will be made.
    pub payment_date: DateTime<Utc>,
    /// Method used to disburse the funds.
    pub payment_method: PaymentMethodAP,
    /// Total amount disbursed.
    pub amount: Decimal,
    /// Currency of the payment.
    pub currency: CurrencyCode,
    /// External transaction or confirmation reference.
    pub reference_number: Option<String>,
    /// Bank account from which the payment was made.
    pub bank_account: Option<String>,
    /// Check number, if payment method is `Check`.
    pub check_number: Option<String>,
    /// Free-text notes for internal use.
    pub memo: Option<String>,
    /// Current processing status.
    pub status: PaymentStatusAP,
    /// Timestamp of record creation.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last update.
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
    /// Bill is being entered; not yet submitted for approval.
    #[default]
    Draft,
    /// Bill has been submitted and is awaiting approval.
    Pending,
    /// Bill is approved and scheduled for payment.
    Approved,
    /// Bill has been partially paid; a balance remains.
    PartiallyPaid,
    /// Bill has been paid in full.
    Paid,
    /// Payment is past the due date.
    Overdue,
    /// Bill has been cancelled and will not be paid.
    Cancelled,
    /// Bill is under dispute with the supplier.
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
            _ => Err(format!("Unknown bill status: {s}")),
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
    /// Paper check mailed or hand-delivered to the supplier.
    #[default]
    Check,
    /// Automated Clearing House electronic transfer.
    Ach,
    /// Domestic or international wire transfer.
    Wire,
    /// Corporate credit or charge card payment.
    #[strum(serialize = "credit_card", serialize = "creditcard")]
    CreditCard,
    /// Physical currency payment.
    Cash,
    /// Any payment method not covered by the other variants.
    Other,
}

/// AP payment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentStatusAP {
    /// Payment has been created but not yet submitted to the bank.
    #[default]
    Pending,
    /// Payment has been submitted to the bank for processing.
    Processed,
    /// Bank has confirmed the funds have been debited.
    Cleared,
    /// Payment was cancelled before or after submission.
    Voided,
    /// Payment was rejected or returned by the bank.
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
            _ => Err(format!("Unknown payment status: {s}")),
        }
    }
}

/// Payment run status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PaymentRunStatus {
    /// Run is being assembled; bills can still be added or removed.
    #[default]
    Draft,
    /// Run has been submitted and is awaiting approval.
    Pending,
    /// Run is approved and queued for disbursement.
    Approved,
    /// Payments are actively being transmitted to the bank.
    Processing,
    /// All payments in the run have been submitted successfully.
    Completed,
    /// Run has been cancelled; no payments were made.
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
            _ => Err(format!("Unknown payment run status: {s}")),
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

    mod three_way_match {
        use super::super::*;
        use crate::{PurchaseOrderItem, ReceiptItem, ReceiptItemStatus};
        use rust_decimal_macros::dec;

        fn po_item(id: Uuid, qty: Decimal, unit_cost: Decimal) -> PurchaseOrderItem {
            let now = Utc::now();
            PurchaseOrderItem {
                id,
                purchase_order_id: stateset_primitives::PurchaseOrderId::new(),
                product_id: None,
                sku: "SKU-1".into(),
                name: "Widget".into(),
                supplier_sku: None,
                quantity_ordered: qty,
                quantity_received: Decimal::ZERO,
                unit_of_measure: None,
                unit_cost,
                line_total: qty * unit_cost,
                tax_amount: Decimal::ZERO,
                discount_amount: Decimal::ZERO,
                expected_date: None,
                notes: None,
                created_at: now,
                updated_at: now,
            }
        }

        fn receipt_item(po_line_id: Uuid, received: Decimal) -> ReceiptItem {
            let now = Utc::now();
            ReceiptItem {
                id: Uuid::new_v4(),
                receipt_id: Uuid::new_v4(),
                line_number: 1,
                sku: "SKU-1".into(),
                description: None,
                po_line_id: Some(po_line_id),
                expected_quantity: received,
                received_quantity: received,
                rejected_quantity: Decimal::ZERO,
                unit_cost: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                status: ReceiptItemStatus::Received,
                notes: None,
                created_at: now,
                updated_at: now,
            }
        }

        fn bill_item(po_line_id: Option<Uuid>, qty: Decimal, unit_price: Decimal) -> BillItem {
            BillItem {
                id: Uuid::new_v4(),
                bill_id: Uuid::new_v4(),
                line_number: 1,
                description: "Widget".into(),
                account_code: None,
                quantity: qty,
                unit_price,
                amount: qty * unit_price,
                tax_rate: None,
                tax_amount: Decimal::ZERO,
                po_line_id,
                created_at: Utc::now(),
            }
        }

        #[test]
        fn exact_match_is_matched() {
            let line = Uuid::new_v4();
            let result = perform_three_way_match(
                &[po_item(line, dec!(10), dec!(5))],
                &[receipt_item(line, dec!(10))],
                &[bill_item(Some(line), dec!(10), dec!(5))],
                Decimal::ZERO,
            );
            assert_eq!(result.match_status, MatchStatus::Matched);
            assert_eq!(result.lines.len(), 1);
            assert!(result.lines[0].matched);
            assert_eq!(result.lines[0].quantity_variance, Decimal::ZERO);
        }

        #[test]
        fn quantity_variance_within_tolerance_matches() {
            let line = Uuid::new_v4();
            // Billed 102 vs ordered/received 100 => 2% variance, 5% tolerance.
            let result = perform_three_way_match(
                &[po_item(line, dec!(100), dec!(5))],
                &[receipt_item(line, dec!(100))],
                &[bill_item(Some(line), dec!(102), dec!(5))],
                dec!(5),
            );
            assert_eq!(result.match_status, MatchStatus::Matched);
        }

        #[test]
        fn quantity_variance_over_tolerance_is_variance() {
            let line = Uuid::new_v4();
            // Billed 110 vs 100 => 10% variance, 5% tolerance.
            let result = perform_three_way_match(
                &[po_item(line, dec!(100), dec!(5))],
                &[receipt_item(line, dec!(100))],
                &[bill_item(Some(line), dec!(110), dec!(5))],
                dec!(5),
            );
            assert_eq!(result.match_status, MatchStatus::Variance { variance_line_count: 1 });
            assert!(!result.lines[0].matched);
            assert_eq!(result.lines[0].issues.len(), 2); // vs ordered and vs received
            assert_eq!(result.lines[0].quantity_variance, dec!(10));
        }

        #[test]
        fn price_variance_over_tolerance_is_variance() {
            let line = Uuid::new_v4();
            let result = perform_three_way_match(
                &[po_item(line, dec!(10), dec!(5))],
                &[receipt_item(line, dec!(10))],
                &[bill_item(Some(line), dec!(10), dec!(6))],
                dec!(5),
            );
            assert_eq!(result.match_status, MatchStatus::Variance { variance_line_count: 1 });
            assert_eq!(result.lines[0].price_variance, dec!(1));
            assert!(result.lines[0].issues[0].contains("unit price"));
        }

        #[test]
        fn missing_receipt_is_pending() {
            let line = Uuid::new_v4();
            let result = perform_three_way_match(
                &[po_item(line, dec!(10), dec!(5))],
                &[],
                &[bill_item(Some(line), dec!(10), dec!(5))],
                Decimal::ZERO,
            );
            assert_eq!(result.match_status, MatchStatus::Pending);
            assert!(!result.lines[0].matched);
            assert!(result.lines[0].issues.iter().any(|i| i.contains("no quantity received")));
        }

        #[test]
        fn partial_receipt_is_variance() {
            let line = Uuid::new_v4();
            // Only 4 of 10 received, billed 10.
            let result = perform_three_way_match(
                &[po_item(line, dec!(10), dec!(5))],
                &[receipt_item(line, dec!(4))],
                &[bill_item(Some(line), dec!(10), dec!(5))],
                dec!(5),
            );
            assert_eq!(result.match_status, MatchStatus::Variance { variance_line_count: 1 });
            assert_eq!(result.lines[0].received_quantity, dec!(4));
            assert_eq!(result.lines[0].quantity_variance, dec!(6));
        }

        #[test]
        fn partial_receipt_across_multiple_receipts_sums() {
            let line = Uuid::new_v4();
            let result = perform_three_way_match(
                &[po_item(line, dec!(10), dec!(5))],
                &[receipt_item(line, dec!(4)), receipt_item(line, dec!(6))],
                &[bill_item(Some(line), dec!(10), dec!(5))],
                Decimal::ZERO,
            );
            assert_eq!(result.match_status, MatchStatus::Matched);
            assert_eq!(result.lines[0].received_quantity, dec!(10));
        }

        #[test]
        fn unlinked_bill_line_is_variance() {
            let line = Uuid::new_v4();
            let result = perform_three_way_match(
                &[po_item(line, dec!(10), dec!(5))],
                &[receipt_item(line, dec!(10))],
                &[bill_item(None, dec!(10), dec!(5))],
                Decimal::ZERO,
            );
            assert_eq!(result.match_status, MatchStatus::Variance { variance_line_count: 1 });
            assert!(result.lines[0].issues[0].contains("not linked"));
        }
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
// Three-Way Match (PO <-> Receipt <-> Bill)
// ============================================================================

/// Outcome of a three-way match evaluation for a bill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum MatchStatus {
    /// The bill is not linked to a purchase order, so no match is required.
    NotRequired,
    /// No goods have been received yet; matching cannot complete.
    Pending,
    /// Every bill line matched its PO line and receipts within tolerance.
    Matched,
    /// One or more lines fell outside tolerance.
    Variance {
        /// Number of bill lines with at least one variance issue.
        variance_line_count: usize,
    },
}

/// Per-line comparison of ordered vs received vs billed quantities/costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreeWayMatchLine {
    /// PO line the bill line references (if any).
    pub po_line_id: Option<Uuid>,
    /// Bill line item ID.
    pub bill_item_id: Uuid,
    /// Bill line description.
    pub description: String,
    /// Quantity ordered on the PO line (if linked).
    pub ordered_quantity: Option<Decimal>,
    /// Unit cost on the PO line (if linked).
    pub ordered_unit_cost: Option<Decimal>,
    /// Total quantity received against the PO line (across all receipts).
    pub received_quantity: Decimal,
    /// Quantity billed on this bill line.
    pub billed_quantity: Decimal,
    /// Unit price billed on this bill line.
    pub billed_unit_cost: Decimal,
    /// `billed_quantity - received_quantity`.
    pub quantity_variance: Decimal,
    /// `billed_unit_cost - ordered_unit_cost` (zero when no PO line).
    pub price_variance: Decimal,
    /// Whether this line matched within tolerance.
    pub matched: bool,
    /// Human-readable variance issues for this line.
    pub issues: Vec<String>,
}

/// Result of [`perform_three_way_match`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreeWayMatchResult {
    /// Overall match outcome.
    pub match_status: MatchStatus,
    /// Tolerance applied, as a percentage (e.g. `5` = 5%).
    pub tolerance_percent: Decimal,
    /// Per-line comparison details.
    pub lines: Vec<ThreeWayMatchLine>,
}

impl ThreeWayMatchResult {
    /// A result for a bill with no purchase order linkage.
    #[must_use]
    pub const fn not_required() -> Self {
        Self {
            match_status: MatchStatus::NotRequired,
            tolerance_percent: Decimal::ZERO,
            lines: Vec::new(),
        }
    }
}

/// Whether `actual` is within `tolerance_percent` of `expected` (relative).
fn within_tolerance(expected: Decimal, actual: Decimal, tolerance_percent: Decimal) -> bool {
    let diff = (actual - expected).abs();
    if expected.is_zero() {
        return diff.is_zero();
    }
    diff * Decimal::ONE_HUNDRED <= expected.abs() * tolerance_percent
}

/// Perform a three-way match between a purchase order's lines, the receipt
/// lines recorded against it, and the lines of a supplier bill.
///
/// Lines are correlated by PO line ID: each bill line's `po_line_id` is matched
/// to a [`crate::PurchaseOrderItem`] and to the sum of received quantities of
/// all [`crate::ReceiptItem`]s referencing that PO line. `tolerance_percent` is
/// a relative tolerance (e.g. `dec!(5)` allows a 5% deviation) applied to both
/// quantity and unit-cost comparisons.
///
/// Returns [`MatchStatus::Pending`] when nothing has been received yet,
/// [`MatchStatus::Matched`] when all lines agree within tolerance, and
/// [`MatchStatus::Variance`] otherwise. Callers should short-circuit to
/// [`MatchStatus::NotRequired`] when the bill has no purchase order.
#[must_use]
pub fn perform_three_way_match(
    po_items: &[crate::PurchaseOrderItem],
    receipt_items: &[crate::ReceiptItem],
    bill_lines: &[BillItem],
    tolerance_percent: Decimal,
) -> ThreeWayMatchResult {
    let tolerance_percent = tolerance_percent.max(Decimal::ZERO);
    let nothing_received =
        receipt_items.iter().fold(Decimal::ZERO, |acc, r| acc + r.received_quantity).is_zero();

    let mut lines = Vec::with_capacity(bill_lines.len());
    for bill_line in bill_lines {
        let po_item = bill_line.po_line_id.and_then(|id| po_items.iter().find(|p| p.id == id));
        let received_quantity = bill_line.po_line_id.map_or(Decimal::ZERO, |id| {
            receipt_items
                .iter()
                .filter(|r| r.po_line_id == Some(id))
                .fold(Decimal::ZERO, |acc, r| acc + r.received_quantity)
        });

        let mut issues = Vec::new();
        match po_item {
            None => issues.push("bill line is not linked to a purchase order line".to_string()),
            Some(po) => {
                if !within_tolerance(po.quantity_ordered, bill_line.quantity, tolerance_percent) {
                    issues.push(format!(
                        "billed quantity {} differs from ordered quantity {} beyond tolerance",
                        bill_line.quantity, po.quantity_ordered
                    ));
                }
                if !within_tolerance(po.unit_cost, bill_line.unit_price, tolerance_percent) {
                    issues.push(format!(
                        "billed unit price {} differs from ordered unit cost {} beyond tolerance",
                        bill_line.unit_price, po.unit_cost
                    ));
                }
                if !within_tolerance(received_quantity, bill_line.quantity, tolerance_percent) {
                    issues.push(if received_quantity.is_zero() {
                        "no quantity received against this purchase order line".to_string()
                    } else {
                        format!(
                            "billed quantity {} differs from received quantity {received_quantity} beyond tolerance",
                            bill_line.quantity
                        )
                    });
                }
            }
        }

        lines.push(ThreeWayMatchLine {
            po_line_id: bill_line.po_line_id,
            bill_item_id: bill_line.id,
            description: bill_line.description.clone(),
            ordered_quantity: po_item.map(|p| p.quantity_ordered),
            ordered_unit_cost: po_item.map(|p| p.unit_cost),
            received_quantity,
            billed_quantity: bill_line.quantity,
            billed_unit_cost: bill_line.unit_price,
            quantity_variance: bill_line.quantity - received_quantity,
            price_variance: po_item.map_or(Decimal::ZERO, |p| bill_line.unit_price - p.unit_cost),
            matched: issues.is_empty(),
            issues,
        });
    }

    let variance_line_count = lines.iter().filter(|l| !l.matched).count();
    let match_status = if nothing_received {
        MatchStatus::Pending
    } else if variance_line_count == 0 {
        MatchStatus::Matched
    } else {
        MatchStatus::Variance { variance_line_count }
    };

    ThreeWayMatchResult { match_status, tolerance_percent, lines }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a bill number.
#[must_use]
pub fn generate_bill_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("BILL-{timestamp}-{random}")
}

/// Generate a payment number.
#[must_use]
pub fn generate_ap_payment_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..6].to_uppercase();
    format!("APMT-{timestamp}-{random}")
}

/// Generate a payment run number.
#[must_use]
pub fn generate_payment_run_number() -> String {
    let timestamp = chrono::Utc::now().format("%Y%m%d%H%M").to_string();
    let random = &uuid::Uuid::new_v4().to_string()[..4].to_uppercase();
    format!("RUN-{timestamp}-{random}")
}
