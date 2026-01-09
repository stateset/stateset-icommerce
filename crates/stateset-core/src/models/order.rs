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

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ============================================================================
    // Test Helpers
    // ============================================================================

    fn create_test_address() -> Address {
        Address {
            line1: "123 Main St".to_string(),
            line2: Some("Apt 4".to_string()),
            city: "San Francisco".to_string(),
            state: Some("CA".to_string()),
            postal_code: "94102".to_string(),
            country: "US".to_string(),
        }
    }

    fn create_test_order_item(quantity: i32, unit_price: Decimal) -> OrderItem {
        let order_id = Uuid::new_v4();
        let discount = dec!(0.00);
        let tax = (unit_price * Decimal::from(quantity) * dec!(0.08)).round_dp(2);
        let total = OrderItem::calculate_total(quantity, unit_price, discount, tax);

        OrderItem {
            id: Uuid::new_v4(),
            order_id,
            product_id: Uuid::new_v4(),
            variant_id: None,
            sku: "TEST-SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity,
            unit_price,
            discount,
            tax_amount: tax,
            total,
        }
    }

    fn create_test_order(status: OrderStatus, payment_status: PaymentStatus) -> Order {
        let now = Utc::now();
        let items = vec![
            create_test_order_item(2, dec!(29.99)),
            create_test_order_item(1, dec!(49.99)),
        ];
        let total: Decimal = items.iter().map(|i| i.total).sum();

        Order {
            id: Uuid::new_v4(),
            order_number: "ORD-2024-001".to_string(),
            customer_id: Uuid::new_v4(),
            status,
            order_date: now,
            total_amount: total,
            currency: "USD".to_string(),
            payment_status,
            fulfillment_status: FulfillmentStatus::Unfulfilled,
            payment_method: Some("credit_card".to_string()),
            shipping_method: Some("standard".to_string()),
            tracking_number: None,
            notes: None,
            shipping_address: Some(create_test_address()),
            billing_address: None,
            items,
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    // ============================================================================
    // Order Tests
    // ============================================================================

    #[test]
    fn test_order_calculate_total() {
        let order = create_test_order(OrderStatus::Pending, PaymentStatus::Pending);
        let calculated = order.calculate_total();
        let expected: Decimal = order.items.iter().map(|i| i.total).sum();
        assert_eq!(calculated, expected);
    }

    #[test]
    fn test_order_calculate_total_empty_items() {
        let mut order = create_test_order(OrderStatus::Pending, PaymentStatus::Pending);
        order.items.clear();
        assert_eq!(order.calculate_total(), dec!(0));
    }

    #[test]
    fn test_order_can_cancel_pending() {
        let order = create_test_order(OrderStatus::Pending, PaymentStatus::Pending);
        assert!(order.can_cancel());
    }

    #[test]
    fn test_order_can_cancel_confirmed() {
        let order = create_test_order(OrderStatus::Confirmed, PaymentStatus::Authorized);
        assert!(order.can_cancel());
    }

    #[test]
    fn test_order_can_cancel_processing() {
        let order = create_test_order(OrderStatus::Processing, PaymentStatus::Paid);
        assert!(order.can_cancel());
    }

    #[test]
    fn test_order_cannot_cancel_shipped() {
        let order = create_test_order(OrderStatus::Shipped, PaymentStatus::Paid);
        assert!(!order.can_cancel());
    }

    #[test]
    fn test_order_cannot_cancel_delivered() {
        let order = create_test_order(OrderStatus::Delivered, PaymentStatus::Paid);
        assert!(!order.can_cancel());
    }

    #[test]
    fn test_order_cannot_cancel_already_cancelled() {
        let order = create_test_order(OrderStatus::Cancelled, PaymentStatus::Refunded);
        assert!(!order.can_cancel());
    }

    #[test]
    fn test_order_can_refund_when_paid() {
        let order = create_test_order(OrderStatus::Delivered, PaymentStatus::Paid);
        assert!(order.can_refund());
    }

    #[test]
    fn test_order_can_refund_when_partially_paid() {
        let order = create_test_order(OrderStatus::Delivered, PaymentStatus::PartiallyPaid);
        assert!(order.can_refund());
    }

    #[test]
    fn test_order_cannot_refund_when_pending() {
        let order = create_test_order(OrderStatus::Pending, PaymentStatus::Pending);
        assert!(!order.can_refund());
    }

    #[test]
    fn test_order_cannot_refund_when_already_refunded() {
        let order = create_test_order(OrderStatus::Refunded, PaymentStatus::Refunded);
        assert!(!order.can_refund());
    }

    // ============================================================================
    // OrderItem Tests
    // ============================================================================

    #[test]
    fn test_order_item_calculate_total_basic() {
        let total = OrderItem::calculate_total(2, dec!(29.99), dec!(0), dec!(4.80));
        // 2 * 29.99 = 59.98, - 0 + 4.80 = 64.78
        assert_eq!(total, dec!(64.78));
    }

    #[test]
    fn test_order_item_calculate_total_with_discount() {
        let total = OrderItem::calculate_total(2, dec!(29.99), dec!(10.00), dec!(4.00));
        // 2 * 29.99 = 59.98, - 10.00 + 4.00 = 53.98
        assert_eq!(total, dec!(53.98));
    }

    #[test]
    fn test_order_item_calculate_total_zero_quantity() {
        let total = OrderItem::calculate_total(0, dec!(29.99), dec!(0), dec!(0));
        assert_eq!(total, dec!(0));
    }

    #[test]
    fn test_order_item_calculate_total_high_quantity() {
        let total = OrderItem::calculate_total(1000, dec!(9.99), dec!(0), dec!(799.20));
        // 1000 * 9.99 = 9990, + 799.20 = 10789.20
        assert_eq!(total, dec!(10789.20));
    }

    // ============================================================================
    // OrderStatus Tests
    // ============================================================================

    #[test]
    fn test_order_status_default() {
        assert_eq!(OrderStatus::default(), OrderStatus::Pending);
    }

    #[test]
    fn test_order_status_display() {
        assert_eq!(format!("{}", OrderStatus::Pending), "pending");
        assert_eq!(format!("{}", OrderStatus::Confirmed), "confirmed");
        assert_eq!(format!("{}", OrderStatus::Processing), "processing");
        assert_eq!(format!("{}", OrderStatus::Shipped), "shipped");
        assert_eq!(format!("{}", OrderStatus::Delivered), "delivered");
        assert_eq!(format!("{}", OrderStatus::Cancelled), "cancelled");
        assert_eq!(format!("{}", OrderStatus::Refunded), "refunded");
    }

    #[test]
    fn test_order_status_serialization() {
        let status = OrderStatus::Processing;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"processing\"");

        let deserialized: OrderStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    // ============================================================================
    // PaymentStatus Tests
    // ============================================================================

    #[test]
    fn test_payment_status_default() {
        assert_eq!(PaymentStatus::default(), PaymentStatus::Pending);
    }

    #[test]
    fn test_payment_status_display() {
        assert_eq!(format!("{}", PaymentStatus::Pending), "pending");
        assert_eq!(format!("{}", PaymentStatus::Authorized), "authorized");
        assert_eq!(format!("{}", PaymentStatus::Paid), "paid");
        assert_eq!(format!("{}", PaymentStatus::PartiallyPaid), "partially_paid");
        assert_eq!(format!("{}", PaymentStatus::Refunded), "refunded");
        assert_eq!(format!("{}", PaymentStatus::PartiallyRefunded), "partially_refunded");
        assert_eq!(format!("{}", PaymentStatus::Failed), "failed");
    }

    #[test]
    fn test_payment_status_serialization() {
        let status = PaymentStatus::PartiallyPaid;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"partially_paid\"");

        let deserialized: PaymentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    // ============================================================================
    // FulfillmentStatus Tests
    // ============================================================================

    #[test]
    fn test_fulfillment_status_default() {
        assert_eq!(FulfillmentStatus::default(), FulfillmentStatus::Unfulfilled);
    }

    #[test]
    fn test_fulfillment_status_display() {
        assert_eq!(format!("{}", FulfillmentStatus::Unfulfilled), "unfulfilled");
        assert_eq!(format!("{}", FulfillmentStatus::PartiallyFulfilled), "partially_fulfilled");
        assert_eq!(format!("{}", FulfillmentStatus::Fulfilled), "fulfilled");
        assert_eq!(format!("{}", FulfillmentStatus::Shipped), "shipped");
        assert_eq!(format!("{}", FulfillmentStatus::Delivered), "delivered");
    }

    // ============================================================================
    // CreateOrder Tests
    // ============================================================================

    #[test]
    fn test_create_order_default() {
        let create_order = CreateOrder::default();
        assert_eq!(create_order.customer_id, Uuid::nil());
        assert!(create_order.items.is_empty());
        assert!(create_order.currency.is_none());
        assert!(create_order.shipping_address.is_none());
    }

    // ============================================================================
    // CreateOrderItem Tests
    // ============================================================================

    #[test]
    fn test_create_order_item_default() {
        let item = CreateOrderItem::default();
        assert_eq!(item.product_id, Uuid::nil());
        assert_eq!(item.quantity, 0);
        assert_eq!(item.unit_price, Decimal::ZERO);
        assert!(item.sku.is_empty());
    }

    // ============================================================================
    // Address Tests
    // ============================================================================

    #[test]
    fn test_address_serialization_roundtrip() {
        let address = create_test_address();
        let json = serde_json::to_string(&address).unwrap();
        let deserialized: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(address, deserialized);
    }

    #[test]
    fn test_address_without_optional_fields() {
        let address = Address {
            line1: "123 Main St".to_string(),
            line2: None,
            city: "NYC".to_string(),
            state: None,
            postal_code: "10001".to_string(),
            country: "US".to_string(),
        };

        let json = serde_json::to_string(&address).unwrap();
        let deserialized: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(address, deserialized);
    }

    // ============================================================================
    // Order Serialization Tests
    // ============================================================================

    #[test]
    fn test_order_serialization_roundtrip() {
        let order = create_test_order(OrderStatus::Confirmed, PaymentStatus::Paid);
        let json = serde_json::to_string(&order).unwrap();
        let deserialized: Order = serde_json::from_str(&json).unwrap();
        assert_eq!(order, deserialized);
    }

    #[test]
    fn test_order_item_serialization_roundtrip() {
        let item = create_test_order_item(3, dec!(19.99));
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: OrderItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, deserialized);
    }

    // ============================================================================
    // UpdateOrder Tests
    // ============================================================================

    #[test]
    fn test_update_order_default() {
        let update = UpdateOrder::default();
        assert!(update.status.is_none());
        assert!(update.payment_status.is_none());
        assert!(update.fulfillment_status.is_none());
        assert!(update.tracking_number.is_none());
    }

    #[test]
    fn test_update_order_partial() {
        let update = UpdateOrder {
            status: Some(OrderStatus::Shipped),
            tracking_number: Some("1Z999AA10123456784".to_string()),
            ..Default::default()
        };

        assert_eq!(update.status, Some(OrderStatus::Shipped));
        assert!(update.tracking_number.is_some());
        assert!(update.payment_status.is_none());
    }

    // ============================================================================
    // OrderFilter Tests
    // ============================================================================

    #[test]
    fn test_order_filter_default() {
        let filter = OrderFilter::default();
        assert!(filter.customer_id.is_none());
        assert!(filter.status.is_none());
        assert!(filter.limit.is_none());
        assert!(filter.offset.is_none());
    }

    #[test]
    fn test_order_filter_with_values() {
        let customer_id = Uuid::new_v4();
        let filter = OrderFilter {
            customer_id: Some(customer_id),
            status: Some(OrderStatus::Pending),
            limit: Some(10),
            offset: Some(0),
            ..Default::default()
        };

        assert_eq!(filter.customer_id, Some(customer_id));
        assert_eq!(filter.status, Some(OrderStatus::Pending));
        assert_eq!(filter.limit, Some(10));
    }
}
