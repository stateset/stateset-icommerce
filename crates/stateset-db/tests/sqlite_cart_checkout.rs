#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CartRepository, CreateCart, CreateCustomer, CreateOrder,
    CreateOrderItem, CustomerRepository, OrderRepository, OrderStatus, PaymentStatus, ProductId,
    SetCartPayment,
};
use stateset_db::SqliteDatabase;

fn test_address() -> CartAddress {
    CartAddress {
        first_name: "John".into(),
        last_name: "Doe".into(),
        company: None,
        line1: "123 Main St".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: Some("555-1234".into()),
        email: Some("john.doe@example.com".into()),
    }
}

#[test]
fn sqlite_cart_checkout_reuses_existing_order_by_cart_id() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let carts = db.carts();
    let orders = db.orders();
    let customers = db.customers();

    let product_id = ProductId::new();

    let customer = customers
        .create(CreateCustomer {
            email: "checkout-reuse@example.com".into(),
            first_name: "Checkout".into(),
            last_name: "Reuse".into(),
            ..Default::default()
        })
        .expect("create customer");

    let cart = carts
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some("checkout-reuse@example.com".into()),
            customer_name: Some("Checkout Reuse".into()),
            ..Default::default()
        })
        .expect("create cart");

    carts
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(product_id),
                sku: "SKU-REUSE-001".into(),
                name: "Retry-Safe Widget".into(),
                quantity: 2,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("add cart item");

    carts.set_shipping_address(cart.id, test_address()).expect("set shipping address");

    carts
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                ..Default::default()
            },
        )
        .expect("set payment");

    // Simulate a partial failure after order creation: create the order for the cart, but do not
    // mark the cart completed.
    let cart_for_order = carts.get(cart.id).expect("get cart").expect("cart exists");
    let order_items: Vec<CreateOrderItem> = cart_for_order
        .items
        .iter()
        .map(|item| CreateOrderItem {
            product_id: item.product_id.expect("product_id set on cart item"),
            variant_id: item.variant_id,
            sku: item.sku.clone(),
            name: item.name.clone(),
            quantity: item.quantity,
            unit_price: item.unit_price,
            discount: Some(item.discount_amount),
            tax_amount: Some(item.tax_amount),
        })
        .collect();

    let order = orders
        .create_from_cart(
            cart.id.into(),
            CreateOrder {
                customer_id: customer.id,
                items: order_items,
                currency: Some(cart_for_order.currency),
                shipping_address: cart_for_order.shipping_address.clone().map(Into::into),
                billing_address: None,
                notes: cart_for_order.notes.clone(),
                payment_method: cart_for_order.payment_method.clone(),
                shipping_method: cart_for_order.shipping_method,
            },
        )
        .expect("create order for cart");

    // Retry checkout: should reuse the same order (by cart_id) instead of creating a new one.
    let checkout = carts.complete(cart.id).expect("complete checkout");
    assert_eq!(checkout.order_id, order.id);
    assert_eq!(checkout.order_number, order.order_number);

    // Checkout confirms and marks the order paid.
    let updated_order = orders.get(order.id).expect("get order").expect("order exists");
    assert_eq!(updated_order.status, OrderStatus::Confirmed);
    assert_eq!(updated_order.payment_status, PaymentStatus::Paid);
}

#[test]
fn sqlite_cart_checkout_rolls_back_order_when_cart_finalize_fails() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let carts = db.carts();
    let orders = db.orders();
    let customers = db.customers();

    let customer = customers
        .create(CreateCustomer {
            email: "checkout-rollback@example.com".into(),
            first_name: "Checkout".into(),
            last_name: "Rollback".into(),
            ..Default::default()
        })
        .expect("create customer");

    let cart = carts
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some("checkout-rollback@example.com".into()),
            customer_name: Some("Checkout Rollback".into()),
            ..Default::default()
        })
        .expect("create cart");

    carts
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(ProductId::new()),
                sku: "SKU-ROLLBACK-001".into(),
                name: "Rollback Widget".into(),
                quantity: 1,
                unit_price: dec!(29.99),
                ..Default::default()
            },
        )
        .expect("add cart item");

    carts.set_shipping_address(cart.id, test_address()).expect("set shipping address");
    carts
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_rollback".into()),
                ..Default::default()
            },
        )
        .expect("set payment");

    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute_batch(
            r#"
            CREATE TRIGGER abort_cart_completion
            BEFORE UPDATE OF status ON carts
            WHEN NEW.status = 'completed'
            BEGIN
                SELECT RAISE(ABORT, 'cart completion blocked');
            END;
            "#,
        )
        .expect("create abort trigger");
    }

    let err = carts.complete(cart.id).expect_err("checkout should fail when cart update aborts");
    assert!(err.to_string().contains("cart completion blocked"), "unexpected error: {err}");

    assert!(
        orders.get_by_cart_id(cart.id.into()).expect("query order by cart").is_none(),
        "order insert should have rolled back with cart finalization failure"
    );

    let cart = carts.get(cart.id).expect("reload cart").expect("cart exists");
    assert_ne!(cart.status, stateset_core::CartStatus::Completed);
    assert!(cart.order_id.is_none());
}
