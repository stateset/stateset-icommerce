//! Invoice domain models
//!
//! Handles invoice generation, tracking, and payment reconciliation.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, CustomerId, InvoiceId, OrderId, OrderItemId, ProductId};
use uuid::Uuid;

/// Invoice status
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InvoiceStatus {
    /// Draft - not yet sent
    #[default]
    Draft,
    /// Sent to customer
    Sent,
    /// Viewed by customer
    Viewed,
    /// Partially paid
    PartiallyPaid,
    /// Fully paid
    Paid,
    /// Past due
    Overdue,
    /// Voided/cancelled
    Voided,
    /// Written off as uncollectible
    WrittenOff,
    /// In dispute
    Disputed,
}

impl std::str::FromStr for InvoiceStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "sent" => Ok(Self::Sent),
            "viewed" => Ok(Self::Viewed),
            "partially_paid" => Ok(Self::PartiallyPaid),
            "paid" => Ok(Self::Paid),
            "overdue" => Ok(Self::Overdue),
            "voided" => Ok(Self::Voided),
            "written_off" => Ok(Self::WrittenOff),
            "disputed" => Ok(Self::Disputed),
            _ => Err(format!("Unknown invoice status: {s}")),
        }
    }
}

/// Invoice type
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InvoiceType {
    /// Standard invoice
    #[default]
    Standard,
    /// Credit memo/note
    CreditMemo,
    /// Debit memo/note
    DebitMemo,
    /// Proforma invoice
    Proforma,
    /// Recurring invoice
    Recurring,
    /// Final invoice
    Final,
}

impl std::str::FromStr for InvoiceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(Self::Standard),
            "credit_memo" | "credit_note" => Ok(Self::CreditMemo),
            "debit_memo" | "debit_note" => Ok(Self::DebitMemo),
            "proforma" => Ok(Self::Proforma),
            "recurring" => Ok(Self::Recurring),
            "final" => Ok(Self::Final),
            _ => Err(format!("Unknown invoice type: {s}")),
        }
    }
}

/// An invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    /// Unique ID
    pub id: InvoiceId,
    /// Human-readable invoice number
    pub invoice_number: String,
    /// Customer ID
    pub customer_id: CustomerId,
    /// Associated order ID (optional)
    pub order_id: Option<OrderId>,
    /// Invoice status
    pub status: InvoiceStatus,
    /// Invoice type
    pub invoice_type: InvoiceType,
    /// Invoice date
    pub invoice_date: DateTime<Utc>,
    /// Due date
    pub due_date: DateTime<Utc>,
    /// Payment terms description
    pub payment_terms: Option<String>,
    /// Currency code
    pub currency: CurrencyCode,

    // Billing information
    /// Billing name
    pub billing_name: Option<String>,
    /// Billing email
    pub billing_email: Option<String>,
    /// Billing address
    pub billing_address: Option<String>,
    /// Billing city
    pub billing_city: Option<String>,
    /// Billing state
    pub billing_state: Option<String>,
    /// Billing postal code
    pub billing_postal_code: Option<String>,
    /// Billing country
    pub billing_country: Option<String>,

    // Amounts
    /// Subtotal (before tax/discounts)
    pub subtotal: Decimal,
    /// Discount amount
    pub discount_amount: Decimal,
    /// Discount percentage (if applicable)
    pub discount_percent: Option<Decimal>,
    /// Tax amount
    pub tax_amount: Decimal,
    /// Tax rate (percentage)
    pub tax_rate: Option<Decimal>,
    /// Shipping/handling charges
    pub shipping_amount: Decimal,
    /// Total amount due
    pub total: Decimal,
    /// Amount paid
    pub amount_paid: Decimal,
    /// Balance due
    pub balance_due: Decimal,

    /// Purchase order reference
    pub po_number: Option<String>,
    /// Internal notes
    pub notes: Option<String>,
    /// Terms and conditions
    pub terms: Option<String>,
    /// Footer text
    pub footer: Option<String>,

    /// When invoice was sent
    pub sent_at: Option<DateTime<Utc>>,
    /// When invoice was viewed
    pub viewed_at: Option<DateTime<Utc>>,
    /// When invoice was paid in full
    pub paid_at: Option<DateTime<Utc>>,
    /// When invoice was voided
    pub voided_at: Option<DateTime<Utc>>,

    /// Line items
    pub items: Vec<InvoiceItem>,

    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

