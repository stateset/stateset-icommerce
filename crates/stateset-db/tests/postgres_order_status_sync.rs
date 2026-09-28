#![cfg(feature = "postgres")]

//! Postgres twin of `sqlite_order_status_sync.rs`: the kernel executor paths
//! (`payments.create`, `orders.ship`, `orders.transition`) keep an order's
//! `payment_status` / `fulfillment_status` in step with its payment ledger and
//! shipments. Skipped without `POSTGRES_URL` / `DATABASE_URL`.

use rust_decimal_macros::dec;
use stateset_core::{
    CommandEnvelope, CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem,
    CreatePayment, CurrencyCode, ExecutionMode, ExecutionStatus, FulfillmentStatus,
    KernelCommandPolicy, KernelPolicy, KernelPrincipal, Order, OrderStatus, PaymentMethodType,
    PaymentStatus, PrincipalKind, ProductId, ShipOrderCommand, ShipmentLineInput, TransitionOrder,
    UpdateOrder,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

async fn connect() -> Option<PostgresDatabase> {
    let url = std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())?;
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

fn policy() -> KernelPolicy {
    KernelPolicy::new("commerce-policy-1")
        .allow("payments.create", KernelCommandPolicy::requiring(["payments.create"]))
        .allow("orders.transition", KernelCommandPolicy::requiring(["orders.transition"]))
        .allow("orders.ship", KernelCommandPolicy::requiring(["orders.ship"]))
}

fn apply<T>(command_type: &str, key: String, payload: T) -> CommandEnvelope<T> {
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
async fn order(db: &PostgresDatabase, sku: &str) -> Order {
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.into(),
            name: sku.into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .await
        .expect("create stock");
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("{sku}@example.com"),
            first_name: "Kernel".into(),
            last_name: "Sync".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    db.orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: sku.into(),
                name: "Kernel item".into(),
                quantity: 2,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .await
        .expect("create order")
}

async fn get(db: &PostgresDatabase, order: &Order) -> Order {
    db.orders().get_async(order.id.into_uuid()).await.expect("get").expect("order exists")
}

fn payment_input(order: &Order) -> CreatePayment {
    CreatePayment {
        order_id: Some(order.id),
        payment_method: PaymentMethodType::CreditCard,
        amount: order.total_amount,
        currency: Some(CurrencyCode::USD),
        ..Default::default()
    }
}

#[tokio::test]
async fn postgres_kernel_ship_and_deliver_move_fulfillment_status() {
    let db = require_db!();
    let suffix = Uuid::new_v4().simple().to_string();
    let order = order(&db, &format!("SYNC-PG-SHIP-{suffix}")).await;
    for status in [OrderStatus::Confirmed, OrderStatus::Processing] {
        db.orders()
            .update_async(
                order.id.into_uuid(),
                UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .expect("advance");
    }
    let line = order.items[0].id;

    let partial = db
        .kernel_executor(policy())
        .execute_ship_order_async(&apply(
            "orders.ship",
            format!("sync-pg-ship-1-{suffix}"),
            ShipOrderCommand {
                order_id: order.id,
                tracking_number: None,
                lines: Some(vec![ShipmentLineInput { order_item_id: line, quantity: 1 }]),
            },
        ))
        .await
        .expect("partial ship");
    assert_eq!(partial.status, ExecutionStatus::Succeeded, "{partial:?}");
    let shipped = partial.result.expect("order");
    assert_eq!(shipped.status, OrderStatus::PartiallyShipped);
    assert_eq!(shipped.fulfillment_status, FulfillmentStatus::PartiallyFulfilled);

    let rest = db
        .kernel_executor(policy())
        .execute_ship_order_async(&apply(
            "orders.ship",
            format!("sync-pg-ship-2-{suffix}"),
            ShipOrderCommand { order_id: order.id, tracking_number: None, lines: None },
        ))
        .await
        .expect("ship rest")
        .result
        .expect("order");
    assert_eq!(rest.status, OrderStatus::Shipped);
    assert_eq!(rest.fulfillment_status, FulfillmentStatus::Shipped);

    let delivered = db
        .kernel_executor(policy())
        .execute_transition_order_async(&apply(
            "orders.transition",
            format!("sync-pg-deliver-{suffix}"),
            TransitionOrder {
                order_id: order.id,
                status: OrderStatus::Delivered,
                payment_status: None,
                void_payments: false,
            },
        ))
        .await
        .expect("deliver")
        .result
        .expect("order");
    assert_eq!(delivered.fulfillment_status, FulfillmentStatus::Delivered);
    assert_eq!(get(&db, &order).await.fulfillment_status, FulfillmentStatus::Delivered);
}

#[tokio::test]
async fn postgres_kernel_payment_and_completion_move_payment_status() {
    let db = require_db!();
    let suffix = Uuid::new_v4().simple().to_string();
    let order = order(&db, &format!("SYNC-PG-PAY-{suffix}")).await;

    let first = db
        .kernel_executor(policy())
        .execute_create_payment_async(&apply(
            "payments.create",
            format!("sync-pg-pay-1-{suffix}"),
            payment_input(&order),
        ))
        .await
        .expect("kernel payment")
        .result
        .expect("payment");
    db.payments().mark_failed_async(first.id.into_uuid(), "declined", None).await.expect("fail");
    assert_eq!(get(&db, &order).await.payment_status, PaymentStatus::Failed);

    let before = get(&db, &order).await;
    let receipt = db
        .kernel_executor(policy())
        .execute_create_payment_async(&apply(
            "payments.create",
            format!("sync-pg-pay-2-{suffix}"),
            payment_input(&order),
        ))
        .await
        .expect("retry payment");
    assert_eq!(receipt.event_ids.len(), 2, "payment fact + order status fact");
    let after = get(&db, &order).await;
    assert_eq!(after.payment_status, PaymentStatus::Pending);
    assert_eq!(after.version, before.version + 1);

    let second = receipt.result.expect("payment");
    db.payments().mark_completed_async(second.id.into_uuid()).await.expect("complete");
    assert_eq!(get(&db, &order).await.payment_status, PaymentStatus::Paid);
}

#[tokio::test]
async fn postgres_kernel_cancel_with_void_rederives_payment_status_in_one_write() {
    let db = require_db!();
    let suffix = Uuid::new_v4().simple().to_string();
    let order = order(&db, &format!("SYNC-PG-VOID-{suffix}")).await;
    let payment = db
        .kernel_executor(policy())
        .execute_create_payment_async(&apply(
            "payments.create",
            format!("sync-pg-void-pay-{suffix}"),
            payment_input(&order),
        ))
        .await
        .expect("kernel payment")
        .result
        .expect("payment");
    db.payments().mark_processing_async(payment.id.into_uuid()).await.expect("processing");
    let authorized = get(&db, &order).await;
    assert_eq!(authorized.payment_status, PaymentStatus::Authorized);

    let receipt = db
        .kernel_executor(policy())
        .execute_transition_order_async(&apply(
            "orders.transition",
            format!("sync-pg-void-cancel-{suffix}"),
            TransitionOrder {
                order_id: order.id,
                status: OrderStatus::Cancelled,
                payment_status: None,
                void_payments: true,
            },
        ))
        .await
        .expect("cancel");
    assert_eq!(receipt.status, ExecutionStatus::Succeeded, "{receipt:?}");
    let cancelled = receipt.result.expect("order");
    assert_eq!(cancelled.status, OrderStatus::Cancelled);
    assert_eq!(cancelled.payment_status, PaymentStatus::Pending, "the voided hold is gone");
    assert_eq!(cancelled.version, authorized.version + 1, "one version bump");
}
