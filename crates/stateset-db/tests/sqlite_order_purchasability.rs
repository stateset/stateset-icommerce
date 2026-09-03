#![cfg(feature = "sqlite")]
//! Catalogue guard on order lines (SQLite backend).
//!
//! Verified defect (orders re-audit, Sep 2026): `validate_order_input` never
//! consulted `variant_is_purchasable_with_conn`, even though the product
//! module documents order lines as its intended second caller alongside
//! `carts.rs::add_item`. An archived product or a soft-deleted variant could
//! therefore still reach an order through `orders.create`, the batch creators,
//! `orders.add_item` — or through checkout, when the SKU was withdrawn after
//! the cart line was added and so never re-checked.
//!
//! The guard now runs inside the order-creating transaction and is the same
//! rule `carts.rs::add_item` has always applied, so an order line and a cart
//! line agree on what may be sold: an `Active` product with an active variant.
//! `products.create` mints a `Draft` product, so a fixture must publish it
//! before ordering its SKU. A SKU that is not
//! in the catalogue at all stays allowed
//! ([`stateset_core::VariantPurchasability::NotInCatalog`]) so ad-hoc and
//! external lines keep working.
//!
//! Mirrored on Postgres in `postgres_order_purchasability.rs`.

use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CartRepository, CommerceError, CreateCart, CreateCustomer,
    CreateInventoryItem, CreateOrder, CreateOrderItem, CreateProduct, CreateProductVariant,
    CustomerId, CustomerRepository, InventoryRepository, OrderRepository, ProductId,
    ProductRepository, ProductStatus, SetCartPayment, UpdateProduct,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn setup() -> (SqliteDatabase, CustomerId) {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("catalogue-{}@example.com", Uuid::new_v4()),
            first_name: "Cat".into(),
            last_name: "Alogue".into(),
            ..Default::default()
        })
        .expect("create customer");
    (db, customer.id)
}

