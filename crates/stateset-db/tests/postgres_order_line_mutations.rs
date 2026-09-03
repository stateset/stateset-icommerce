#![cfg(feature = "postgres")]

//! Postgres mirror of `sqlite_order_line_mutations.rs`: order line mutation,
//! batch update and delete semantics. Skips without `POSTGRES_URL` /
//! `DATABASE_URL`.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    BackorderStatus, CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder,
    CreateOrderItem, CustomerId, OrderFilter, OrderStatus, ProductId, ReservationStatus,
    UpdateOrder,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

struct Ctx {
    db: PostgresDatabase,
    customer_id: CustomerId,
    sku_a: String,
    sku_b: String,
}

async fn setup() -> Option<Ctx> {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres line mutation test");
            return None;
        }
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let unique = Uuid::new_v4();
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("lines-{unique}@example.com"),
            first_name: "Line".into(),
            last_name: "Tester".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    let sku_a = format!("LINE-A-{unique}");
    let sku_b = format!("LINE-B-{unique}");
    for sku in [&sku_a, &sku_b] {
        db.inventory()
            .create_item_async(CreateInventoryItem {
                sku: sku.clone(),
                name: format!("Widget {sku}"),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .await
            .expect("create inventory item");
    }
    Some(Ctx { db, customer_id: customer.id, sku_a, sku_b })
}

fn line(sku: &str, quantity: i32, unit_price: Decimal) -> CreateOrderItem {
    CreateOrderItem {
        product_id: ProductId::new(),
        sku: sku.to_string(),
        name: format!("Widget {sku}"),
        quantity,
        unit_price,
        ..Default::default()
    }
}

impl Ctx {
    /// One line of 2 × 10.00 plus 1.50 tax, 5.00 shipping, 2.00 discount → 24.50.
    async fn order_with_order_level_money(&self) -> stateset_core::Order {
        self.db
            .orders()
            .create_async(CreateOrder {
                customer_id: self.customer_id,
                items: vec![line(&self.sku_a, 2, dec!(10.00))],
                tax_amount: Some(dec!(1.50)),
                shipping_amount: Some(dec!(5.00)),
                discount_amount: Some(dec!(2.00)),
                ..Default::default()
            })
            .await
            .expect("create order")
    }

    async fn get(&self, id: Uuid) -> stateset_core::Order {
        self.db.orders().get_async(id).await.expect("get order").expect("order exists")
    }

    async fn advance(&self, id: Uuid, statuses: &[OrderStatus]) -> stateset_core::Order {
        let mut last = None;
        for status in statuses {
            last = Some(
                self.db
                    .orders()
                    .update_async(id, UpdateOrder { status: Some(*status), ..Default::default() })
                    .await
                    .unwrap_or_else(|e| panic!("transition to {status}: {e:?}")),
            );
        }
        last.expect("at least one transition")
    }

    async fn open_reserved_qty(&self, id: Uuid) -> Decimal {
        self.db
            .inventory()
            .list_reservations_by_reference_async("order", &id.to_string())
            .await
            .expect("list reservations")
            .into_iter()
            .filter(|r| {
                matches!(r.status, ReservationStatus::Pending | ReservationStatus::Allocated)
            })
            .map(|r| r.quantity)
            .sum()
    }

    async fn available(&self, sku: &str) -> Decimal {
        self.db
            .inventory()
            .get_stock_async(sku)
            .await
            .expect("stock")
            .expect("exists")
            .total_available
    }

    async fn backorders(&self, id: Uuid) -> Vec<stateset_core::Backorder> {
        self.db.backorder().get_backorders_for_order_async(id).await.expect("backorders")
    }
}

fn expected_total(order: &stateset_core::Order) -> Decimal {
    let lines: Decimal = order.items.iter().map(|i| i.total).sum();
    lines + order.tax_amount + order.shipping_amount - order.discount_amount
}

// Defect 1 ------------------------------------------------------------------

#[tokio::test]
async fn postgres_add_and_remove_item_keep_order_level_money_in_total() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    assert_eq!(order.total_amount, dec!(24.50));
    assert_eq!(order.total_amount, order.calculate_total());

    let added =
        ctx.db.orders().add_item_async(id, line(&ctx.sku_b, 1, dec!(3.00))).await.expect("add");
    let after_add = ctx.get(id).await;
    assert_eq!(after_add.tax_amount, dec!(1.50));
    assert_eq!(after_add.total_amount, dec!(27.50), "20 + 3 + 1.50 + 5 - 2");
    assert_eq!(after_add.total_amount, expected_total(&after_add));
    assert_eq!(after_add.total_amount, after_add.calculate_total());

    ctx.db.orders().remove_item_async(id, added.id.into_uuid()).await.expect("remove");
    let after_remove = ctx.get(id).await;
    assert_eq!(after_remove.total_amount, dec!(24.50));
    assert_eq!(after_remove.total_amount, after_remove.calculate_total());
}

