//! Order domain models

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{CurrencyCode, CustomerId, OrderId, OrderItemId, ProductId};
use strum::{Display, EnumString};
use uuid::Uuid;

use crate::errors::Result;
use crate::validation::{Validate, ValidationBuilder, validate_money_scale};

/// Order aggregate root
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderId,
    pub order_number: String,
    pub customer_id: CustomerId,
    pub status: OrderStatus,
    pub order_date: DateTime<Utc>,
    pub total_amount: Decimal,
    pub currency: CurrencyCode,
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
    pub id: OrderItemId,
    pub order_id: OrderId,
    pub product_id: ProductId,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub quantity: i32,
    /// Units of this line that have physically shipped so far.
    ///
    /// Incremented by [`OrderRepository::ship`](crate::OrderRepository::ship);
    /// never exceeds `quantity`. Returns validate against this once the order
    /// has shipped (see the returns repository).
    #[serde(default)]
    pub shipped_quantity: i32,
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

/// Order status enumeration.
///
/// Uses [`strum`] derives for `Display` (`snake_case`) and `FromStr` (case-insensitive).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum OrderStatus {
    #[default]
    Pending,
    Confirmed,
    Processing,
    /// Some, but not all, ordered units have shipped (Σ shipped < Σ ordered).
    ///
    /// Derived from per-line `shipped_quantity` by
    /// [`OrderRepository::ship`](crate::OrderRepository::ship); it cannot be
    /// set directly through a plain status update.
    #[strum(serialize = "partially_shipped", serialize = "partiallyshipped")]
    PartiallyShipped,
    Shipped,
    Delivered,
    #[strum(serialize = "cancelled", serialize = "canceled")]
    Cancelled,
    Refunded,
}

impl OrderStatus {
    /// Check if a status transition is allowed.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }

        match self {
            Self::Pending => matches!(next, Self::Confirmed | Self::Cancelled),
            Self::Confirmed => matches!(next, Self::Processing | Self::Cancelled),
            Self::Processing => {
                matches!(next, Self::PartiallyShipped | Self::Shipped | Self::Cancelled)
            }
            Self::PartiallyShipped => matches!(next, Self::Shipped),
            Self::Shipped => matches!(next, Self::Delivered),
            Self::Delivered => matches!(next, Self::Refunded),
            Self::Cancelled | Self::Refunded => false,
        }
    }
}

/// Payment status enumeration.
///
/// Uses [`strum`] derives for `Display` (`snake_case`) and `FromStr` (case-insensitive).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum PaymentStatus {
    #[default]
    Pending,
    Authorized,
    Paid,
    #[strum(serialize = "partially_paid", serialize = "partiallypaid")]
    PartiallyPaid,
    Refunded,
    #[strum(serialize = "partially_refunded", serialize = "partiallyrefunded")]
    PartiallyRefunded,
    Failed,
}

/// Fulfillment status enumeration.
///
/// Uses [`strum`] derives for `Display` (`snake_case`) and `FromStr` (case-insensitive).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum FulfillmentStatus {
    #[default]
    Unfulfilled,
    #[strum(serialize = "partially_fulfilled", serialize = "partiallyfulfilled")]
    PartiallyFulfilled,
    Fulfilled,
    Shipped,
    Delivered,
}

/// What order creation does when a line cannot be fully reserved from stock.
///
/// Order creation reserves inventory for every line whose SKU is a tracked
/// inventory item. When the available quantity is short, this policy decides
/// whether the order is still created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StockPolicy {
    /// Reserve what is available and create a backorder for the remainder.
    ///
    /// This is the default and matches historical behaviour: the order is
    /// always created, and shortfalls surface as backorders.
    #[default]
    AllowBackorder,
    /// Fail the whole order with [`CommerceError::InsufficientStock`] if any
    /// line cannot be fully reserved. Nothing is persisted: no order row, no
    /// reservations, no backorders.
    ///
    /// [`CommerceError::InsufficientStock`]: crate::CommerceError::InsufficientStock
    RejectIfInsufficient,
}

impl StockPolicy {
    /// Whether this policy tolerates a partial (or zero) reservation.
    #[must_use]
    pub const fn allows_backorder(self) -> bool {
        matches!(self, Self::AllowBackorder)
    }
}

/// Input for creating a new order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrder {
    pub customer_id: CustomerId,
    pub items: Vec<CreateOrderItem>,
    pub currency: Option<CurrencyCode>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub notes: Option<String>,
    pub payment_method: Option<String>,
    pub shipping_method: Option<String>,
    /// How to handle lines that cannot be fully reserved from stock.
    /// Defaults to [`StockPolicy::AllowBackorder`].
    #[serde(default)]
    pub stock_policy: StockPolicy,
}

