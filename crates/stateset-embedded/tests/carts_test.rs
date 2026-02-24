//! Integration tests for Cart management

use rust_decimal_macros::dec;
use stateset_core::CartId;
use stateset_embedded::{
    AddCartItem, Cart, CartAddress, CartFilter, CartStatus, CheckoutResult, Commerce, CreateCart,
    CreateCouponCode, CreateCustomer, CreatePromotion, PromotionType, SetCartPayment,
    SetCartShipping, UpdateCart, UpdateCartItem,
};
use uuid::Uuid;

// ============================================================================
// Test Helpers
// ============================================================================

/// Helper to create a test customer
fn create_test_customer(commerce: &Commerce) -> stateset_embedded::CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("test-{}@example.com", Uuid::new_v4()),
            first_name: "Test".into(),
            last_name: "User".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer")
        .id
}

/// Helper to create a test cart with default items
fn create_test_cart(commerce: &Commerce) -> Cart {
    commerce
        .carts()
        .create(CreateCart {
            customer_email: Some(format!("cart-test-{}@example.com", Uuid::new_v4())),
            customer_name: Some("Cart Test User".into()),
            ..Default::default()
        })
        .expect("Failed to create cart")
}

/// Helper to create a test cart for a specific customer
fn create_test_cart_for_customer(
    commerce: &Commerce,
    customer_id: stateset_embedded::CustomerId,
) -> Cart {
    commerce
        .carts()
        .create(CreateCart { customer_id: Some(customer_id), ..Default::default() })
        .expect("Failed to create cart for customer")
}

/// Helper to create a test shipping address
fn create_test_address() -> CartAddress {
    CartAddress {
        first_name: "John".into(),
        last_name: "Doe".into(),
        company: Some("Acme Corp".into()),
        line1: "123 Main St".into(),
        line2: Some("Apt 4B".into()),
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: Some("555-1234".into()),
        email: Some("john.doe@example.com".into()),
    }
}

/// Helper to add test item to cart
fn add_test_item(commerce: &Commerce, cart_id: CartId) -> stateset_embedded::CartItem {
    commerce
        .carts()
        .add_item(
            cart_id,
            AddCartItem {
                sku: "TEST-SKU-001".into(),
                name: "Test Product".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item to cart")
}

// ============================================================================
// Basic Cart Creation Tests
// ============================================================================

#[test]
fn test_create_cart() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("test@example.com".into()),
            customer_name: Some("Test User".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    assert!(!cart.id.is_nil());
    assert!(!cart.cart_number.is_empty());
    assert!(cart.cart_number.starts_with("CART-"));
    assert_eq!(cart.status, CartStatus::Active);
    assert_eq!(cart.customer_email, Some("test@example.com".into()));
    assert_eq!(cart.customer_name, Some("Test User".into()));
}

#[test]
fn test_create_cart_for_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    let cart = commerce
        .carts()
        .create(CreateCart { customer_id: Some(customer_id), ..Default::default() })
        .expect("Failed to create cart");

    assert!(!cart.id.is_nil());
    assert_eq!(cart.customer_id, Some(customer_id));
}

#[test]
fn test_create_cart_with_currency() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("test@example.com".into()),
            currency: Some("EUR".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    assert_eq!(cart.currency, "EUR");
}

#[test]
fn test_create_cart_default_currency() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("test@example.com".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    // Default currency should be USD
    assert_eq!(cart.currency, "USD");
}

#[test]
fn test_create_cart_with_expiry() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("test@example.com".into()),
            expires_in_minutes: Some(60),
            ..Default::default()
        })
        .expect("Failed to create cart");

    assert!(cart.expires_at.is_some());
}

#[test]
fn test_create_cart_with_initial_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("test@example.com".into()),
            items: Some(vec![
                AddCartItem {
                    sku: "SKU-001".into(),
                    name: "Widget".into(),
                    quantity: 2,
                    unit_price: dec!(29.99),
                    ..Default::default()
                },
                AddCartItem {
                    sku: "SKU-002".into(),
                    name: "Gadget".into(),
                    quantity: 1,
                    unit_price: dec!(49.99),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        })
        .expect("Failed to create cart");

    assert_eq!(cart.items.len(), 2);
}

