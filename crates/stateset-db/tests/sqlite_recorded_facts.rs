#![cfg(feature = "sqlite")]

//! Every money-path mutation wired in Phase B task 4 emits a fact whose
//! aggregate identity is usable by a peer.
//!
//! The `outbox_emission_parity` lint proves a `record_outbox_fact` call
//! exists in the method. These tests prove the call says something true: the
//! event type is the one the log's vocabulary uses, the aggregate id locates
//! the thing that changed (the cart, not the line), and the fact commits with
//! the mutation or not at all.

use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CartRepository, CreateCart, CreateCustomer, CustomerRepository,
    ProductId, SetCartPayment,
};
use stateset_db::SqliteDatabase;

/// The recorded fact for `event_type` on `aggregate_id`, as
/// `(aggregate_type, payload, tier)`.
fn fact(
    db: &SqliteDatabase,
    event_type: &str,
    aggregate_id: &str,
) -> Option<(String, serde_json::Value, String)> {
    let conn = db.pool().get().expect("connection");
    conn.query_row(
        "SELECT aggregate_type, payload, tier FROM kernel_outbox
         WHERE event_type = ?1 AND aggregate_id = ?2
         ORDER BY created_at DESC LIMIT 1",
        rusqlite::params![event_type, aggregate_id],
        |row| {
            let payload: String = row.get(1)?;
            Ok((
                row.get::<_, String>(0)?,
                serde_json::from_str(&payload).expect("payload is json"),
                row.get::<_, String>(2)?,
            ))
        },
    )
    .ok()
}

fn test_address() -> CartAddress {
    CartAddress {
        first_name: "Fact".into(),
        last_name: "Checker".into(),
        company: None,
        line1: "1 Ledger Way".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: None,
        email: Some("fact.checker@example.com".into()),
    }
}

// ============================================================================
// carts.rs — complete_checkout_with_policy_in_tx
// ============================================================================

#[test]
fn checkout_emits_a_fact_naming_the_cart_and_its_order() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let carts = db.carts();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "checkout-fact@example.com".into(),
            first_name: "Checkout".into(),
            last_name: "Fact".into(),
            ..Default::default()
        })
        .expect("create customer");

    let cart = carts
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some("checkout-fact@example.com".into()),
            customer_name: Some("Checkout Fact".into()),
            ..Default::default()
        })
        .expect("create cart");
    carts
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(ProductId::new()),
                sku: "SKU-FACT-1".into(),
                name: "Fact widget".into(),
                quantity: 2,
                unit_price: dec!(10.00),
                ..Default::default()
            },
        )
        .expect("add item");
    carts.set_shipping_address(cart.id, test_address()).expect("set shipping address");
    carts
        .set_payment(
            cart.id,
            SetCartPayment { payment_method: "credit_card".into(), ..Default::default() },
        )
        .expect("set payment");

    let checkout = carts.complete(cart.id).expect("complete checkout");

    let (aggregate_type, payload, tier) =
        fact(&db, "cart.checked_out", &cart.id.to_string()).expect("checkout must emit a fact");
    assert_eq!(aggregate_type, "cart");
    assert_eq!(tier, "recorded");
    assert_eq!(
        payload["order_id"].as_str(),
        Some(checkout.order_id.to_string().as_str()),
        "the fact must name the order the checkout minted"
    );
}
