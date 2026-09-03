#![cfg(feature = "sqlite")]
//! Returns hardening against the live SQLite engine.
//!
//! Regression coverage for the return report-card defects:
//! - R1: `refund_amount` bounds (non-negative, ≤ Σ line refunds), terminal
//!   returns immutable, completion settles a payment refund in the same
//!   transaction.
//! - R2: per-line refund is the proportional share of `order_items.total`,
//!   so a line discount is honoured.
//! - R3: rejecting after a stock-affecting disposition is refused, so the
//!   over-return guard (which releases rejected returns' claims) stays sound.
//! - R4: idempotency key replay returns the original; reuse with a different
//!   payload is a `Conflict`; the key is unique at the database and the
//!   lookup runs inside the write transaction.
//! - R5: completion requires every item dispositioned unless written off;
//!   quarantine without a bin still holds stock at warehouse level.
//! - R6: serials are transitioned and lot on-hand restored on disposition.
//! - R8: approve/reject/complete read the status in-transaction and raise
//!   typed errors.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateLot, CreateOrder, CreateOrderItem,
    CreatePayment, CreateProduct, CreateReturn, CreateReturnItem, CreateSerialNumber,
    CreateWarehouse, CurrencyCode, CustomerRepository, InventoryRepository, LotRepository,
    OrderRepository, OrderStatus, PaymentMethodType, PaymentRepository, ProductRepository,
    RefundStatus, Return, ReturnDisposition, ReturnRepository, ReturnStatus, SerialEventType,
    SerialHistoryFilter, SerialRepository, SerialStatus, SetReturnDisposition, UpdateOrder,
    UpdateReturn, WarehouseAddress, WarehouseRepository, WarehouseType,
};
use stateset_db::SqliteDatabase;
use std::sync::{Arc, Barrier};
use std::thread;
use uuid::Uuid;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("in-memory sqlite")
}

/// Customer + product + shipped order with one line of `quantity` units of
/// `sku` at `unit_price`, optionally discounted.
fn shipped_order(
    db: &SqliteDatabase,
    sku: &str,
    quantity: i32,
    unit_price: Decimal,
    discount: Option<Decimal>,
) -> stateset_core::Order {
    let unique = Uuid::new_v4().simple().to_string();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("ret-{unique}@example.com"),
            first_name: "Ret".into(),
            last_name: "Urn".into(),
            ..Default::default()
        })
        .expect("create customer");
    let product = db
        .products()
        .create(CreateProduct { name: format!("Widget {unique}"), ..Default::default() })
        .expect("create product");
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: sku.into(),
                name: "Widget".into(),
                quantity,
                unit_price,
                discount,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .expect("advance order status");
    }
    db.orders().get(order.id).expect("get order").expect("order exists")
}

fn create_return(db: &SqliteDatabase, order: &stateset_core::Order, quantity: i32) -> Return {
    db.returns()
        .create(CreateReturn {
            order_id: order.id,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity,
                condition: None,
            }],
            ..Default::default()
        })
        .expect("create return")
}

fn advance(db: &SqliteDatabase, id: stateset_core::ReturnId, statuses: &[ReturnStatus]) -> Return {
    for status in statuses {
        db.returns()
            .update(id, UpdateReturn { status: Some(*status), ..Default::default() })
            .expect("advance status");
    }
    db.returns().get(id).expect("get").expect("present")
}

/// A return of `quantity` units of `sku` (unit price 10), advanced to
/// `received`.
fn received_return(db: &SqliteDatabase, sku: &str, quantity: i32) -> Return {
    let order = shipped_order(db, sku, quantity, dec!(10), None);
    let ret = create_return(db, &order, quantity);
    db.returns().approve(ret.id).expect("approve");
    advance(db, ret.id, &[ReturnStatus::InTransit, ReturnStatus::Received])
}

fn warehouse(db: &SqliteDatabase) -> i32 {
    db.warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WH-{}", &Uuid::new_v4().simple().to_string()[..6]),
            name: "Returns DC".into(),
            warehouse_type: WarehouseType::Returns,
            address: WarehouseAddress { country: "US".into(), ..Default::default() },
            timezone: None,
        })
        .expect("create warehouse")
        .id
}

fn item(db: &SqliteDatabase, sku: &str) {
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: sku.into(),
            ..Default::default()
        })
        .expect("create inventory item");
}