// Defect 2 ------------------------------------------------------------------

#[tokio::test]
async fn postgres_add_item_reserves_stock_and_backorders_shortfall() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(2));

    let added =
        ctx.db.orders().add_item_async(id, line(&ctx.sku_b, 12, dec!(1.00))).await.expect("add");
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(12), "2 (A) + 10 (B)");
    assert_eq!(ctx.available(&ctx.sku_b).await, dec!(0));

    let backorders = ctx.backorders(id).await;
    assert_eq!(backorders.len(), 1);
    assert_eq!(backorders[0].order_line_id, Some(added.id.into_uuid()));
    assert_eq!(backorders[0].quantity_ordered, dec!(2));
}

#[tokio::test]
async fn postgres_remove_item_releases_its_reservation_and_cancels_its_backorder() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    let added =
        ctx.db.orders().add_item_async(id, line(&ctx.sku_b, 12, dec!(1.00))).await.expect("add");
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(12));

    ctx.db.orders().remove_item_async(id, added.id.into_uuid()).await.expect("remove");

    assert_eq!(ctx.open_reserved_qty(id).await, dec!(2), "only A's reservation remains");
    assert_eq!(ctx.available(&ctx.sku_b).await, dec!(10));
    let backorders = ctx.backorders(id).await;
    assert_eq!(backorders.len(), 1);
    assert_eq!(backorders[0].status, BackorderStatus::Cancelled);

    let after = ctx.get(id).await;
    assert_eq!(after.items.len(), 1);
    assert_eq!(after.total_amount, dec!(24.50));
}

#[tokio::test]
async fn postgres_remove_item_rejects_a_line_from_another_order() {
    let Some(ctx) = setup().await else { return };
    let a = ctx.order_with_order_level_money().await;
    let b = ctx.order_with_order_level_money().await;

    let err = ctx
        .db
        .orders()
        .remove_item_async(a.id.into_uuid(), b.items[0].id.into_uuid())
        .await
        .expect_err("foreign line");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(ctx.get(b.id.into_uuid()).await.items.len(), 1);
}

#[tokio::test]
async fn postgres_line_mutation_is_refused_once_the_order_has_shipped() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    let shipped = ctx
        .advance(id, &[OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped])
        .await;
    assert_eq!(shipped.status, OrderStatus::Shipped);

    let err = ctx
        .db
        .orders()
        .add_item_async(id, line(&ctx.sku_b, 1, dec!(3.00)))
        .await
        .expect_err("add on shipped");
    match &err {
        CommerceError::Conflict(msg) => assert!(msg.contains("shipped"), "{msg}"),
        other => panic!("expected Conflict, got {other:?}"),
    }
    let err = ctx
        .db
        .orders()
        .remove_item_async(id, shipped.items[0].id.into_uuid())
        .await
        .expect_err("remove on shipped");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");

    let after = ctx.get(id).await;
    assert_eq!(after.items.len(), 1);
    assert_eq!(after.version, shipped.version, "refused mutation writes nothing");
    assert_eq!(after.total_amount, dec!(24.50));
}

#[tokio::test]
async fn postgres_line_mutation_is_refused_on_cancelled_orders() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.advance(id, &[OrderStatus::Cancelled]).await;

    let err = ctx
        .db
        .orders()
        .add_item_async(id, line(&ctx.sku_b, 1, dec!(3.00)))
        .await
        .expect_err("add on cancelled");
    match err {
        CommerceError::Conflict(msg) => assert!(msg.contains("cancelled"), "{msg}"),
        other => panic!("expected Conflict, got {other:?}"),
    }
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(0));
}

#[tokio::test]
async fn postgres_line_mutation_is_allowed_while_pre_fulfilment() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.advance(id, &[OrderStatus::Confirmed, OrderStatus::Processing]).await;

    let added =
        ctx.db.orders().add_item_async(id, line(&ctx.sku_b, 1, dec!(3.00))).await.expect("add");
    ctx.db.orders().remove_item_async(id, added.id.into_uuid()).await.expect("remove");
}

// Defect 3 ------------------------------------------------------------------