// ============================================================================
// Cart Retrieval Tests
// ============================================================================

#[test]
fn test_get_cart_by_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let created = create_test_cart(&commerce);

    let retrieved =
        commerce.carts().get(created.id).expect("Failed to get cart").expect("Cart not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.cart_number, created.cart_number);
}

#[test]
fn test_get_cart_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result =
        commerce.carts().get(Uuid::new_v4().into()).expect("Should not error for missing cart");

    assert!(result.is_none());
}

#[test]
fn test_get_cart_by_number() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let created = create_test_cart(&commerce);

    let retrieved = commerce
        .carts()
        .get_by_number(&created.cart_number)
        .expect("Failed to get cart by number")
        .expect("Cart not found");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.cart_number, created.cart_number);
}

#[test]
fn test_get_cart_by_number_not_found() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let result = commerce
        .carts()
        .get_by_number("CART-NONEXISTENT-123")
        .expect("Should not error for missing cart");

    assert!(result.is_none());
}

// ============================================================================
// Cart Item Operations Tests
// ============================================================================

#[test]
fn test_add_item_to_cart() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let item = commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-001".into(),
                name: "Test Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                description: Some("A test widget".into()),
                image_url: Some("https://example.com/widget.jpg".into()),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    assert!(!item.id.is_nil());
    assert_eq!(item.cart_id, cart.id);
    assert_eq!(item.sku, "SKU-001");
    assert_eq!(item.name, "Test Widget");
    assert_eq!(item.quantity, 2);
    assert_eq!(item.unit_price, dec!(29.99));
}

#[test]
fn test_add_item_with_product_id() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    let product_id: stateset_embedded::ProductId = Uuid::new_v4().into();

    let item = commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(product_id),
                sku: "SKU-001".into(),
                name: "Test Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    assert_eq!(item.product_id, Some(product_id));
}

#[test]
fn test_add_item_with_original_price() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let item = commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-001".into(),
                name: "Sale Widget".into(),
                quantity: 1,
                unit_price: dec!(24.99),
                original_price: Some(dec!(29.99)),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    assert_eq!(item.unit_price, dec!(24.99));
    assert_eq!(item.original_price, Some(dec!(29.99)));
}

#[test]
fn test_update_cart_item_quantity() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    let item = add_test_item(&commerce, cart.id);

    let updated = commerce
        .carts()
        .update_item(item.id, UpdateCartItem { quantity: Some(5), ..Default::default() })
        .expect("Failed to update item");

    assert_eq!(updated.quantity, 5);
}

#[test]
fn test_remove_item_from_cart() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    let item = add_test_item(&commerce, cart.id);

    commerce.carts().remove_item(item.id).expect("Failed to remove item");

    let items = commerce.carts().get_items(cart.id).expect("Failed to get items");
    assert_eq!(items.len(), 0);
}

#[test]
fn test_get_cart_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    // Add multiple items
    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-002".into(),
                name: "Gadget".into(),
                quantity: 1,
                unit_price: dec!(49.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    let items = commerce.carts().get_items(cart.id).expect("Failed to get items");

    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|i| i.sku == "SKU-001"));
    assert!(items.iter().any(|i| i.sku == "SKU-002"));
}

#[test]
fn test_empty_cart() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    // Add items
    add_test_item(&commerce, cart.id);
    add_test_item(&commerce, cart.id);

    // Clear all items
    commerce.carts().clear_items(cart.id).expect("Failed to clear items");

    let items = commerce.carts().get_items(cart.id).expect("Failed to get items");
    assert_eq!(items.len(), 0);
}

// ============================================================================
// Cart Total Calculation Tests
// ============================================================================