fn on_hand(db: &SqliteDatabase, sku: &str, wh: i32) -> (Decimal, Decimal) {
    let stock = db.inventory().get_stock(sku).expect("stock").expect("stock present");
    stock
        .locations
        .iter()
        .find(|l| l.location_id == wh)
        .map_or((dec!(0), dec!(0)), |l| (l.on_hand, l.allocated))
}

fn disposition(d: ReturnDisposition, wh: i32) -> SetReturnDisposition {
    SetReturnDisposition {
        disposition: d,
        warehouse_id: Some(wh),
        disposition_by: Some("inspector".into()),
        ..Default::default()
    }
}

fn completed_payment(
    db: &SqliteDatabase,
    order: &stateset_core::Order,
    amount: Decimal,
) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create(CreatePayment {
            order_id: Some(order.id),
            customer_id: Some(order.customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .expect("create payment");
    db.payments().mark_completed(payment.id).expect("complete payment")
}

// ---------------------------------------------------------------------------
// R2: per-line refund honours the line discount
// ---------------------------------------------------------------------------

#[test]
fn line_refund_is_proportional_share_of_charged_total() {
    let db = db();
    // 2 × 10 with a 4 discount: the line charged 16, so one unit refunds 8.
    let order = shipped_order(&db, "SKU-DISC", 2, dec!(10), Some(dec!(4)));
    assert_eq!(order.items[0].total, dec!(16));
    let ret = create_return(&db, &order, 1);
    assert_eq!(ret.items[0].refund_amount, dec!(8));
    assert_eq!(ret.refund_amount, Some(dec!(8)));

    // Returning the whole line refunds exactly what was charged.
    let order = shipped_order(&db, "SKU-DISC-2", 3, dec!(9.99), Some(dec!(5)));
    let ret = create_return(&db, &order, 3);
    assert_eq!(ret.refund_amount, Some(order.items[0].total));
}

// ---------------------------------------------------------------------------
// R1: refund bounds, terminal immutability, settlement on completion
// ---------------------------------------------------------------------------

#[test]
fn refund_amount_must_be_non_negative_and_within_line_total() {
    let db = db();
    let order = shipped_order(&db, "SKU-CAP", 2, dec!(10), None);
    let ret = create_return(&db, &order, 2);
    assert_eq!(ret.refund_amount, Some(dec!(20)));

    let err = db
        .returns()
        .update(ret.id, UpdateReturn { refund_amount: Some(dec!(-1)), ..Default::default() })
        .expect_err("negative refund");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    let err = db
        .returns()
        .update(ret.id, UpdateReturn { refund_amount: Some(dec!(20.01)), ..Default::default() })
        .expect_err("refund above the line total");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // A partial refund within the cap is fine.
    let updated = db
        .returns()
        .update(ret.id, UpdateReturn { refund_amount: Some(dec!(12.50)), ..Default::default() })
        .expect("partial refund");
    assert_eq!(updated.refund_amount, Some(dec!(12.50)));
    assert_eq!(db.returns().get(ret.id).unwrap().unwrap().refund_amount, Some(dec!(12.50)));
}

#[test]
fn terminal_returns_are_immutable() {
    let db = db();
    let wh = warehouse(&db);
    let ret = received_return(&db, "SKU-TERM", 1);
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Scrap, wh))
        .unwrap();
    let done = db.returns().complete(ret.id).expect("complete");
    assert_eq!(done.status, ReturnStatus::Completed);

    for input in [
        UpdateReturn { refund_amount: Some(dec!(0)), ..Default::default() },
        UpdateReturn { notes: Some("late edit".into()), ..Default::default() },
        UpdateReturn { status: Some(ReturnStatus::Completed), ..Default::default() },
    ] {
        let err =
            db.returns().update(ret.id, input).expect_err("terminal return must be immutable");
        assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
    }
    let err = db.returns().cancel(ret.id).expect_err("cancel completed");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
    assert_eq!(db.returns().get(ret.id).unwrap().unwrap().version, done.version);

    // Rejected is terminal too.
    let order = shipped_order(&db, "SKU-TERM-2", 1, dec!(10), None);
    let ret = create_return(&db, &order, 1);
    db.returns().reject(ret.id, "no").unwrap();
    let err = db
        .returns()
        .update(ret.id, UpdateReturn { notes: Some("x".into()), ..Default::default() })
        .expect_err("rejected return must be immutable");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
}

