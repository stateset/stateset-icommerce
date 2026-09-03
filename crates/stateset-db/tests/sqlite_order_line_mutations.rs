#![cfg(feature = "sqlite")]

//! Order line mutation, batch update and delete semantics on SQLite.
//!
//! Pins the five order defects from the Sep 2026 re-audit:
//!
//! 1. `add_item`/`remove_item` recomputed `total_amount` as the bare line sum,
//!    discarding the order-level tax/shipping/discount that `create` stores.
//! 2. `add_item`/`remove_item` had no status guard and did not touch stock:
//!    lines could be added to shipped orders, added lines were never reserved,
//!    and removed lines leaked their reservation and backorder.
//! 3. `update_batch_atomic` wrote `status` directly, skipping the shipment /
//!    reservation / backorder / outbox side effects `update` performs, and
//!    accepted `PartiallyShipped` which `update` rejects.
//! 4. `delete` leaked reservations and backorders.
//! 5. Order-level `tax_amount`/`shipping_amount`/`discount_amount` were not
//!    validated on create.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    BackorderRepository, BackorderStatus, CommerceError, CreateCustomer, CreateInventoryItem,
    CreateOrder, CreateOrderItem, CustomerId, CustomerRepository, InventoryRepository, OrderId,
    OrderRepository, OrderStatus, ProductId, ReservationStatus, UpdateOrder,
};
use stateset_db::SqliteDatabase;

const SKU_A: &str = "LINE-SKU-A";
const SKU_B: &str = "LINE-SKU-B";

fn setup() -> (SqliteDatabase, CustomerId) {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: "lines@example.com".to_string(),
            first_name: "Line".to_string(),
            last_name: "Tester".to_string(),
            ..Default::default()
        })
        .expect("create customer");
    for sku in [SKU_A, SKU_B] {
        db.inventory()
            .create_item(CreateInventoryItem {
                sku: sku.to_string(),
                name: format!("Widget {sku}"),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .expect("create inventory item");
    }
    (db, customer.id)
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

/// One line of 2 × 10.00 plus 1.50 tax, 5.00 shipping, 2.00 discount → 24.50.
fn order_with_order_level_money(
    db: &SqliteDatabase,
    customer_id: CustomerId,
) -> stateset_core::Order {
    db.orders()
        .create(CreateOrder {
            customer_id,
            items: vec![line(SKU_A, 2, dec!(10.00))],
            tax_amount: Some(dec!(1.50)),
            shipping_amount: Some(dec!(5.00)),
            discount_amount: Some(dec!(2.00)),
            ..Default::default()
        })
        .expect("create order")
}

fn expected_total(order: &stateset_core::Order) -> Decimal {
    let lines: Decimal = order.items.iter().map(|i| i.total).sum();
    lines + order.tax_amount + order.shipping_amount - order.discount_amount
}

fn advance(db: &SqliteDatabase, id: OrderId, statuses: &[OrderStatus]) -> stateset_core::Order {
    let mut last = None;
    for status in statuses {
        last = Some(
            db.orders()
                .update(id, UpdateOrder { status: Some(*status), ..Default::default() })
                .unwrap_or_else(|e| panic!("transition to {status}: {e:?}")),
        );
    }
    last.expect("at least one transition")
}

fn open_reserved_qty(db: &SqliteDatabase, id: OrderId) -> Decimal {
    db.inventory()
        .list_reservations_by_reference("order", &id.to_string())
        .expect("list reservations")
        .into_iter()
        .filter(|r| matches!(r.status, ReservationStatus::Pending | ReservationStatus::Allocated))
        .map(|r| r.quantity)
        .sum()
}

// ---------------------------------------------------------------------------
// Defect 1: total must foot to lines + tax + shipping - discount after line
// mutation.
// ---------------------------------------------------------------------------

#[test]
fn add_and_remove_item_keep_order_level_money_in_total() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    assert_eq!(order.total_amount, dec!(24.50));
    assert_eq!(order.total_amount, order.calculate_total(), "model fn must foot too");

    let added = db.orders().add_item(order.id, line(SKU_B, 1, dec!(3.00))).expect("add item");
    let after_add = db.orders().get(order.id).unwrap().unwrap();
    assert_eq!(after_add.tax_amount, dec!(1.50), "order-level money survives add_item");
    assert_eq!(after_add.total_amount, dec!(27.50), "20 + 3 + 1.50 + 5 - 2");
    assert_eq!(after_add.total_amount, expected_total(&after_add));
    assert_eq!(after_add.total_amount, after_add.calculate_total());

    db.orders().remove_item(order.id, added.id).expect("remove item");
    let after_remove = db.orders().get(order.id).unwrap().unwrap();
    assert_eq!(after_remove.total_amount, dec!(24.50), "back to lines + tax + ship - disc");
    assert_eq!(after_remove.total_amount, after_remove.calculate_total());
}

// ---------------------------------------------------------------------------
// Defect 2: status guard + stock effects on line mutation.
// ---------------------------------------------------------------------------

#[test]
fn add_item_reserves_stock_and_backorders_shortfall() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    assert_eq!(open_reserved_qty(&db, order.id), dec!(2), "create reserved the first line");

    // 10 in stock, ask for 12: 10 reserved, 2 backordered — same policy as create.
    let added = db.orders().add_item(order.id, line(SKU_B, 12, dec!(1.00))).expect("add item");
    assert_eq!(open_reserved_qty(&db, order.id), dec!(12), "2 (SKU_A) + 10 (SKU_B)");
    let stock_b = db.inventory().get_stock(SKU_B).unwrap().unwrap();
    assert_eq!(stock_b.total_available, dec!(0), "added line consumed available stock");

    let backorders = db.backorder().get_backorders_for_order(order.id.into_uuid()).unwrap();
    assert_eq!(backorders.len(), 1, "shortfall on the added line is backordered");
    assert_eq!(backorders[0].order_line_id, Some(added.id.into_uuid()));
    assert_eq!(backorders[0].quantity_ordered, dec!(2));
}