#[test]
fn test_cart_total_calculation() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    // Add items
    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-001".into(),
                name: "Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "SKU-002".into(),
                name: "Gadget".into(),
                quantity: 1,
                unit_price: dec!(49.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    // Recalculate totals
    let updated = commerce.carts().recalculate(cart.id).expect("Failed to recalculate cart");

    // Calculate expected subtotal: (2 * 29.99) + (1 * 49.99) = 59.98 + 49.99 = 109.97
    let expected_subtotal = dec!(109.97);
    assert_eq!(updated.subtotal, expected_subtotal);
}

#[test]
fn test_cart_set_tax() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    let updated = commerce.carts().set_tax(cart.id, dec!(5.40)).expect("Failed to set tax");

    assert_eq!(updated.tax_amount, dec!(5.40));
}

// ============================================================================
// Cart Address Tests
// ============================================================================

#[test]
fn test_cart_with_shipping_address() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    let address = create_test_address();

    let updated = commerce
        .carts()
        .set_shipping_address(cart.id, address)
        .expect("Failed to set shipping address");

    assert!(updated.shipping_address.is_some());
    let ship_addr = updated.shipping_address.unwrap();
    assert_eq!(ship_addr.first_name, "John");
    assert_eq!(ship_addr.last_name, "Doe");
    assert_eq!(ship_addr.line1, "123 Main St");
    assert_eq!(ship_addr.city, "San Francisco");
    assert_eq!(ship_addr.postal_code, "94102");
}

#[test]
fn test_cart_with_billing_address() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let billing_address = CartAddress {
        first_name: "Jane".into(),
        last_name: "Doe".into(),
        company: None,
        line1: "456 Oak Ave".into(),
        line2: None,
        city: "Los Angeles".into(),
        state: Some("CA".into()),
        postal_code: "90001".into(),
        country: "US".into(),
        phone: None,
        email: None,
    };

    let updated = commerce
        .carts()
        .set_billing_address(cart.id, billing_address)
        .expect("Failed to set billing address");

    assert!(updated.billing_address.is_some());
    let bill_addr = updated.billing_address.unwrap();
    assert_eq!(bill_addr.first_name, "Jane");
    assert_eq!(bill_addr.city, "Los Angeles");
}

#[test]
fn test_cart_with_both_addresses() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let shipping_address = create_test_address();
    let billing_address = CartAddress {
        first_name: "Jane".into(),
        last_name: "Doe".into(),
        company: None,
        line1: "456 Oak Ave".into(),
        line2: None,
        city: "Los Angeles".into(),
        state: Some("CA".into()),
        postal_code: "90001".into(),
        country: "US".into(),
        phone: None,
        email: None,
    };

    commerce
        .carts()
        .set_shipping_address(cart.id, shipping_address)
        .expect("Failed to set shipping address");

    let updated = commerce
        .carts()
        .set_billing_address(cart.id, billing_address)
        .expect("Failed to set billing address");

    assert!(updated.shipping_address.is_some());
    assert!(updated.billing_address.is_some());
}

// ============================================================================
// Cart Shipping Tests
// ============================================================================

#[test]
fn test_cart_set_shipping() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    let updated = commerce
        .carts()
        .set_shipping(
            cart.id,
            SetCartShipping {
                shipping_address: create_test_address(),
                shipping_method: Some("standard".into()),
                shipping_carrier: Some("ups".into()),
                shipping_amount: Some(dec!(9.99)),
            },
        )
        .expect("Failed to set shipping");

    assert_eq!(updated.shipping_method, Some("standard".into()));
    assert_eq!(updated.shipping_amount, dec!(9.99));
}

#[test]
fn test_get_shipping_rates() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    // Set shipping address first
    commerce
        .carts()
        .set_shipping_address(cart.id, create_test_address())
        .expect("Failed to set shipping address");

    let rates = commerce.carts().get_shipping_rates(cart.id).expect("Failed to get shipping rates");

    // Should return available rates (implementation dependent)
    assert!(!rates.is_empty());
}

// ============================================================================
// Cart Payment Tests
// ============================================================================

#[test]
fn test_cart_set_payment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    let updated = commerce
        .carts()
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_visa_4242".into()),
                ..Default::default()
            },
        )
        .expect("Failed to set payment");

    assert_eq!(updated.payment_method, Some("credit_card".into()));
    assert_eq!(updated.payment_token, Some("tok_visa_4242".into()));
}