#[test]
fn completing_creates_pending_payment_refund_in_same_transaction() {
    let db = db();
    let wh = warehouse(&db);
    let order = shipped_order(&db, "SKU-SETTLE", 2, dec!(10), None);
    let payment = completed_payment(&db, &order, dec!(20));

    let ret = create_return(&db, &order, 1);
    db.returns().approve(ret.id).unwrap();
    advance(&db, ret.id, &[ReturnStatus::InTransit, ReturnStatus::Received]);
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Scrap, wh))
        .unwrap();
    let done = db.returns().complete(ret.id).expect("complete");
    assert_eq!(done.status, ReturnStatus::Completed);
    assert_eq!(done.refund_amount, Some(dec!(10)));

    let refunds = db.payments().get_refunds(payment.id).expect("refunds");
    assert_eq!(refunds.len(), 1, "exactly one refund: {refunds:?}");
    assert_eq!(refunds[0].amount, dec!(10));
    assert_eq!(refunds[0].status, RefundStatus::Pending);
    assert_eq!(
        refunds[0].idempotency_key.as_deref(),
        Some(format!("return:{}:{}", ret.id, payment.id).as_str())
    );

    // The completion event carries the refund ids.
    let events = db.kernel_outbox().pending(200).unwrap();
    let completion = events
        .iter()
        .find(|e| {
            e.aggregate_id == ret.id.to_string()
                && e.payload["status_after"] == ReturnStatus::Completed.to_string()
        })
        .expect("completion event");
    assert_eq!(completion.payload["payment_refund_ids"][0], refunds[0].id.to_string());
    assert_eq!(completion.payload["uncovered_refund_amount"], "0");

    // The pending refund reserves against the payment: the remaining 10 is all
    // that can still be refunded.
    let err = db
        .payments()
        .create_refund(stateset_core::CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(10.01)),
            ..Default::default()
        })
        .expect_err("over-refund");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "got {err:?}");
}

#[test]
fn completion_splits_refund_across_captures_and_reports_uncovered_remainder() {
    let db = db();
    let wh = warehouse(&db);
    let order = shipped_order(&db, "SKU-SPLIT", 3, dec!(10), None);
    let first = completed_payment(&db, &order, dec!(12));
    let second = completed_payment(&db, &order, dec!(5));

    let ret = received_return(&db, "SKU-SPLIT-ALT", 1);
    // Use a return on the paid order instead.
    let _ = ret;
    let ret = create_return(&db, &order, 3);
    db.returns().approve(ret.id).unwrap();
    advance(&db, ret.id, &[ReturnStatus::InTransit, ReturnStatus::Received]);
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Scrap, wh))
        .unwrap();
    let done = db.returns().complete(ret.id).expect("complete");
    assert_eq!(done.refund_amount, Some(dec!(30)));

    let first_refunds = db.payments().get_refunds(first.id).unwrap();
    let second_refunds = db.payments().get_refunds(second.id).unwrap();
    assert_eq!(first_refunds.len(), 1);
    assert_eq!(first_refunds[0].amount, dec!(12));
    assert_eq!(second_refunds.len(), 1);
    assert_eq!(second_refunds[0].amount, dec!(5));

    let events = db.kernel_outbox().pending(500).unwrap();
    let completion = events
        .iter()
        .find(|e| {
            e.aggregate_id == ret.id.to_string()
                && e.payload["status_after"] == ReturnStatus::Completed.to_string()
        })
        .expect("completion event");
    assert_eq!(completion.payload["uncovered_refund_amount"], "13");
    assert_eq!(completion.payload["payment_refund_ids"].as_array().unwrap().len(), 2);
}

#[test]
fn store_credit_refund_is_recorded_but_not_settled_through_payments() {
    let db = db();
    let wh = warehouse(&db);
    let order = shipped_order(&db, "SKU-CREDIT", 1, dec!(10), None);
    let payment = completed_payment(&db, &order, dec!(10));
    let ret = create_return(&db, &order, 1);
    db.returns().approve(ret.id).unwrap();
    advance(&db, ret.id, &[ReturnStatus::InTransit, ReturnStatus::Received]);
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Scrap, wh))
        .unwrap();
    let done = db
        .returns()
        .update(
            ret.id,
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                refund_method: Some("store_credit".into()),
                ..Default::default()
            },
        )
        .expect("complete with store credit");
    assert_eq!(done.refund_method.as_deref(), Some("store_credit"));
    assert_eq!(done.refund_amount, Some(dec!(10)));
    assert!(db.payments().get_refunds(payment.id).unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// R3: reject after restock
// ---------------------------------------------------------------------------

#[test]
fn reject_after_restock_is_refused_and_over_return_guard_holds() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-REJECT-RESTOCK";
    item(&db, sku);
    let order = shipped_order(&db, sku, 2, dec!(10), None);
    let ret = create_return(&db, &order, 2);
    db.returns().approve(ret.id).unwrap();
    advance(
        &db,
        ret.id,
        &[ReturnStatus::InTransit, ReturnStatus::Received, ReturnStatus::Inspecting],
    );
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Restock, wh))
        .expect("restock");
    assert_eq!(on_hand(&db, sku, wh), (dec!(2), dec!(0)));

    let err = db.returns().reject(ret.id, "changed my mind").expect_err("reject after restock");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    let err = db
        .returns()
        .update(ret.id, UpdateReturn { status: Some(ReturnStatus::Rejected), ..Default::default() })
        .expect_err("reject via update after restock");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(db.returns().get(ret.id).unwrap().unwrap().status, ReturnStatus::Inspecting);

    // The return still holds its claim on the line: nothing more is returnable.
    let err = create_return_result(&db, &order, 1).expect_err("units already claimed");
    assert!(matches!(err, CommerceError::ReturnExceedsReturnable { .. }), "got {err:?}");
    assert_eq!(on_hand(&db, sku, wh), (dec!(2), dec!(0)));
}