impl Default for CreateOrder {
    fn default() -> Self {
        Self {
            customer_id: CustomerId::nil(),
            items: vec![],
            currency: None,
            shipping_address: None,
            billing_address: None,
            notes: None,
            payment_method: None,
            shipping_method: None,
            stock_policy: StockPolicy::AllowBackorder,
        }
    }
}

/// Input for creating an order item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateOrderItem {
    pub product_id: ProductId,
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
            product_id: ProductId::nil(),
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

/// One order line in a shipment request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShipmentLineInput {
    /// The order line being shipped.
    pub order_item_id: OrderItemId,
    /// Units shipped now; must be `> 0` and `<= quantity - shipped_quantity`.
    pub quantity: i32,
}

/// Input for shipping an order (fully or partially).
///
/// `lines: None` (or an empty vector) ships every remaining unit on every
/// line, which is the legacy "flip the order to shipped" behaviour. Explicit
/// lines increment each line's `shipped_quantity`; the order becomes
/// [`OrderStatus::PartiallyShipped`] while Σ shipped < Σ ordered and
/// [`OrderStatus::Shipped`] once every unit has left.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShipOrder {
    /// Carrier tracking number to record on the order.
    pub tracking_number: Option<String>,
    /// Per-line shipped quantities; `None` ships all remaining units.
    pub lines: Option<Vec<ShipmentLineInput>>,
}

/// Order filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrderFilter {
    pub customer_id: Option<CustomerId>,
    pub status: Option<OrderStatus>,
    pub payment_status: Option<PaymentStatus>,
    pub fulfillment_status: Option<FulfillmentStatus>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    /// Keyset cursor: return records after this `(sort_key, id)` pair.
    /// Sort key is `order_date` (DESC ordering).
    pub after_cursor: Option<(String, String)>,
}

impl Order {
    /// Calculate total from items
    #[must_use]
    pub fn calculate_total(&self) -> Decimal {
        self.items.iter().map(|item| item.total).sum()
    }

    /// Check if order can be cancelled
    #[must_use]
    pub const fn can_cancel(&self) -> bool {
        matches!(
            self.status,
            OrderStatus::Pending | OrderStatus::Confirmed | OrderStatus::Processing
        )
    }

    /// Check if order can be refunded
    #[must_use]
    pub const fn can_refund(&self) -> bool {
        matches!(self.payment_status, PaymentStatus::Paid | PaymentStatus::PartiallyPaid)
    }
}

/// Decimal places a line/order money total is rounded to.
///
/// Currency minor units vary (JPY has 0, most have 2, a few have 3), but the
/// order pipeline has historically assumed 2; keeping a single constant here
/// keeps every backend's rounding identical. A currency-aware version would
/// thread the currency into `calculate_total`.
pub(crate) const MONEY_SCALE: u32 = 2;

impl OrderItem {
    /// Units not yet shipped on this line (`quantity - shipped_quantity`, never negative).
    #[must_use]
    pub const fn remaining_to_ship(&self) -> i32 {
        let remaining = self.quantity - self.shipped_quantity;
        if remaining < 0 { 0 } else { remaining }
    }

    /// Calculate a line item's money total, rounded to the currency minor unit.
    ///
    /// The result is rounded to `MONEY_SCALE` (2) decimal places so the stored
    /// line total is a real money amount and an order's `total_amount` (the sum
    /// of these line totals) foots exactly to its line items. All order-creation
    /// paths on both backends route through this function so they agree to the
    /// cent.
    #[must_use]
    pub fn calculate_total(
        quantity: i32,
        unit_price: Decimal,
        discount: Decimal,
        tax: Decimal,
    ) -> Decimal {
        let subtotal = unit_price * Decimal::from(quantity);
        (subtotal - discount + tax).round_dp(MONEY_SCALE)
    }
}

impl CreateOrder {
    /// The currency this order's money amounts are denominated in.
    ///
    /// Falls back to [`CurrencyCode::default`] (USD) when the request omits it,
    /// matching what every backend persists on the order row.
    #[must_use]
    pub fn effective_currency(&self) -> CurrencyCode {
        self.currency.unwrap_or_default()
    }

