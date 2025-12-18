//! Purchase Order domain models
//!
//! Handles supplier ordering for inventory replenishment.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Purchase order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PurchaseOrderStatus {
    /// Draft - not yet submitted
    #[default]
    Draft,
    /// Pending approval
    PendingApproval,
    /// Approved, ready to send
    Approved,
    /// Sent to supplier
    Sent,
    /// Acknowledged by supplier
    Acknowledged,
    /// Partially received
    PartiallyReceived,
    /// Fully received
    Received,
    /// Completed/closed
    Completed,
    /// Cancelled
    Cancelled,
    /// On hold
    OnHold,
}

impl std::fmt::Display for PurchaseOrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::Approved => write!(f, "approved"),
            Self::Sent => write!(f, "sent"),
            Self::Acknowledged => write!(f, "acknowledged"),
            Self::PartiallyReceived => write!(f, "partially_received"),
            Self::Received => write!(f, "received"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::OnHold => write!(f, "on_hold"),
        }
    }
}

impl std::str::FromStr for PurchaseOrderStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "pending_approval" => Ok(Self::PendingApproval),
            "approved" => Ok(Self::Approved),
            "sent" => Ok(Self::Sent),
            "acknowledged" => Ok(Self::Acknowledged),
            "partially_received" => Ok(Self::PartiallyReceived),
            "received" => Ok(Self::Received),
            "completed" => Ok(Self::Completed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            "on_hold" => Ok(Self::OnHold),
            _ => Err(format!("Unknown purchase order status: {}", s)),
        }
    }
}

/// Payment terms for purchase orders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaymentTerms {
    /// Payment due on receipt
    #[default]
    DueOnReceipt,
    /// Net 15 days
    Net15,
    /// Net 30 days
    Net30,
    /// Net 45 days
    Net45,
    /// Net 60 days
    Net60,
    /// Net 90 days
    Net90,
    /// 2% discount if paid in 10 days, net 30
    TwoTenNet30,
    /// Prepaid
    Prepaid,
    /// Cash on delivery
    CashOnDelivery,
    /// Letter of credit
    LetterOfCredit,
}

impl std::fmt::Display for PaymentTerms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DueOnReceipt => write!(f, "due_on_receipt"),
            Self::Net15 => write!(f, "net_15"),
            Self::Net30 => write!(f, "net_30"),
            Self::Net45 => write!(f, "net_45"),
            Self::Net60 => write!(f, "net_60"),
            Self::Net90 => write!(f, "net_90"),
            Self::TwoTenNet30 => write!(f, "2_10_net_30"),
            Self::Prepaid => write!(f, "prepaid"),
            Self::CashOnDelivery => write!(f, "cash_on_delivery"),
            Self::LetterOfCredit => write!(f, "letter_of_credit"),
        }
    }
}

impl std::str::FromStr for PaymentTerms {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "due_on_receipt" => Ok(Self::DueOnReceipt),
            "net_15" | "net15" => Ok(Self::Net15),
            "net_30" | "net30" => Ok(Self::Net30),
            "net_45" | "net45" => Ok(Self::Net45),
            "net_60" | "net60" => Ok(Self::Net60),
            "net_90" | "net90" => Ok(Self::Net90),
            "2_10_net_30" | "2/10_net_30" => Ok(Self::TwoTenNet30),
            "prepaid" => Ok(Self::Prepaid),
            "cash_on_delivery" | "cod" => Ok(Self::CashOnDelivery),
            "letter_of_credit" | "lc" => Ok(Self::LetterOfCredit),
            _ => Err(format!("Unknown payment terms: {}", s)),
        }
    }
}

/// A supplier/vendor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Supplier {
    /// Unique supplier ID
    pub id: Uuid,
    /// Supplier code/number
    pub supplier_code: String,
    /// Company name
    pub name: String,
    /// Contact person name
    pub contact_name: Option<String>,
    /// Contact email
    pub email: Option<String>,
    /// Contact phone
    pub phone: Option<String>,
    /// Website
    pub website: Option<String>,
    /// Address
    pub address: Option<String>,
    /// City
    pub city: Option<String>,
    /// State/Province
    pub state: Option<String>,
    /// Postal code
    pub postal_code: Option<String>,
    /// Country
    pub country: Option<String>,
    /// Tax ID / VAT number
    pub tax_id: Option<String>,
    /// Default payment terms
    pub payment_terms: PaymentTerms,
    /// Default currency
    pub currency: String,
    /// Lead time in days
    pub lead_time_days: Option<i32>,
    /// Minimum order amount
    pub minimum_order: Option<Decimal>,
    /// Whether supplier is active
    pub is_active: bool,
    /// Notes
    pub notes: Option<String>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a supplier
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreateSupplier {
    /// Company name
    pub name: String,
    /// Supplier code (auto-generated if not provided)
    pub supplier_code: Option<String>,
    /// Contact person
    pub contact_name: Option<String>,
    /// Email
    pub email: Option<String>,
    /// Phone
    pub phone: Option<String>,
    /// Website
    pub website: Option<String>,
    /// Address
    pub address: Option<String>,
    /// City
    pub city: Option<String>,
    /// State
    pub state: Option<String>,
    /// Postal code
    pub postal_code: Option<String>,
    /// Country
    pub country: Option<String>,
    /// Tax ID
    pub tax_id: Option<String>,
    /// Payment terms
    pub payment_terms: Option<PaymentTerms>,
    /// Currency (defaults to USD)
    pub currency: Option<String>,
    /// Lead time in days
    pub lead_time_days: Option<i32>,
    /// Minimum order amount
    pub minimum_order: Option<Decimal>,
    /// Notes
    pub notes: Option<String>,
}

