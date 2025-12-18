//! Cart and Checkout domain models
//!
//! Based on the Agentic Commerce Protocol (ACP) checkout system.
//! Supports full checkout flow with items, totals, addresses, and fulfillment.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Address;

/// Cart/Checkout Session aggregate
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cart {
    pub id: Uuid,
    pub cart_number: String,
    pub customer_id: Option<Uuid>,
    pub status: CartStatus,
    pub currency: String,

    // Items
    pub items: Vec<CartItem>,

    // Totals
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub shipping_amount: Decimal,
    pub discount_amount: Decimal,
    pub grand_total: Decimal,

    // Customer info (for guest checkout)
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_name: Option<String>,

    // Addresses
    pub shipping_address: Option<CartAddress>,
    pub billing_address: Option<CartAddress>,
    pub billing_same_as_shipping: bool,

    // Fulfillment
    pub fulfillment_type: Option<FulfillmentType>,
    pub shipping_method: Option<String>,
    pub shipping_carrier: Option<String>,
    pub estimated_delivery: Option<DateTime<Utc>>,

    // Payment
    pub payment_method: Option<String>,
    pub payment_token: Option<String>,
    pub payment_status: CartPaymentStatus,

    // Discount/Promo
    pub coupon_code: Option<String>,
    pub discount_description: Option<String>,

    // Order reference (after checkout completes)
    pub order_id: Option<Uuid>,
    pub order_number: Option<String>,

    // Metadata
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,

    // Inventory reservations
    pub inventory_reserved: bool,
    pub reservation_expires_at: Option<DateTime<Utc>>,

    // Timestamps
    pub expires_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Cart line item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartItem {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub original_price: Option<Decimal>,
    pub discount_amount: Decimal,
    pub tax_amount: Decimal,
    pub total: Decimal,
    pub weight: Option<Decimal>,
    pub requires_shipping: bool,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Cart address (detailed for checkout)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartAddress {
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub line1: String,
    pub line2: Option<String>,
    pub city: String,
    pub state: Option<String>,
    pub postal_code: String,
    pub country: String,
    pub phone: Option<String>,
    pub email: Option<String>,
}

impl Default for CartAddress {
    fn default() -> Self {
        Self {
            first_name: String::new(),
            last_name: String::new(),
            company: None,
            line1: String::new(),
            line2: None,
            city: String::new(),
            state: None,
            postal_code: String::new(),
            country: String::new(),
            phone: None,
            email: None,
        }
    }
}

impl From<CartAddress> for Address {
    fn from(addr: CartAddress) -> Self {
        Address {
            line1: addr.line1,
            line2: addr.line2,
            city: addr.city,
            state: addr.state,
            postal_code: addr.postal_code,
            country: addr.country,
        }
    }
}

impl From<Address> for CartAddress {
    fn from(addr: Address) -> Self {
        CartAddress {
            first_name: String::new(),
            last_name: String::new(),
            company: None,
            line1: addr.line1,
            line2: addr.line2,
            city: addr.city,
            state: addr.state,
            postal_code: addr.postal_code,
            country: addr.country,
            phone: None,
            email: None,
        }
    }
}

/// Cart status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartStatus {
    /// Cart is active and being modified
    Active,
    /// Ready for payment (all required info collected)
    ReadyForPayment,
    /// Payment processing
    PaymentPending,
    /// Checkout completed, order created
    Completed,
    /// Cart abandoned by customer
    Abandoned,
    /// Cart cancelled
    Cancelled,
    /// Cart expired
    Expired,
}

impl Default for CartStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl std::fmt::Display for CartStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::ReadyForPayment => write!(f, "ready_for_payment"),
            Self::PaymentPending => write!(f, "payment_pending"),
            Self::Completed => write!(f, "completed"),
            Self::Abandoned => write!(f, "abandoned"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

/// Cart payment status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CartPaymentStatus {
    /// No payment attempted
    None,
    /// Payment method selected
    MethodSelected,
    /// Payment authorized but not captured
    Authorized,
    /// Payment captured/completed
    Captured,
    /// Payment failed
    Failed,
    /// Payment refunded
    Refunded,
}

impl Default for CartPaymentStatus {
    fn default() -> Self {
        Self::None
    }
}

impl std::fmt::Display for CartPaymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "none"),
            Self::MethodSelected => write!(f, "method_selected"),
            Self::Authorized => write!(f, "authorized"),
            Self::Captured => write!(f, "captured"),
            Self::Failed => write!(f, "failed"),
            Self::Refunded => write!(f, "refunded"),
        }
    }
}

