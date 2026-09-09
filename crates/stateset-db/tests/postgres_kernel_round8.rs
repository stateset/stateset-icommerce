#![cfg(feature = "postgres")]
//! Kernel executor round 8 (PostgreSQL): the observed-quantity binding.
//!
//! Mirrors `sqlite_kernel_round8.rs` against a live Postgres so the async
//! executor is proven to bind a declared `commitment.quantity` to the units
//! the mutation actually moves, on every quantity-carrying command.
//!
//! Requires a live Postgres (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CommandEnvelope, CommitCheckout, ConfirmInventoryReservation,
    CreateCart, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CurrencyCode,
    EconomicCommitment, ExecutionMode, ExecutionStatus, KernelCommandPolicy, KernelPolicy,
    KernelPrincipal, Money, Order, OrderStatus, PrincipalKind, ProductId, ReserveInventory,
    SetCartPayment, ShipOrderCommand, UpdateOrder,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

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

/// Every quantity-carrying command capped at fifty units.
fn quantity_policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow(
            "inventory.item.create",
            KernelCommandPolicy::requiring(["inventory.item.create"]).with_max_quantity(dec!(50)),
        )
        .allow(
            "inventory.reservation.confirm",
            KernelCommandPolicy::requiring(["inventory.reservation.confirm"])
                .with_max_quantity(dec!(50)),
        )
        .allow(
            "checkout.commit",
            KernelCommandPolicy::requiring(["checkout.commit"]).with_max_quantity(dec!(50)),
        )
        .allow(
            "orders.ship",
            KernelCommandPolicy::requiring(["orders.ship"]).with_max_quantity(dec!(50)),
        )
}

fn command<C>(command_type: &str, key: String, payload: C) -> CommandEnvelope<C> {
    let mut command = CommandEnvelope::preview(
        command_type,
        key,
        KernelPrincipal {
            id: "agent:round8".into(),
            kind: PrincipalKind::Agent,
            tenant_id: Some("tenant-1".into()),
            delegated_by: Some("user-1".into()),
            capabilities: vec![command_type.into()],
        },
        payload,
    );
    command.store_id = Some("store-1".into());
    command.policy_version = Some("commerce-policy-1".into());
    command.mode = ExecutionMode::Apply;
    command
}

fn declaring(quantity: &str) -> Option<EconomicCommitment> {
    Some(EconomicCommitment {
        budget_id: None,
        amount: None,
        asset_amount: None,
        counterparty_id: None,
        quantity: Some(quantity.into()),
        evidence: vec![],
    })
}

/// `checkout.commit` also binds money, and that guard fails closed on a
/// commitment without an amount, so a checkout declaration carries both.
fn declaring_units_and_money(quantity: &str, total: Decimal) -> Option<EconomicCommitment> {
    Some(EconomicCommitment {
        budget_id: None,
        amount: Some(Money::new(total, CurrencyCode::USD).to_wire()),
        asset_amount: None,
        counterparty_id: None,
        quantity: Some(quantity.into()),
        evidence: vec![],
    })
}

async fn stock(db: &PostgresDatabase, sku: &str, quantity: Decimal) {
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.into(),
            name: format!("Stock {sku}"),
            initial_quantity: Some(quantity),
            ..Default::default()
        })
        .await
        .expect("create stock");
}

async fn reservation_of(db: &PostgresDatabase, sku: &str, quantity: Decimal) -> Uuid {
    db.inventory()
        .reserve_async(ReserveInventory {
            sku: sku.into(),
            location_id: Some(1),
            quantity,
            reference_type: "order".into(),
            reference_id: format!("order-{sku}"),
            expires_in_seconds: Some(900),
        })
        .await
        .expect("reserve inventory")
        .id
}

async fn allocated(db: &PostgresDatabase, sku: &str) -> Decimal {
    db.inventory().get_stock_async(sku).await.expect("stock query").expect("stock").total_allocated
}

/// Seeding a SKU's opening stock moves units, so a declared ceiling has to
/// bind to `initial_quantity`. Until `inventory.item.create` was listed in
/// `supports_observed_quantity_binding` the whole command failed closed under
/// any quantity rule, so this ceiling could not be used at all.
#[tokio::test]
async fn postgres_item_create_rejects_a_declared_quantity_the_opening_stock_does_not_match() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-ITEM-SUB-{suffix}");

    let mut create = command(
        "inventory.item.create",
        format!("r8-pg-item-create-substitution-{suffix}"),
        CreateInventoryItem {
            sku: sku.clone(),
            name: "Seeded stock".into(),
            initial_quantity: Some(dec!(1000)),
            ..Default::default()
        },
    );
    create.commitment = declaring("1");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_create_inventory_item_async(&create)
        .await
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected, "{receipt:?}");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    assert!(
        db.inventory().get_stock_async(&sku).await.expect("stock query").is_none(),
        "a refused create must seed nothing"
    );
}