#[test]
fn remove_item_releases_its_reservation_and_cancels_its_backorder() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    let added = db.orders().add_item(order.id, line(SKU_B, 12, dec!(1.00))).expect("add item");
    assert_eq!(open_reserved_qty(&db, order.id), dec!(12));

    db.orders().remove_item(order.id, added.id).expect("remove item");

    assert_eq!(open_reserved_qty(&db, order.id), dec!(2), "only SKU_A's reservation remains");
    let stock_b = db.inventory().get_stock(SKU_B).unwrap().unwrap();
    assert_eq!(stock_b.total_available, dec!(10), "released stock is available again");
    let backorders = db.backorder().get_backorders_for_order(order.id.into_uuid()).unwrap();
    assert_eq!(backorders.len(), 1);
    assert_eq!(backorders[0].status, BackorderStatus::Cancelled, "line backorder cancelled");

    let after = db.orders().get(order.id).unwrap().unwrap();
    assert_eq!(after.items.len(), 1);
    assert_eq!(after.total_amount, dec!(24.50));
}

#[test]
fn remove_item_rejects_a_line_from_another_order() {
    let (db, customer_id) = setup();
    let a = order_with_order_level_money(&db, customer_id);
    let b = order_with_order_level_money(&db, customer_id);

    let err = db.orders().remove_item(a.id, b.items[0].id).expect_err("foreign line");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(db.orders().get(b.id).unwrap().unwrap().items.len(), 1, "b untouched");
}

#[test]
fn line_mutation_is_refused_once_the_order_has_shipped() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    let shipped = advance(
        &db,
        order.id,
        &[OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped],
    );
    assert_eq!(shipped.status, OrderStatus::Shipped);
    let version_before = shipped.version;

    let err = db.orders().add_item(order.id, line(SKU_B, 1, dec!(3.00))).expect_err("add");
    match &err {
        CommerceError::Conflict(msg) => assert!(msg.contains("shipped"), "names status: {msg}"),
        other => panic!("expected Conflict naming the status, got {other:?}"),
    }
    let err =
        db.orders().remove_item(order.id, shipped.items[0].id).expect_err("remove shipped line");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");

    let after = db.orders().get(order.id).unwrap().unwrap();
    assert_eq!(after.items.len(), 1, "no line added or removed");
    assert_eq!(after.version, version_before, "refused mutation writes nothing");
    assert_eq!(after.total_amount, dec!(24.50));
}

