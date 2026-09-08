#![cfg(feature = "sqlite")]
//! Kernel executor round 8 (SQLite): the observed-quantity binding.
//!
//! A `max_quantity` ceiling is only a ceiling if the executor proves the
//! declared figure is the one the domain acts on. These proofs declare one
//! unit under a fifty-unit ceiling and move far more, on every command that
//! carries a quantity. Every scenario has a Postgres mirror in
//! `postgres_kernel_round8.rs`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AddCartItem, CartAddress, CartRepository, CommandEnvelope, CommitCheckout,
    ConfirmInventoryReservation, CreateCart, CreateCustomer, CreateInventoryItem, CreateOrder,
    CreateOrderItem, CurrencyCode, CustomerRepository, EconomicCommitment, ExecutionMode,
    ExecutionStatus, InventoryRepository, KernelCommandPolicy, KernelPolicy, KernelPrincipal,
    Money, OrderRepository, OrderStatus, PrincipalKind, ProductId, ReserveInventory,
    SetCartPayment, ShipOrderCommand, UpdateOrder,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

/// Every quantity-carrying command capped at fifty units.
fn quantity_policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
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

fn principal(capability: &str) -> KernelPrincipal {
    KernelPrincipal {
        id: "agent:round8".into(),
        kind: PrincipalKind::Agent,
        tenant_id: Some("tenant-1".into()),
        delegated_by: Some("user-1".into()),
        capabilities: vec![capability.into()],
    }
}

fn command<C>(command_type: &str, key: &str, payload: C) -> CommandEnvelope<C> {
    let mut command = CommandEnvelope::preview(command_type, key, principal(command_type), payload);
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

fn create_stock(db: &SqliteDatabase, sku: &str, quantity: Decimal) {
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: format!("Stock {sku}"),
            initial_quantity: Some(quantity),
            ..Default::default()
        })
        .expect("create stock");
}

fn reservation_of(db: &SqliteDatabase, sku: &str, quantity: Decimal) -> Uuid {
    db.inventory()
        .reserve(ReserveInventory {
            sku: sku.into(),
            location_id: Some(1),
            quantity,
            reference_type: "order".into(),
            reference_id: format!("order-{sku}"),
            expires_in_seconds: Some(900),
        })
        .expect("reserve inventory")
        .id
}

fn allocated(db: &SqliteDatabase, sku: &str) -> Decimal {
    db.inventory().get_stock(sku).expect("stock query").expect("stock").total_allocated
}

#[test]
fn confirm_rejects_a_declared_quantity_the_reservation_does_not_move() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "R8-CONFIRM-SUB", dec!(2000));
    let reservation = reservation_of(&db, "R8-CONFIRM-SUB", dec!(1000));

    let mut confirm = command(
        "inventory.reservation.confirm",
        "r8-confirm-substitution",
        ConfirmInventoryReservation { reservation_id: reservation, quantity: Some(dec!(1000)) },
    );
    confirm.commitment = declaring("1");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation(&confirm)
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    assert_eq!(allocated(&db, "R8-CONFIRM-SUB"), dec!(1000), "the hold must be untouched");
}

#[test]
fn confirm_in_full_binds_the_whole_reservation_not_the_empty_payload() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "R8-CONFIRM-FULL", dec!(2000));
    let reservation = reservation_of(&db, "R8-CONFIRM-FULL", dec!(1000));

    let mut confirm = command(
        "inventory.reservation.confirm",
        "r8-confirm-in-full",
        ConfirmInventoryReservation { reservation_id: reservation, quantity: None },
    );
    confirm.commitment = declaring("1");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation(&confirm)
        .expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
}

#[test]
fn confirm_accepts_a_declaration_that_matches_the_confirmed_units() {
    let db = SqliteDatabase::in_memory().expect("create database");
    create_stock(&db, "R8-CONFIRM-OK", dec!(100));
    let reservation = reservation_of(&db, "R8-CONFIRM-OK", dec!(40));

    let mut confirm = command(
        "inventory.reservation.confirm",
        "r8-confirm-match",
        ConfirmInventoryReservation { reservation_id: reservation, quantity: Some(dec!(40)) },
    );
    confirm.commitment = declaring("40");

    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_confirm_inventory_reservation(&confirm)
        .expect("confirm reservation");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}