#[tokio::test]
async fn postgres_item_create_accepts_a_declaration_that_matches_the_opening_stock() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-ITEM-OK-{suffix}");

    let mut create = command(
        "inventory.item.create",
        format!("r8-pg-item-create-match-{suffix}"),
        CreateInventoryItem {
            sku: sku.clone(),
            name: "Seeded stock".into(),
            initial_quantity: Some(dec!(40)),
            ..Default::default()
        },
    );
    create.commitment = declaring("40");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_create_inventory_item_async(&create)
        .await
        .expect("create inventory item");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    assert_eq!(
        db.inventory()
            .get_stock_async(&sku)
            .await
            .expect("stock query")
            .expect("stock")
            .total_on_hand,
        dec!(40)
    );
}

/// An over-request is clamped by the repository — `quantity >= reserved`
/// confirms the reservation in full — so the observed figure is the clamped
/// movement, not the payload. Binding to the raw payload both refused a
/// correct declaration and would have accepted an inflated one.
#[tokio::test]
async fn postgres_confirm_binds_the_clamped_movement_not_the_over_request() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-CONFIRM-CLAMP-{suffix}");
    stock(&db, &sku, dec!(200)).await;
    let reservation = reservation_of(&db, &sku, dec!(40)).await;

    let mut over = command(
        "inventory.reservation.confirm",
        format!("r8-pg-confirm-over-request-match-{suffix}"),
        ConfirmInventoryReservation { reservation_id: reservation, quantity: Some(dec!(60)) },
    );
    over.commitment = declaring("40");
    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation_async(&over)
        .await
        .expect("confirm reservation");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    assert_eq!(allocated(&db, &sku).await, dec!(40));

    let second = reservation_of(&db, &sku, dec!(40)).await;
    let mut inflated = command(
        "inventory.reservation.confirm",
        format!("r8-pg-confirm-over-request-inflated-{suffix}"),
        ConfirmInventoryReservation { reservation_id: second, quantity: Some(dec!(60)) },
    );
    inflated.commitment = declaring("45");
    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation_async(&inflated)
        .await
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected, "{receipt:?}");
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
}

#[tokio::test]
async fn postgres_confirm_rejects_a_declared_quantity_the_reservation_does_not_move() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-CONFIRM-SUB-{suffix}");
    stock(&db, &sku, dec!(2000)).await;
    let reservation = reservation_of(&db, &sku, dec!(1000)).await;

    let mut confirm = command(
        "inventory.reservation.confirm",
        format!("r8-pg-confirm-substitution-{suffix}"),
        ConfirmInventoryReservation { reservation_id: reservation, quantity: Some(dec!(1000)) },
    );
    confirm.commitment = declaring("1");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation_async(&confirm)
        .await
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    assert_eq!(allocated(&db, &sku).await, dec!(1000), "the hold must be untouched");
}

#[tokio::test]
async fn postgres_confirm_in_full_binds_the_whole_reservation_not_the_empty_payload() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-CONFIRM-FULL-{suffix}");
    stock(&db, &sku, dec!(2000)).await;
    let reservation = reservation_of(&db, &sku, dec!(1000)).await;

    let mut confirm = command(
        "inventory.reservation.confirm",
        format!("r8-pg-confirm-in-full-{suffix}"),
        ConfirmInventoryReservation { reservation_id: reservation, quantity: None },
    );
    confirm.commitment = declaring("1");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation_async(&confirm)
        .await
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
}

#[tokio::test]
async fn postgres_confirm_accepts_a_declaration_that_matches_the_confirmed_units() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-CONFIRM-OK-{suffix}");
    stock(&db, &sku, dec!(100)).await;
    let reservation = reservation_of(&db, &sku, dec!(40)).await;

    let mut confirm = command(
        "inventory.reservation.confirm",
        format!("r8-pg-confirm-match-{suffix}"),
        ConfirmInventoryReservation { reservation_id: reservation, quantity: Some(dec!(40)) },
    );
    confirm.commitment = declaring("40");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation_async(&confirm)
        .await
        .expect("confirm reservation");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}