impl Invoice {
    /// Check if the invoice is overdue
    #[must_use]
    pub fn is_overdue(&self) -> bool {
        if self.status == InvoiceStatus::Paid || self.status == InvoiceStatus::Voided {
            return false;
        }
        Utc::now() > self.due_date
    }

    /// Get days until due (negative if overdue)
    #[must_use]
    pub fn days_until_due(&self) -> i64 {
        (self.due_date - Utc::now()).num_days()
    }

    /// Calculate the balance due
    #[must_use]
    pub fn calculate_balance(&self) -> Decimal {
        self.total - self.amount_paid
    }
}

/// A line item on an invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvoiceItem {
    /// Unique ID
    pub id: Uuid,
    /// Parent invoice ID
    pub invoice_id: InvoiceId,
    /// Associated order item ID
    pub order_item_id: Option<OrderItemId>,
    /// Product ID
    pub product_id: Option<ProductId>,
    /// SKU
    pub sku: Option<String>,
    /// Item description
    pub description: String,
    /// Quantity
    pub quantity: Decimal,
    /// Unit of measure
    pub unit_of_measure: Option<String>,
    /// Unit price
    pub unit_price: Decimal,
    /// Discount amount for this line
    pub discount_amount: Decimal,
    /// Tax amount for this line
    pub tax_amount: Decimal,
    /// Line total (quantity * `unit_price` - discount + tax)
    pub line_total: Decimal,
    /// Sort order
    pub sort_order: i32,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating an invoice
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateInvoice {
    /// Customer ID
    pub customer_id: CustomerId,
    /// Order ID (optional)
    pub order_id: Option<OrderId>,
    /// Invoice type
    pub invoice_type: Option<InvoiceType>,
    /// Invoice date (defaults to now)
    pub invoice_date: Option<DateTime<Utc>>,
    /// Due date (defaults to invoice date + payment terms)
    pub due_date: Option<DateTime<Utc>>,
    /// Days until due (used if `due_date` not provided)
    pub days_until_due: Option<i32>,
    /// Payment terms description
    pub payment_terms: Option<String>,
    /// Currency (defaults to USD)
    pub currency: Option<CurrencyCode>,

    // Billing info
    /// Billing name
    pub billing_name: Option<String>,
    /// Billing email
    pub billing_email: Option<String>,
    /// Billing address
    pub billing_address: Option<String>,
    /// Billing city
    pub billing_city: Option<String>,
    /// Billing state
    pub billing_state: Option<String>,
    /// Billing postal code
    pub billing_postal_code: Option<String>,
    /// Billing country
    pub billing_country: Option<String>,

    /// Discount amount
    pub discount_amount: Option<Decimal>,
    /// Discount percentage
    pub discount_percent: Option<Decimal>,
    /// Tax amount (or calculated from items)
    pub tax_amount: Option<Decimal>,
    /// Tax rate
    pub tax_rate: Option<Decimal>,
    /// Shipping amount
    pub shipping_amount: Option<Decimal>,

    /// PO number reference
    pub po_number: Option<String>,
    /// Notes
    pub notes: Option<String>,
    /// Terms and conditions
    pub terms: Option<String>,
    /// Footer text
    pub footer: Option<String>,

    /// Line items
    pub items: Vec<CreateInvoiceItem>,
}

/// Input for creating an invoice line item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateInvoiceItem {
    /// Order item ID
    pub order_item_id: Option<OrderItemId>,
    /// Product ID
    pub product_id: Option<ProductId>,
    /// SKU
    pub sku: Option<String>,
    /// Description
    pub description: String,
    /// Quantity
    pub quantity: Decimal,
    /// Unit of measure
    pub unit_of_measure: Option<String>,
    /// Unit price
    pub unit_price: Decimal,
    /// Discount amount
    pub discount_amount: Option<Decimal>,
    /// Tax amount
    pub tax_amount: Option<Decimal>,
    /// Sort order
    pub sort_order: Option<i32>,
}

/// Input for updating an invoice
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateInvoice {
    /// Update due date
    pub due_date: Option<DateTime<Utc>>,
    /// Update payment terms
    pub payment_terms: Option<String>,
    /// Update billing name
    pub billing_name: Option<String>,
    /// Update billing email
    pub billing_email: Option<String>,
    /// Update billing address
    pub billing_address: Option<String>,
    /// Update billing city
    pub billing_city: Option<String>,
    /// Update billing state
    pub billing_state: Option<String>,
    /// Update billing postal code
    pub billing_postal_code: Option<String>,
    /// Update billing country
    pub billing_country: Option<String>,
    /// Update discount amount
    pub discount_amount: Option<Decimal>,
    /// Update discount percent
    pub discount_percent: Option<Decimal>,
    /// Update tax amount
    pub tax_amount: Option<Decimal>,
    /// Update tax rate
    pub tax_rate: Option<Decimal>,
    /// Update shipping amount
    pub shipping_amount: Option<Decimal>,
    /// Update PO number
    pub po_number: Option<String>,
    /// Update notes
    pub notes: Option<String>,
    /// Update terms
    pub terms: Option<String>,
    /// Update footer
    pub footer: Option<String>,
}