/// Input for updating a supplier
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateSupplier {
    /// Update name
    pub name: Option<String>,
    /// Update contact name
    pub contact_name: Option<String>,
    /// Update email
    pub email: Option<String>,
    /// Update phone
    pub phone: Option<String>,
    /// Update website
    pub website: Option<String>,
    /// Update address
    pub address: Option<String>,
    /// Update city
    pub city: Option<String>,
    /// Update state
    pub state: Option<String>,
    /// Update postal code
    pub postal_code: Option<String>,
    /// Update country
    pub country: Option<String>,
    /// Update tax ID
    pub tax_id: Option<String>,
    /// Update payment terms
    pub payment_terms: Option<PaymentTerms>,
    /// Update currency
    pub currency: Option<String>,
    /// Update lead time
    pub lead_time_days: Option<i32>,
    /// Update minimum order
    pub minimum_order: Option<Decimal>,
    /// Update active status
    pub is_active: Option<bool>,
    /// Update notes
    pub notes: Option<String>,
}

/// Filter for listing suppliers
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SupplierFilter {
    /// Search by name
    pub name: Option<String>,
    /// Filter by country
    pub country: Option<String>,
    /// Filter by active only
    pub active_only: Option<bool>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// A purchase order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrder {
    /// Unique ID
    pub id: Uuid,
    /// Human-readable PO number
    pub po_number: String,
    /// Supplier ID
    pub supplier_id: Uuid,
    /// PO status
    pub status: PurchaseOrderStatus,
    /// Order date
    pub order_date: DateTime<Utc>,
    /// Expected delivery date
    pub expected_date: Option<DateTime<Utc>>,
    /// Actual delivery date
    pub delivered_date: Option<DateTime<Utc>>,
    /// Ship to address
    pub ship_to_address: Option<String>,
    /// Ship to city
    pub ship_to_city: Option<String>,
    /// Ship to state
    pub ship_to_state: Option<String>,
    /// Ship to postal code
    pub ship_to_postal_code: Option<String>,
    /// Ship to country
    pub ship_to_country: Option<String>,
    /// Payment terms
    pub payment_terms: PaymentTerms,
    /// Currency
    pub currency: String,
    /// Subtotal (sum of line items)
    pub subtotal: Decimal,
    /// Tax amount
    pub tax_amount: Decimal,
    /// Shipping cost
    pub shipping_cost: Decimal,
    /// Discount amount
    pub discount_amount: Decimal,
    /// Total amount
    pub total: Decimal,
    /// Amount paid
    pub amount_paid: Decimal,
    /// Supplier reference number
    pub supplier_reference: Option<String>,
    /// Internal notes
    pub notes: Option<String>,
    /// Supplier notes (visible to supplier)
    pub supplier_notes: Option<String>,
    /// Who approved the PO
    pub approved_by: Option<String>,
    /// When approved
    pub approved_at: Option<DateTime<Utc>>,
    /// Line items
    pub items: Vec<PurchaseOrderItem>,
    /// When sent to supplier
    pub sent_at: Option<DateTime<Utc>>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

/// A line item on a purchase order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseOrderItem {
    /// Unique ID
    pub id: Uuid,
    /// Parent PO ID
    pub purchase_order_id: Uuid,
    /// Product ID (if linked)
    pub product_id: Option<Uuid>,
    /// SKU
    pub sku: String,
    /// Item name/description
    pub name: String,
    /// Supplier's part number
    pub supplier_sku: Option<String>,
    /// Quantity ordered
    pub quantity_ordered: Decimal,
    /// Quantity received
    pub quantity_received: Decimal,
    /// Unit of measure
    pub unit_of_measure: Option<String>,
    /// Unit cost
    pub unit_cost: Decimal,
    /// Line total
    pub line_total: Decimal,
    /// Tax amount for this line
    pub tax_amount: Decimal,
    /// Discount amount for this line
    pub discount_amount: Decimal,
    /// Expected date for this item
    pub expected_date: Option<DateTime<Utc>>,
    /// Notes for this line
    pub notes: Option<String>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a purchase order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePurchaseOrder {
    /// Supplier ID
    pub supplier_id: Uuid,
    /// Order date (defaults to now)
    pub order_date: Option<DateTime<Utc>>,
    /// Expected delivery date
    pub expected_date: Option<DateTime<Utc>>,
    /// Ship to address
    pub ship_to_address: Option<String>,
    /// Ship to city
    pub ship_to_city: Option<String>,
    /// Ship to state
    pub ship_to_state: Option<String>,
    /// Ship to postal code
    pub ship_to_postal_code: Option<String>,
    /// Ship to country
    pub ship_to_country: Option<String>,
    /// Payment terms (defaults to supplier's terms)
    pub payment_terms: Option<PaymentTerms>,
    /// Currency (defaults to supplier's currency)
    pub currency: Option<String>,
    /// Tax amount
    pub tax_amount: Option<Decimal>,
    /// Shipping cost
    pub shipping_cost: Option<Decimal>,
    /// Discount amount
    pub discount_amount: Option<Decimal>,
    /// Notes
    pub notes: Option<String>,
    /// Supplier notes
    pub supplier_notes: Option<String>,
    /// Line items
    pub items: Vec<CreatePurchaseOrderItem>,
}

/// Input for creating a PO line item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatePurchaseOrderItem {
    /// Product ID
    pub product_id: Option<Uuid>,
    /// SKU
    pub sku: String,
    /// Item name
    pub name: String,
    /// Supplier's part number
    pub supplier_sku: Option<String>,
    /// Quantity to order
    pub quantity: Decimal,
    /// Unit of measure
    pub unit_of_measure: Option<String>,
    /// Unit cost
    pub unit_cost: Decimal,
    /// Tax amount
    pub tax_amount: Option<Decimal>,
    /// Discount amount
    pub discount_amount: Option<Decimal>,
    /// Expected date
    pub expected_date: Option<DateTime<Utc>>,
    /// Notes
    pub notes: Option<String>,
}