// ============================================================================
// Cart Discount Tests
// ============================================================================

#[test]
fn test_cart_apply_discount() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    // Create and activate a promotion with a coupon code
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "10% Off".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.10)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    commerce.promotions().activate(promotion.id).expect("Failed to activate promotion");

    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            code: "SAVE10".into(),
            promotion_id: promotion.id,
            usage_limit: Some(100),
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon");

    let updated =
        commerce.carts().apply_discount(cart.id, "SAVE10").expect("Failed to apply discount");

    assert_eq!(updated.coupon_code, Some("SAVE10".into()));
}

#[test]
fn test_cart_remove_discount() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    // Create and activate a promotion with a coupon code
    let promotion = commerce
        .promotions()
        .create(CreatePromotion {
            name: "10% Off".into(),
            promotion_type: PromotionType::PercentageOff,
            percentage_off: Some(dec!(0.10)),
            ..Default::default()
        })
        .expect("Failed to create promotion");

    commerce.promotions().activate(promotion.id).expect("Failed to activate promotion");

    commerce
        .promotions()
        .create_coupon(CreateCouponCode {
            code: "SAVE10".into(),
            promotion_id: promotion.id,
            usage_limit: Some(100),
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .expect("Failed to create coupon");

    // Apply discount first
    commerce.carts().apply_discount(cart.id, "SAVE10").expect("Failed to apply discount");

    // Remove discount
    let updated = commerce.carts().remove_discount(cart.id).expect("Failed to remove discount");

    assert!(updated.coupon_code.is_none());
}

// ============================================================================
// Cart Update Tests
// ============================================================================

#[test]
fn test_update_cart() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let updated = commerce
        .carts()
        .update(
            cart.id,
            UpdateCart { notes: Some("Updated notes for the cart".into()), ..Default::default() },
        )
        .expect("Failed to update cart");

    assert_eq!(updated.notes, Some("Updated notes for the cart".into()));
}

#[test]
fn test_update_cart_customer_email() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let updated = commerce
        .carts()
        .update(
            cart.id,
            UpdateCart {
                customer_email: Some("newemail@example.com".into()),
                ..Default::default()
            },
        )
        .expect("Failed to update cart");

    assert_eq!(updated.customer_email, Some("newemail@example.com".into()));
}

// ============================================================================
// Cart Listing Tests
// ============================================================================

#[test]
fn test_list_customer_carts() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create multiple carts for this customer
    for _ in 0..3 {
        create_test_cart_for_customer(&commerce, customer_id);
    }

    let carts = commerce.carts().for_customer(customer_id).expect("Failed to list customer carts");

    assert_eq!(carts.len(), 3);
    assert!(carts.iter().all(|c| c.customer_id == Some(customer_id)));
}

#[test]
fn test_list_carts() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create multiple carts
    for _ in 0..5 {
        create_test_cart(&commerce);
    }

    let carts = commerce.carts().list(CartFilter::default()).expect("Failed to list carts");

    assert!(carts.len() >= 5);
}

#[test]
fn test_list_carts_by_status() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create some carts
    let cart1 = create_test_cart(&commerce);
    let cart2 = create_test_cart(&commerce);
    let _cart3 = create_test_cart(&commerce);

    // Cancel one cart
    commerce.carts().cancel(cart1.id).expect("Failed to cancel cart");

    // Abandon another
    commerce.carts().abandon(cart2.id).expect("Failed to abandon cart");

    // List active carts only
    let active_carts = commerce
        .carts()
        .list(CartFilter { status: Some(CartStatus::Active), ..Default::default() })
        .expect("Failed to list carts");

    assert!(active_carts.iter().all(|c| c.status == CartStatus::Active));
}

#[test]
fn test_list_carts_with_limit() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create multiple carts
    for _ in 0..10 {
        create_test_cart(&commerce);
    }

    let carts = commerce
        .carts()
        .list(CartFilter { limit: Some(5), ..Default::default() })
        .expect("Failed to list carts");

    assert_eq!(carts.len(), 5);
}

