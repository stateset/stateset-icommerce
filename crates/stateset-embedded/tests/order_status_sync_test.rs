#![cfg(feature = "sqlite")]

//! An order's `payment_status` / `fulfillment_status` follow its payment
//! ledger and its shipments.
//!
//! Regression: after checkout nothing maintained either field, so an order
//! that was paid in full, shipped, returned and refunded still read
//! `payment_status = pending, fulfillment_status = unfulfilled` — an agent or
//! UI reading the order concluded it was never paid or shipped.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CreateCart, CreateCustomer, CreatePayment, CreateRefund,
    FulfillmentStatus, Order, OrderId, OrderStatus, PaymentMethodType, PaymentStatus,
    SetCartPayment, ShipmentLineInput,
};
use stateset_embedded::Commerce;
use uuid::Uuid;

fn address() -> CartAddress {
    CartAddress {
        first_name: "Ada".into(),
        last_name: "Lovelace".into(),
        company: None,
        line1: "1 Analytical Way".into(),
        line2: None,
        city: "London".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: None,
        email: Some("ada@example.com".into()),
    }
}

/// Check out a cart with two lines and return the minted order.
fn checkout(commerce: &Commerce) -> Order {
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("sync-{}@example.com", Uuid::new_v4()),
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
            ..Default::default()
        })
        .expect("create customer");
    let cart = commerce
        .carts()
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(customer.email),
            customer_name: Some("Ada Lovelace".into()),
            ..Default::default()
        })
        .expect("create cart");
    for (sku, qty, price) in [("SYNC-A", 2, dec!(29.99)), ("SYNC-B", 1, dec!(10.03))] {
        commerce
            .carts()
            .add_item(
                cart.id,
                AddCartItem {
                    sku: sku.into(),
                    name: sku.into(),
                    quantity: qty,
                    unit_price: price,
                    ..Default::default()
                },
            )
            .expect("add item");
    }
    commerce.carts().set_shipping_address(cart.id, address()).expect("shipping address");
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
        .expect("set payment");
    let result = commerce.carts().complete(cart.id).expect("checkout");
    let order = get(commerce, result.order_id);
    assert_eq!(order.payment_status, PaymentStatus::Pending);
    assert_eq!(order.fulfillment_status, FulfillmentStatus::Unfulfilled);
    assert!(order.total_amount > Decimal::ZERO);
    order
}

fn get(commerce: &Commerce, id: OrderId) -> Order {
    commerce.orders().get(id).expect("get order").expect("order exists")
}

fn pay(commerce: &Commerce, order: &Order, amount: Decimal) -> stateset_core::Payment {
    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            customer_id: Some(order.customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            currency: Some(order.currency),
            ..Default::default()
        })
        .expect("create payment");
    commerce.payments().mark_completed(payment.id).expect("complete payment")
}

fn refund(commerce: &Commerce, payment_id: stateset_core::PaymentId, amount: Decimal) {
    let refund = commerce
        .payments()
        .create_refund(CreateRefund {
            payment_id,
            amount: Some(amount),
            reason: Some("customer return".into()),
            ..Default::default()
        })
        .expect("create refund");
    commerce.payments().complete_refund(refund.id).expect("complete refund");
}

#[test]
fn paid_shipped_then_refunded_order_reports_each_state() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let order = checkout(&commerce);

    let payment = pay(&commerce, &order, order.total_amount);
    let paid = get(&commerce, order.id);
    assert_eq!(paid.payment_status, PaymentStatus::Paid, "fully captured order is paid");
    assert!(paid.version > order.version, "the status write bumps the order version");
    assert!(paid.updated_at >= order.updated_at);

    commerce.orders().update_status(order.id, OrderStatus::Processing).expect("process");
    let shipped = commerce.orders().ship(order.id, Some("1Z999")).expect("ship");
    assert_eq!(shipped.status, OrderStatus::Shipped);
    assert_eq!(shipped.fulfillment_status, FulfillmentStatus::Shipped);
    assert_eq!(get(&commerce, order.id).fulfillment_status, FulfillmentStatus::Shipped);
    assert_eq!(shipped.payment_status, PaymentStatus::Paid, "shipping keeps the money state");

    refund(&commerce, payment.id, dec!(10.03));
    let partially = get(&commerce, order.id);
    assert_eq!(partially.payment_status, PaymentStatus::PartiallyRefunded);
    assert_eq!(partially.fulfillment_status, FulfillmentStatus::Shipped);

    refund(&commerce, payment.id, order.total_amount - dec!(10.03));
    let refunded = get(&commerce, order.id);
    assert_eq!(refunded.payment_status, PaymentStatus::Refunded);

    let delivered = commerce.orders().deliver(order.id).expect("deliver");
    assert_eq!(delivered.fulfillment_status, FulfillmentStatus::Delivered);
    assert_eq!(delivered.payment_status, PaymentStatus::Refunded);
}

