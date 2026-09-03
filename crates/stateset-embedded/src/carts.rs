//! Cart and Checkout operations for shopping cart management
//!
//! Protocol-neutral embedded cart and checkout API.
//!
//! # Example
//!
//! ```ignore
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
    AddCartItem, Cart, CartAddress, CartFilter, CartId, CartItem, CheckoutResult, CreateCart,
    CustomerId, Result, SetCartPayment, SetCartShipping, ShippingRate, UpdateCart, UpdateCartItem,
    Validate,
};
use stateset_observability::Metrics;
use std::sync::Arc;
use uuid::Uuid;

/// Cart and Checkout operations
pub struct Carts {
    db: Arc<dyn Database>,
    metrics: Metrics,
}

impl std::fmt::Debug for Carts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Carts").finish_non_exhaustive()
    }
}

impl Carts {
    pub(crate) fn new(db: Arc<dyn Database>, metrics: Metrics) -> Self {
        Self { db, metrics }
    }

    /// Create a new cart/checkout session
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, CreateCart, AddCartItem, CurrencyCode};
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
    ///     customer_id: Some(Uuid::new_v4().into()),
    ///     currency: Some(CurrencyCode::USD),
    ///     expires_in_minutes: Some(60),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn create(&self, input: CreateCart) -> Result<Cart> {
        // Reject obviously-invalid input (negative prices, non-positive
        // quantities) before persisting any line items.
        input.validate()?;
        if let Some(items) = &input.items {
            for item in items {
                self.enforce_catalog_price(item)?;
            }
        }
        let cart = self.db.carts().create(input)?;
        self.metrics.record_cart_created(&cart.id.to_string());
        Ok(cart)
    }

    /// Catalog price for the line `item` names, if it names a catalog line.
    ///
    /// Resolution order: the explicit `variant_id`, then the SKU (catalog
    /// SKUs are variant-level). A `product_id` alone does not carry a price.
    fn catalog_price(&self, item: &AddCartItem) -> Result<Option<Decimal>> {
        let products = self.db.products();
        if let Some(variant_id) = item.variant_id {
            if let Some(variant) = products.get_variant(variant_id)? {
                return Ok(Some(variant.price));
            }
        }
        if !item.sku.trim().is_empty() {
            if let Some(variant) = products.get_variant_by_sku(&item.sku)? {
                return Ok(Some(variant.price));
            }
        }
        Ok(None)
    }

    /// Prices are never client-trusted for catalog lines: when the line's
    /// variant / SKU resolves to a catalog product, the caller's `unit_price`
    /// must equal the catalog price or the line is refused. Ad-hoc lines (no
    /// catalog match) keep the caller's price.
    fn enforce_catalog_price(&self, item: &AddCartItem) -> Result<()> {
        if let Some(catalog) = self.catalog_price(item)? {
            if catalog != item.unit_price {
                return Err(stateset_core::CommerceError::ValidationError(format!(
                    "unit_price {} for SKU '{}' does not match the catalog price {catalog}; \
                     catalog lines are priced from the catalog",
                    item.unit_price, item.sku
                )));
            }
        }
        Ok(())
    }

    /// [`Self::enforce_catalog_price`] for a line already in the cart whose
    /// price is being changed.
    fn enforce_catalog_price_on_update(&self, item_id: Uuid, input: &UpdateCartItem) -> Result<()> {
        let Some(new_price) = input.unit_price else {
            return Ok(());
        };
        let Some(line) = self.find_item(item_id)? else {
            return Ok(());
        };
        let probe = AddCartItem {
            variant_id: line.variant_id,
            sku: line.sku,
            unit_price: new_price,
            ..Default::default()
        };
        self.enforce_catalog_price(&probe)
    }

    /// The cart line `item_id`, if it exists.
    fn find_item(&self, item_id: Uuid) -> Result<Option<CartItem>> {
        self.db.carts().get_item(item_id)
    }

    /// Re-run the tax engine for a cart that carries a tax context (a tax
    /// amount was set and it has a shipping address) after its lines changed.
    ///
    /// The storage layer has already carried the tax proportionally to the
    /// new subtotal (see `rescale_tax` in the cart repositories); this
    /// replaces that estimate with the engine's figure whenever the engine
    /// can price the cart. When no tax rate covers the address the engine
    /// has nothing to say and the proportional estimate stands, so a tax set
    /// manually via `set_tax` is not silently zeroed.
    fn refresh_tax(&self, cart_id: CartId) -> Result<()> {
        use stateset_core::{ProductTaxCategory, TaxAddress, TaxCalculationRequest, TaxLineItem};
        let Some(cart) = self.db.carts().get(cart_id)? else {
            return Ok(());
        };
        if cart.tax_amount <= Decimal::ZERO {
            return Ok(());
        }
        let Some(address) = cart.shipping_address else {
            return Ok(());
        };
        let request = TaxCalculationRequest {
            line_items: cart
                .items
                .iter()
                .map(|item| TaxLineItem {
                    id: item.id.to_string(),
                    sku: Some(item.sku.clone()),
                    product_id: item.product_id,
                    quantity: Decimal::from(item.quantity),
                    unit_price: item.unit_price,
                    discount_amount: item.discount_amount,
                    tax_category: ProductTaxCategory::Standard,
                    tax_code: None,
                    description: Some(item.name.clone()),
                })
                .collect(),
            shipping_address: TaxAddress {
                country: address.country,
                state: address.state,
                city: Some(address.city),
                postal_code: Some(address.postal_code),
                line1: Some(address.line1),
                line2: address.line2,
            },
            customer_id: cart.customer_id.map(Into::into),
            currency: cart.currency,
            shipping_amount: Some(cart.shipping_amount),
            ..Default::default()
        };
        let Ok(result) = self.db.tax().calculate_tax(request) else {
            return Ok(());
        };
        if result.tax_breakdown.is_empty() && result.total_tax <= Decimal::ZERO {
            return Ok(());
        }
        if result.total_tax != cart.tax_amount {
            self.db.carts().set_tax(cart_id, result.total_tax)?;
        }
        Ok(())
    }

    /// Get a cart by ID
    pub fn get(&self, id: CartId) -> Result<Option<Cart>> {
        self.db.carts().get(id)
    }

    /// Get a cart by cart number (e.g., "CART-1234567890-0001")
    pub fn get_by_number(&self, cart_number: &str) -> Result<Option<Cart>> {
        self.db.carts().get_by_number(cart_number)
    }

    /// Update a cart
    pub fn update(&self, id: CartId, input: UpdateCart) -> Result<Cart> {
        self.db.carts().update(id, input)
    }

    /// List carts with optional filtering
    pub fn list(&self, filter: CartFilter) -> Result<Vec<Cart>> {
        self.db.carts().list(filter)
    }

    /// Get all carts for a customer
    pub fn for_customer(&self, customer_id: CustomerId) -> Result<Vec<Cart>> {
        self.db.carts().for_customer(customer_id)
    }

    /// Delete a cart
    pub fn delete(&self, id: CartId) -> Result<()> {
        self.db.carts().delete(id)
    }

    // === Item Operations ===

    /// Add an item to the cart
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::{Commerce, AddCartItem, ProductId};
    /// use rust_decimal_macros::dec;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// commerce.carts().add_item(Uuid::new_v4().into(), AddCartItem {
    ///     product_id: Some(ProductId::new()),
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
    pub fn add_item(&self, cart_id: CartId, item: AddCartItem) -> Result<CartItem> {
        // Reject a negative price or non-positive quantity before persisting.
        item.validate()?;
        self.enforce_catalog_price(&item)?;
        let added = self.db.carts().add_item(cart_id, item)?;
        self.refresh_tax(cart_id)?;
        Ok(added)
    }

    /// Update a cart item (quantity, etc.)
    pub fn update_item(&self, item_id: Uuid, input: UpdateCartItem) -> Result<CartItem> {
        self.enforce_catalog_price_on_update(item_id, &input)?;
        let updated = self.db.carts().update_item(item_id, input)?;
        self.refresh_tax(updated.cart_id)?;
        Ok(updated)
    }

    /// Remove an item from the cart
    pub fn remove_item(&self, item_id: Uuid) -> Result<()> {
        let cart_id = self.find_item(item_id)?.map(|item| item.cart_id);
        self.db.carts().remove_item(item_id)?;
        if let Some(cart_id) = cart_id {
            self.refresh_tax(cart_id)?;
        }
        Ok(())
    }

    /// Get all items in the cart
    pub fn get_items(&self, cart_id: CartId) -> Result<Vec<CartItem>> {
        self.db.carts().get_items(cart_id)
    }

    /// Clear all items from the cart
    pub fn clear_items(&self, cart_id: CartId) -> Result<()> {
        self.db.carts().clear_items(cart_id)?;
        self.refresh_tax(cart_id)
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
    /// let cart = commerce.carts().set_shipping_address(Uuid::new_v4().into(), CartAddress {
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
    pub fn set_shipping_address(&self, id: CartId, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_shipping_address(id, address)
    }

    /// Set the billing address
    pub fn set_billing_address(&self, id: CartId, address: CartAddress) -> Result<Cart> {
        self.db.carts().set_billing_address(id, address)
    }

    // === Shipping Operations ===

    /// Set shipping method and address
    pub fn set_shipping(&self, id: CartId, shipping: SetCartShipping) -> Result<Cart> {
        self.db.carts().set_shipping(id, shipping)
    }

    /// Get available shipping rates for the cart
    ///
    /// Returns available shipping options based on cart contents and shipping address.
    pub fn get_shipping_rates(&self, id: CartId) -> Result<Vec<ShippingRate>> {
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
    /// let cart = commerce.carts().set_payment(Uuid::new_v4().into(), SetCartPayment {
    ///     payment_method: "credit_card".into(),
    ///     payment_token: Some("tok_visa".into()),
    ///     ..Default::default()
    /// })?;
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn set_payment(&self, id: CartId, payment: SetCartPayment) -> Result<Cart> {
        self.db.carts().set_payment(id, payment)
    }

    // === Discount Operations ===

    /// Apply a coupon/discount code to the cart
    pub fn apply_discount(&self, id: CartId, coupon_code: &str) -> Result<Cart> {
        self.db.carts().apply_discount(id, coupon_code)
    }

    /// Remove the discount from the cart
    pub fn remove_discount(&self, id: CartId) -> Result<Cart> {
        self.db.carts().remove_discount(id)
    }

    // === Checkout Flow ===

    /// Mark the cart as ready for payment
    ///
    /// This validates that all required information is present (shipping address, etc.)
    pub fn mark_ready_for_payment(&self, id: CartId) -> Result<Cart> {
        self.db.carts().mark_ready_for_payment(id)
    }

    /// Begin the checkout process (payment pending)
    pub fn begin_checkout(&self, id: CartId) -> Result<Cart> {
        self.db.carts().begin_checkout(id)
    }

    /// Complete the checkout and create an order.
    ///
    /// The minted order is `Confirmed` with payment left `Pending` — record
    /// the payment through [`Payments`](crate::Payments), or use
    /// [`complete_settled_externally`](Self::complete_settled_externally) when
    /// settlement genuinely happened outside the engine.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use stateset_embedded::Commerce;
    /// use uuid::Uuid;
    ///
    /// let commerce = Commerce::new("./store.db")?;
    ///
    /// let result = commerce.carts().complete(Uuid::new_v4().into())?;
    /// println!("Order ID: {}", result.order_id);
    /// println!("Order Number: {}", result.order_number);
    /// println!("Total Charged: {} {}", result.total_charged, result.currency);
    /// # Ok::<(), stateset_embedded::CommerceError>(())
    /// ```
    pub fn complete(&self, id: CartId) -> Result<CheckoutResult> {
        let result = self.db.carts().complete(id)?;
        self.metrics.record_cart_checkout_completed(
            &result.cart_id.to_string(),
            &result.order_id.to_string(),
        );
        Ok(result)
    }

    /// Complete the checkout for a cart settled outside the engine (for
    /// example, by a protocol adapter or external PSP): the minted order is `Confirmed` + `Paid` with no
    /// engine-side payment record. This is an explicit opt-in — prefer
    /// [`complete`](Self::complete) plus a payment record when the engine
    /// processes the payment.
    pub fn complete_settled_externally(&self, id: CartId) -> Result<CheckoutResult> {
        let result = self.db.carts().complete_settled_externally(id)?;
        self.metrics.record_cart_checkout_completed(
            &result.cart_id.to_string(),
            &result.order_id.to_string(),
        );
        Ok(result)
    }

    /// Cancel the cart
    pub fn cancel(&self, id: CartId) -> Result<Cart> {
        self.db.carts().cancel(id)
    }

    /// Mark the cart as abandoned
    pub fn abandon(&self, id: CartId) -> Result<Cart> {
        self.db.carts().abandon(id)
    }

    /// Expire the cart
    pub fn expire(&self, id: CartId) -> Result<Cart> {
        self.db.carts().expire(id)
    }

    // === Inventory Operations ===

    /// Reserve inventory for cart items
    ///
    /// Creates inventory reservations for all items in the cart.
    /// Reservations typically expire after 15 minutes.
    pub fn reserve_inventory(&self, id: CartId) -> Result<Cart> {
        self.db.carts().reserve_inventory(id)
    }

    /// Release inventory reservations for the cart
    pub fn release_inventory(&self, id: CartId) -> Result<Cart> {
        self.db.carts().release_inventory(id)
    }

    // === Totals Operations ===

    /// Recalculate cart totals
    pub fn recalculate(&self, id: CartId) -> Result<Cart> {
        self.db.carts().recalculate(id)
    }

    /// Set the tax amount for the cart
    pub fn set_tax(&self, id: CartId, tax_amount: Decimal) -> Result<Cart> {
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

// The test helpers build a `Commerce::in_memory()`, which only exists with the
// `sqlite` feature — gate the module so `--all-targets` builds (e.g. the
// postgres-only compatibility matrix) don't try to compile it without sqlite.
#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use crate::Commerce;
    use rust_decimal_macros::dec;
    use stateset_core::CommerceError;

    fn carts() -> Carts {
        Commerce::in_memory().expect("in-memory commerce").carts()
    }

    #[test]
    fn create_accepts_valid_cart_with_items() {
        let carts = carts();
        let cart = carts
            .create(CreateCart {
                customer_email: Some("buyer@example.com".into()),
                items: Some(vec![AddCartItem {
                    sku: "SKU-OK".into(),
                    name: "Widget".into(),
                    quantity: 2,
                    unit_price: dec!(9.99),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .expect("valid cart should be created");
        assert!(!cart.cart_number.is_empty());
    }

    #[test]
    fn create_rejects_negative_price_item() {
        let carts = carts();
        let err = carts
            .create(CreateCart {
                items: Some(vec![AddCartItem {
                    sku: "SKU-NEG".into(),
                    name: "Widget".into(),
                    quantity: 1,
                    unit_price: dec!(-1.00),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .expect_err("negative price must be rejected");
        assert!(matches!(err, CommerceError::InvalidInput { .. }), "got {err:?}");
    }

    #[test]
    fn create_rejects_non_positive_quantity_item() {
        let carts = carts();
        let err = carts
            .create(CreateCart {
                items: Some(vec![AddCartItem {
                    sku: "SKU-ZERO".into(),
                    name: "Widget".into(),
                    quantity: 0,
                    unit_price: dec!(5.00),
                    ..Default::default()
                }]),
                ..Default::default()
            })
            .expect_err("zero quantity must be rejected");
        assert!(matches!(err, CommerceError::InvalidInput { .. }), "got {err:?}");
    }

    #[test]
    fn add_item_rejects_negative_quantity() {
        let carts = carts();
        let cart = carts.create(CreateCart::default()).expect("create");
        let err = carts
            .add_item(
                cart.id,
                AddCartItem {
                    sku: "SKU-NEG".into(),
                    name: "Widget".into(),
                    quantity: -1,
                    unit_price: dec!(5.00),
                    ..Default::default()
                },
            )
            .expect_err("negative quantity must be rejected");
        assert!(matches!(err, CommerceError::InvalidInput { .. }), "got {err:?}");
    }

    #[test]
    fn add_item_accepts_free_item() {
        let carts = carts();
        let cart = carts.create(CreateCart::default()).expect("create");
        let item = carts
            .add_item(
                cart.id,
                AddCartItem {
                    sku: "FREE".into(),
                    name: "Free Gift".into(),
                    quantity: 1,
                    unit_price: dec!(0),
                    ..Default::default()
                },
            )
            .expect("free item should be accepted");
        assert_eq!(item.unit_price, dec!(0));
    }
}