#[test]
fn test_count_carts() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create multiple carts
    for _ in 0..7 {
        create_test_cart(&commerce);
    }

    let count = commerce.carts().count(CartFilter::default()).expect("Failed to count carts");

    assert!(count >= 7);
}

// ============================================================================
// Cart Checkout Flow Tests
// ============================================================================

#[test]
fn test_cart_mark_ready_for_payment() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    // Set required addresses
    commerce
        .carts()
        .set_shipping_address(cart.id, create_test_address())
        .expect("Failed to set shipping address");

    let updated =
        commerce.carts().mark_ready_for_payment(cart.id).expect("Failed to mark ready for payment");

    assert_eq!(updated.status, CartStatus::ReadyForPayment);
}

#[test]
fn test_cart_begin_checkout() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    let updated = commerce.carts().begin_checkout(cart.id).expect("Failed to begin checkout");

    assert_eq!(updated.status, CartStatus::PaymentPending);
}

#[test]
fn test_cart_checkout_creates_order() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let product_id: stateset_embedded::ProductId = Uuid::new_v4().into();

    // Create cart for customer
    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer_id),
            customer_email: Some("checkout@example.com".into()),
            customer_name: Some("Checkout User".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    // Add items
    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(product_id),
                sku: "SKU-001".into(),
                name: "Checkout Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    // Set shipping address
    commerce
        .carts()
        .set_shipping_address(cart.id, create_test_address())
        .expect("Failed to set shipping address");

    // Set payment
    commerce
        .carts()
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                ..Default::default()
            },
        )
        .expect("Failed to set payment");

    // Complete checkout
    let result: CheckoutResult =
        commerce.carts().complete(cart.id).expect("Failed to complete checkout");

    assert!(!result.order_id.is_nil());
    assert!(!result.order_number.is_empty());
    assert!(result.order_number.starts_with("ORD-"));
    assert!(result.total_charged > dec!(0));

    // Verify cart is now completed
    let updated_cart =
        commerce.carts().get(cart.id).expect("Failed to get cart").expect("Cart not found");
    assert_eq!(updated_cart.status, CartStatus::Completed);

    // Verify order exists
    let order = commerce
        .orders()
        .get(result.order_id)
        .expect("Failed to get order")
        .expect("Order not found");
    assert_eq!(order.customer_id, customer_id);
}

#[test]
fn test_cart_checkout_is_idempotent() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);
    let product_id: stateset_embedded::ProductId = Uuid::new_v4().into();

    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer_id),
            customer_email: Some("checkout-idem@example.com".into()),
            customer_name: Some("Checkout Idempotent".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(product_id),
                sku: "SKU-IDEM-001".into(),
                name: "Checkout Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    commerce
        .carts()
        .set_shipping_address(cart.id, create_test_address())
        .expect("Failed to set shipping address");

    commerce
        .carts()
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                ..Default::default()
            },
        )
        .expect("Failed to set payment");

    let first = commerce.carts().complete(cart.id).expect("Failed to complete checkout (first)");
    let second = commerce.carts().complete(cart.id).expect("Failed to complete checkout (second)");

    assert_eq!(second.order_id, first.order_id);
    assert_eq!(second.order_number, first.order_number);
}

// ============================================================================
// Cart Status Tests
// ============================================================================

#[test]
fn test_cart_cancel() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let cancelled = commerce.carts().cancel(cart.id).expect("Failed to cancel cart");

    assert_eq!(cancelled.status, CartStatus::Cancelled);
}

#[test]
fn test_cart_abandon() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let abandoned = commerce.carts().abandon(cart.id).expect("Failed to abandon cart");

    assert_eq!(abandoned.status, CartStatus::Abandoned);
}

#[test]
fn test_cart_expire() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let expired = commerce.carts().expire(cart.id).expect("Failed to expire cart");

    assert_eq!(expired.status, CartStatus::Expired);
}

