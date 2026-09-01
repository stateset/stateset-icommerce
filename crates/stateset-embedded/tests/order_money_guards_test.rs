#![cfg(feature = "sqlite")]

//! Order money guards, against the SQLite backend.
//!
//! Two defects are pinned here:
//!
//! 1. Checkout minted an order whose total was only the sum of its line
//!    amounts while charging the cart's grand total (subtotal + tax +
//!    shipping - discount). The order had nowhere to record tax, shipping or
//!    a discount, so a legitimate capture of what the customer actually paid
//!    was rejected by the over-capture guard as exceeding the order total.
//! 2. Batch order creation inserted orders and their items directly, with no
//!    stock check, no reservation and no backorder — so a batch could sell
//!    stock that did not exist and shipping later found nothing reserved.

use rust_decimal_macros::dec;
use stateset_embedded::{
    AddCartItem, CartAddress, Commerce, CreateCart, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, CreatePayment, SetCartShipping, StockPolicy,
};
use uuid::Uuid;

fn new_commerce() -> Commerce {
    Commerce::new(":memory:").expect("create in-memory Commerce")
}

fn test_address() -> CartAddress {
    CartAddress {
        first_name: "Test".into(),
        last_name: "Buyer".into(),
        line1: "123 Main St".into(),
        city: "Anytown".into(),
        state: Some("CA".into()),
        postal_code: "94000".into(),
        country: "US".into(),
        ..Default::default()
    }
}

/// A cart holding one $100 line, shipped for $10, ready to check out.
fn cart_with_shipping(commerce: &Commerce) -> stateset_embedded::Cart {
    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_email: Some(format!("money-{}@example.com", Uuid::new_v4())),
            customer_name: Some("Money Tester".into()),
            ..Default::default()
        })
        .expect("create cart");

    commerce
        .carts()
        .add_item(
            cart.id,
            AddCartItem {
                sku: "MONEY-SKU".into(),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(100.00),
                ..Default::default()
            },
        )
        .expect("add item");

    commerce
        .carts()
        .set_shipping(
            cart.id,
            SetCartShipping {
                shipping_address: test_address(),
                shipping_method: Some("standard".into()),
                shipping_carrier: Some("ups".into()),
                shipping_amount: Some(dec!(10.00)),
            },
        )
        .expect("set shipping")
}

#[test]
fn checkout_carries_cart_shipping_onto_the_order_total() {
    let commerce = new_commerce();
    let cart = cart_with_shipping(&commerce);

    // The cart charges merchandise plus shipping.
    assert_eq!(cart.subtotal, dec!(100.00));
    assert_eq!(cart.shipping_amount, dec!(10.00));
    assert_eq!(cart.grand_total, dec!(110.00));

    let result = commerce.carts().complete(cart.id).expect("complete checkout");
    assert_eq!(result.total_charged, dec!(110.00), "the customer is charged the grand total");

    let order = commerce.orders().get(result.order_id).expect("get order").expect("order exists");

    // The order must record what the customer is charged — not just the line
    // sum, which is what made a legitimate capture look like an over-capture.
    assert_eq!(
        order.total_amount,
        dec!(110.00),
        "order total must equal the amount charged, not the line sum"
    );
    assert_eq!(order.shipping_amount, dec!(10.00), "shipping must be recorded on the order");
    assert_eq!(order.tax_amount, dec!(0.00));
    assert_eq!(order.discount_amount, dec!(0.00));
}

#[test]
fn capturing_the_full_charged_amount_is_accepted_after_checkout() {
    // The end-to-end money scenario: before the fix the order totalled $100
    // while $110 was charged, so recording that capture was refused with
    // `commerce.capture.exceeds_order_total` and the payment could not be
    // booked at all.
    let commerce = new_commerce();
    let cart = cart_with_shipping(&commerce);
    let result = commerce.carts().complete(cart.id).expect("complete checkout");

    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(result.order_id),
            amount: result.total_charged,
            ..Default::default()
        })
        .expect("capturing the charged amount must be accepted");
    assert_eq!(payment.amount, dec!(110.00));

    // The over-capture guard still bites beyond the real total.
    let err = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(result.order_id),
            amount: dec!(0.01),
            ..Default::default()
        })
        .expect_err("a cent beyond the order total must still be refused");
    assert_eq!(err.invariant_code(), Some("commerce.capture.exceeds_order_total"), "{err:?}");
}

#[test]
fn batch_order_create_respects_the_stock_policy() {
    let commerce = new_commerce();
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "BATCH-SKU".into(),
            name: "Scarce Widget".into(),
            ..Default::default()
        })
        .expect("create item");
    commerce.inventory().adjust("BATCH-SKU", dec!(5), "seed").expect("seed stock");

    let customer_id = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("batch-{}@example.com", Uuid::new_v4()),
            first_name: "Batch".into(),
            last_name: "Buyer".into(),
            ..Default::default()
        })
        .expect("create customer")
        .id;

    let order_for = |qty: i32, policy: StockPolicy| CreateOrder {
        customer_id,
        items: vec![CreateOrderItem {
            product_id: stateset_embedded::ProductId::new(),
            sku: "BATCH-SKU".into(),
            name: "Scarce Widget".into(),
            quantity: qty,
            unit_price: dec!(10.00),
            ..Default::default()
        }],
        stock_policy: policy,
        ..Default::default()
    };

    // Batch creation used to insert straight through with no stock check at
    // all: 100 units against 5 in stock succeeded, and shipping later found
    // zero reservations. Under a reject policy the whole batch must fail.
    let err = commerce
        .database()
        .orders()
        .create_batch_atomic(vec![order_for(100, StockPolicy::RejectIfInsufficient)])
        .expect_err("batch create must honour the stock policy");
    assert!(
        matches!(err, stateset_embedded::CommerceError::InsufficientStock { .. }),
        "expected InsufficientStock, got {err:?}"
    );

    // The rejected batch must leave nothing behind.
    assert!(
        commerce.orders().list(Default::default()).expect("list").is_empty(),
        "a rejected batch must not persist any order"
    );
    assert_eq!(
        commerce.inventory().get_stock("BATCH-SKU").expect("stock").expect("item").total_available,
        dec!(5),
        "a rejected batch must not reserve anything"
    );

    // Within stock, the batch succeeds AND actually reserves.
    let orders = commerce
        .database()
        .orders()
        .create_batch_atomic(vec![order_for(3, StockPolicy::RejectIfInsufficient)])
        .expect("batch within stock must succeed");
    assert_eq!(orders.len(), 1);
    assert_eq!(
        commerce.inventory().get_stock("BATCH-SKU").expect("stock").expect("item").total_available,
        dec!(2),
        "batch creation must reserve the stock it sold"
    );
}
