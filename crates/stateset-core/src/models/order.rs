//! Order domain models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Order aggregate root
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: Uuid,
    pub order_number: String,
    pub customer_id: Uuid,
    pub status: OrderStatus,
    pub order_date: DateTime<Utc>,
    pub total_amount: Decimal,
    pub currency: String,
    pub payment_status: PaymentStatus,
    pub fulfillment_status: FulfillmentStatus,
    pub payment_method: Option<String>,
    pub shipping_method: Option<String>,
    pub tracking_number: Option<String>,
    pub notes: Option<String>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub items: Vec<OrderItem>,
    /// Version for optimistic locking
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Order line item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderItem {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub discount: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
}

/// Address structure (shipping/billing)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
}

/// Order status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Refunded,
}

impl Default for OrderStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Confirmed => write!(f, "confirmed"),
            Self::Processing => write!(f, "processing"),
            Self::Shipped => write!(f, "shipped"),
            Self::Delivered => write!(f, "delivered"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Refunded => write!(f, "refunded"),
        }
    }
}

/// Payment status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Authorized,
    Paid,
    PartiallyPaid,
    Refunded,
    PartiallyRefunded,
    Failed,
}

impl Default for PaymentStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Authorized => write!(f, "authorized"),
            Self::Paid => write!(f, "paid"),
            Self::PartiallyPaid => write!(f, "partially_paid"),
            Self::Refunded => write!(f, "refunded"),
            Self::PartiallyRefunded => write!(f, "partially_refunded"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Fulfillment status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentStatus {
    Unfulfilled,
    PartiallyFulfilled,
    Fulfilled,
    Shipped,
    Delivered,
}

impl Default for FulfillmentStatus {
    fn default() -> Self {
        Self::Unfulfilled
    }
}

impl std::fmt::Display for FulfillmentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unfulfilled => write!(f, "unfulfilled"),
            Self::PartiallyFulfilled => write!(f, "partially_fulfilled"),
            Self::Fulfilled => write!(f, "fulfilled"),
            Self::Shipped => write!(f, "shipped"),
            Self::Delivered => write!(f, "delivered"),
        }
    }
}

/// Input for creating a new order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrder {
    pub customer_id: Uuid,
    pub items: Vec<CreateOrderItem>,
    pub currency: Option<String>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub notes: Option<String>,
    pub payment_method: Option<String>,
    pub shipping_method: Option<String>,
}

impl Default for CreateOrder {
    fn default() -> Self {
        Self {
            customer_id: Uuid::nil(),
            items: vec![],
            currency: None,
            shipping_address: None,
            billing_address: None,
            notes: None,
            payment_method: None,
            shipping_method: None,
        }
    }
}

/// Input for creating an order item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderItem {
    pub product_id: Uuid,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub discount: Option<Decimal>,
    pub tax_amount: Option<Decimal>,
}

impl Default for CreateOrderItem {
    fn default() -> Self {
        Self {
            product_id: Uuid::nil(),
            variant_id: None,
            sku: String::new(),
            name: String::new(),
            quantity: 0,
            unit_price: Decimal::ZERO,
            discount: None,
            tax_amount: None,
        }
    }
}

/// Input for updating an order
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateOrder {
    pub status: Option<OrderStatus>,
    pub payment_status: Option<PaymentStatus>,
    pub fulfillment_status: Option<FulfillmentStatus>,
    pub tracking_number: Option<String>,
    pub notes: Option<String>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
}

/// Order filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderFilter {
    pub customer_id: Option<Uuid>,
    pub status: Option<OrderStatus>,
    pub payment_status: Option<PaymentStatus>,
    pub fulfillment_status: Option<FulfillmentStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Order {
    /// Calculate total from items
    pub fn calculate_total(&self) -> Decimal {
        self.items.iter().map(|item| item.total).sum()
    }

    /// Check if order can be cancelled
    pub fn can_cancel(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Confirmed | OrderStatus::Processing
        )
    }

    /// Check if order can be refunded
    pub fn can_refund(&self) -> bool {
        matches!(self.payment_status, PaymentStatus::Paid | PaymentStatus::PartiallyPaid)
    }
}

impl OrderItem {
    /// Calculate item total
    pub fn calculate_total(quantity: i32, unit_price: Decimal, discount: Decimal, tax: Decimal) -> Decimal {
        let subtotal = unit_price * Decimal::from(quantity);
        subtotal - discount + tax
    }
}