/// Fulfillment type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentType {
    /// Ship to address
    Shipping,
    /// Store/local pickup
    Pickup,
    /// Digital delivery
    Digital,
}

impl Default for FulfillmentType {
    fn default() -> Self {
        Self::Shipping
    }
}

impl std::fmt::Display for FulfillmentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shipping => write!(f, "shipping"),
            Self::Pickup => write!(f, "pickup"),
            Self::Digital => write!(f, "digital"),
        }
    }
}

/// Shipping rate/option
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingRate {
    pub id: String,
    pub carrier: String,
    pub service: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
    pub estimated_days: Option<i32>,
    pub estimated_delivery: Option<DateTime<Utc>>,
}

/// Input for creating a new cart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCart {
    pub customer_id: Option<Uuid>,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub currency: Option<String>,
    pub items: Option<Vec<AddCartItem>>,
    pub shipping_address: Option<CartAddress>,
    pub billing_address: Option<CartAddress>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub expires_in_minutes: Option<i64>,
}

impl Default for CreateCart {
    fn default() -> Self {
        Self {
            customer_id: None,
            customer_email: None,
            customer_name: None,
            currency: None,
            items: None,
            shipping_address: None,
            billing_address: None,
            notes: None,
            metadata: None,
            expires_in_minutes: None,
        }
    }
}

/// Input for adding an item to cart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddCartItem {
    pub product_id: Option<Uuid>,
    pub variant_id: Option<Uuid>,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub original_price: Option<Decimal>,
    pub weight: Option<Decimal>,
    pub requires_shipping: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

impl Default for AddCartItem {
    fn default() -> Self {
        Self {
            product_id: None,
            variant_id: None,
            sku: String::new(),
            name: String::new(),
            description: None,
            image_url: None,
            quantity: 1,
            unit_price: Decimal::ZERO,
            original_price: None,
            weight: None,
            requires_shipping: Some(true),
            metadata: None,
        }
    }
}

/// Input for updating a cart item
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCartItem {
    pub quantity: Option<i32>,
    pub unit_price: Option<Decimal>,
    pub metadata: Option<serde_json::Value>,
}

/// Input for updating a cart
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateCart {
    pub customer_id: Option<Uuid>,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_name: Option<String>,
    pub shipping_address: Option<CartAddress>,
    pub billing_address: Option<CartAddress>,
    pub billing_same_as_shipping: Option<bool>,
    pub fulfillment_type: Option<FulfillmentType>,
    pub shipping_method: Option<String>,
    pub shipping_carrier: Option<String>,
    pub coupon_code: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Input for setting shipping on cart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCartShipping {
    pub shipping_address: CartAddress,
    pub shipping_method: Option<String>,
    pub shipping_carrier: Option<String>,
    pub shipping_amount: Option<Decimal>,
}

/// Input for setting payment on cart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetCartPayment {
    pub payment_method: String,
    pub payment_token: Option<String>,
    pub billing_address: Option<CartAddress>,
}

impl Default for SetCartPayment {
    fn default() -> Self {
        Self {
            payment_method: String::new(),
            payment_token: None,
            billing_address: None,
        }
    }
}

/// Input for applying discount/coupon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCartDiscount {
    pub coupon_code: String,
}