/// A catalogued, sellable SKU: an `Active` product with one active variant,
/// plus stock so the order path has something to reserve.
fn catalogued_sku(db: &SqliteDatabase) -> (ProductId, String) {
    let tag = Uuid::new_v4().simple().to_string();
    let sku = format!("CAT-{}", &tag[..12]);
    let product = db
        .products()
        .create(CreateProduct {
            name: format!("Product {sku}"),
            slug: Some(format!("product-{}", &tag[..12])),
            variants: Some(vec![CreateProductVariant {
                sku: sku.clone(),
                price: dec!(19.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .expect("create product");
    // `products.create` mints a `Draft` product; publishing it is the separate
    // `Active` transition, and only an `Active` product is sellable.
    db.products()
        .update(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .expect("publish product");
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: sku.clone(),
            name: sku.clone(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("create inventory item");
    (product.id, sku)
}

fn archive(db: &SqliteDatabase, product_id: ProductId) {
    db.products()
        .update(
            product_id,
            UpdateProduct { status: Some(ProductStatus::Archived), ..Default::default() },
        )
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

#[test]
fn creating_an_order_for_an_archived_sku_is_refused() {
    let (db, customer_id) = setup();
    let (product_id, sku) = catalogued_sku(&db);
    archive(&db, product_id);

    let err = db.orders().create(order_input(customer_id, &[&sku])).expect_err("refused");
    assert_not_purchasable(&err, &sku);
    assert!(db.orders().list(Default::default()).expect("list").is_empty(), "nothing persisted");
}

#[test]
fn creating_an_order_for_an_unpublished_draft_sku_is_refused() {
    // Parity with `carts.rs::add_item`: `Draft` is "not published", and the
    // engine has always refused it in a cart. An order line is now held to the
    // same rule instead of quietly bypassing it.
    let (db, customer_id) = setup();
    let tag = Uuid::new_v4().simple().to_string();
    let sku = format!("DRAFT-{}", &tag[..12]);
    db.products()
        .create(CreateProduct {
            name: format!("Product {sku}"),
            slug: Some(format!("product-{}", &tag[..12])),
            variants: Some(vec![CreateProductVariant {
                sku: sku.clone(),
                price: dec!(19.99),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .expect("create product (left Draft)");

    let err = db.orders().create(order_input(customer_id, &[&sku])).expect_err("refused");
    assert_not_purchasable(&err, &sku);
}

#[test]
fn creating_an_order_for_a_deleted_variant_is_refused() {
    let (db, customer_id) = setup();
    let (_product_id, sku) = catalogued_sku(&db);
    let variant_id =
        db.products().get_variant_by_sku(&sku).expect("get variant").expect("variant exists").id;
    db.products().delete_variant(variant_id).expect("soft-delete variant");

    let err = db.orders().create(order_input(customer_id, &[&sku])).expect_err("refused");
    assert_not_purchasable(&err, &sku);
}

#[test]
fn creating_an_order_for_a_live_sku_still_works() {
    let (db, customer_id) = setup();
    let (_product_id, sku) = catalogued_sku(&db);
    let order = db.orders().create(order_input(customer_id, &[&sku])).expect("created");
    assert_eq!(order.items.len(), 1);
}

#[test]
fn an_ad_hoc_sku_outside_the_catalogue_is_still_allowed() {
    let (db, customer_id) = setup();
    let order = db
        .orders()
        .create(order_input(customer_id, &["NOT-IN-CATALOGUE-0001"]))
        .expect("ad-hoc lines keep working");
    assert_eq!(order.items.len(), 1);
}

#[test]
fn the_guard_names_the_offending_line_of_a_mixed_order() {
    let (db, customer_id) = setup();
    let (_ok_product, ok_sku) = catalogued_sku(&db);
    let (bad_product, bad_sku) = catalogued_sku(&db);
    archive(&db, bad_product);

    let err =
        db.orders().create(order_input(customer_id, &[&ok_sku, &bad_sku])).expect_err("refused");
    assert_not_purchasable(&err, &bad_sku);
}

#[test]
fn create_batch_atomic_refuses_an_archived_sku_and_persists_nothing() {
    let (db, customer_id) = setup();
    let (_ok_product, ok_sku) = catalogued_sku(&db);
    let (bad_product, bad_sku) = catalogued_sku(&db);
    archive(&db, bad_product);

    let err = db
        .orders()
        .create_batch_atomic(vec![
            order_input(customer_id, &[&ok_sku]),
            order_input(customer_id, &[&bad_sku]),
        ])
        .expect_err("refused");
    assert_not_purchasable(&err, &bad_sku);
    assert!(
        db.orders().list(Default::default()).expect("list").is_empty(),
        "atomic batch persists nothing"
    );
}

#[test]
fn adding_an_archived_sku_to_an_existing_order_is_refused() {
    let (db, customer_id) = setup();
    let (_ok_product, ok_sku) = catalogued_sku(&db);
    let (bad_product, bad_sku) = catalogued_sku(&db);
    let order = db.orders().create(order_input(customer_id, &[&ok_sku])).expect("create");
    archive(&db, bad_product);

    let err = db.orders().add_item(order.id, line(&bad_sku)).expect_err("refused");
    assert_not_purchasable(&err, &bad_sku);
    let reloaded = db.orders().get(order.id).expect("get").expect("exists");
    assert_eq!(reloaded.items.len(), 1, "no line was inserted");
    assert_eq!(reloaded.total_amount, order.total_amount, "total untouched");
}

// ---------------------------------------------------------------------------
// Checkout: the cart line was purchasable when added, and is not any more
// ---------------------------------------------------------------------------

#[test]
fn checking_out_a_cart_holding_a_since_archived_sku_is_refused() {
    let (db, customer_id) = setup();
    let (product_id, sku) = catalogued_sku(&db);

    let cart = db
        .carts()
        .create(stateset_core::CreateCart {
            customer_id: Some(customer_id),
            customer_email: Some("cat@example.com".into()),
            customer_name: Some("Cat Alogue".into()),
            ..Default::default()
        })
        .expect("create cart");
    db.carts()
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(ProductId::new()),
                sku: sku.clone(),
                name: sku.clone(),
                quantity: 1,
                unit_price: dec!(19.99),
                ..Default::default()
            },
        )
        .expect("the SKU is purchasable at add time");
    db.carts().set_shipping_address(cart.id, cart_address()).expect("shipping address");
    db.carts()
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_test".into()),
                ..Default::default()
            },
        )
        .expect("payment");

    // Withdrawn between "add to cart" and "check out". The product module's
    // own withdrawal guard counts only lines on carts still in `active`
    // status, so once the cart has moved to `ready_for_payment` the catalogue
    // is free to withdraw the SKU underneath it — this is exactly the window
    // in which checkout used to mint an order for a withdrawn SKU.
    db.carts().mark_ready_for_payment(cart.id).expect("ready for payment");
    archive(&db, product_id);

    let err = db.carts().complete(cart.id).expect_err("checkout refused");
    assert_not_purchasable(&err, &sku);
    assert!(db.orders().list(Default::default()).expect("list").is_empty(), "no order was minted");
}

#[test]
fn a_replayed_checkout_still_returns_its_order_after_the_sku_is_withdrawn() {
    let (db, customer_id) = setup();
    let (product_id, sku) = catalogued_sku(&db);
    let cart = db
        .carts()
        .create(CreateCart {
            customer_id: Some(customer_id),
            customer_email: Some("cat@example.com".into()),
            customer_name: Some("Cat Alogue".into()),
            ..Default::default()
        })
        .expect("create cart");

    let items = vec![line(&sku)];
    let first = db
        .orders()
        .create_from_cart(
            cart.id.into_uuid(),
            CreateOrder { customer_id, items: items.clone(), ..Default::default() },
        )
        .expect("first checkout mints the order");

    // The catalogue changes underneath, then the adapter retries. (The order is
    // cancelled first only so the product module's own withdrawal guard — which
    // counts lines on OPEN orders and live reservations — lets the archive
    // through.)
    db.orders()
        .update(
            first.id,
            stateset_core::UpdateOrder {
                status: Some(stateset_core::OrderStatus::Cancelled),
                ..Default::default()
            },
        )
        .expect("cancel");
    archive(&db, product_id);

    let replay = db
        .orders()
        .create_from_cart(
            cart.id.into_uuid(),
            CreateOrder { customer_id, items, ..Default::default() },
        )
        .expect("a replay must resolve to the order already minted");
    assert_eq!(replay.id, first.id);
}