/// Input for recording a payment on an invoice
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordInvoicePayment {
    /// Amount being paid
    pub amount: Decimal,
    /// Payment ID (if linked to a payment record)
    pub payment_id: Option<Uuid>,
    /// Payment method description
    pub payment_method: Option<String>,
    /// Payment reference/check number
    pub reference: Option<String>,
    /// Notes
    pub notes: Option<String>,
}

/// Filter for listing invoices
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InvoiceFilter {
    /// Filter by customer ID
    pub customer_id: Option<CustomerId>,
    /// Filter by order ID
    pub order_id: Option<OrderId>,
    /// Filter by status
    pub status: Option<InvoiceStatus>,
    /// Filter by invoice type
    pub invoice_type: Option<InvoiceType>,
    /// Filter overdue only
    pub overdue_only: Option<bool>,
    /// Filter by date range start (invoice date)
    pub from_date: Option<DateTime<Utc>>,
    /// Filter by date range end (invoice date)
    pub to_date: Option<DateTime<Utc>>,
    /// Filter by due date range start
    pub due_from: Option<DateTime<Utc>>,
    /// Filter by due date range end
    pub due_to: Option<DateTime<Utc>>,
    /// Filter by minimum total
    pub min_total: Option<Decimal>,
    /// Filter by maximum total
    pub max_total: Option<Decimal>,
    /// Filter by minimum balance due
    pub min_balance: Option<Decimal>,
    /// Search by invoice number
    pub invoice_number: Option<String>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Generate a unique invoice number
#[must_use]
pub fn generate_invoice_number() -> String {
    let now = chrono::Utc::now();
    let short_id = &uuid::Uuid::new_v4().simple().to_string()[..8];
    format!("INV-{}-{short_id}", now.format("%Y%m%d%H%M%S%3f"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rust_decimal_macros::dec;

    // ========================================================================
    // Test Helpers
    // ========================================================================

    fn create_test_invoice(status: InvoiceStatus, due_in_days: i64) -> Invoice {
        let now = Utc::now();
        Invoice {
            id: InvoiceId::new(),
            invoice_number: generate_invoice_number(),
            customer_id: CustomerId::new(),
            order_id: None,
            status,
            invoice_type: InvoiceType::Standard,
            invoice_date: now,
            due_date: now + Duration::days(due_in_days),
            payment_terms: None,
            currency: CurrencyCode::USD,
            billing_name: None,
            billing_email: None,
            billing_address: None,
            billing_city: None,
            billing_state: None,
            billing_postal_code: None,
            billing_country: None,
            subtotal: dec!(100.00),
            discount_amount: Decimal::ZERO,
            discount_percent: None,
            tax_amount: dec!(8.00),
            tax_rate: None,
            shipping_amount: Decimal::ZERO,
            total: dec!(108.00),
            amount_paid: Decimal::ZERO,
            balance_due: dec!(108.00),
            po_number: None,
            notes: None,
            terms: None,
            footer: None,
            sent_at: None,
            viewed_at: None,
            paid_at: None,
            voided_at: None,
            items: vec![],
            created_at: now,
            updated_at: now,
        }
    }

    // ========================================================================
    // Number Generation
    // ========================================================================

    #[test]
    fn generated_invoice_numbers_include_entropy_suffix() {
        let first = generate_invoice_number();
        let second = generate_invoice_number();

        assert!(first.starts_with("INV-"));
        assert!(first.len() > "INV-20260101120000000".len());
        assert_ne!(first, second);
    }

    // ========================================================================
    // Balance Math
    // ========================================================================

    #[test]
    fn calculate_balance_with_no_payments_equals_total() {
        let invoice = create_test_invoice(InvoiceStatus::Sent, 30);
        assert_eq!(invoice.calculate_balance(), dec!(108.00));
    }

    #[test]
    fn calculate_balance_after_partial_payment() {
        let mut invoice = create_test_invoice(InvoiceStatus::PartiallyPaid, 30);
        invoice.amount_paid = dec!(50.00);
        assert_eq!(invoice.calculate_balance(), dec!(58.00));
    }

    #[test]
    fn calculate_balance_when_fully_paid_is_zero() {
        let mut invoice = create_test_invoice(InvoiceStatus::Paid, 30);
        invoice.amount_paid = dec!(108.00);
        assert_eq!(invoice.calculate_balance(), Decimal::ZERO);
    }

    #[test]
    fn calculate_balance_on_overpayment_is_negative() {
        let mut invoice = create_test_invoice(InvoiceStatus::Paid, 30);
        invoice.amount_paid = dec!(120.00);
        assert_eq!(invoice.calculate_balance(), dec!(-12.00));
    }

    // ========================================================================
    // Overdue Logic
    // ========================================================================

    #[test]
    fn unpaid_invoice_past_due_date_is_overdue() {
        let invoice = create_test_invoice(InvoiceStatus::Sent, -1);
        assert!(invoice.is_overdue());
    }

    #[test]
    fn unpaid_invoice_before_due_date_is_not_overdue() {
        let invoice = create_test_invoice(InvoiceStatus::Sent, 30);
        assert!(!invoice.is_overdue());
    }

    #[test]
    fn paid_invoice_is_never_overdue() {
        let invoice = create_test_invoice(InvoiceStatus::Paid, -90);
        assert!(!invoice.is_overdue());
    }

    #[test]
    fn voided_invoice_is_never_overdue() {
        let invoice = create_test_invoice(InvoiceStatus::Voided, -90);
        assert!(!invoice.is_overdue());
    }

    #[test]
    fn partially_paid_invoice_past_due_is_overdue() {
        let mut invoice = create_test_invoice(InvoiceStatus::PartiallyPaid, -5);
        invoice.amount_paid = dec!(50.00);
        assert!(invoice.is_overdue());
    }

    #[test]
    fn days_until_due_is_positive_before_due_date() {
        let invoice = create_test_invoice(InvoiceStatus::Sent, 30);
        let days = invoice.days_until_due();
        assert!((29..=30).contains(&days), "expected ~30 days, got {days}");
    }

    #[test]
    fn days_until_due_is_negative_when_overdue() {
        let invoice = create_test_invoice(InvoiceStatus::Sent, -10);
        let days = invoice.days_until_due();
        assert!(days <= -10, "expected <= -10 days, got {days}");
    }

    // ========================================================================
    // Enum Round-Trips
    // ========================================================================

    #[test]
    fn invoice_status_round_trips_through_strings() {
        for status in [
            InvoiceStatus::Draft,
            InvoiceStatus::Sent,
            InvoiceStatus::Viewed,
            InvoiceStatus::PartiallyPaid,
            InvoiceStatus::Paid,
            InvoiceStatus::Overdue,
            InvoiceStatus::Voided,
            InvoiceStatus::WrittenOff,
            InvoiceStatus::Disputed,
        ] {
            let parsed: InvoiceStatus =
                status.to_string().parse().expect("status should round-trip");
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn invoice_status_parse_is_case_insensitive_and_rejects_unknown() {
        assert_eq!("PAID".parse::<InvoiceStatus>(), Ok(InvoiceStatus::Paid));
        assert_eq!("Partially_Paid".parse::<InvoiceStatus>(), Ok(InvoiceStatus::PartiallyPaid));
        assert!("bogus".parse::<InvoiceStatus>().is_err());
    }

    #[test]
    fn invoice_type_round_trips_through_strings() {
        for invoice_type in [
            InvoiceType::Standard,
            InvoiceType::CreditMemo,
            InvoiceType::DebitMemo,
            InvoiceType::Proforma,
            InvoiceType::Recurring,
            InvoiceType::Final,
        ] {
            let parsed: InvoiceType =
                invoice_type.to_string().parse().expect("type should round-trip");
            assert_eq!(parsed, invoice_type);
        }
    }

    #[test]
    fn invoice_type_accepts_note_aliases() {
        assert_eq!("credit_note".parse::<InvoiceType>(), Ok(InvoiceType::CreditMemo));
        assert_eq!("debit_note".parse::<InvoiceType>(), Ok(InvoiceType::DebitMemo));
        assert!("unknown".parse::<InvoiceType>().is_err());
    }

    #[test]
    fn invoice_status_serde_round_trips_snake_case() {
        let json = serde_json::to_string(&InvoiceStatus::PartiallyPaid).expect("serialize");
        assert_eq!(json, "\"partially_paid\"");
        let back: InvoiceStatus = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, InvoiceStatus::PartiallyPaid);
    }
}