fn ready_checkout_cart(db: &SqliteDatabase, email: &str, quantity: i32) -> stateset_core::CartId {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: email.into(),
            first_name: "Round".into(),
            last_name: "Eight".into(),
            ..Default::default()
        })
        .expect("create customer");
    let carts = db.carts();
    let cart = carts
        .create(CreateCart {
            customer_id: Some(customer.id),
            customer_email: Some(email.into()),
            customer_name: Some("Round Eight".into()),
            ..Default::default()
        })
        .expect("create cart");
    carts
        .add_item(
            cart.id,
            AddCartItem {
                product_id: Some(ProductId::new()),
                sku: "R8-CHECKOUT-SKU".into(),
                name: "Round Eight Item".into(),
                quantity,
                unit_price: dec!(1.00),
                ..Default::default()
            },
        )
        .expect("add item");
    carts
        .set_shipping_address(
            cart.id,
            CartAddress {
                first_name: "Round".into(),
                last_name: "Eight".into(),
                line1: "1 Atomic Way".into(),
                city: "Vancouver".into(),
                state: Some("BC".into()),
                postal_code: "V6B 1A1".into(),
                country: "CA".into(),
                email: Some(email.into()),
                ..Default::default()
            },
        )
        .expect("set shipping");
    carts
        .set_payment(
            cart.id,
            SetCartPayment {
                payment_method: "credit_card".into(),
                payment_token: Some("tok_round8".into()),
                ..Default::default()
            },
        )
        .expect("set payment");
    cart.id
}

#[test]
fn checkout_rejects_a_declared_quantity_below_the_summed_cart_lines() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let cart = ready_checkout_cart(&db, "r8-checkout-sub@example.com", 30);
    let total = db.carts().get(cart).expect("load cart").expect("cart").grand_total;

    for mode in [ExecutionMode::Preview, ExecutionMode::Apply] {
        let mut checkout = command(
            "checkout.commit",
            &format!("r8-checkout-substitution-{mode:?}"),
            CommitCheckout::new(cart),
        );
        checkout.mode = mode;
        checkout.commitment = declaring_units_and_money("1", total);
        let receipt = db
            .kernel_executor(quantity_policy())
            .execute_commit_checkout(&checkout)
            .expect("sealed rejection");
        assert_eq!(receipt.status, ExecutionStatus::Rejected, "{mode:?}: {receipt:?}");
        assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    }
    let connection = db.pool().get().expect("connection");
    let orders: i64 =
        connection.query_row("SELECT COUNT(*) FROM orders", [], |row| row.get(0)).expect("orders");
    assert_eq!(orders, 0, "a rejected checkout must not leave an order behind");
}

#[test]
fn checkout_accepts_a_declaration_that_matches_the_summed_cart_lines() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let cart = ready_checkout_cart(&db, "r8-checkout-ok@example.com", 30);
    let total = db.carts().get(cart).expect("load cart").expect("cart").grand_total;

    let mut checkout = command("checkout.commit", "r8-checkout-match", CommitCheckout::new(cart));
    checkout.commitment = declaring_units_and_money("30", total);
    let receipt = db
        .kernel_executor(quantity_policy())
        .execute_commit_checkout(&checkout)
        .expect("commit checkout");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
}

fn processing_order(db: &SqliteDatabase, sku: &str, quantity: i32) -> stateset_core::Order {
    create_stock(db, sku, dec!(1000));
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("{sku}@example.com"),
            first_name: "Round".into(),
            last_name: "Eight".into(),
            ..Default::default()
        })
        .expect("create customer");
    let order = db
        .orders()
        .create(CreateOrder {
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
        .expect("create order");
    db.orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Confirmed), ..Default::default() },
        )
        .expect("confirm order");
    db.orders()
        .update(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Processing), ..Default::default() },
        )
        .expect("process order")
}

#[test]
fn shipment_rejects_a_declared_quantity_below_the_units_it_moves() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let order = processing_order(&db, "R8-SHIP-SUB", 30);

    let mut ship = command(
        "orders.ship",
        "r8-ship-substitution",
        ShipOrderCommand { order_id: order.id, tracking_number: None, lines: None },
    );
    ship.commitment = declaring("1");
    let receipt =
        db.kernel_executor(quantity_policy()).execute_ship_order(&ship).expect("sealed rejection");
    assert_eq!(receipt.status, ExecutionStatus::Rejected);
    assert_eq!(receipt.error_code.as_deref(), Some("kernel.commitment_quantity_mismatch"));
    let unchanged = db.orders().get(order.id).expect("load order").expect("order");
    assert_eq!(unchanged.status, OrderStatus::Processing);
    assert_eq!(unchanged.items[0].shipped_quantity, 0);
}

#[test]
fn shipment_accepts_a_declaration_that_matches_the_units_it_moves() {
    let db = SqliteDatabase::in_memory().expect("create database");
    let order = processing_order(&db, "R8-SHIP-OK", 30);

    let mut ship = command(
        "orders.ship",
        "r8-ship-match",
        ShipOrderCommand { order_id: order.id, tracking_number: None, lines: None },
    );
    ship.commitment = declaring("30");
    let receipt =
        db.kernel_executor(quantity_policy()).execute_ship_order(&ship).expect("ship order");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    assert_eq!(receipt.result.expect("order").items[0].shipped_quantity, 30);
}
