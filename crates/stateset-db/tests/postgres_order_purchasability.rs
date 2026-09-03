#![cfg(feature = "postgres")]
//! Postgres mirror of `sqlite_order_purchasability.rs`: the catalogue guard on
//! order lines.
//!
//! Verified defect (orders re-audit, Sep 2026): order creation never consulted
//! `variant_is_purchasable_with_conn_pg`, so an archived product or a
//! soft-deleted variant could still reach an order through `orders.create`,
//! the batch creators, `orders.add_item` or checkout. The guard is the same
//! rule `carts.rs::add_item_internal` has always applied, so an order line and
//! a cart line agree on what may be sold: an `Active` product with an active
//! variant. `products.create` mints a `Draft` product, so a fixture must
//! publish it before ordering its SKU.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CommerceError, CreateCart, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, CreateProduct, CreateProductVariant, CustomerId, OrderFilter,
    ProductId, ProductStatus, SetCartPayment, UpdateProduct,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let url = postgres_url()?;
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

macro_rules! require_db {
    () => {
        match connect().await {
            Some(db) => db,
            None => {
                eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping");
                return;
            }
        }
    };
}

async fn customer(db: &PostgresDatabase) -> CustomerId {
    db.customers()
        .create_async(CreateCustomer {
            email: format!("catalogue-{}@example.com", Uuid::new_v4()),
            first_name: "Cat".into(),
            last_name: "Alogue".into(),
            ..Default::default()
        })
        .await
        .expect("create customer")
        .id
}

