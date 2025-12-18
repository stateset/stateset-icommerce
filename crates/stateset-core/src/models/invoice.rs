//! Invoice domain models
//!
//! Handles invoice generation, tracking, and payment reconciliation.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Invoice status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for InvoiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Sent => write!(f, "sent"),
            Self::Viewed => write!(f, "viewed"),
            Self::PartiallyPaid => write!(f, "partially_paid"),
            Self::Paid => write!(f, "paid"),
            Self::Overdue => write!(f, "overdue"),
            Self::Voided => write!(f, "voided"),
            Self::WrittenOff => write!(f, "written_off"),
            Self::Disputed => write!(f, "disputed"),
        }
    }
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
            _ => Err(format!("Unknown invoice status: {}", s)),
        }
    }
}

/// Invoice type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
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

impl std::fmt::Display for InvoiceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::CreditMemo => write!(f, "credit_memo"),
            Self::DebitMemo => write!(f, "debit_memo"),
            Self::Proforma => write!(f, "proforma"),
            Self::Recurring => write!(f, "recurring"),
            Self::Final => write!(f, "final"),
        }
    }
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
            _ => Err(format!("Unknown invoice type: {}", s)),
        }
    }
}

/// An invoice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoice {
    /// Unique ID
    pub id: Uuid,
    /// Human-readable invoice number
    pub invoice_number: String,
    /// Customer ID
    pub customer_id: Uuid,
    /// Associated order ID (optional)
    pub order_id: Option<Uuid>,
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
    pub currency: String,

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
    pub fn is_overdue(&self) -> bool {
        if self.status == InvoiceStatus::Paid || self.status == InvoiceStatus::Voided {
            return false;
        }
        Utc::now() > self.due_date
    }

    /// Get days until due (negative if overdue)
    pub fn days_until_due(&self) -> i64 {
        (self.due_date - Utc::now()).num_days()
    }

    /// Calculate the balance due
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
    pub invoice_id: Uuid,
    /// Associated order item ID
    pub order_item_id: Option<Uuid>,
    /// Product ID
    pub product_id: Option<Uuid>,
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
    /// Line total (quantity * unit_price - discount + tax)
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
    pub customer_id: Uuid,
    /// Order ID (optional)
    pub order_id: Option<Uuid>,
    /// Invoice type
    pub invoice_type: Option<InvoiceType>,
    /// Invoice date (defaults to now)
    pub invoice_date: Option<DateTime<Utc>>,
    /// Due date (defaults to invoice date + payment terms)
    pub due_date: Option<DateTime<Utc>>,
    /// Days until due (used if due_date not provided)
    pub days_until_due: Option<i32>,
    /// Payment terms description
    pub payment_terms: Option<String>,
    /// Currency (defaults to USD)
    pub currency: Option<String>,

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
    pub order_item_id: Option<Uuid>,
    /// Product ID
    pub product_id: Option<Uuid>,
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
    pub customer_id: Option<Uuid>,
    /// Filter by order ID
    pub order_id: Option<Uuid>,
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
pub fn generate_invoice_number() -> String {
    let now = chrono::Utc::now();
    format!("INV-{}", now.format("%Y%m%d%H%M%S%3f"))
}