/// Input for updating a purchase order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdatePurchaseOrder {
    /// Update expected date
    pub expected_date: Option<DateTime<Utc>>,
    /// Update ship to address
    pub ship_to_address: Option<String>,
    /// Update ship to city
    pub ship_to_city: Option<String>,
    /// Update ship to state
    pub ship_to_state: Option<String>,
    /// Update ship to postal code
    pub ship_to_postal_code: Option<String>,
    /// Update ship to country
    pub ship_to_country: Option<String>,
    /// Update payment terms
    pub payment_terms: Option<PaymentTerms>,
    /// Update tax amount
    pub tax_amount: Option<Decimal>,
    /// Update shipping cost
    pub shipping_cost: Option<Decimal>,
    /// Update discount amount
    pub discount_amount: Option<Decimal>,
    /// Update notes
    pub notes: Option<String>,
    /// Update supplier notes
    pub supplier_notes: Option<String>,
    /// Update supplier reference
    pub supplier_reference: Option<String>,
}

/// Input for receiving items on a purchase order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceivePurchaseOrderItems {
    /// Items being received
    pub items: Vec<ReceivePurchaseOrderItem>,
    /// Notes about the receipt
    pub notes: Option<String>,
}

/// Input for receiving a single PO line item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceivePurchaseOrderItem {
    /// PO item ID
    pub item_id: Uuid,
    /// Quantity being received
    pub quantity_received: Decimal,
    /// Notes
    pub notes: Option<String>,
}

/// Filter for listing purchase orders
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PurchaseOrderFilter {
    /// Filter by supplier ID
    pub supplier_id: Option<Uuid>,
    /// Filter by status
    pub status: Option<PurchaseOrderStatus>,
    /// Filter by date range start
    pub from_date: Option<DateTime<Utc>>,
    /// Filter by date range end
    pub to_date: Option<DateTime<Utc>>,
    /// Filter by minimum total
    pub min_total: Option<Decimal>,
    /// Filter by maximum total
    pub max_total: Option<Decimal>,
    /// Limit results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Generate a unique supplier code
pub fn generate_supplier_code() -> String {
    let now = chrono::Utc::now();
    format!("SUP-{}", now.format("%Y%m%d%H%M%S"))
}

/// Generate a unique purchase order number
pub fn generate_po_number() -> String {
    let now = chrono::Utc::now();
    format!("PO-{}", now.format("%Y%m%d%H%M%S%3f"))
}