#[tokio::test]
async fn postgres_update_batch_atomic_cancel_releases_reservations_like_update() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.db.orders().add_item_async(id, line(&ctx.sku_b, 12, dec!(1.00))).await.expect("add");
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(12));

    let updated = ctx
        .db
        .orders()
        .update_batch_atomic_async(vec![(
            id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )])
        .await
        .expect("batch cancel");
    assert_eq!(updated[0].status, OrderStatus::Cancelled);
    assert_eq!(updated[0].items.len(), 2);

    assert_eq!(ctx.open_reserved_qty(id).await, dec!(0), "batch cancel released stock");
    assert!(ctx.backorders(id).await.iter().all(|b| b.status == BackorderStatus::Cancelled));
    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(10));
}

#[tokio::test]
async fn postgres_update_batch_atomic_ship_confirms_reservations_and_ships_lines() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.advance(id, &[OrderStatus::Confirmed, OrderStatus::Processing]).await;

    let updated = ctx
        .db
        .orders()
        .update_batch_atomic_async(vec![(
            id,
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )])
        .await
        .expect("batch ship");
    assert_eq!(updated[0].status, OrderStatus::Shipped);
    assert_eq!(updated[0].items[0].shipped_quantity, 2);

    let reservations = ctx
        .db
        .inventory()
        .list_reservations_by_reference_async("order", &id.to_string())
        .await
        .unwrap();
    assert!(
        reservations.iter().all(|r| r.status == ReservationStatus::Confirmed),
        "{reservations:?}"
    );
    // Confirming keeps the allocation against the SKU; nothing is released.
    let stock = ctx.db.inventory().get_stock_async(&ctx.sku_a).await.unwrap().unwrap();
    assert_eq!(stock.total_allocated, dec!(2), "shipped units stay allocated");
    assert_eq!(stock.total_available, dec!(8), "nothing released back to available");
}

#[tokio::test]
async fn postgres_update_batch_atomic_rejects_partially_shipped_like_update() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.advance(id, &[OrderStatus::Confirmed, OrderStatus::Processing]).await;

    let err = ctx
        .db
        .orders()
        .update_batch_atomic_async(vec![(
            id,
            UpdateOrder { status: Some(OrderStatus::PartiallyShipped), ..Default::default() },
        )])
        .await
        .expect_err("partially_shipped is derived");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(ctx.get(id).await.status, OrderStatus::Processing);
}

#[tokio::test]
async fn postgres_update_batch_atomic_rolls_back_every_row_on_one_failure() {
    let Some(ctx) = setup().await else { return };
    let a = ctx.order_with_order_level_money().await;
    let b = ctx.order_with_order_level_money().await;

    let err = ctx
        .db
        .orders()
        .update_batch_atomic_async(vec![
            (
                a.id.into_uuid(),
                UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
            ),
            (
                b.id.into_uuid(),
                UpdateOrder { status: Some(OrderStatus::Delivered), ..Default::default() },
            ),
        ])
        .await
        .expect_err("second row is an invalid transition");
    assert!(matches!(err, CommerceError::InvalidOrderStatusTransition { .. }), "{err:?}");

    assert_eq!(ctx.get(a.id.into_uuid()).await.status, OrderStatus::Pending);
    assert_eq!(ctx.open_reserved_qty(a.id.into_uuid()).await, dec!(2));
}

// Defect 4 ------------------------------------------------------------------

#[tokio::test]
async fn postgres_delete_releases_reservations_and_cancels_backorders() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.db.orders().add_item_async(id, line(&ctx.sku_b, 12, dec!(1.00))).await.expect("add");
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(12));

    ctx.db.orders().delete_async(id).await.expect("delete");

    assert!(ctx.db.orders().get_async(id).await.unwrap().is_none());
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(0));
    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(10));
    assert_eq!(ctx.available(&ctx.sku_b).await, dec!(10));
    let backorders = ctx.backorders(id).await;
    assert!(backorders.iter().all(|b| b.status == BackorderStatus::Cancelled), "{backorders:?}");
}

#[tokio::test]
async fn postgres_delete_batch_atomic_releases_reservations() {
    let Some(ctx) = setup().await else { return };
    let a = ctx.order_with_order_level_money().await;
    let b = ctx.order_with_order_level_money().await;
    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(6));

    ctx.db
        .orders()
        .delete_batch_atomic_async(vec![a.id.into_uuid(), b.id.into_uuid()])
        .await
        .expect("delete batch");

    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(10));
}