#[test]
fn test_get_abandoned_carts() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create and abandon some carts
    let cart1 = create_test_cart(&commerce);
    let cart2 = create_test_cart(&commerce);
    let _cart3 = create_test_cart(&commerce); // Keep active

    commerce.carts().abandon(cart1.id).expect("Failed to abandon");
    commerce.carts().abandon(cart2.id).expect("Failed to abandon");

    let abandoned = commerce.carts().get_abandoned().expect("Failed to get abandoned carts");

    assert!(abandoned.len() >= 2);
    assert!(abandoned.iter().all(|c| c.status == CartStatus::Abandoned));
}

#[test]
fn test_get_expired_carts() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create and expire some carts
    let cart1 = create_test_cart(&commerce);
    let _cart2 = create_test_cart(&commerce); // Keep active

    commerce.carts().expire(cart1.id).expect("Failed to expire");

    let expired = commerce.carts().get_expired().expect("Failed to get expired carts");

    assert!(!expired.is_empty());
    assert!(expired.iter().all(|c| c.status == CartStatus::Expired));
}

// ============================================================================
// Cart Inventory Tests
// ============================================================================

#[test]
fn test_cart_reserve_inventory() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    let updated = commerce.carts().reserve_inventory(cart.id).expect("Failed to reserve inventory");

    assert!(updated.inventory_reserved);
}

#[test]
fn test_cart_release_inventory() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);
    add_test_item(&commerce, cart.id);

    // Reserve first
    commerce.carts().reserve_inventory(cart.id).expect("Failed to reserve inventory");

    // Release
    let updated = commerce.carts().release_inventory(cart.id).expect("Failed to release inventory");

    assert!(!updated.inventory_reserved);
}

// ============================================================================
// Cart Delete Test
// ============================================================================

#[test]
fn test_delete_cart() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    commerce.carts().delete(cart.id).expect("Failed to delete cart");

    let result = commerce.carts().get(cart.id).expect("Should not error for deleted cart");
    assert!(result.is_none());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_cart_number_uniqueness() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    let cart1 = create_test_cart(&commerce);
    let cart2 = create_test_cart(&commerce);
    let cart3 = create_test_cart(&commerce);

    // All cart numbers should be unique
    assert_ne!(cart1.cart_number, cart2.cart_number);
    assert_ne!(cart2.cart_number, cart3.cart_number);
    assert_ne!(cart1.cart_number, cart3.cart_number);
}

#[test]
fn test_cart_timestamps() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    // created_at and updated_at should be set
    assert!(cart.created_at <= chrono::Utc::now());
    assert!(cart.updated_at <= chrono::Utc::now());
}

#[test]
fn test_cart_with_large_quantities() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let item = commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "BULK-SKU".into(),
                name: "Bulk Product".into(),
                quantity: 10000,
                unit_price: dec!(0.01),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    assert_eq!(item.quantity, 10000);
}

#[test]
fn test_cart_with_high_value_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let item = commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "LUXURY-SKU".into(),
                name: "Luxury Item".into(),
                quantity: 1,
                unit_price: dec!(99999.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    assert_eq!(item.unit_price, dec!(99999.99));
}

#[test]
fn test_cart_with_many_items() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    // Add 50 different items
    for i in 0..50 {
        commerce
            .carts()
            .add_item(
                cart.id,
                AddCartItem {
                    sku: format!("SKU-{:03}", i),
                    name: format!("Product {}", i),
                    quantity: 1,
                    unit_price: dec!(9.99),
                    ..Default::default()
                },
            )
            .expect("Failed to add item");
    }

    let items = commerce.carts().get_items(cart.id).expect("Failed to get items");

    assert_eq!(items.len(), 50);
}

#[test]
fn test_cart_item_with_variant_options() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let cart = create_test_cart(&commerce);

    let item = commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "TSHIRT-BLU-L".into(),
                name: "T-Shirt".into(),
                quantity: 1,
                unit_price: dec!(24.99),
                variant_id: Some(Uuid::new_v4()),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    assert!(item.variant_id.is_some());
}

