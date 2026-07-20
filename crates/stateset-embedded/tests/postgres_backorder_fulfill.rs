//! Postgres parity for the `fulfill_backorder` guards.
//!
//! The two backends historically validated different things: Postgres guarded the
//! status (rejecting cancelled/fulfilled) but had NO remaining-quantity bound —
//! its `.max(0)` clamp silently swallowed over-fulfillment, so fulfilling 8 twice
//! against a 10-unit backorder recorded 16 units fulfilled. SQLite had the
//! quantity bound but no status guard. Both now enforce: quantity > 0, not
//! cancelled/fulfilled, and quantity <= remaining — inside one transaction.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateBackorder, CreateCustomer, CreateOrder, CreateOrderItem, CreateProduct,
    FulfillBackorder, FulfillmentSourceType,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_fulfill_backorder_guards() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let unique = uuid::Uuid::new_v4().to_string();

    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("bo-{unique}@example.com"),
            first_name: "BO".into(),
            last_name: "Guard".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let product = commerce
        .products()
        .create(CreateProduct { name: format!("BO {unique}"), ..Default::default() })
        .await
        .expect("create product");
    let order = commerce
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: "BO-GUARD".into(),
                quantity: 10,
                unit_price: dec!(5.00),
                name: "BO Guard Item".into(),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");

    let mk = || CreateBackorder {
        order_id: order.id.into_uuid(),
        customer_id: customer.id.into_uuid(),
        sku: "BO-GUARD".into(),
        quantity: dec!(10),
        priority: None,
        order_line_id: None,
        expected_date: None,
        promised_date: None,
        source_location_id: None,
        notes: None,
    };
    let fulfill = |id: uuid::Uuid, qty| FulfillBackorder {
        backorder_id: id,
        quantity: qty,
        source_type: FulfillmentSourceType::Inventory,
        source_id: None,
        notes: None,
        fulfilled_by: None,
    };

    // --- over-fulfill: 8 then 8 (only 2 remain) is rejected, state unchanged.
    let bo = commerce.backorder().create_backorder(mk()).await.expect("create bo");
    commerce.backorder().fulfill_backorder(fulfill(bo.id, dec!(8))).await.expect("first partial");
    let err = commerce
        .backorder()
        .fulfill_backorder(fulfill(bo.id, dec!(8)))
        .await
        .expect_err("over-fulfill must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let after = commerce.backorder().get_backorder(bo.id).await.expect("get").expect("exists");
    assert_eq!(after.quantity_fulfilled, dec!(8), "over-fulfill must not fold in");
    assert_eq!(after.quantity_remaining, dec!(2));

    // --- cancelled backorder cannot be fulfilled.
    let bo2 = commerce.backorder().create_backorder(mk()).await.expect("create bo2");
    commerce.backorder().cancel_backorder(bo2.id).await.expect("cancel");
    let err = commerce
        .backorder()
        .fulfill_backorder(fulfill(bo2.id, dec!(5)))
        .await
        .expect_err("fulfilling a cancelled backorder must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // --- non-positive quantity is rejected.
    let bo3 = commerce.backorder().create_backorder(mk()).await.expect("create bo3");
    let err = commerce
        .backorder()
        .fulfill_backorder(fulfill(bo3.id, dec!(0)))
        .await
        .expect_err("zero-quantity fulfillment must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // --- valid: fulfilling exactly the remainder completes it.
    let bo4 = commerce.backorder().create_backorder(mk()).await.expect("create bo4");
    commerce.backorder().fulfill_backorder(fulfill(bo4.id, dec!(4))).await.expect("partial");
    let done = commerce
        .backorder()
        .fulfill_backorder(fulfill(bo4.id, dec!(6)))
        .await
        .expect("fulfilling the remainder is allowed");
    assert_eq!(done.quantity_fulfilled, dec!(10));
    assert_eq!(done.quantity_remaining, dec!(0));
}
