//! Cart and Checkout operations for shopping cart management
//!
//! Based on the Agentic Commerce Protocol (ACP) checkout system.
//!
//! # Example
//!
//! ```rust,no_run
//! use stateset_embedded::{Commerce, CreateCart, AddCartItem};
//! use rust_decimal_macros::dec;
//! use uuid::Uuid;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! // Create a cart
//! let cart = commerce.carts().create(CreateCart {
//!     customer_email: Some("alice@example.com".into()),
//!     customer_name: Some("Alice Smith".into()),
//!     ..Default::default()
//! })?;
//!
//! // Add items to cart
//! commerce.carts().add_item(cart.id, AddCartItem {
//!     sku: "SKU-001".into(),
//!     name: "Premium Widget".into(),
//!     quantity: 2,
//!     unit_price: dec!(49.99),
//!     ..Default::default()
//! })?;
//!
//! // Set shipping address
//! commerce.carts().set_shipping_address(cart.id, stateset_embedded::CartAddress {
//!     first_name: "Alice".into(),
//!     last_name: "Smith".into(),
//!     line1: "123 Main St".into(),
//!     city: "Anytown".into(),
//!     postal_code: "12345".into(),
//!     country: "US".into(),
//!     ..Default::default()
//! })?;
//!
//! // Complete checkout
//! let result = commerce.carts().complete(cart.id)?;
//! println!("Order created: {}", result.order_number);
//! # Ok::<(), stateset_embedded::CommerceError>(())
//! ```

use crate::Database;
use rust_decimal::Decimal;
use stateset_core::{
    AddCartItem, Cart, CartAddress, CartFilter, CartItem, CheckoutResult, CreateCart, Result,
    SetCartPayment, SetCartShipping, ShippingRate, UpdateCart, UpdateCartItem,
};
use std::sync::Arc;
use uuid::Uuid;

/// Cart and Checkout operations
pub struct Carts {
    db: Arc<dyn Database>,
}

