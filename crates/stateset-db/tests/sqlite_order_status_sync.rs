#![cfg(feature = "sqlite")]

//! The kernel executor paths (`payments.create`, `orders.ship`,
//! `orders.transition`) keep an order's `payment_status` /
//! `fulfillment_status` in step with its payment ledger and shipments, in the
//! same transaction as the command. The MCP `create_payment` / `ship_order`
//! tools run through these paths.

use rust_decimal_macros::dec;
use stateset_core::{
    CommandEnvelope, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreatePayment, CurrencyCode, CustomerRepository, ExecutionMode, ExecutionStatus,
    FulfillmentStatus, InventoryRepository, KernelCommandPolicy, KernelPolicy, KernelPrincipal,
    Order, OrderId, OrderRepository, OrderStatus, PaymentMethodType, PaymentRepository,
    PaymentStatus, PrincipalKind, ProductId, ShipOrderCommand, ShipmentLineInput, TransitionOrder,
    UpdateOrder,
};
use stateset_db::SqliteDatabase;

fn policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]))
        .allow("orders.transition", KernelCommandPolicy::requiring(["orders.transition"]))
        .allow("orders.ship", KernelCommandPolicy::requiring(["orders.ship"]))
}

fn apply<T>(command_type: &str, key: &str, payload: T) -> CommandEnvelope<T> {
    let mut command = CommandEnvelope::preview(
        command_type,
        key,
        KernelPrincipal {
            id: "agent:ops-1".into(),
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

/// A 20.00 USD order (2 x 10.00) with stock to ship.
fn order(db: &SqliteDatabase, sku: &str) -> Order {
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: sku.into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .expect("create stock");
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("{sku}@example.com"),
            first_name: "Kernel".into(),
            last_name: "Sync".into(),
            ..Default::default()
        })
        .expect("create customer");
    db.orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.into(),
                name: "Kernel item".into(),
                quantity: 2,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order")
}

fn get(db: &SqliteDatabase, id: OrderId) -> Order {
    db.orders().get(id).expect("get").expect("order exists")
}

fn kernel_payment(db: &SqliteDatabase, key: &str, order: &Order) -> stateset_core::Payment {
    let receipt = db
        .kernel_executor(policy())
        .execute_create_payment(&apply(
            "payments.create",
            key,
            CreatePayment {
                order_id: Some(order.id),
                payment_method: PaymentMethodType::CreditCard,
                amount: order.total_amount,
                currency: Some(CurrencyCode::USD),
                ..Default::default()
            },
        ))
        .expect("kernel payment");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
    receipt.result.expect("payment")
}

#[test]
fn kernel_ship_and_deliver_move_fulfillment_status() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = order(&db, "SYNC-KERNEL-SHIP");
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .expect("advance");
    }
    let line = order.items[0].id;

    let partial = db
        .kernel_executor(policy())
        .execute_ship_order(&apply(
            "orders.ship",
            "sync-ship-1",
            ShipOrderCommand {
                order_id: order.id,
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: line, quantity: 1 }]),
            },
        ))
        .expect("partial ship");
    assert_eq!(partial.status, ExecutionStatus::Succeeded);
    let shipped = partial.result.expect("order");
    assert_eq!(shipped.status, OrderStatus::PartiallyShipped);
    assert_eq!(shipped.fulfillment_status, FulfillmentStatus::PartiallyFulfilled);

    let rest = db
        .kernel_executor(policy())
        .execute_ship_order(&apply(
            "orders.ship",
            "sync-ship-2",
            ShipOrderCommand { order_id: order.id, tracking_number: None, lines: None },
        ))
        .expect("ship rest")
        .result
        .expect("order");
    assert_eq!(rest.status, OrderStatus::Shipped);
    assert_eq!(rest.fulfillment_status, FulfillmentStatus::Shipped);

    let delivered = db
        .kernel_executor(policy())
        .execute_transition_order(&apply(
            "orders.transition",
            "sync-deliver",
            TransitionOrder {
                order_id: order.id,
                status: OrderStatus::Delivered,
                payment_status: None,
                void_payments: false,
            },
        ))
        .expect("deliver")
        .result
        .expect("order");
    assert_eq!(delivered.fulfillment_status, FulfillmentStatus::Delivered);
    assert_eq!(get(&db, order.id).fulfillment_status, FulfillmentStatus::Delivered);
}

#[test]
fn kernel_payment_and_completion_move_payment_status() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = order(&db, "SYNC-KERNEL-PAY");

    // A failed attempt marks the order failed ...
    let first = kernel_payment(&db, "sync-pay-1", &order);
    db.payments().mark_failed(first.id, "declined", None).expect("fail");
    assert_eq!(get(&db, order.id).payment_status, PaymentStatus::Failed);

    // ... and a new kernel payment puts it back to pending, in the same commit,
    // with the order fact linked on the receipt.
    let before = get(&db, order.id);
    let receipt = db
        .kernel_executor(policy())
        .execute_create_payment(&apply(
            "payments.create",
            "sync-pay-2",
            CreatePayment {
                order_id: Some(order.id),
                payment_method: PaymentMethodType::CreditCard,
                amount: order.total_amount,
                currency: Some(CurrencyCode::USD),
                ..Default::default()
            },
        ))
        .expect("retry payment");
    assert_eq!(receipt.event_ids.len(), 2, "payment fact + order status fact");
    let after = get(&db, order.id);
    assert_eq!(after.payment_status, PaymentStatus::Pending);
    assert_eq!(after.version, before.version + 1);

    let second = receipt.result.expect("payment");
    db.payments().mark_completed(second.id).expect("complete");
    assert_eq!(get(&db, order.id).payment_status, PaymentStatus::Paid);
}

#[test]
fn kernel_cancel_with_void_rederives_payment_status_in_one_write() {
    let db = SqliteDatabase::in_memory().expect("db");
    let order = order(&db, "SYNC-KERNEL-VOID");
    let payment = kernel_payment(&db, "sync-void-pay", &order);
    db.payments().mark_processing(payment.id).expect("processing");
    let authorized = get(&db, order.id);
    assert_eq!(authorized.payment_status, PaymentStatus::Authorized);

    let receipt = db
        .kernel_executor(policy())
        .execute_transition_order(&apply(
            "orders.transition",
            "sync-void-cancel",
            TransitionOrder {
                order_id: order.id,
                status: OrderStatus::Cancelled,
                payment_status: None,
                void_payments: true,
            },
        ))
        .expect("cancel");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded);
    let cancelled = receipt.result.expect("order");
    assert_eq!(cancelled.status, OrderStatus::Cancelled);
    assert_eq!(cancelled.payment_status, PaymentStatus::Pending, "the voided hold is gone");
    assert_eq!(cancelled.version, authorized.version + 1, "one version bump");
}
