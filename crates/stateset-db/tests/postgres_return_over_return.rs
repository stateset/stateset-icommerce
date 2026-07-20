//! Regression tests for the Postgres return-create over-return guard.
//!
//! The Postgres return-create path historically fetched only `sku, name,
//! unit_price` for each return line and never checked the order item's owning
//! order or the ordered quantity. A caller could therefore:
//! - return more units than were purchased (over-return → over-refund),
//! - keep returning the same item across separate returns past the ordered
//!   quantity (cumulative over-return), or
//! - return an order item belonging to a *different* order.
//!
//! The single-line `create_async` path was also non-transactional: it inserted
//! the return header and then each item on separate pool connections, so a
//! rejected item would leave a partially-created return behind.
//!
//! The fix validates each line (ownership + remaining returnable quantity,
//! excluding rejected/cancelled returns) inside one transaction, mirroring the
//! SQLite backend, and rolls the whole return back if any line is rejected.
//!
//! These tests require a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`)
//! and are skipped otherwise.

#[cfg(feature = "postgres")]
use rust_decimal_macros::dec;
#[cfg(feature = "postgres")]
use stateset_core::{
    CommerceError, CreateCustomer, CreateOrder, CreateOrderItem, CreateProduct, CreateReturn,
    CreateReturnItem, ItemCondition, Order, ReturnFilter, ReturnReason,
};
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::env;
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

/// Create a customer + product + order whose single line item has `quantity`
/// units at `unit_price`, returning the persisted order.
#[cfg(feature = "postgres")]
async fn order_with_item(
    db: &PostgresDatabase,
    quantity: i32,
    unit_price: rust_decimal::Decimal,
) -> Order {
    let unique = Uuid::new_v4().to_string();
    let sku = format!("SKU-{}", unique.replace('-', ""));

    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("ret-{unique}@example.com"),
            first_name: "Ret".into(),
            last_name: "Urn".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let product = db
        .products()
        .create_async(CreateProduct {
            name: format!("Widget {unique}"),
            slug: Some(format!("widget-{unique}")),
            description: None,
            product_type: None,
            attributes: None,
            seo: None,
            variants: None,
        })
        .await
        .expect("create product");

    db.orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                variant_id: None,
                sku,
                name: "Widget".into(),
                quantity,
                unit_price,
                discount: None,
                tax_amount: None,
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
}

/// Returning more units than were ordered in a single line must be rejected,
/// and no partial return may be left behind.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_return_rejects_more_than_ordered_quantity() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

    let order = order_with_item(&db, 2, dec!(9.99)).await;
    let order_item_id = order.items[0].id;

    let err = db
        .returns()
        .create_async(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::Defective,
            reason_details: None,
            idempotency_key: None,
            items: vec![CreateReturnItem {
                order_item_id,
                quantity: 5,
                condition: Some(ItemCondition::Defective),
            }],
            notes: None,
        })
        .await
        .expect_err("returning 5 of a 2-unit item must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // The whole return must have rolled back — no orphaned header.
    let existing = db
        .returns()
        .list_async(ReturnFilter { order_id: Some(order.id), ..Default::default() })
        .await
        .expect("list returns");
    assert!(existing.is_empty(), "rejected over-return left a partial return: {existing:?}");
}

/// Returning the ordered quantity is allowed, but a further return that pushes
/// the cumulative total past what was ordered must be rejected.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_return_rejects_cumulative_over_return() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

    let order = order_with_item(&db, 2, dec!(9.99)).await;
    let order_item_id = order.items[0].id;

    // First return: both units — allowed.
    db.returns()
        .create_async(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::ChangedMind,
            reason_details: None,
            idempotency_key: None,
            items: vec![CreateReturnItem { order_item_id, quantity: 2, condition: None }],
            notes: None,
        })
        .await
        .expect("returning the full ordered quantity is allowed");

    // Second return: one more unit — nothing remains returnable.
    let err = db
        .returns()
        .create_async(CreateReturn {
            order_id: order.id,
            reason: ReturnReason::ChangedMind,
            reason_details: None,
            idempotency_key: None,
            items: vec![CreateReturnItem { order_item_id, quantity: 1, condition: None }],
            notes: None,
        })
        .await
        .expect_err("cumulative over-return must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

/// Returning an order item that belongs to a *different* order must be rejected.
#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_return_rejects_item_from_another_order() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
            return;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

    let order_a = order_with_item(&db, 2, dec!(9.99)).await;
    let order_b = order_with_item(&db, 2, dec!(19.99)).await;
    let b_item_id = order_b.items[0].id;

    let err = db
        .returns()
        .create_async(CreateReturn {
            order_id: order_a.id,
            reason: ReturnReason::WrongItem,
            reason_details: None,
            idempotency_key: None,
            items: vec![CreateReturnItem {
                order_item_id: b_item_id,
                quantity: 1,
                condition: None,
            }],
            notes: None,
        })
        .await
        .expect_err("returning another order's item must be rejected");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}