fn create_return_result(
    db: &SqliteDatabase,
    order: &stateset_core::Order,
    quantity: i32,
) -> stateset_core::Result<Return> {
    db.returns().create(CreateReturn {
        order_id: order.id,
        items: vec![CreateReturnItem {
            order_item_id: order.items[0].id,
            quantity,
            condition: None,
        }],
        ..Default::default()
    })
}

/// Scrap and return-to-vendor destroy the goods without touching stock, so the
/// old `affects_stock`-only guard let a scrapped return be rejected — which
/// released its claim on the order line and made the destroyed units returnable
/// (and refundable) a second time. Any disposition at all now pins the return.
#[test]
fn reject_after_any_disposition_is_refused_even_without_a_stock_effect() {
    for (label, disp) in [
        ("scrap", ReturnDisposition::Scrap),
        ("return_to_vendor", ReturnDisposition::ReturnToVendor),
        ("refurbish", ReturnDisposition::Refurbish),
    ] {
        let db = db();
        let wh = warehouse(&db);
        let sku = format!("SKU-REJECT-{label}");
        item(&db, &sku);
        let order = shipped_order(&db, &sku, 1, dec!(10), None);
        let ret = create_return(&db, &order, 1);
        db.returns().approve(ret.id).unwrap();
        advance(
            &db,
            ret.id,
            &[ReturnStatus::InTransit, ReturnStatus::Received, ReturnStatus::Inspecting],
        );
        db.returns().set_item_disposition(ret.id, ret.items[0].id, disposition(disp, wh)).unwrap();

        let err = db.returns().reject(ret.id, "damaged by customer").expect_err("reject");
        assert!(matches!(err, CommerceError::Conflict(_)), "{label}: got {err:?}");
        let err = db
            .returns()
            .cancel(ret.id)
            .expect_err("cancel is refused for the same reason (illegal edge or conflict)");
        assert!(
            matches!(err, CommerceError::Conflict(_) | CommerceError::ValidationError(_)),
            "{label}: got {err:?}"
        );
        assert_eq!(db.returns().get(ret.id).unwrap().unwrap().status, ReturnStatus::Inspecting);

        // The claim still stands: the destroyed unit is not returnable again.
        let err = create_return_result(&db, &order, 1).expect_err("units already claimed");
        assert!(matches!(err, CommerceError::ReturnExceedsReturnable { .. }), "{label}: {err:?}");
    }
}

/// Rejecting is still allowed while nothing has been dispositioned.
#[test]
fn reject_from_inspecting_is_allowed_while_nothing_is_dispositioned() {
    let db = db();
    let ret = received_return(&db, "SKU-REJECT-CLEAN", 1);
    advance(&db, ret.id, &[ReturnStatus::Inspecting]);
    let rejected = db.returns().reject(ret.id, "not our product").expect("reject");
    assert_eq!(rejected.status, ReturnStatus::Rejected);
    assert_eq!(rejected.notes.as_deref(), Some("not our product"));
}

// ---------------------------------------------------------------------------
// R9: deleting a return may not free the order-line claim
// ---------------------------------------------------------------------------