/// Cart filter for querying
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CartFilter {
    pub customer_id: Option<Uuid>,
    pub customer_email: Option<String>,
    pub status: Option<CartStatus>,
    pub has_items: Option<bool>,
    pub is_abandoned: Option<bool>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Checkout result after completing a cart
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckoutResult {
    pub cart_id: Uuid,
    pub order_id: Uuid,
    pub order_number: String,
    pub payment_id: Option<Uuid>,
    pub total_charged: Decimal,
    pub currency: String,
}

impl Cart {
    /// Check if cart has items
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Get total item count
    pub fn item_count(&self) -> i32 {
        self.items.iter().map(|i| i.quantity).sum()
    }

    /// Check if cart requires shipping
    pub fn requires_shipping(&self) -> bool {
        self.items.iter().any(|i| i.requires_shipping)
    }

    /// Check if cart is ready for checkout
    pub fn is_ready_for_checkout(&self) -> bool {
        if self.items.is_empty() {
            return false;
        }

        // Must have customer info (either customer_id or email)
        if self.customer_id.is_none() && self.customer_email.is_none() {
            return false;
        }

        // Must have shipping address if requires shipping
        if self.requires_shipping() && self.shipping_address.is_none() {
            return false;
        }

        true
    }

    /// Check if cart can be modified
    pub fn can_modify(&self) -> bool {
        matches!(self.status, CartStatus::Active)
    }

    /// Check if cart can be completed
    pub fn can_complete(&self) -> bool {
        matches!(self.status, CartStatus::ReadyForPayment | CartStatus::PaymentPending)
    }

    /// Check if cart can be cancelled
    pub fn can_cancel(&self) -> bool {
        matches!(
            self.status,
            CartStatus::Active | CartStatus::ReadyForPayment | CartStatus::PaymentPending
        )
    }

    /// Check if cart is abandoned
    pub fn is_abandoned(&self) -> bool {
        self.status == CartStatus::Abandoned
    }

    /// Check if cart is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Recalculate totals from items
    pub fn recalculate_totals(&mut self) {
        self.subtotal = self.items.iter().map(|i| i.total).sum();
        self.grand_total = self.subtotal + self.tax_amount + self.shipping_amount - self.discount_amount;
    }
}

impl CartItem {
    /// Calculate item total
    pub fn calculate_total(quantity: i32, unit_price: Decimal, discount: Decimal, tax: Decimal) -> Decimal {
        let subtotal = unit_price * Decimal::from(quantity);
        subtotal - discount + tax
    }

    /// Recalculate this item's total
    pub fn recalculate(&mut self) {
        self.total = Self::calculate_total(
            self.quantity,
            self.unit_price,
            self.discount_amount,
            self.tax_amount,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_cart_item_total_calculation() {
        let total = CartItem::calculate_total(2, dec!(10.00), dec!(0), dec!(1.60));
        assert_eq!(total, dec!(21.60));
    }

    #[test]
    fn test_cart_is_ready_for_checkout() {
        let mut cart = Cart {
            id: Uuid::new_v4(),
            cart_number: "CART-001".to_string(),
            customer_id: Some(Uuid::new_v4()),
            status: CartStatus::Active,
            currency: "USD".to_string(),
            items: vec![CartItem {
                id: Uuid::new_v4(),
                cart_id: Uuid::new_v4(),
                product_id: None,
                variant_id: None,
                sku: "SKU-001".to_string(),
                name: "Test Item".to_string(),
                description: None,
                image_url: None,
                quantity: 1,
                unit_price: dec!(10.00),
                original_price: None,
                discount_amount: dec!(0),
                tax_amount: dec!(0.80),
                total: dec!(10.80),
                weight: None,
                requires_shipping: true,
                metadata: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }],
            subtotal: dec!(10.00),
            tax_amount: dec!(0.80),
            shipping_amount: dec!(5.00),
            discount_amount: dec!(0),
            grand_total: dec!(15.80),
            customer_email: None,
            customer_phone: None,
            customer_name: None,
            shipping_address: None,
            billing_address: None,
            billing_same_as_shipping: true,
            fulfillment_type: Some(FulfillmentType::Shipping),
            shipping_method: None,
            shipping_carrier: None,
            estimated_delivery: None,
            payment_method: None,
            payment_token: None,
            payment_status: CartPaymentStatus::None,
            coupon_code: None,
            discount_description: None,
            order_id: None,
            order_number: None,
            notes: None,
            metadata: None,
            inventory_reserved: false,
            reservation_expires_at: None,
            expires_at: None,
            completed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Not ready - no shipping address
        assert!(!cart.is_ready_for_checkout());

        // Add shipping address
        cart.shipping_address = Some(CartAddress {
            first_name: "John".to_string(),
            last_name: "Doe".to_string(),
            company: None,
            line1: "123 Main St".to_string(),
            line2: None,
            city: "Anytown".to_string(),
            state: Some("CA".to_string()),
            postal_code: "12345".to_string(),
            country: "US".to_string(),
            phone: None,
            email: None,
        });

        // Now ready
        assert!(cart.is_ready_for_checkout());
    }
}