/// A catalogued, sellable SKU: an `Active` product with one active variant,
/// plus stock so the order path has something to reserve.
async fn catalogued_sku(db: &PostgresDatabase) -> (ProductId, String) {
    let tag = Uuid::new_v4().simple().to_string();
    let sku = format!("CAT-{}", &tag[..12]);
    let product = db
        .products()
        .create_async(CreateProduct {
            name: format!("Product {sku}"),
            slug: Some(format!("product-{}", &tag[..12])),
            variants: Some(vec![CreateProductVariant {
                sku: sku.clone(),
                price: dec!(19.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .await
        .expect("create product");
    // `products.create` mints a `Draft` product; publishing it is the separate
    // `Active` transition, and only an `Active` product is sellable.
    db.products()
        .update_async(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .await
        .expect("publish product");
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.clone(),
            name: sku.clone(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .await
        .expect("create inventory item");
    (product.id, sku)
}

async fn archive(db: &PostgresDatabase, product_id: ProductId) {
    db.products()
        .update_async(
            product_id,
            UpdateProduct { status: Some(ProductStatus::Archived), ..Default::default() },
        )
        .await
        .expect("archive product");
}

fn line(sku: &str) -> CreateOrderItem {
    CreateOrderItem {
        product_id: ProductId::new(),
        sku: sku.to_string(),
        name: sku.to_string(),
        quantity: 1,
        unit_price: dec!(19.99),
        ..Default::default()
    }
}

fn order_input(customer_id: CustomerId, skus: &[&str]) -> CreateOrder {
    CreateOrder {
        customer_id,
        items: skus.iter().map(|sku| line(sku)).collect(),
        ..Default::default()
    }
}

fn assert_not_purchasable(err: &CommerceError, sku: &str) {
    match err {
        CommerceError::ValidationError(msg) => {
            assert!(msg.contains(sku), "expected {sku:?} named in {msg:?}");
            assert!(
                msg.contains("not purchasable") || msg.contains("no longer available"),
                "expected a purchasability reason in {msg:?}"
            );
        }
        other => panic!("expected ValidationError for {sku}, got {other:?}"),
    }
}

/// Orders belonging to `customer_id` — the Postgres database is shared, so
/// every "nothing persisted" assertion is scoped to this test's customer.
async fn orders_for(db: &PostgresDatabase, customer_id: CustomerId) -> usize {
    db.orders()
        .list_async(OrderFilter { customer_id: Some(customer_id), ..Default::default() })
        .await
        .expect("list orders")
        .len()
}

fn cart_address() -> CartAddress {
    CartAddress {
        first_name: "Cat".into(),
        last_name: "Alogue".into(),
        company: None,
        line1: "1 Market St".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94105".into(),
        country: "US".into(),
        phone: Some("555-0100".into()),
        email: Some("cat@example.com".into()),
    }
}

// ---------------------------------------------------------------------------
// create / create_batch_atomic / add_item
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_creating_an_order_for_an_archived_sku_is_refused() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (product_id, sku) = catalogued_sku(&db).await;
    archive(&db, product_id).await;

    let err =
        db.orders().create_async(order_input(customer_id, &[&sku])).await.expect_err("refused");
    assert_not_purchasable(&err, &sku);
    assert_eq!(orders_for(&db, customer_id).await, 0, "nothing persisted");
}

#[tokio::test]
async fn postgres_creating_an_order_for_an_unpublished_draft_sku_is_refused() {
    // Parity with `carts.rs::add_item_internal`: `Draft` is "not published",
    // and the engine has always refused it in a cart. An order line is now held
    // to the same rule instead of quietly bypassing it.
    let db = require_db!();
    let customer_id = customer(&db).await;
    let tag = Uuid::new_v4().simple().to_string();
    let sku = format!("DRAFT-{}", &tag[..12]);
    db.products()
        .create_async(CreateProduct {
            name: format!("Product {sku}"),
            slug: Some(format!("product-{}", &tag[..12])),
            variants: Some(vec![CreateProductVariant {
                sku: sku.clone(),
                price: dec!(19.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .await
        .expect("create product (left Draft)");

    let err =
        db.orders().create_async(order_input(customer_id, &[&sku])).await.expect_err("refused");
    assert_not_purchasable(&err, &sku);
}

#[tokio::test]
async fn postgres_creating_an_order_for_a_deleted_variant_is_refused() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (_product_id, sku) = catalogued_sku(&db).await;
    let variant_id = db
        .products()
        .get_variant_by_sku_async(&sku)
        .await
        .expect("get variant")
        .expect("variant exists")
        .id;
    db.products().delete_variant_async(variant_id).await.expect("soft-delete variant");

    let err =
        db.orders().create_async(order_input(customer_id, &[&sku])).await.expect_err("refused");
    assert_not_purchasable(&err, &sku);
}

#[tokio::test]
async fn postgres_creating_an_order_for_a_live_sku_still_works() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (_product_id, sku) = catalogued_sku(&db).await;
    let order = db.orders().create_async(order_input(customer_id, &[&sku])).await.expect("created");
    assert_eq!(order.items.len(), 1);
}

#[tokio::test]
async fn postgres_an_ad_hoc_sku_outside_the_catalogue_is_still_allowed() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let sku = format!("NOT-IN-CATALOGUE-{}", Uuid::new_v4().simple());
    let order = db
        .orders()
        .create_async(order_input(customer_id, &[&sku]))
        .await
        .expect("ad-hoc lines keep working");
    assert_eq!(order.items.len(), 1);
}

#[tokio::test]
async fn postgres_create_batch_atomic_refuses_an_archived_sku_and_persists_nothing() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (_ok_product, ok_sku) = catalogued_sku(&db).await;
    let (bad_product, bad_sku) = catalogued_sku(&db).await;
    archive(&db, bad_product).await;

    let err = db
        .orders()
        .create_batch_atomic_async(vec![
            order_input(customer_id, &[&ok_sku]),
            order_input(customer_id, &[&bad_sku]),
        ])
        .await
        .expect_err("refused");
    assert_not_purchasable(&err, &bad_sku);
    assert_eq!(orders_for(&db, customer_id).await, 0, "atomic batch persists nothing");
}

#[tokio::test]
async fn postgres_adding_an_archived_sku_to_an_existing_order_is_refused() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (_ok_product, ok_sku) = catalogued_sku(&db).await;
    let (bad_product, bad_sku) = catalogued_sku(&db).await;
    let order =
        db.orders().create_async(order_input(customer_id, &[&ok_sku])).await.expect("create");
    archive(&db, bad_product).await;

    let err = db
        .orders()
        .add_item_async(order.id.into_uuid(), line(&bad_sku))
        .await
        .expect_err("refused");
    assert_not_purchasable(&err, &bad_sku);
    let reloaded = db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("exists");
    assert_eq!(reloaded.items.len(), 1, "no line was inserted");
    assert_eq!(reloaded.total_amount, order.total_amount, "total untouched");
}

// ---------------------------------------------------------------------------
// Checkout: the cart line was purchasable when added, and is not any more
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_checking_out_a_cart_holding_a_since_archived_sku_is_refused() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (product_id, sku) = catalogued_sku(&db).await;

    let cart = db
        .carts()
        .create_async(CreateCart {
            customer_id: Some(customer_id),
            customer_email: Some("cat@example.com".into()),
            customer_name: Some("Cat Alogue".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");
    db.carts()
        .add_item_async(
            cart.id.into_uuid(),
            AddCartItem {
                // Postgres has an FK on `cart_items.product_id`, so this must
                // be the real product.
                product_id: Some(product_id),
                sku: sku.clone(),
                name: sku.clone(),
                quantity: 1,
                unit_price: dec!(19.99),
                ..Default::default()
            },
        )
        .await
        .expect("the SKU is purchasable at add time");
    db.carts()
        .set_shipping_address_async(cart.id.into_uuid(), cart_address())
        .await
        .expect("shipping address");
    db.carts()
        .set_payment_async(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                ..Default::default()
            },
        )
        .await
        .expect("payment");

    // Withdrawn between "add to cart" and "check out". The product module's
    // own withdrawal guard counts only lines on carts still in `active`
    // status, so once the cart has moved to `ready_for_payment` the catalogue
    // is free to withdraw the SKU underneath it — this is exactly the window
    // in which checkout used to mint an order for a withdrawn SKU.
    db.carts().mark_ready_for_payment_async(cart.id.into_uuid()).await.expect("ready for payment");
    archive(&db, product_id).await;

    let err = db.carts().complete_async(cart.id.into_uuid()).await.expect_err("checkout refused");
    assert_not_purchasable(&err, &sku);
    assert_eq!(orders_for(&db, customer_id).await, 0, "no order was minted");
}

#[tokio::test]
async fn postgres_a_replayed_checkout_still_returns_its_order_after_the_sku_is_withdrawn() {
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (product_id, sku) = catalogued_sku(&db).await;
    let cart = db
        .carts()
        .create_async(CreateCart {
            customer_id: Some(customer_id),
            customer_email: Some("cat@example.com".into()),
            customer_name: Some("Cat Alogue".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");

    let items = vec![line(&sku)];
    let first = db
        .orders()
        .create_from_cart_async(
            cart.id.into_uuid(),
            CreateOrder { customer_id, items: items.clone(), ..Default::default() },
        )
        .await
        .expect("first checkout mints the order");

    // The catalogue changes underneath, then the adapter retries. (The order
    // is cancelled first only so the product module's own withdrawal guard —
    // which counts lines on OPEN orders — lets the archive through.)
    db.orders()
        .update_async(
            first.id.into_uuid(),
            stateset_core::UpdateOrder {
                status: Some(stateset_core::OrderStatus::Cancelled),
                ..Default::default()
            },
        )
        .await
        .expect("cancel");
    archive(&db, product_id).await;
    let replay = db
        .orders()
        .create_from_cart_async(
            cart.id.into_uuid(),
            CreateOrder { customer_id, items, ..Default::default() },
        )
        .await
        .expect("a replay must resolve to the order already minted");
    assert_eq!(replay.id, first.id);
}