#[test]
fn test_multiple_carts_for_same_customer() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // Create multiple active carts for the same customer
    let cart1 = create_test_cart_for_customer(&commerce, customer_id);
    let cart2 = create_test_cart_for_customer(&commerce, customer_id);

    assert_ne!(cart1.id, cart2.id);
    assert_eq!(cart1.customer_id, cart2.customer_id);
}

#[test]
fn test_guest_checkout_cart() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");

    // Create a cart without customer_id (guest checkout)
    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some("guest@example.com".into()),
            customer_name: Some("Guest User".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");

    assert!(cart.customer_id.is_none());
    assert_eq!(cart.customer_email, Some("guest@example.com".into()));
}

// ============================================================================
// Full Checkout Flow Test
// ============================================================================

#[test]
fn test_full_checkout_flow() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let customer_id = create_test_customer(&commerce);

    // 1. Create cart
    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer_id),
            customer_email: Some("fullflow@example.com".into()),
            customer_name: Some("Full Flow User".into()),
            ..Default::default()
        })
        .expect("Failed to create cart");
    let product_id_a: stateset_embedded::ProductId = Uuid::new_v4().into();
    let product_id_b: stateset_embedded::ProductId = Uuid::new_v4().into();

    assert_eq!(cart.status, CartStatus::Active);

    // 2. Add items
    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(product_id_a),
                sku: "SKU-001".into(),
                name: "Widget A".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(product_id_b),
                sku: "SKU-002".into(),
                name: "Widget B".into(),
                quantity: 1,
                unit_price: dec!(49.99),
                ..Default::default()
            },
        )
        .expect("Failed to add item");

    // 3. Set shipping address
    commerce
        .carts()
        .set_shipping_address(cart.id, create_test_address())
        .expect("Failed to set shipping address");

    // 4. Set billing address
    commerce
        .carts()
        .set_billing_address(
            cart.id,
            CartAddress {
                first_name: "John".into(),
                last_name: "Doe".into(),
                company: None,
                line1: "456 Billing St".into(),
                line2: None,
                city: "San Francisco".into(),
                state: Some("CA".into()),
                postal_code: "94102".into(),
                country: "US".into(),
                phone: None,
                email: None,
            },
        )
        .expect("Failed to set billing address");

    // 5. Set shipping method
    commerce
        .carts()
        .set_shipping(
            cart.id,
            SetCartShipping {
                shipping_address: create_test_address(),
                shipping_method: Some("standard".into()),
                shipping_carrier: Some("fedex".into()),
                shipping_amount: Some(dec!(9.99)),
            },
        )
        .expect("Failed to set shipping");

    // 6. Set tax
    commerce.carts().set_tax(cart.id, dec!(8.52)).expect("Failed to set tax");

    // 7. Recalculate totals
    let cart = commerce.carts().recalculate(cart.id).expect("Failed to recalculate");

    // Expected: (2 * 29.99) + (1 * 49.99) + 9.99 (shipping) + 8.52 (tax) = 128.48
    assert!(cart.grand_total >= dec!(100));

    // 8. Set payment method
    commerce
        .carts()
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_visa".into()),
                ..Default::default()
            },
        )
        .expect("Failed to set payment");

    // 9. Mark ready for payment
    let cart = commerce.carts().mark_ready_for_payment(cart.id).expect("Failed to mark ready");

    assert_eq!(cart.status, CartStatus::ReadyForPayment);

    // 10. Begin checkout
    let cart = commerce.carts().begin_checkout(cart.id).expect("Failed to begin checkout");

    assert_eq!(cart.status, CartStatus::PaymentPending);

    // 11. Complete checkout
    let result = commerce.carts().complete(cart.id).expect("Failed to complete checkout");

    assert!(!result.order_id.is_nil());
    assert!(!result.order_number.is_empty());
    assert!(result.total_charged > dec!(0));

    // 12. Verify final cart status
    let final_cart =
        commerce.carts().get(cart.id).expect("Failed to get cart").expect("Cart not found");

    assert_eq!(final_cart.status, CartStatus::Completed);
    assert!(final_cart.order_id.is_some());
    assert_eq!(final_cart.order_id.unwrap(), result.order_id);
}