/// A completed, restocked, refunded return used to be deletable in any status.
/// `validate_return_item_tx` counts claims from the surviving `return_items`,
/// so the delete made the same units returnable and refundable again while the
/// restocked stock stayed on the shelf.
#[test]
fn delete_is_refused_once_the_return_has_had_any_effect() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-DELETE-EFFECT";
    item(&db, sku);
    let order = shipped_order(&db, sku, 2, dec!(10), None);
    completed_payment(&db, &order, dec!(20));
    let ret = create_return(&db, &order, 2);
    db.returns().approve(ret.id).unwrap();
    advance(
        &db,
        ret.id,
        &[ReturnStatus::InTransit, ReturnStatus::Received, ReturnStatus::Inspecting],
    );
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Restock, wh))
        .expect("restock");
    db.returns().complete(ret.id).expect("complete");
    assert_eq!(on_hand(&db, sku, wh), (dec!(2), dec!(0)));

    let err = db
        .returns()
        .delete_batch_atomic(vec![ret.id])
        .expect_err("a completed, restocked return is not deletable");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");

    // The partial-success batch records the refusal instead of raising it.
    let batch = db.returns().delete_batch(vec![ret.id]).expect("batch call itself succeeds");
    assert_eq!(batch.succeeded.len(), 0, "nothing may be deleted");
    assert_eq!(batch.failed.len(), 1);

    // The return survives, so its claim on the order line survives with it.
    assert!(db.returns().get(ret.id).unwrap().is_some());
    let err = create_return_result(&db, &order, 1).expect_err("units already claimed");
    assert!(matches!(err, CommerceError::ReturnExceedsReturnable { .. }), "got {err:?}");
    assert_eq!(on_hand(&db, sku, wh), (dec!(2), dec!(0)));
}

/// Every status past the early, no-effect window refuses deletion; the early
/// window itself still deletes header and items together.
#[test]
fn delete_window_is_requested_and_approved_only() {
    let db = db();
    let order = shipped_order(&db, "SKU-DELETE-WINDOW", 6, dec!(10), None);

    // Deletable: an untouched request, and an approved (nothing physical yet).
    let requested = create_return(&db, &order, 1);
    db.returns().delete_batch_atomic(vec![requested.id]).expect("delete requested");
    assert!(db.returns().get(requested.id).unwrap().is_none());

    let approved = create_return(&db, &order, 1);
    db.returns().approve(approved.id).unwrap();
    db.returns().delete_batch_atomic(vec![approved.id]).expect("delete approved");
    assert!(db.returns().get(approved.id).unwrap().is_none());

    // Not deletable once the goods are moving, or once terminal.
    for statuses in [
        vec![ReturnStatus::InTransit],
        vec![ReturnStatus::InTransit, ReturnStatus::Received],
        vec![ReturnStatus::InTransit, ReturnStatus::Received, ReturnStatus::Inspecting],
    ] {
        let ret = create_return(&db, &order, 1);
        db.returns().approve(ret.id).unwrap();
        advance(&db, ret.id, &statuses);
        let err = db
            .returns()
            .delete_batch_atomic(vec![ret.id])
            .expect_err("in-flight returns are not deletable");
        assert!(matches!(err, CommerceError::NotPermitted(_)), "{statuses:?}: got {err:?}");
    }

    let cancelled = create_return(&db, &order, 1);
    db.returns().cancel(cancelled.id).unwrap();
    let err = db
        .returns()
        .delete_batch_atomic(vec![cancelled.id])
        .expect_err("terminal returns are not deletable");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
}

// ---------------------------------------------------------------------------
// R4: idempotency
// ---------------------------------------------------------------------------

#[test]
fn idempotency_key_replays_original_and_conflicts_on_different_payload() {
    let db = db();
    let order = shipped_order(&db, "SKU-IDEM", 3, dec!(10), None);
    let key = format!("ret-{}", Uuid::new_v4());
    let request = CreateReturn {
        order_id: order.id,
        idempotency_key: Some(key),
        items: vec![CreateReturnItem {
            order_item_id: order.items[0].id,
            quantity: 1,
            condition: None,
        }],
        ..Default::default()
    };
    let first = db.returns().create(request.clone()).expect("first");
    let replay = db.returns().create(request.clone()).expect("replay");
    assert_eq!(replay.id, first.id);
    assert_eq!(db.returns().count(Default::default()).unwrap(), 1);

    let err = db
        .returns()
        .create(CreateReturn {
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 2,
                condition: None,
            }],
            ..request
        })
        .expect_err("key reuse with a different payload");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(db.returns().count(Default::default()).unwrap(), 1);
}