async fn ready_checkout_cart(
    db: &PostgresDatabase,
    suffix: Uuid,
    quantity: i32,
) -> stateset_core::CartId {
    let email = format!("r8-pg-checkout-{suffix}@example.com");
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: email.clone(),
            first_name: "Round".into(),
            last_name: "Eight".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let carts = db.carts();
    let cart = carts
        .create_async(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(email.clone()),
            customer_name: Some("Round Eight".into()),
            ..Default::default()
        })
        .await
        .expect("create cart");
    carts
        .add_item_async(
            cart.id.into_uuid(),
            AddCartItem {
                product_id: None,
                sku: format!("R8-PG-CHECKOUT-{suffix}"),
                name: "Round Eight Item".into(),
                quantity,
                unit_price: dec!(1.00),
                ..Default::default()
            },
        )
        .await
        .expect("add item");
    carts
        .set_shipping_address_async(
            cart.id.into_uuid(),
            CartAddress {
                first_name: "Round".into(),
                last_name: "Eight".into(),
                line1: "1 Atomic Way".into(),
                city: "Vancouver".into(),
                state: Some("BC".into()),
                postal_code: "V6B 1A1".into(),
                country: "CA".into(),
                email: Some(email),
                ..Default::default()
            },
        )
        .await
        .expect("set shipping");
    carts
        .set_payment_async(
            cart.id.into_uuid(),
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_round8".into()),
                ..Default::default()
            },
        )
        .await
        .expect("set payment");
    cart.id
}

#[tokio::test]
async fn postgres_checkout_rejects_a_declared_quantity_below_the_summed_cart_lines() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let cart = ready_checkout_cart(&db, suffix, 30).await;
    let total =
        db.carts().get_async(cart.into_uuid()).await.expect("load cart").expect("cart").grand_total;

    for mode in [ExecutionMode::Preview, ExecutionMode::Apply] {
        let mut checkout = command(
            "checkout.commit",
            format!("r8-pg-checkout-substitution-{mode:?}-{suffix}"),
            CommitCheckout::new(cart),
        );
        checkout.mode = mode;
        checkout.commitment = declaring_units_and_money("1", total);
        let receipt = db
            .kernel_executor(quantity_policy())
            .execute_commit_checkout_async(&checkout)
            .await
            .expect("sealed rejection");
        assert_eq!(receipt.status, ExecutionStatus::Rejected, "{mode:?}: {receipt:?}");
        assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    }
    let orders: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM orders WHERE cart_id = $1")
        .bind(cart.into_uuid())
        .fetch_one(db.pool())
        .await
        .expect("count orders");
    assert_eq!(orders, 0, "a rejected checkout must not leave an order behind");
}

#[tokio::test]
async fn postgres_checkout_accepts_a_declaration_that_matches_the_summed_cart_lines() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let cart = ready_checkout_cart(&db, suffix, 30).await;
    let total =
        db.carts().get_async(cart.into_uuid()).await.expect("load cart").expect("cart").grand_total;

    let mut checkout = command(
        "checkout.commit",
        format!("r8-pg-checkout-match-{suffix}"),
        CommitCheckout::new(cart),
    );
    checkout.commitment = declaring_units_and_money("30", total);
    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_commit_checkout_async(&checkout)
        .await
        .expect("commit checkout");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}

async fn processing_order(db: &PostgresDatabase, sku: &str, quantity: i32) -> Order {
    stock(db, sku, dec!(1000)).await;
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("{sku}@example.com"),
            first_name: "Round".into(),
            last_name: "Eight".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.into(),
                name: "Round Eight item".into(),
                quantity,
                unit_price: dec!(1.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    db.orders()
        .update_async(
            order.id.into_uuid(),
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .await
        .expect("confirm order");
    db.orders()
        .update_async(
            order.id.into_uuid(),
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .await
        .expect("process order")
}

#[tokio::test]
async fn postgres_shipment_rejects_a_declared_quantity_below_the_units_it_moves() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-SHIP-SUB-{suffix}");
    let order = processing_order(&db, &sku, 30).await;

    let mut ship = command(
        "orders.ship",
        format!("r8-pg-ship-substitution-{suffix}"),
        ShipOrderCommand { order_id: order.id, tracking_number: None, lines: None },
    );
    ship.commitment = declaring("1");
    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_ship_order_async(&ship)
        .await
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    let unchanged =
        db.orders().get_async(order.id.into_uuid()).await.expect("load order").expect("order");
    assert_eq!(unchanged.status, OrderStatus::Processing);
    assert_eq!(unchanged.items[0].shipped_quantity, 0);
}

#[tokio::test]
async fn postgres_shipment_accepts_a_declaration_that_matches_the_units_it_moves() {
    let db = require_db!();
    let suffix = Uuid::new_v4();
    let sku = format!("R8-PG-SHIP-OK-{suffix}");
    let order = processing_order(&db, &sku, 30).await;

    let mut ship = command(
        "orders.ship",
        format!("r8-pg-ship-match-{suffix}"),
        ShipOrderCommand { order_id: order.id, tracking_number: None, lines: None },
    );
    ship.commitment = declaring("30");
    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_ship_order_async(&ship)
        .await
        .expect("ship order");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    assert_eq!(receipt.result.expect("order").items[0].shipped_quantity, 30);
}