    /// Reject any line money amount carrying more decimal places than the
    /// order's currency allows (invariant M1,
    /// `commerce.money.scale_exceeds_currency`).
    ///
    /// Covers every monetary field on the create request: each line's
    /// `unit_price`, `discount` and `tax_amount`. `CreateOrder` itself carries
    /// no order-level money field — the order total is derived from the lines
    /// and rounded to the currency minor unit — so guarding the lines guards
    /// the whole request.
    ///
    /// Scale is significant scale: `10.9900` is accepted for USD, `10.999` is
    /// not. See [`validate_money_scale`].
    ///
    /// # Errors
    ///
    /// Returns [`CommerceError::MoneyScaleExceedsCurrency`] for the first
    /// offending amount.
    ///
    /// [`CommerceError::MoneyScaleExceedsCurrency`]: crate::CommerceError::MoneyScaleExceedsCurrency
    pub fn validate_money_scale(&self) -> Result<()> {
        let currency = self.effective_currency();
        for item in &self.items {
            validate_money_scale(currency, item.unit_price)?;
            if let Some(discount) = item.discount {
                validate_money_scale(currency, discount)?;
            }
            if let Some(tax) = item.tax_amount {
                validate_money_scale(currency, tax)?;
            }
        }
        Ok(())
    }
}

impl Validate for CreateOrderItem {
    /// Validate a single order line item.
    ///
    /// Rejects empty SKU/name, a non-positive quantity, a negative unit price,
    /// and negative discount/tax amounts. A zero unit price is permitted (e.g.
    /// free/gift items); only negative monetary amounts are rejected.
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .required("sku", &self.sku)
            .required("name", &self.name)
            .positive_i32("quantity", self.quantity)
            .non_negative("unit_price", self.unit_price)
            .non_negative("discount", self.discount.unwrap_or(Decimal::ZERO))
            .non_negative("tax_amount", self.tax_amount.unwrap_or(Decimal::ZERO))
            .build()
    }
}

impl Validate for CreateOrder {
    /// Validate an order create request.
    ///
    /// Requires a non-nil customer, at least one line item, and validates each
    /// item. Currency consistency is enforced at the item level where prices
    /// share the order's single currency. Finally every line money amount is
    /// checked against the order currency's minor units
    /// ([`CreateOrder::validate_money_scale`]).
    fn validate(&self) -> Result<()> {
        ValidationBuilder::new()
            .uuid_not_nil("customer_id", self.customer_id.into_uuid())
            .non_empty_list("items", &self.items)
            .build()?;

        for item in &self.items {
            item.validate()?;
        }

        self.validate_money_scale()?;

        Ok(())
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
        let order_id = OrderId::new();
        let discount = dec!(0.00);
        let tax = (unit_price * Decimal::from(quantity) * dec!(0.08)).round_dp(2);
        let total = OrderItem::calculate_total(quantity, unit_price, discount, tax);

        OrderItem {
            id: OrderItemId::new(),
            order_id,
            product_id: ProductId::new(),
            variant_id: None,
            sku: "TEST-SKU-001".to_string(),
            name: "Test Product".to_string(),
            quantity,
            shipped_quantity: 0,
            unit_price,
            discount,
            tax_amount: tax,
            total,
        }
    }