#[test]
fn idempotency_key_is_unique_at_the_database() {
    let db = db();
    let order = shipped_order(&db, "SKU-IDEM-DB", 3, dec!(10), None);
    let key = format!("ret-{}", Uuid::new_v4());
    let first = create_return(&db, &order, 1);
    let second = create_return(&db, &order, 1);
    // Bypass the application: the unique index is the backstop.
    let conn = db.pool().get().unwrap();
    conn.execute(
        "UPDATE returns SET idempotency_key = ?1 WHERE id = ?2",
        rusqlite::params![key, first.id.to_string()],
    )
    .unwrap();
    let err = conn
        .execute(
            "UPDATE returns SET idempotency_key = ?1 WHERE id = ?2",
            rusqlite::params![key, second.id.to_string()],
        )
        .expect_err("duplicate idempotency_key must be refused by the unique index");
    assert!(err.to_string().contains("UNIQUE"), "got {err}");
}

// ---------------------------------------------------------------------------
// R5: completion requires dispositions; quarantine without bins holds stock
// ---------------------------------------------------------------------------

#[test]
fn complete_requires_every_item_dispositioned_unless_written_off() {
    let db = db();
    let wh = warehouse(&db);
    let ret = received_return(&db, "SKU-UNDISP", 2);

    let err = db.returns().complete(ret.id).expect_err("undispositioned");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
    let err = db
        .returns()
        .update(
            ret.id,
            UpdateReturn { status: Some(ReturnStatus::Completed), ..Default::default() },
        )
        .expect_err("undispositioned via update");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
    assert_eq!(db.returns().get(ret.id).unwrap().unwrap().status, ReturnStatus::Received);

    // Explicit write-off completes and records the written-off units.
    let done = db
        .returns()
        .update(
            ret.id,
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                write_off_undispositioned: true,
                ..Default::default()
            },
        )
        .expect("write-off completion");
    assert_eq!(done.status, ReturnStatus::Completed);
    let events = db.kernel_outbox().pending(200).unwrap();
    let completion = events
        .iter()
        .find(|e| {
            e.aggregate_id == ret.id.to_string()
                && e.payload["status_after"] == ReturnStatus::Completed.to_string()
        })
        .expect("completion event");
    assert_eq!(completion.payload["undispositioned_units"], 2);

    // With every item dispositioned, plain complete works.
    let ret = received_return(&db, "SKU-DISP-OK", 1);
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Scrap, wh))
        .unwrap();
    assert_eq!(db.returns().complete(ret.id).unwrap().status, ReturnStatus::Completed);
}

#[test]
fn quarantine_without_bins_holds_stock_at_warehouse_level() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-QUAR-HOLD";
    item(&db, sku);
    let ret = received_return(&db, sku, 4);
    let updated = db
        .returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            disposition(ReturnDisposition::Quarantine, wh),
        )
        .unwrap();
    assert_eq!(updated.disposition, Some(ReturnDisposition::Quarantine));
    // On hand and allocated: tracked, not sellable.
    assert_eq!(on_hand(&db, sku, wh), (dec!(4), dec!(4)));
    let stock = db.inventory().get_stock(sku).unwrap().unwrap();
    assert_eq!(stock.total_available, dec!(0));
}

// ---------------------------------------------------------------------------
// R6: serials and lots
// ---------------------------------------------------------------------------

fn shipped_serial(db: &SqliteDatabase, sku: &str) -> stateset_core::SerialNumber {
    let serial = db
        .serials()
        .create(CreateSerialNumber { sku: sku.into(), ..Default::default() })
        .expect("create serial");
    db.serials().mark_shipped(serial.id, Uuid::new_v4()).expect("ship serial")
}

#[test]
fn restock_disposition_returns_serials_to_available_at_warehouse() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-SERIAL-RESTOCK";
    item(&db, sku);
    let ret = received_return(&db, sku, 2);
    let a = shipped_serial(&db, sku);
    let b = shipped_serial(&db, sku);
    assert_eq!(a.status, SerialStatus::Shipped);

    let updated = db
        .returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                serial_ids: vec![a.id, b.id],
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .expect("restock with serials");
    assert_eq!(updated.serial_ids, vec![a.id, b.id]);
    assert_eq!(on_hand(&db, sku, wh), (dec!(2), dec!(0)));

    for id in [a.id, b.id] {
        let serial = db.serials().get(id).unwrap().unwrap();
        assert_eq!(serial.status, SerialStatus::Available);
        assert_eq!(serial.current_location_id, Some(wh));
        let history = db.serials().get_history(id, SerialHistoryFilter::default()).unwrap();
        let events: Vec<_> = history.iter().map(|h| h.event_type).collect();
        assert!(events.contains(&SerialEventType::Returned), "history {events:?}");
        let returned = history.iter().find(|h| h.event_type == SerialEventType::Returned).unwrap();
        assert_eq!(returned.reference_id, Some(ret.id.into_uuid()));
        assert_eq!(returned.to_status, SerialStatus::Returned);
    }
    // Persisted on the item.
    let reloaded = db.returns().get(ret.id).unwrap().unwrap();
    assert_eq!(reloaded.items[0].serial_ids, vec![a.id, b.id]);
}