#[test]
fn line_mutation_is_refused_on_cancelled_orders() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    advance(&db, order.id, &[OrderStatus::Cancelled]);

    let err = db.orders().add_item(order.id, line(SKU_B, 1, dec!(3.00))).expect_err("add");
    match err {
        CommerceError::Conflict(msg) => assert!(msg.contains("cancelled"), "{msg}"),
        other => panic!("expected Conflict, got {other:?}"),
    }
    assert_eq!(open_reserved_qty(&db, order.id), dec!(0), "nothing reserved on refusal");
}

#[test]
fn line_mutation_is_allowed_while_pre_fulfilment() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    advance(&db, order.id, &[OrderStatus::Confirmed, OrderStatus::Processing]);

    let added = db.orders().add_item(order.id, line(SKU_B, 1, dec!(3.00))).expect("processing");
    db.orders().remove_item(order.id, added.id).expect("processing remove");
}

// ---------------------------------------------------------------------------
// Defect 3: update_batch_atomic == N single updates in one transaction.
// ---------------------------------------------------------------------------

#[test]
fn update_batch_atomic_cancel_releases_reservations_like_update() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    db.orders().add_item(order.id, line(SKU_B, 12, dec!(1.00))).expect("add item");
    assert_eq!(open_reserved_qty(&db, order.id), dec!(12));

    let updated = db
        .orders()
        .update_batch_atomic(vec![(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() },
        )])
        .expect("batch cancel");
    assert_eq!(updated[0].status, OrderStatus::Cancelled);
    assert_eq!(updated[0].items.len(), 2, "batch result carries items");

    assert_eq!(open_reserved_qty(&db, order.id), dec!(0), "batch cancel released stock");
    let backorders = db.backorder().get_backorders_for_order(order.id.into_uuid()).unwrap();
    assert!(backorders.iter().all(|b| b.status == BackorderStatus::Cancelled));
    let stock_a = db.inventory().get_stock(SKU_A).unwrap().unwrap();
    assert_eq!(stock_a.total_available, dec!(10));
}

#[test]
fn update_batch_atomic_ship_confirms_reservations_and_ships_lines() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    advance(&db, order.id, &[OrderStatus::Confirmed, OrderStatus::Processing]);

    let updated = db
        .orders()
        .update_batch_atomic(vec![(
            order.id,
            UpdateOrder { status: Some(OrderStatus::Shipped), ..Default::default() },
        )])
        .expect("batch ship");
    assert_eq!(updated[0].status, OrderStatus::Shipped);
    assert_eq!(updated[0].items[0].shipped_quantity, 2, "lines shipped like update()");

    let reservations =
        db.inventory().list_reservations_by_reference("order", &order.id.to_string()).unwrap();
    assert!(
        reservations.iter().all(|r| r.status == ReservationStatus::Confirmed),
        "reservations confirmed: {reservations:?}"
    );
    // Confirming keeps the allocation against the SKU (on-hand is only
    // decremented by the shipment's inventory transaction); nothing was
    // released back to available.
    let stock_a = db.inventory().get_stock(SKU_A).unwrap().unwrap();
    assert_eq!(stock_a.total_allocated, dec!(2), "shipped units stay allocated");
    assert_eq!(stock_a.total_available, dec!(8), "nothing released back to available");
}

#[test]
fn update_batch_atomic_rejects_partially_shipped_like_update() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    advance(&db, order.id, &[OrderStatus::Confirmed, OrderStatus::Processing]);

    let err = db
        .orders()
        .update_batch_atomic(vec![(
            order.id,
            UpdateOrder { status: Some(OrderStatus::PartiallyShipped), ..Default::default() },
        )])
        .expect_err("partially_shipped is derived");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert_eq!(db.orders().get(order.id).unwrap().unwrap().status, OrderStatus::Processing);
}