#[tokio::test]
async fn postgres_delete_refuses_shipped_orders_and_stays_idempotent_for_missing() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.advance(id, &[OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped]).await;

    let err = ctx.db.orders().delete_async(id).await.expect_err("shipped orders are records");
    match err {
        CommerceError::Conflict(msg) => assert!(msg.contains("shipped"), "{msg}"),
        other => panic!("expected Conflict, got {other:?}"),
    }
    assert!(ctx.db.orders().get_async(id).await.unwrap().is_some());

    ctx.db.orders().delete_async(Uuid::new_v4()).await.expect("idempotent delete");
}

// Defect 5 ------------------------------------------------------------------

#[tokio::test]
async fn postgres_create_validates_order_level_money() {
    let Some(ctx) = setup().await else { return };
    let base = || CreateOrder {
        customer_id: ctx.customer_id,
        items: vec![line(&ctx.sku_a, 1, dec!(10.00))],
        ..Default::default()
    };

    for (tax, shipping, discount) in [
        (Some(dec!(-0.01)), None, None),
        (None, Some(dec!(-1.00)), None),
        (None, None, Some(dec!(-1.00))),
    ] {
        let err = ctx
            .db
            .orders()
            .create_async(CreateOrder {
                tax_amount: tax,
                shipping_amount: shipping,
                discount_amount: discount,
                ..base()
            })
            .await
            .expect_err("negative order-level money");
        assert!(
            matches!(err, CommerceError::ValidationError(_) | CommerceError::InvalidInput { .. }),
            "{err:?}"
        );
    }

    for (tax, shipping, discount) in [
        (Some(dec!(1.005)), None, None),
        (None, Some(dec!(4.999)), None),
        (None, None, Some(dec!(0.001))),
    ] {
        let err = ctx
            .db
            .orders()
            .create_async(CreateOrder {
                tax_amount: tax,
                shipping_amount: shipping,
                discount_amount: discount,
                ..base()
            })
            .await
            .expect_err("three-scale money in USD");
        assert_eq!(err.invariant_code(), Some("commerce.money.scale_exceeds_currency"), "{err:?}");
    }

    let err = ctx
        .db
        .orders()
        .create_async(CreateOrder {
            shipping_amount: Some(dec!(2.00)),
            discount_amount: Some(dec!(12.01)),
            ..base()
        })
        .await
        .expect_err("total would be -0.01");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");

    let mine = ctx
        .db
        .orders()
        .list_async(OrderFilter { customer_id: Some(ctx.customer_id), ..Default::default() })
        .await
        .expect("list");
    assert!(mine.is_empty(), "nothing persisted for any rejected create");

    let free = ctx
        .db
        .orders()
        .create_async(CreateOrder {
            shipping_amount: Some(dec!(2.00)),
            discount_amount: Some(dec!(12.00)),
            ..base()
        })
        .await
        .expect("zero total is allowed");
    assert_eq!(free.total_amount, Decimal::ZERO);
}

// ---------------------------------------------------------------------------
// Round 4 mirrors: line-keyed reservations (migration 087), line-edit outbox
// events, and the money guard on delete.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_remove_item_releases_only_its_own_reservation_when_lines_share_a_sku() {
    let Some(ctx) = setup().await else { return };
    let order = ctx
        .db
        .orders()
        .create_async(CreateOrder {
            customer_id: ctx.customer_id,
            items: vec![line(&ctx.sku_a, 5, dec!(1.00)), line(&ctx.sku_a, 1, dec!(1.00))],
            ..Default::default()
        })
        .await
        .expect("create order");
    let id = order.id.into_uuid();
    let line_a = order.items.iter().find(|i| i.quantity == 5).expect("line A").id;
    let line_b = order.items.iter().find(|i| i.quantity == 1).expect("line B").id;
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(6));
    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(4));

    ctx.db.orders().remove_item_async(id, line_b.into_uuid()).await.expect("remove line B");

    assert_eq!(ctx.open_reserved_qty(id).await, dec!(5), "line A's reservation must survive");
    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(5));
    let after = ctx.get(id).await;
    assert_eq!(after.items.len(), 1);
    assert_eq!(after.items[0].id, line_a);

    ctx.db.orders().remove_item_async(id, line_a.into_uuid()).await.expect("remove line A");
    assert_eq!(ctx.open_reserved_qty(id).await, Decimal::ZERO);
    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(10));
}