#[test]
fn quarantine_and_scrap_dispositions_move_serials_accordingly() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-SERIAL-QUAR";
    item(&db, sku);
    let ret = received_return(&db, sku, 1);
    let s = shipped_serial(&db, sku);
    db.returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                serial_ids: vec![s.id],
                ..disposition(ReturnDisposition::Quarantine, wh)
            },
        )
        .unwrap();
    assert_eq!(db.serials().get(s.id).unwrap().unwrap().status, SerialStatus::Quarantined);

    let sku = "SKU-SERIAL-SCRAP";
    item(&db, sku);
    let ret = received_return(&db, sku, 1);
    let s = shipped_serial(&db, sku);
    db.returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                serial_ids: vec![s.id],
                ..disposition(ReturnDisposition::Scrap, wh)
            },
        )
        .unwrap();
    assert_eq!(db.serials().get(s.id).unwrap().unwrap().status, SerialStatus::Scrapped);
}

#[test]
fn serial_validation_rolls_back_the_whole_disposition() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-SERIAL-BAD";
    item(&db, sku);
    let ret = received_return(&db, sku, 2);
    let a = shipped_serial(&db, sku);
    let other = shipped_serial(&db, "SKU-OTHER");

    // Wrong count.
    let err = db
        .returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                serial_ids: vec![a.id],
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .expect_err("count mismatch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    // Wrong SKU.
    let err = db
        .returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                serial_ids: vec![a.id, other.id],
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .expect_err("sku mismatch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    // Duplicate.
    let err = db
        .returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                serial_ids: vec![a.id, a.id],
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .expect_err("duplicate serial");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // Nothing was half-written: no stock, no disposition, serial untouched.
    assert_eq!(on_hand(&db, sku, wh), (dec!(0), dec!(0)));
    assert_eq!(db.returns().get(ret.id).unwrap().unwrap().items[0].disposition, None);
    assert_eq!(db.serials().get(a.id).unwrap().unwrap().status, SerialStatus::Shipped);
}

#[test]
fn restock_disposition_restores_lot_on_hand() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-LOT";
    item(&db, sku);
    let lot = db
        .lots()
        .create(CreateLot { sku: sku.into(), quantity: dec!(10), ..Default::default() })
        .expect("create lot");
    let ret = received_return(&db, sku, 3);
    let updated = db
        .returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                lot_id: Some(lot.id),
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .expect("restock to lot");
    assert_eq!(updated.lot_id, Some(lot.id));
    let lot_after = db.lots().get(lot.id).unwrap().unwrap();
    assert_eq!(lot_after.quantity_remaining, lot.quantity_remaining + dec!(3));
    assert_eq!(lot_after.quantity_quarantined, lot.quantity_quarantined);
    let conn = db.pool().get().unwrap();
    let placed: String = conn
        .query_row(
            "SELECT quantity FROM lot_locations WHERE lot_id = ?1 AND location_id = ?2",
            rusqlite::params![lot.id.to_string(), wh],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(placed.parse::<Decimal>().unwrap(), dec!(3));
    let tx_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM lot_transactions WHERE lot_id = ?1 AND transaction_type = 'returned' AND reference_id = ?2",
            rusqlite::params![lot.id.to_string(), ret.items[0].id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tx_count, 1);

    // Quarantine holds the units on the lot as well.
    let ret = received_return(&db, sku, 2);
    db.returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                lot_id: Some(lot.id),
                ..disposition(ReturnDisposition::Quarantine, wh)
            },
        )
        .unwrap();
    let lot_after = db.lots().get(lot.id).unwrap().unwrap();
    assert_eq!(lot_after.quantity_remaining, lot.quantity_remaining + dec!(5));
    assert_eq!(lot_after.quantity_quarantined, lot.quantity_quarantined + dec!(2));

    // A lot of another SKU is refused and nothing is written.
    let wrong = db
        .lots()
        .create(CreateLot { sku: "SKU-LOT-OTHER".into(), quantity: dec!(1), ..Default::default() })
        .unwrap();
    let ret = received_return(&db, sku, 1);
    let err = db
        .returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                lot_id: Some(wrong.id),
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .expect_err("lot sku mismatch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(db.returns().get(ret.id).unwrap().unwrap().items[0].disposition, None);
}