#[test]
fn update_batch_atomic_rolls_back_every_row_on_one_failure() {
    let (db, customer_id) = setup();
    let a = order_with_order_level_money(&db, customer_id);
    let b = order_with_order_level_money(&db, customer_id);

    let err = db
        .orders()
        .update_batch_atomic(vec![
            (a.id, UpdateOrder { status: Some(OrderStatus::Cancelled), ..Default::default() }),
            (b.id, UpdateOrder { status: Some(OrderStatus::Delivered), ..Default::default() }),
        ])
        .expect_err("second row is an invalid transition");
    assert!(matches!(err, CommerceError::InvalidOrderStatusTransition { .. }), "{err:?}");

    let a_after = db.orders().get(a.id).unwrap().unwrap();
    assert_eq!(a_after.status, OrderStatus::Pending, "first row rolled back");
    assert_eq!(open_reserved_qty(&db, a.id), dec!(2), "first row's reservation intact");
}

// ---------------------------------------------------------------------------
// Defect 4: delete releases reservations and cancels backorders; shipped
// orders cannot be deleted.
// ---------------------------------------------------------------------------

#[test]
fn delete_releases_reservations_and_cancels_backorders() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    db.orders().add_item(order.id, line(SKU_B, 12, dec!(1.00))).expect("add item");
    assert_eq!(open_reserved_qty(&db, order.id), dec!(12));

    db.orders().delete(order.id).expect("delete");

    assert!(db.orders().get(order.id).unwrap().is_none());
    assert_eq!(open_reserved_qty(&db, order.id), dec!(0), "reservations released");
    assert_eq!(db.inventory().get_stock(SKU_A).unwrap().unwrap().total_available, dec!(10));
    assert_eq!(db.inventory().get_stock(SKU_B).unwrap().unwrap().total_available, dec!(10));
    let backorders = db.backorder().get_backorders_for_order(order.id.into_uuid()).unwrap();
    assert!(
        backorders.iter().all(|b| b.status == BackorderStatus::Cancelled),
        "backorders cancelled: {backorders:?}"
    );
}

#[test]
fn delete_batch_atomic_releases_reservations() {
    let (db, customer_id) = setup();
    let a = order_with_order_level_money(&db, customer_id);
    let b = order_with_order_level_money(&db, customer_id);
    assert_eq!(db.inventory().get_stock(SKU_A).unwrap().unwrap().total_available, dec!(6));

    db.orders().delete_batch_atomic(vec![a.id, b.id]).expect("delete batch");

    assert_eq!(db.inventory().get_stock(SKU_A).unwrap().unwrap().total_available, dec!(10));
}

#[test]
fn delete_refuses_shipped_orders_and_stays_idempotent_for_missing() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    advance(
        &db,
        order.id,
        &[OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped],
    );

    let err = db.orders().delete(order.id).expect_err("shipped orders are records");
    match err {
        CommerceError::Conflict(msg) => assert!(msg.contains("shipped"), "{msg}"),
        other => panic!("expected Conflict, got {other:?}"),
    }
    assert!(db.orders().get(order.id).unwrap().is_some(), "still there");

    // Deleting an unknown id is a no-op (existing contract).
    db.orders().delete(OrderId::new()).expect("idempotent delete");
}

// ---------------------------------------------------------------------------
// Defect 5: order-level money validated on create.
// ---------------------------------------------------------------------------

#[test]
fn create_rejects_negative_order_level_money() {
    let (db, customer_id) = setup();
    for (tax, shipping, discount) in [
        (Some(dec!(-0.01)), None, None),
        (None, Some(dec!(-1.00)), None),
        (None, None, Some(dec!(-1.00))),
    ] {
        let err = db
            .orders()
            .create(CreateOrder {
                customer_id,
                items: vec![line(SKU_A, 1, dec!(10.00))],
                tax_amount: tax,
                shipping_amount: shipping,
                discount_amount: discount,
                ..Default::default()
            })
            .expect_err("negative order-level money");
        assert!(
            matches!(err, CommerceError::ValidationError(_) | CommerceError::InvalidInput { .. }),
            "{err:?}"
        );
    }
    assert!(db.orders().list(Default::default()).unwrap().is_empty(), "nothing persisted");
}