#[test]
fn partial_payment_then_top_up_is_partially_paid_then_paid() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let order = checkout(&commerce);

    let first = order.total_amount - dec!(0.01);
    pay(&commerce, &order, first);
    assert_eq!(get(&commerce, order.id).payment_status, PaymentStatus::PartiallyPaid);

    // One cent short of the total is still partial; the last cent settles it.
    pay(&commerce, &order, dec!(0.01));
    assert_eq!(get(&commerce, order.id).payment_status, PaymentStatus::Paid);
}

#[test]
fn processing_payment_authorizes_and_failed_payment_fails_the_order() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let order = checkout(&commerce);

    let payment = commerce
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            payment_method: PaymentMethodType::CreditCard,
            amount: order.total_amount,
            currency: Some(order.currency),
            ..Default::default()
        })
        .expect("create payment");
    assert_eq!(get(&commerce, order.id).payment_status, PaymentStatus::Pending);

    commerce.payments().mark_processing(payment.id).expect("processing");
    assert_eq!(get(&commerce, order.id).payment_status, PaymentStatus::Authorized);

    commerce.payments().mark_failed(payment.id, "card declined", None).expect("fail");
    assert_eq!(get(&commerce, order.id).payment_status, PaymentStatus::Failed);

    // A retry that captures in full supersedes the failure.
    pay(&commerce, &order, order.total_amount);
    assert_eq!(get(&commerce, order.id).payment_status, PaymentStatus::Paid);
}

#[test]
fn partial_shipment_is_partially_fulfilled() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let order = checkout(&commerce);
    commerce.orders().update_status(order.id, OrderStatus::Processing).expect("process");

    let line = order.items.iter().find(|item| item.quantity == 2).expect("two-unit line");
    let partial = commerce
        .orders()
        .ship_lines(
            order.id,
            None,
            Some(vec![ShipmentLineInput { order_item_id: line.id, quantity: 1 }]),
        )
        .expect("partial ship");
    assert_eq!(partial.status, OrderStatus::PartiallyShipped);
    assert_eq!(partial.fulfillment_status, FulfillmentStatus::PartiallyFulfilled);

    let rest = commerce.orders().ship(order.id, None).expect("ship the rest");
    assert_eq!(rest.status, OrderStatus::Shipped);
    assert_eq!(rest.fulfillment_status, FulfillmentStatus::Shipped);
}

/// Regression (found by the cross-entity invariant harness): a ship that is
/// refused — here, more units than the line has left — used to advance a
/// confirmed order to `processing` anyway, because the pre-advance ran in its
/// own transaction before the ship was validated. A refused ship now writes
/// nothing, and a valid ship still moves a confirmed order straight through.
#[test]
fn refused_ship_leaves_a_confirmed_order_untouched() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let order = checkout(&commerce);
    let before = get(&commerce, order.id);
    assert_eq!(before.status, OrderStatus::Confirmed);

    let line = order.items.iter().find(|item| item.quantity == 2).expect("two-unit line");
    let err = commerce
        .orders()
        .ship_lines(
            order.id,
            None,
            Some(vec![ShipmentLineInput { order_item_id: line.id, quantity: 3 }]),
        )
        .expect_err("3 of 2 units must be refused");
    assert!(matches!(err, stateset_core::CommerceError::ShipmentExceedsOrdered { .. }), "{err:?}");
    let after = get(&commerce, order.id);
    assert_eq!(after.status, OrderStatus::Confirmed, "a refused ship moved the order");
    assert_eq!(after.version, before.version, "a refused ship bumped the order version");

    let partial = commerce
        .orders()
        .ship_lines(
            order.id,
            None,
            Some(vec![ShipmentLineInput { order_item_id: line.id, quantity: 1 }]),
        )
        .expect("a valid ship of a confirmed order succeeds");
    assert_eq!(partial.status, OrderStatus::PartiallyShipped);
}