#[tokio::test]
async fn postgres_legacy_unkeyed_reservations_still_release_by_sku() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    sqlx::query("UPDATE inventory_reservations SET order_item_id = NULL WHERE reference_id = $1")
        .bind(id.to_string())
        .execute(ctx.db.pool())
        .await
        .expect("strip line keys");
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(2));

    ctx.db
        .orders()
        .remove_item_async(id, order.items[0].id.into_uuid())
        .await
        .expect("remove legacy line");
    assert_eq!(ctx.open_reserved_qty(id).await, Decimal::ZERO);
    assert_eq!(ctx.available(&ctx.sku_a).await, dec!(10));
}

#[tokio::test]
async fn postgres_line_edits_write_kernel_outbox_events_in_the_same_transaction() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    let added = ctx
        .db
        .orders()
        .add_item_async(id, line(&ctx.sku_b, 3, dec!(2.00)))
        .await
        .expect("add item");
    ctx.db.orders().remove_item_async(id, added.id.into_uuid()).await.expect("remove item");

    let events = ctx.db.kernel_outbox().pending_async(1000).await.expect("pending");
    let added_event = events
        .iter()
        .find(|e| e.event_type == "orders.item_added.v1" && e.aggregate_id == id.to_string())
        .expect("orders.item_added.v1 event");
    assert_eq!(added_event.aggregate_type, "order");
    assert_eq!(added_event.payload["order_item_id"], added.id.to_string());
    assert_eq!(added_event.payload["sku"], ctx.sku_b);
    assert_eq!(added_event.payload["quantity"], 3);
    assert_eq!(added_event.payload["total_amount"], "30.5000", "24.50 + 3 × 2.00");
    let removed_event = events
        .iter()
        .find(|e| e.event_type == "orders.item_removed.v1" && e.aggregate_id == id.to_string())
        .expect("orders.item_removed.v1 event");
    assert_eq!(removed_event.payload["order_item_id"], added.id.to_string());
    assert_eq!(removed_event.payload["total_amount"], "24.5000");

    // A refused edit writes nothing.
    assert!(ctx.db.orders().remove_item_async(id, Uuid::new_v4()).await.is_err());
    let removed_count = ctx
        .db
        .kernel_outbox()
        .pending_async(1000)
        .await
        .expect("pending")
        .iter()
        .filter(|e| e.event_type == "orders.item_removed.v1" && e.aggregate_id == id.to_string())
        .count();
    assert_eq!(removed_count, 1);
}

#[tokio::test]
async fn postgres_delete_refuses_an_order_whose_payment_status_holds_money() {
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    ctx.db
        .orders()
        .update_async(
            id,
            UpdateOrder {
                payment_status: Some(stateset_core::PaymentStatus::Paid),
                ..Default::default()
            },
        )
        .await
        .expect("mark paid");

    let err = ctx.db.orders().delete_async(id).await.expect_err("paid orders are records");
    assert!(matches!(err, CommerceError::Conflict(ref m) if m.contains("paid")), "{err:?}");
    assert!(ctx.db.orders().get_async(id).await.unwrap().is_some());
    assert_eq!(ctx.open_reserved_qty(id).await, dec!(2), "nothing released on a refused delete");

    ctx.db
        .orders()
        .update_async(
            id,
            UpdateOrder {
                status: Some(OrderStatus::Cancelled),
                payment_status: Some(stateset_core::PaymentStatus::PartiallyRefunded),
                ..Default::default()
            },
        )
        .await
        .expect("cancel");
    let err = ctx.db.orders().delete_async(id).await.expect_err("partially refunded is a record");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert!(ctx.db.orders().get_async(id).await.unwrap().is_some());
}

#[tokio::test]
async fn postgres_delete_refuses_an_order_referenced_by_a_payment_row() {
    use stateset_core::{CreatePayment, PaymentMethodType};
    let Some(ctx) = setup().await else { return };
    let order = ctx.order_with_order_level_money().await;
    let id = order.id.into_uuid();
    let payment = ctx
        .db
        .payments()
        .create_async(CreatePayment {
            order_id: Some(order.id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(10.00),
            ..Default::default()
        })
        .await
        .expect("create payment");
    ctx.db
        .payments()
        .mark_failed_async(payment.id.into_uuid(), "declined", None)
        .await
        .expect("fail payment");

    let err = ctx.db.orders().delete_async(id).await.expect_err("orders with payment rows");
    assert!(matches!(err, CommerceError::Conflict(ref m) if m.contains("payments")), "{err:?}");
    assert!(ctx.db.orders().get_async(id).await.unwrap().is_some());
    assert_eq!(ctx.db.payments().for_order_async(id).await.unwrap().len(), 1);

    let err = ctx.db.orders().delete_batch_atomic_async(vec![id]).await.expect_err("batch");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
}