#[test]
fn create_rejects_order_level_money_exceeding_currency_scale() {
    let (db, customer_id) = setup();
    for (tax, shipping, discount) in [
        (Some(dec!(1.005)), None, None),
        (None, Some(dec!(4.999)), None),
        (None, None, Some(dec!(0.001))),
    ] {
        let err = db
            .orders()
            .create(CreateOrder {
                customer_id,
                items: vec![line(SKU_A, 1, dec!(10.00))],
                tax_amount: tax,
                shipping_amount: shipping,
                discount_amount: discount,
                ..Default::default()
            })
            .expect_err("three-scale money in USD");
        assert_eq!(err.invariant_code(), Some("commerce.money.scale_exceeds_currency"), "{err:?}");
    }
}

#[test]
fn create_refuses_discount_that_drives_total_negative() {
    let (db, customer_id) = setup();
    let err = db
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![line(SKU_A, 1, dec!(10.00))],
            shipping_amount: Some(dec!(2.00)),
            discount_amount: Some(dec!(12.01)),
            ..Default::default()
        })
        .expect_err("total would be -0.01");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    assert!(db.orders().list(Default::default()).unwrap().is_empty());

    // Exactly zero is fine (fully discounted order).
    let free = db
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![line(SKU_A, 1, dec!(10.00))],
            shipping_amount: Some(dec!(2.00)),
            discount_amount: Some(dec!(12.00)),
            ..Default::default()
        })
        .expect("zero total is allowed");
    assert_eq!(free.total_amount, Decimal::ZERO);
}

// ---------------------------------------------------------------------------
// Round 4: reservations are keyed to the order LINE (migration 080).
//
// `remove_item` used to release "whole reservations for this SKU, oldest
// first, until the removed line's quantity is covered". With two lines on the
// same SKU (A qty 5 reserved first, B qty 1) removing B released A's 5-unit
// hold and left B's own reservation open.
// ---------------------------------------------------------------------------

#[test]
fn remove_item_releases_only_its_own_reservation_when_lines_share_a_sku() {
    let (db, customer_id) = setup();
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id,
            items: vec![line(SKU_A, 5, dec!(1.00)), line(SKU_A, 1, dec!(1.00))],
            ..Default::default()
        })
        .expect("create order");
    let line_a = order.items.iter().find(|i| i.quantity == 5).expect("line A").id;
    let line_b = order.items.iter().find(|i| i.quantity == 1).expect("line B").id;
    assert_eq!(open_reserved_qty(&db, order.id), dec!(6));
    assert_eq!(db.inventory().get_stock(SKU_A).unwrap().unwrap().total_available, dec!(4));

    db.orders().remove_item(order.id, line_b).expect("remove line B");

    // Exactly B's unit came back; A's 5-unit hold is untouched.
    assert_eq!(open_reserved_qty(&db, order.id), dec!(5), "line A's reservation must survive");
    assert_eq!(db.inventory().get_stock(SKU_A).unwrap().unwrap().total_available, dec!(5));
    let after = db.orders().get(order.id).unwrap().unwrap();
    assert_eq!(after.items.len(), 1);
    assert_eq!(after.items[0].id, line_a);

    // Removing A afterwards frees the rest.
    db.orders().remove_item(order.id, line_a).expect("remove line A");
    assert_eq!(open_reserved_qty(&db, order.id), Decimal::ZERO);
    assert_eq!(db.inventory().get_stock(SKU_A).unwrap().unwrap().total_available, dec!(10));
}

#[test]
fn legacy_unkeyed_reservations_still_release_by_sku() {
    // Rows created before migration 080 have no `order_item_id`; the SKU-based
    // path must keep working for them.
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute(
            "UPDATE inventory_reservations SET order_item_id = NULL WHERE reference_id = ?",
            [order.id.to_string()],
        )
        .expect("strip line keys");
    }
    assert_eq!(open_reserved_qty(&db, order.id), dec!(2));

    db.orders().remove_item(order.id, order.items[0].id).expect("remove legacy line");
    assert_eq!(open_reserved_qty(&db, order.id), Decimal::ZERO);
    assert_eq!(db.inventory().get_stock(SKU_A).unwrap().unwrap().total_available, dec!(10));
}