    fn create_test_order(status: OrderStatus, payment_status: PaymentStatus) -> Order {
        let now = Utc::now();
        let items =
            vec![create_test_order_item(2, dec!(29.99)), create_test_order_item(1, dec!(49.99))];
        let total: Decimal = items.iter().map(|i| i.total).sum();

        Order {
            id: OrderId::new(),
            order_number: "ORD-2024-001".to_string(),
            customer_id: CustomerId::new(),
            status,
            order_date: now,
            total_amount: total,
            currency: CurrencyCode::USD,
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
    fn test_order_status_allows_valid_transitions() {
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Confirmed));
        assert!(OrderStatus::Confirmed.can_transition_to(OrderStatus::Processing));
        assert!(OrderStatus::Processing.can_transition_to(OrderStatus::Shipped));
        assert!(OrderStatus::Shipped.can_transition_to(OrderStatus::Delivered));
        assert!(OrderStatus::Delivered.can_transition_to(OrderStatus::Refunded));
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Cancelled));
    }

    #[test]
    fn test_order_status_rejects_invalid_transitions() {
        assert!(!OrderStatus::Pending.can_transition_to(OrderStatus::Delivered));
        assert!(!OrderStatus::Shipped.can_transition_to(OrderStatus::Cancelled));
        assert!(!OrderStatus::Refunded.can_transition_to(OrderStatus::Processing));
    }

    #[test]
    fn test_order_status_allows_idempotent_transition() {
        assert!(OrderStatus::Pending.can_transition_to(OrderStatus::Pending));
        assert!(OrderStatus::Cancelled.can_transition_to(OrderStatus::Cancelled));
    }

    #[test]
    fn test_status_from_str_accepts_legacy_variants() {
        use std::str::FromStr;

        assert_eq!(PaymentStatus::from_str("partiallypaid").unwrap(), PaymentStatus::PartiallyPaid);
        assert_eq!(
            PaymentStatus::from_str("partiallyrefunded").unwrap(),
            PaymentStatus::PartiallyRefunded
        );
        assert_eq!(
            FulfillmentStatus::from_str("partiallyfulfilled").unwrap(),
            FulfillmentStatus::PartiallyFulfilled
        );
        assert_eq!(OrderStatus::from_str("canceled").unwrap(), OrderStatus::Cancelled);
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
        assert!(create_order.customer_id.is_nil());
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
        assert!(item.product_id.is_nil());
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
        let customer_id = CustomerId::new();
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

    fn valid_create_order_item() -> CreateOrderItem {
        CreateOrderItem {
            product_id: ProductId::new(),
            sku: "SKU-1".to_string(),
            name: "Item".to_string(),
            quantity: 1,
            unit_price: dec!(10.00),
            ..Default::default()
        }
    }

    #[test]
    fn test_create_order_item_validate() {
        // Valid item passes.
        assert!(valid_create_order_item().validate().is_ok());

        // Negative unit price is rejected.
        let mut item = valid_create_order_item();
        item.unit_price = dec!(-0.01);
        assert!(item.validate().is_err());

        // Non-positive quantity is rejected.
        let mut item = valid_create_order_item();
        item.quantity = 0;
        assert!(item.validate().is_err());
        item.quantity = -2;
        assert!(item.validate().is_err());

        // Negative discount / tax are rejected.
        let mut item = valid_create_order_item();
        item.discount = Some(dec!(-1));
        assert!(item.validate().is_err());
        let mut item = valid_create_order_item();
        item.tax_amount = Some(dec!(-1));
        assert!(item.validate().is_err());

        // A free ($0) item with positive quantity is valid.
        let mut item = valid_create_order_item();
        item.unit_price = dec!(0);
        assert!(item.validate().is_ok());
    }

    #[test]
    fn test_create_order_validate() {
        // Valid order with one item passes.
        let order = CreateOrder {
            customer_id: CustomerId::new(),
            items: vec![valid_create_order_item()],
            ..Default::default()
        };
        assert!(order.validate().is_ok());

        // Nil customer is rejected.
        let order = CreateOrder {
            customer_id: CustomerId::nil(),
            items: vec![valid_create_order_item()],
            ..Default::default()
        };
        assert!(order.validate().is_err());

        // Empty items list is rejected.
        let order =
            CreateOrder { customer_id: CustomerId::new(), items: vec![], ..Default::default() };
        assert!(order.validate().is_err());

        // An invalid line item is rejected.
        let mut bad_item = valid_create_order_item();
        bad_item.quantity = -1;
        let order = CreateOrder {
            customer_id: CustomerId::new(),
            items: vec![bad_item],
            ..Default::default()
        };
        assert!(order.validate().is_err());
    }

    #[test]
    fn create_order_rejects_over_scaled_line_money() {
        let base = || CreateOrder {
            customer_id: CustomerId::new(),
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: "SKU".into(),
                name: "Item".into(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        };

        // Each money field on the line is covered.
        for mutate in [
            (|o: &mut CreateOrder| o.items[0].unit_price = dec!(10.999)) as fn(&mut CreateOrder),
            |o: &mut CreateOrder| o.items[0].discount = Some(dec!(0.005)),
            |o: &mut CreateOrder| o.items[0].tax_amount = Some(dec!(0.875)),
        ] {
            let mut order = base();
            mutate(&mut order);
            let err = order.validate().expect_err("over-scaled money must be rejected");
            assert_eq!(err.invariant_code(), Some("commerce.money.scale_exceeds_currency"));
        }

        // Trailing zeros are insignificant: 10.9900 is two-scale USD.
        let mut ok = base();
        ok.items[0].unit_price = dec!(10.9900);
        ok.validate().expect("10.9900 is valid USD");

        // The order's currency, not a hard-coded 2, sets the bound.
        let mut jpy = base();
        jpy.currency = Some(CurrencyCode::JPY);
        jpy.items[0].unit_price = dec!(1000.50);
        assert_eq!(
            jpy.validate().expect_err("JPY has no minor units").invariant_code(),
            Some("commerce.money.scale_exceeds_currency")
        );
    }
}