// ---------------------------------------------------------------------------
// R8: typed wrong-status errors; batch updates are guarded
// ---------------------------------------------------------------------------

#[test]
fn wrong_status_transitions_raise_typed_errors() {
    let db = db();
    let order = shipped_order(&db, "SKU-TYPED", 1, dec!(10), None);
    let ret = create_return(&db, &order, 1);

    let err = db.returns().complete(ret.id).expect_err("complete from requested");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");

    db.returns().approve(ret.id).unwrap();
    let err = db.returns().approve(ret.id).expect_err("approve twice");
    assert!(matches!(err, CommerceError::ReturnCannotBeApproved(_)), "got {err:?}");
    let err = db.returns().reject(ret.id, "late").expect_err("reject approved");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");

    let err = db.returns().approve(stateset_core::ReturnId::new()).expect_err("unknown");
    assert!(matches!(err, CommerceError::ReturnNotFound(_)), "got {err:?}");
}

#[test]
fn atomic_batch_update_is_guarded_and_rolls_back() {
    let db = db();
    let order = shipped_order(&db, "SKU-BATCH", 2, dec!(10), None);
    let a = create_return(&db, &order, 1);
    let b = create_return(&db, &order, 1);
    let err = db
        .returns()
        .update_batch_atomic(vec![
            (a.id, UpdateReturn { status: Some(ReturnStatus::Approved), ..Default::default() }),
            (b.id, UpdateReturn { status: Some(ReturnStatus::Completed), ..Default::default() }),
        ])
        .expect_err("illegal transition in batch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(db.returns().get(a.id).unwrap().unwrap().status, ReturnStatus::Requested);
    assert_eq!(db.returns().get(b.id).unwrap().unwrap().status, ReturnStatus::Requested);
}

// ---------------------------------------------------------------------------
// R10: SQLite concurrency twins of the Postgres race tests
// ---------------------------------------------------------------------------

/// Two threads racing to return the whole of one order line: the over-return
/// guard and the insert share one `IMMEDIATE` write transaction, so exactly
/// one wins and the loser sees `ReturnExceedsReturnable`.
#[test]
fn concurrent_full_returns_on_one_line_admit_exactly_one() {
    let db = Arc::new(db());
    let order = shipped_order(&db, "SKU-RACE-FULL", 3, dec!(10), None);
    let barrier = Arc::new(Barrier::new(2));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let order = order.clone();
            thread::spawn(move || {
                barrier.wait();
                create_return_result(&db, &order, 3)
            })
        })
        .collect();

    let (mut ok, mut exceeded) = (0, 0);
    for handle in handles {
        match handle.join().expect("thread") {
            Ok(_) => ok += 1,
            Err(CommerceError::ReturnExceedsReturnable { .. }) => exceeded += 1,
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!((ok, exceeded), (1, 1));
    let stored = db
        .returns()
        .list(stateset_core::ReturnFilter { order_id: Some(order.id), ..Default::default() })
        .expect("list");
    assert_eq!(stored.len(), 1, "exactly one return may claim the line");
}

/// Four threads creating with the same idempotency key produce one return;
/// every loser replays the winner rather than conflicting.
#[test]
fn concurrent_creates_with_same_idempotency_key_yield_one_return() {
    let db = Arc::new(db());
    let order = shipped_order(&db, "SKU-RACE-IDEM", 4, dec!(10), None);
    let key = format!("ret-{}", Uuid::new_v4());
    let barrier = Arc::new(Barrier::new(4));

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let order = order.clone();
            let key = key.clone();
            thread::spawn(move || {
                barrier.wait();
                db.returns().create(CreateReturn {
                    order_id: order.id,
                    idempotency_key: Some(key),
                    items: vec![CreateReturnItem {
                        order_item_id: order.items[0].id,
                        quantity: 1,
                        condition: None,
                    }],
                    ..Default::default()
                })
            })
        })
        .collect();

    let mut ids: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("thread").expect("every caller gets the return").id)
        .collect();
    ids.dedup();
    assert_eq!(ids.len(), 1, "all callers must see the same return: {ids:?}");
    let count = db
        .returns()
        .count(stateset_core::ReturnFilter { order_id: Some(order.id), ..Default::default() })
        .expect("count");
    assert_eq!(count, 1);
}