// ---------------------------------------------------------------------------
// Round 4: line edits are kernel events, not silent writes.
// ---------------------------------------------------------------------------

#[test]
fn line_edits_write_kernel_outbox_events_in_the_same_transaction() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    let added = db.orders().add_item(order.id, line(SKU_B, 3, dec!(2.00))).expect("add item");
    db.orders().remove_item(order.id, added.id).expect("remove item");

    let events = db.kernel_outbox().pending(100).expect("pending outbox events");
    let added_event = events
        .iter()
        .find(|e| e.event_type == "orders.item_added.v1" && e.aggregate_id == order.id.to_string())
        .expect("orders.item_added.v1 event");
    assert_eq!(added_event.aggregate_type, "order");
    assert_eq!(added_event.payload["order_item_id"], added.id.to_string());
    assert_eq!(added_event.payload["sku"], SKU_B);
    assert_eq!(added_event.payload["quantity"], 3);
    assert_eq!(added_event.payload["total_amount"], "30.50", "24.50 + 3 × 2.00");

    let removed_event = events
        .iter()
        .find(|e| {
            e.event_type == "orders.item_removed.v1" && e.aggregate_id == order.id.to_string()
        })
        .expect("orders.item_removed.v1 event");
    assert_eq!(removed_event.payload["order_item_id"], added.id.to_string());
    assert_eq!(removed_event.payload["total_amount"], "24.50");

    // A refused edit writes nothing.
    let bogus = db.orders().remove_item(order.id, stateset_core::OrderItemId::new());
    assert!(bogus.is_err());
    let removed_count = db
        .kernel_outbox()
        .pending(100)
        .expect("pending")
        .iter()
        .filter(|e| e.event_type == "orders.item_removed.v1")
        .count();
    assert_eq!(removed_count, 1);
}

// ---------------------------------------------------------------------------
// Round 4: an order with money against it is a financial record.
// ---------------------------------------------------------------------------

#[test]
fn delete_refuses_an_order_whose_payment_status_holds_money() {
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    db.orders()
        .update(
            order.id,
            UpdateOrder {
                payment_status: Some(stateset_core::PaymentStatus::Paid),
                ..Default::default()
            },
        )
        .expect("mark paid");

    let err = db.orders().delete(order.id).expect_err("paid orders are records");
    assert!(matches!(err, CommerceError::Conflict(ref m) if m.contains("paid")), "{err:?}");
    assert!(db.orders().get(order.id).unwrap().is_some(), "order survives");
    assert_eq!(open_reserved_qty(&db, order.id), dec!(2), "nothing released on a refused delete");

    // A cancelled-but-paid order is refused too (status alone would allow it).
    db.orders()
        .update(
            order.id,
            UpdateOrder {
                status: Some(OrderStatus::Cancelled),
                payment_status: Some(stateset_core::PaymentStatus::PartiallyRefunded),
                ..Default::default()
            },
        )
        .expect("cancel");
    let err = db.orders().delete(order.id).expect_err("partially refunded orders are records");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
    assert!(db.orders().get(order.id).unwrap().is_some());
}

#[test]
fn delete_refuses_an_order_referenced_by_a_payment_row() {
    use stateset_core::{CreatePayment, PaymentMethodType, PaymentRepository};
    let (db, customer_id) = setup();
    let order = order_with_order_level_money(&db, customer_id);
    let payment = db
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(10.00),
            ..Default::default()
        })
        .expect("create payment");
    // Even a failed attempt keeps the order as a record.
    db.payments().mark_failed(payment.id, "declined", None).expect("fail payment");

    let err = db.orders().delete(order.id).expect_err("orders with payment rows are records");
    assert!(matches!(err, CommerceError::Conflict(ref m) if m.contains("payments")), "{err:?}");
    assert!(db.orders().get(order.id).unwrap().is_some());
    assert_eq!(db.payments().for_order(order.id).unwrap().len(), 1);

    // `delete_batch_atomic` shares the guard.
    let err = db.orders().delete_batch_atomic(vec![order.id]).expect_err("atomic batch refused");
    assert!(matches!(err, CommerceError::Conflict(_)), "{err:?}");
}
