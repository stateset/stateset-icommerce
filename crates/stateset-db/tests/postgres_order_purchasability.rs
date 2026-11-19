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
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
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

#[tokio::test]
async fn postgres_external_coupon_order_reserves_and_redeems_once() {
    use stateset_core::{
        AddCartItem, CreateCart, CreateCouponCode, CreatePromotion, PromotionTarget,
        PromotionTrigger, PromotionType, StockPolicy,
    };
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (product_id, sku) = catalogued_sku(&db).await;
    let promotion = db
        .promotions()
        .create_async(CreatePromotion {
            name: format!("Offer {sku}"),
            promotion_type: PromotionType::PercentageOff,
            trigger: PromotionTrigger::CouponCode,
            target: PromotionTarget::Order,
            percentage_off: Some(dec!(0.10)),
            ..Default::default()
        })
        .await
        .unwrap();
    let promos = db.promotions();
    promos.activate_async(promotion.id.into_uuid()).await.unwrap();
    let coupon = promos
        .create_coupon_async(CreateCouponCode {
            promotion_id: promotion.id,
            code: sku.clone(),
            usage_limit: Some(1),
            per_customer_limit: None,
            starts_at: None,
            ends_at: None,
            metadata: None,
        })
        .await
        .unwrap();
    let carts = db.carts();
    let cart = carts
        .create_async(CreateCart { customer_id: Some(customer_id), ..Default::default() })
        .await
        .unwrap();
    let id = cart.id.into_uuid();
    carts
        .add_item_async(
            id,
            AddCartItem {
                product_id: Some(product_id),
                sku: sku.clone(),
                name: sku.clone(),
                quantity: 1,
                unit_price: dec!(19.99),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    carts.apply_discount_async(id, &coupon.code).await.unwrap();
    let orders = db.orders();
    let mut input = CreateOrder {
        customer_id,
        items: vec![CreateOrderItem { product_id, ..line(&sku) }],
        currency: Some(cart.currency),
        discount_amount: Some(dec!(1)),
        stock_policy: StockPolicy::RejectIfInsufficient,
        ..Default::default()
    };
    assert!(
        orders.create_from_cart_with_coupon_async(id, input.clone(), &coupon.code).await.is_err()
    );
    assert_eq!(promos.get_coupon_async(coupon.id).await.unwrap().unwrap().usage_count, 0);
    input.discount_amount = Some(dec!(2));
    let (first, retry) = tokio::join!(
        orders.create_from_cart_with_coupon_async(id, input.clone(), &coupon.code),
        orders.create_from_cart_with_coupon_async(id, input, &coupon.code)
    );
    let order = first.unwrap();
    assert_eq!(retry.unwrap().id, order.id);
    assert_eq!(order.total_amount, dec!(17.99));
    assert_eq!(promos.get_coupon_async(coupon.id).await.unwrap().unwrap().usage_count, 1);
    assert_eq!(
        db.inventory().get_stock_async(&sku).await.unwrap().unwrap().total_allocated,
        dec!(1)
    );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_paid_order_enrollment_seeds_one_settled_cycle() {
    use stateset_core::{
        BillingCycleFilter, BillingCycleStatus, CreatePayment, CreateSubscription,
        CreateSubscriptionPlan, Subscription,
    };
    let db = require_db!();
    let customer_id = customer(&db).await;
    let (_, sku) = catalogued_sku(&db).await;
    let order = db.orders().create_async(order_input(customer_id, &[&sku])).await.unwrap();
    let payment = db
        .payments()
        .create_async(CreatePayment {
            order_id: Some(order.id),
            customer_id: Some(customer_id),
            amount: order.total_amount,
            ..Default::default()
        })
        .await
        .unwrap();
    db.payments().mark_completed_async(payment.id.into_uuid()).await.unwrap();
    let repo = db.subscriptions();
    let plan = repo
        .create_plan_async(CreateSubscriptionPlan {
            name: "Paid membership".into(),
            price: dec!(19.99),
            ..Default::default()
        })
        .await
        .unwrap();
    repo.activate_plan_async(plan.id).await.unwrap();
    let input = CreateSubscription {
        customer_id,
        plan_id: plan.id,
        skip_trial: Some(true),
        metadata: Some(
            serde_json::json!({"purchase_order": order.id, "purchase_payment": payment.id}),
        ),
        ..Default::default()
    };
    assert!(repo.insert_paid_membership_async(order.id, payment.id, input.clone()).await.is_err());
    db.orders()
        .update_async(
            order.id.into_uuid(),
            stateset_core::UpdateOrder {
                status: Some(stateset_core::OrderStatus::Confirmed),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        repo.insert_paid_membership_async(order.id, payment.id, input.clone()),
        repo.insert_paid_membership_async(order.id, payment.id, input)
    );
    let writes = [a, b];
    assert_eq!(writes.iter().filter(|r| r.is_ok()).count(), 1, "{writes:?}");
    let sub = writes.into_iter().find_map(Result::ok).unwrap();
    assert_eq!(sub.id, Subscription::purchased_id(order.id, plan.id));
    assert_eq!(sub.next_billing_date, None);
    assert_eq!(sub.billing_cycle_count, 1);
    let cycles = repo
        .list_billing_cycles_async(BillingCycleFilter {
            subscription_id: Some(sub.id),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].status, BillingCycleStatus::Paid);
    assert_eq!(cycles[0].order_id, Some(order.id));
    assert_eq!(cycles[0].payment_id, Some(payment.id.to_string()));
    let renewal = db
        .payments()
        .create_async(CreatePayment {
            customer_id: Some(customer_id),
            amount: dec!(22.00),
            ..Default::default()
        })
        .await
        .unwrap();
    let receipt = stateset_core::PaidMembershipRenewal {
        subscription_id: sub.id,
        payment_id: renewal.id,
        invoice_key: format!("stripe:in_{}", Uuid::new_v4().simple()),
        period_start: sub.current_period_end,
        period_end: sub.current_period_end + chrono::Duration::days(30),
        tax: dec!(2),
    };
    assert!(repo.record_paid_renewal_async(receipt.clone()).await.is_err());
    db.payments().mark_completed_async(renewal.id.into_uuid()).await.unwrap();
    let (one, two) = tokio::join!(
        repo.record_paid_renewal_async(receipt.clone()),
        repo.record_paid_renewal_async(receipt.clone())
    );
    let cycle = one.unwrap();
    assert_eq!(two.unwrap().id, cycle.id);
    assert_eq!(
        (cycle.cycle_number, cycle.subtotal, cycle.tax, cycle.total),
        (2, dec!(20), dec!(2), dec!(22))
    );
    assert_eq!(cycle.payment_id, Some(renewal.id.to_string()));
    let latest = repo.get_subscription_async(sub.id).await.unwrap().unwrap();
    assert_eq!(latest.current_period_end, receipt.period_end);
    assert_eq!(latest.billing_cycle_count, 2);
    assert!(latest.next_billing_date.is_none());
    assert!(
        repo.record_paid_renewal_async(stateset_core::PaidMembershipRenewal {
            invoice_key: "stripe:other".into(),
            ..receipt.clone()
        })
        .await
        .is_err()
    );
    let partial = db
        .payments()
        .create_refund_async(stateset_core::CreateRefund {
            payment_id: renewal.id,
            amount: Some(dec!(2)),
            ..Default::default()
        })
        .await
        .unwrap();
    db.payments().complete_refund_async(partial.id).await.unwrap();
    assert_eq!(
        repo.get_billing_cycle_async(cycle.id).await.unwrap().unwrap().status,
        BillingCycleStatus::Paid
    );
    let refund = db
        .payments()
        .create_refund_async(stateset_core::CreateRefund {
            payment_id: renewal.id,
            ..Default::default()
        })
        .await
        .unwrap();
    db.payments().complete_refund_async(refund.id).await.unwrap();
    assert_eq!(
        repo.get_billing_cycle_async(cycle.id).await.unwrap().unwrap().status,
        BillingCycleStatus::Refunded
    );
    assert_eq!(
        repo.get_billing_cycle_async(cycles[0].id).await.unwrap().unwrap().status,
        BillingCycleStatus::Paid
    );
    let signup_refund = db
        .payments()
        .create_refund_async(stateset_core::CreateRefund {
            payment_id: payment.id,
            ..Default::default()
        })
        .await
        .unwrap();
    db.payments().complete_refund_async(signup_refund.id).await.unwrap();
    assert_eq!(
        repo.get_billing_cycle_async(cycles[0].id).await.unwrap().unwrap().status,
        BillingCycleStatus::Refunded
    );
    assert_eq!(
        repo.record_paid_renewal_async(receipt).await.unwrap().status,
        BillingCycleStatus::Refunded
    );
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