impl Carts {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Create a new cart/checkout session
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCart, AddCartItem};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// // Guest checkout
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_email: Some("guest@example.com".into()),
    ///     items: Some(vec![AddCartItem {
    ///         sku: "SKU-001".into(),
    ///         name: "Widget".into(),
    ///         quantity: 1,
    ///         unit_price: dec!(19.99),
    ///         ..Default::default()
    ///     }]),
    ///     ..Default::default()
    /// })?;
    ///
    /// // Authenticated customer checkout
    /// let cart = commerce.carts().create(CreateCart {
    ///     customer_id: Some(Uuid::new_v4()),
    ///     currency: Some("USD".into()),
    ///     expires_in_minutes: Some(60),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateCart) -> Result<Cart> {
        self.db.carts().create(input)
    }

    /// Get a cart by ID
    pub fn get(&self, id: Uuid) -> Result<Option<Cart>> {
        self.db.carts().get(id)
    }

    /// Get a cart by cart number (e.g., "CART-1234567890-0001")
    pub fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>> {
        self.db.carts().get_by_number(cart_number)
    }

    /// Update a cart
    pub fn update(&self, id: Uuid, input: UpdateCart) -> Result<Cart> {
        self.db.carts().update(id, input)
    }

    /// List carts with optional filtering
    pub fn list(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        self.db.carts().list(filter)
    }

    /// Get all carts for a customer
    pub fn for_customer(&self, customer_id: Uuid) -> Result<Vec<Cart>> {
        self.db.carts().for_customer(customer_id)
    }

    /// Delete a cart
    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.db.carts().delete(id)
    }

    // === Item Operations ===

    /// Add an item to the cart
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AddCartItem};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// commerce.carts().add_item(Uuid::new_v4(), AddCartItem {
    ///     product_id: Some(Uuid::new_v4()),
    ///     sku: "SKU-001".into(),
    ///     name: "Premium Widget".into(),
    ///     description: Some("A high-quality widget".into()),
    ///     image_url: Some("https://example.com/widget.jpg".into()),
    ///     quantity: 2,
    ///     unit_price: dec!(49.99),
    ///     original_price: Some(dec!(59.99)),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn add_item(&self, cart_id: Uuid, item: AddCartItem) -> Result<CartItem> {
        self.db.carts().add_item(cart_id, item)
    }

    /// Update a cart item (quantity, etc.)
    pub fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        self.db.carts().update_item(item_id, input)
    }

    /// Remove an item from the cart
    pub fn remove_item(&self, item_id: Uuid) -> Result<()> {
        self.db.carts().remove_item(item_id)
    }

    /// Get all items in the cart
    pub fn get_items(&self, cart_id: Uuid) -> Result<Vec<CartItem>> {
        self.db.carts().get_items(cart_id)
    }

    /// Clear all items from the cart
    pub fn clear_items(&self, cart_id: Uuid) -> Result<()> {
        self.db.carts().clear_items(cart_id)
    }

    // === Address Operations ===

    /// Set the shipping address
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CartAddress};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let cart = commerce.carts().set_shipping_address(Uuid::new_v4(), CartAddress {
    ///     first_name: "Alice".into(),
    ///     last_name: "Smith".into(),
    ///     company: Some("Acme Corp".into()),
    ///     line1: "123 Main St".into(),
    ///     line2: Some("Suite 100".into()),
    ///     city: "Anytown".into(),
    ///     state: Some("CA".into()),
    ///     postal_code: "12345".into(),
    ///     country: "US".into(),
    ///     phone: Some("555-1234".into()),
    ///     email: Some("alice@example.com".into()),
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn set_shipping_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_shipping_address(id, address)
    }

    /// Set the billing address
    pub fn set_billing_address(&self, id: Uuid, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_billing_address(id, address)
    }

    // === Shipping Operations ===

    /// Set shipping method and address
    pub fn set_shipping(&self, id: Uuid, shipping: SetCartShipping) -> Result<Cart> {
        self.db.carts().set_shipping(id, shipping)
    }

    /// Get available shipping rates for the cart
    ///
    /// Returns available shipping options based on cart contents and shipping address.
    pub fn get_shipping_rates(&self, id: Uuid) -> Result<Vec<ShippingRate>> {
        self.db.carts().get_shipping_rates(id)
    }

    // === Payment Operations ===

    /// Set payment method and token
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, SetCartPayment};
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let cart = commerce.carts().set_payment(Uuid::new_v4(), SetCartPayment {
    ///     payment_method: "credit_card".into(),
    ///     payment_token: Some("tok_visa".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn set_payment(&self, id: Uuid, payment: SetCartPayment) -> Result<Cart> {
        self.db.carts().set_payment(id, payment)
    }

    // === Discount Operations ===

    /// Apply a coupon/discount code to the cart
    pub fn apply_discount(&self, id: Uuid, coupon_code: &str) -> Result<Cart> {
        self.db.carts().apply_discount(id, coupon_code)
    }

    /// Remove the discount from the cart
    pub fn remove_discount(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().remove_discount(id)
    }

    // === Checkout Flow ===

    /// Mark the cart as ready for payment
    ///
    /// This validates that all required information is present (shipping address, etc.)
    pub fn mark_ready_for_payment(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().mark_ready_for_payment(id)
    }

    /// Begin the checkout process (payment pending)
    pub fn begin_checkout(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().begin_checkout(id)
    }

    /// Complete the checkout and create an order
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let result = commerce.carts().complete(Uuid::new_v4())?;
    /// println!("Order ID: {}", result.order_id);
    /// println!("Order Number: {}", result.order_number);
    /// println!("Total Charged: {} {}", result.total_charged, result.currency);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn complete(&self, id: Uuid) -> Result<CheckoutResult> {
        self.db.carts().complete(id)
    }

    /// Cancel the cart
    pub fn cancel(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().cancel(id)
    }

    /// Mark the cart as abandoned
    pub fn abandon(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().abandon(id)
    }

    /// Expire the cart
    pub fn expire(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().expire(id)
    }

    // === Inventory Operations ===

    /// Reserve inventory for cart items
    ///
    /// Creates inventory reservations for all items in the cart.
    /// Reservations typically expire after 15 minutes.
    pub fn reserve_inventory(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().reserve_inventory(id)
    }

    /// Release inventory reservations for the cart
    pub fn release_inventory(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().release_inventory(id)
    }

    // === Totals Operations ===

    /// Recalculate cart totals
    pub fn recalculate(&self, id: Uuid) -> Result<Cart> {
        self.db.carts().recalculate(id)
    }

    /// Set the tax amount for the cart
    pub fn set_tax(&self, id: Uuid, tax_amount: Decimal) -> Result<Cart> {
        self.db.carts().set_tax(id, tax_amount)
    }

    // === Query Operations ===

    /// Get abandoned carts (for recovery campaigns)
    pub fn get_abandoned(&self) -> Result<Vec<Cart>> {
        self.db.carts().get_abandoned()
    }

    /// Get expired carts
    pub fn get_expired(&self) -> Result<Vec<Cart>> {
        self.db.carts().get_expired()
    }

    /// Count carts matching a filter
    pub fn count(&self, filter: CartFilter) -> Result<u64> {
        self.db.carts().count(filter)
    }
}
