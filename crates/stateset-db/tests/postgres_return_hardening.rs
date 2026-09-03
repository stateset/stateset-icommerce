//! Returns hardening against a live Postgres (mirror of
//! `sqlite_return_hardening.rs`, plus the concurrency cases only Postgres
//! can exhibit: two connections racing the over-return guard and the
//! idempotency key).
//!
//! Requires `POSTGRES_URL` / `DATABASE_URL`; skipped otherwise.
#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateCustomer, CreateInventoryItem, CreateLot, CreateOrder, CreateOrderItem,
    CreatePayment, CreateProduct, CreateReturn, CreateReturnItem, CreateSerialNumber,
    CreateWarehouse, CurrencyCode, Order, PaymentMethodType, RefundStatus, Return,
    ReturnDisposition, ReturnStatus, SerialStatus, SetReturnDisposition, UpdateOrder, UpdateReturn,
    WarehouseAddress, WarehouseType,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

macro_rules! require_pg {
    () => {
        match postgres_url() {
            Some(url) => PostgresDatabase::connect(&url).await.expect("connect + migrate"),
            None => {
                eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
                return;
            }
        }
    };
}

fn unique_sku(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn shipped_order(
    db: &PostgresDatabase,
    sku: &str,
    quantity: i32,
    unit_price: Decimal,
    discount: Option<Decimal>,
) -> Order {
    let unique = Uuid::new_v4().to_string();
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("ret-{unique}@example.com"),
            first_name: "Ret".into(),
            last_name: "Urn".into(),
            phone: None,
            accepts_marketing: Some(false),
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");
    let product = db
        .products()
        .create_async(CreateProduct {
            name: format!("Widget {unique}"),
            slug: Some(format!("widget-{unique}")),
            description: None,
            product_type: None,
            attributes: None,
            seo: None,
            variants: None,
        })
        .await
        .expect("create product");
    let order = db
        .orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                variant_id: None,
                sku: sku.into(),
                name: "Widget".into(),
                quantity,
                unit_price,
                discount,
                tax_amount: None,
            }],
            ..Default::default()
        })
        .await
        .expect("create order");
    let mut shipped = order;
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        shipped = db
            .orders()
            .update_async(
                shipped.id.into_uuid(),
                UpdateOrder { status: Some(status), ..Default::default() },
            )
            .await
            .expect("advance order status");
    }
    shipped
}
use stateset_core::OrderStatus;

fn return_request(order: &Order, quantity: i32) -> CreateReturn {
    CreateReturn {
        order_id: order.id,
        items: vec![CreateReturnItem {
            order_item_id: order.items[0].id,
            quantity,
            condition: None,
        }],
        ..Default::default()
    }
}

async fn received_return(db: &PostgresDatabase, order: &Order, quantity: i32) -> Return {
    let ret = db.returns().create_async(return_request(order, quantity)).await.expect("create");
    db.returns().approve_async(ret.id.into_uuid()).await.expect("approve");
    for status in [ReturnStatus::InTransit, ReturnStatus::Received] {
        db.returns()
            .update_async(
                ret.id.into_uuid(),
                UpdateReturn { status: Some(status), ..Default::default() },
            )
            .await
            .expect("advance");
    }
    db.returns().get_async(ret.id.into_uuid()).await.unwrap().unwrap()
}

async fn warehouse(db: &PostgresDatabase) -> i32 {
    db.warehouse()
        .create_warehouse_async(CreateWarehouse {
            code: format!("WH-{}", &Uuid::new_v4().simple().to_string()[..8]),
            name: "Returns DC".into(),
            warehouse_type: WarehouseType::Returns,
            address: WarehouseAddress { country: "US".into(), ..Default::default() },
            timezone: None,
        })
        .await
        .expect("create warehouse")
        .id
}

async fn item(db: &PostgresDatabase, sku: &str) {
    db.inventory()
        .create_item_async(CreateInventoryItem {
            sku: sku.into(),
            name: sku.into(),
            ..Default::default()
        })
        .await
        .expect("create inventory item");
}

async fn on_hand(db: &PostgresDatabase, sku: &str, wh: i32) -> (Decimal, Decimal) {
    let stock = db.inventory().get_stock_async(sku).await.expect("stock").expect("present");
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

async fn completed_payment(
    db: &PostgresDatabase,
    order: &Order,
    amount: Decimal,
) -> stateset_core::Payment {
    let payment = db
        .payments()
        .create_async(CreatePayment {
            order_id: Some(order.id),
            customer_id: Some(order.customer_id),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            currency: Some(CurrencyCode::USD),
            ..Default::default()
        })
        .await
        .expect("create payment");
    db.payments().mark_completed_async(payment.id.into_uuid()).await.expect("complete payment")
}

// ---------------------------------------------------------------------------
// R4: concurrency
// ---------------------------------------------------------------------------

/// Two concurrent returns of the full shipped quantity on the same line:
/// exactly one may succeed. Each runs on its own pool connection and the
/// `FOR UPDATE` on the order item serializes them.
#[tokio::test]
async fn concurrent_full_returns_on_one_line_admit_exactly_one() {
    let db = require_pg!();
    let order = shipped_order(&db, &unique_sku("SKU-RACE"), 3, dec!(10), None).await;

    let mut handles = Vec::new();
    for _ in 0..2 {
        let db = db.clone();
        let request = return_request(&order, 3);
        handles.push(tokio::spawn(async move { db.returns().create_async(request).await }));
    }
    let mut ok = 0;
    let mut exceeded = 0;
    for handle in handles {
        match handle.await.expect("task") {
            Ok(_) => ok += 1,
            Err(CommerceError::ReturnExceedsReturnable { .. }) => exceeded += 1,
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!((ok, exceeded), (1, 1));
    let returns = db
        .returns()
        .list_async(stateset_core::ReturnFilter { order_id: Some(order.id), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(returns.len(), 1);
}

/// Two concurrent creates with the same idempotency key produce one return;
/// the loser replays the winner.
#[tokio::test]
async fn concurrent_creates_with_same_idempotency_key_yield_one_return() {
    let db = require_pg!();
    let order = shipped_order(&db, &unique_sku("SKU-IDEM-RACE"), 4, dec!(10), None).await;
    let key = format!("ret-{}", Uuid::new_v4());

    let mut handles = Vec::new();
    for _ in 0..4 {
        let db = db.clone();
        let request =
            CreateReturn { idempotency_key: Some(key.clone()), ..return_request(&order, 1) };
        handles.push(tokio::spawn(async move { db.returns().create_async(request).await }));
    }
    let mut ids = Vec::new();
    for handle in handles {
        ids.push(handle.await.expect("task").expect("every caller gets the return").id);
    }
    ids.dedup();
    assert_eq!(ids.len(), 1, "all callers must see the same return: {ids:?}");
    let count = db
        .returns()
        .count_async(stateset_core::ReturnFilter { order_id: Some(order.id), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn idempotency_key_reuse_with_different_payload_conflicts() {
    let db = require_pg!();
    let order = shipped_order(&db, &unique_sku("SKU-IDEM"), 4, dec!(10), None).await;
    let key = format!("ret-{}", Uuid::new_v4());
    let request = CreateReturn { idempotency_key: Some(key.clone()), ..return_request(&order, 1) };
    let first = db.returns().create_async(request.clone()).await.unwrap();
    let replay = db.returns().create_async(request.clone()).await.unwrap();
    assert_eq!(first.id, replay.id);
    let err = db
        .returns()
        .create_async(CreateReturn { items: return_request(&order, 2).items, ..request })
        .await
        .expect_err("different payload");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
}

// ---------------------------------------------------------------------------
// R1 / R2 / R3 / R5 / R6 / R7 / R8 mirrors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn line_refund_honours_discount_and_refund_amount_is_bounded() {
    let db = require_pg!();
    let order = shipped_order(&db, &unique_sku("SKU-DISC"), 2, dec!(10), Some(dec!(4))).await;
    let ret = db.returns().create_async(return_request(&order, 1)).await.unwrap();
    assert_eq!(ret.items[0].refund_amount, dec!(8));
    assert_eq!(ret.refund_amount, Some(dec!(8)));

    for bad in [dec!(-1), dec!(8.01)] {
        let err = db
            .returns()
            .update_async(
                ret.id.into_uuid(),
                UpdateReturn { refund_amount: Some(bad), ..Default::default() },
            )
            .await
            .expect_err("out-of-bounds refund");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    }
    let ok = db
        .returns()
        .update_async(
            ret.id.into_uuid(),
            UpdateReturn { refund_amount: Some(dec!(5)), ..Default::default() },
        )
        .await
        .unwrap();
    assert_eq!(ok.refund_amount, Some(dec!(5)));
}

#[tokio::test]
async fn completion_settles_refund_and_freezes_the_return() {
    let db = require_pg!();
    let wh = warehouse(&db).await;
    let order = shipped_order(&db, &unique_sku("SKU-SETTLE"), 2, dec!(10), None).await;
    let payment = completed_payment(&db, &order, dec!(20)).await;
    let ret = received_return(&db, &order, 1).await;
    db.returns()
        .set_item_disposition_async(
            ret.id,
            ret.items[0].id,
            disposition(ReturnDisposition::Scrap, wh),
        )
        .await
        .unwrap();
    let done = db.returns().complete_async(ret.id.into_uuid()).await.expect("complete");
    assert_eq!(done.status, ReturnStatus::Completed);

    let refunds = db.payments().get_refunds_async(payment.id.into_uuid()).await.unwrap();
    assert_eq!(refunds.len(), 1);
    assert_eq!(refunds[0].amount, dec!(10));
    assert_eq!(refunds[0].status, RefundStatus::Pending);
    assert_eq!(
        refunds[0].idempotency_key.as_deref(),
        Some(format!("return:{}:{}", ret.id, payment.id).as_str())
    );

    let err = db
        .returns()
        .update_async(
            ret.id.into_uuid(),
            UpdateReturn { notes: Some("late".into()), ..Default::default() },
        )
        .await
        .expect_err("terminal");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");

    // Store credit settles out of band.
    let order = shipped_order(&db, &unique_sku("SKU-CREDIT"), 1, dec!(10), None).await;
    let payment = completed_payment(&db, &order, dec!(10)).await;
    let ret = received_return(&db, &order, 1).await;
    db.returns()
        .set_item_disposition_async(
            ret.id,
            ret.items[0].id,
            disposition(ReturnDisposition::Scrap, wh),
        )
        .await
        .unwrap();
    db.returns()
        .update_async(
            ret.id.into_uuid(),
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                refund_method: Some("store_credit".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(db.payments().get_refunds_async(payment.id.into_uuid()).await.unwrap().is_empty());
}

#[tokio::test]
async fn reject_after_restock_is_refused_and_claim_is_kept() {
    let db = require_pg!();
    let wh = warehouse(&db).await;
    let sku = unique_sku("SKU-REJ");
    item(&db, &sku).await;
    let order = shipped_order(&db, &sku, 2, dec!(10), None).await;
    let ret = received_return(&db, &order, 2).await;
    db.returns()
        .update_async(
            ret.id.into_uuid(),
            UpdateReturn { status: Some(ReturnStatus::Inspecting), ..Default::default() },
        )
        .await
        .unwrap();
    db.returns()
        .set_item_disposition_async(
            ret.id,
            ret.items[0].id,
            disposition(ReturnDisposition::Restock, wh),
        )
        .await
        .unwrap();
    assert_eq!(on_hand(&db, &sku, wh).await, (dec!(2), dec!(0)));

    let err = db.returns().reject_async(ret.id.into_uuid(), "nope").await.expect_err("reject");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    let err = db.returns().create_async(return_request(&order, 1)).await.expect_err("claimed");
    assert!(matches!(err, CommerceError::ReturnExceedsReturnable { .. }), "got {err:?}");

    // Scrap and return-to-vendor destroy the goods without a stock effect, so
    // the old `affects_stock`-only guard let a scrapped return be rejected —
    // which released its claim and made the destroyed units returnable and
    // refundable again. ANY disposition now pins the return.
    for (label, disp) in [
        ("scrap", ReturnDisposition::Scrap),
        ("return_to_vendor", ReturnDisposition::ReturnToVendor),
        ("refurbish", ReturnDisposition::Refurbish),
    ] {
        let sku = unique_sku(&format!("SKU-REJ-{label}"));
        item(&db, &sku).await;
        let order = shipped_order(&db, &sku, 1, dec!(10), None).await;
        let ret = received_return(&db, &order, 1).await;
        db.returns()
            .update_async(
                ret.id.into_uuid(),
                UpdateReturn { status: Some(ReturnStatus::Inspecting), ..Default::default() },
            )
            .await
            .unwrap();
        db.returns()
            .set_item_disposition_async(ret.id, ret.items[0].id, disposition(disp, wh))
            .await
            .unwrap();
        let err = db
            .returns()
            .reject_async(ret.id.into_uuid(), "damaged")
            .await
            .expect_err("reject after a disposition");
        assert!(matches!(err, CommerceError::Conflict(_)), "{label}: got {err:?}");
        let err = db
            .returns()
            .create_async(return_request(&order, 1))
            .await
            .expect_err("units already claimed");
        assert!(
            matches!(err, CommerceError::ReturnExceedsReturnable { .. }),
            "{label}: got {err:?}"
        );
    }

    // Rejecting is still allowed while nothing has been dispositioned.
    let order = shipped_order(&db, &unique_sku("SKU-REJ-OK"), 1, dec!(10), None).await;
    let ret = received_return(&db, &order, 1).await;
    db.returns()
        .update_async(
            ret.id.into_uuid(),
            UpdateReturn { status: Some(ReturnStatus::Inspecting), ..Default::default() },
        )
        .await
        .unwrap();
    let rejected = db.returns().reject_async(ret.id.into_uuid(), "damaged").await.unwrap();
    assert_eq!(rejected.status, ReturnStatus::Rejected);
}

// ---------------------------------------------------------------------------
// R9: deleting a return may not free the order-line claim
// ---------------------------------------------------------------------------

/// A completed, restocked, refunded return used to be deletable in any status.
/// The over-return guard counts claims from the surviving `return_items`, so
/// the delete made the same units returnable and refundable again while the
/// restocked stock stayed on the shelf.
#[tokio::test]
async fn delete_is_refused_once_the_return_has_had_any_effect() {
    let db = require_pg!();
    let wh = warehouse(&db).await;
    let sku = unique_sku("SKU-DEL");
    item(&db, &sku).await;
    let order = shipped_order(&db, &sku, 2, dec!(10), None).await;
    completed_payment(&db, &order, dec!(20)).await;
    let ret = received_return(&db, &order, 2).await;
    db.returns()
        .update_async(
            ret.id.into_uuid(),
            UpdateReturn { status: Some(ReturnStatus::Inspecting), ..Default::default() },
        )
        .await
        .unwrap();
    db.returns()
        .set_item_disposition_async(
            ret.id,
            ret.items[0].id,
            disposition(ReturnDisposition::Restock, wh),
        )
        .await
        .unwrap();
    db.returns().complete_async(ret.id.into_uuid()).await.expect("complete");
    assert_eq!(on_hand(&db, &sku, wh).await, (dec!(2), dec!(0)));

    for label in ["delete", "delete_batch_atomic"] {
        let err = if label == "delete" {
            db.returns().delete_async(ret.id.into_uuid()).await
        } else {
            db.returns().delete_batch_atomic_async(vec![ret.id]).await
        }
        .expect_err("a completed, restocked return is not deletable");
        assert!(matches!(err, CommerceError::NotPermitted(_)), "{label}: got {err:?}");
    }

    // The return survives, so its claim on the order line survives with it.
    assert!(db.returns().get_async(ret.id.into_uuid()).await.unwrap().is_some());
    let err = db.returns().create_async(return_request(&order, 1)).await.expect_err("claimed");
    assert!(matches!(err, CommerceError::ReturnExceedsReturnable { .. }), "got {err:?}");
    assert_eq!(on_hand(&db, &sku, wh).await, (dec!(2), dec!(0)));
}

/// Deletion is allowed only in the early, no-effect window: `requested` and
/// `approved`. Everything later — goods in motion, or terminal — is refused.
///
/// Every case gets its own single-unit order (and therefore its own SKU): the
/// undeletable returns survive the test, and piling them onto one SKU would
/// leave a large residue in the shared test database.
#[tokio::test]
async fn delete_window_is_requested_and_approved_only() {
    let db = require_pg!();
    let order = shipped_order(&db, &unique_sku("SKU-DELWIN"), 1, dec!(10), None).await;
    let requested = db.returns().create_async(return_request(&order, 1)).await.expect("create");
    db.returns().delete_async(requested.id.into_uuid()).await.expect("delete requested");
    assert!(db.returns().get_async(requested.id.into_uuid()).await.unwrap().is_none());
    assert!(db.returns().get_items_async(requested.id.into_uuid()).await.unwrap().is_empty());

    let approved = db.returns().create_async(return_request(&order, 1)).await.expect("create");
    db.returns().approve_async(approved.id.into_uuid()).await.expect("approve");
    db.returns().delete_async(approved.id.into_uuid()).await.expect("delete approved");
    assert!(db.returns().get_async(approved.id.into_uuid()).await.unwrap().is_none());

    for statuses in [
        vec![ReturnStatus::InTransit],
        vec![ReturnStatus::InTransit, ReturnStatus::Received],
        vec![ReturnStatus::InTransit, ReturnStatus::Received, ReturnStatus::Inspecting],
    ] {
        let order = shipped_order(&db, &unique_sku("SKU-DELWIN"), 1, dec!(10), None).await;
        let ret = db.returns().create_async(return_request(&order, 1)).await.expect("create");
        db.returns().approve_async(ret.id.into_uuid()).await.expect("approve");
        for status in &statuses {
            db.returns()
                .update_async(
                    ret.id.into_uuid(),
                    UpdateReturn { status: Some(*status), ..Default::default() },
                )
                .await
                .expect("advance");
        }
        let err = db
            .returns()
            .delete_async(ret.id.into_uuid())
            .await
            .expect_err("in-flight returns are not deletable");
        assert!(matches!(err, CommerceError::NotPermitted(_)), "{statuses:?}: got {err:?}");
    }

    let order = shipped_order(&db, &unique_sku("SKU-DELWIN"), 1, dec!(10), None).await;
    let cancelled = db.returns().create_async(return_request(&order, 1)).await.expect("create");
    db.returns().cancel_async(cancelled.id.into_uuid()).await.expect("cancel");
    let err = db
        .returns()
        .delete_async(cancelled.id.into_uuid())
        .await
        .expect_err("terminal returns are not deletable");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
}

#[tokio::test]
async fn completion_requires_dispositions_and_quarantine_holds_without_bins() {
    let db = require_pg!();
    let wh = warehouse(&db).await;
    let sku = unique_sku("SKU-QUAR");
    item(&db, &sku).await;
    let order = shipped_order(&db, &sku, 2, dec!(10), None).await;
    let ret = received_return(&db, &order, 2).await;

    let err = db.returns().complete_async(ret.id.into_uuid()).await.expect_err("undispositioned");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");

    db.returns()
        .set_item_disposition_async(
            ret.id,
            ret.items[0].id,
            disposition(ReturnDisposition::Quarantine, wh),
        )
        .await
        .unwrap();
    assert_eq!(on_hand(&db, &sku, wh).await, (dec!(2), dec!(2)));
    let done = db.returns().complete_async(ret.id.into_uuid()).await.unwrap();
    assert_eq!(done.status, ReturnStatus::Completed);

    // Write-off path records the units on the event.
    let order = shipped_order(&db, &unique_sku("SKU-WO"), 3, dec!(10), None).await;
    let ret = received_return(&db, &order, 3).await;
    db.returns()
        .update_async(
            ret.id.into_uuid(),
            UpdateReturn {
                status: Some(ReturnStatus::Completed),
                write_off_undispositioned: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let events = db.kernel_outbox().pending_async(1000).await.unwrap();
    let completion = events
        .iter()
        .find(|e| {
            e.aggregate_id == ret.id.to_string()
                && e.payload["status_after"] == ReturnStatus::Completed.to_string()
        })
        .expect("completion event");
    assert_eq!(completion.payload["undispositioned_units"], 3);
}

#[tokio::test]
async fn disposition_transitions_serials_and_restores_lot() {
    let db = require_pg!();
    let wh = warehouse(&db).await;
    let sku = unique_sku("SKU-TRACE");
    item(&db, &sku).await;
    let lot = db
        .lots()
        .create_async(CreateLot { sku: sku.clone(), quantity: dec!(10), ..Default::default() })
        .await
        .unwrap();
    let mut serials = Vec::new();
    for _ in 0..2 {
        let s = db
            .serials()
            .create_async(CreateSerialNumber { sku: sku.clone(), ..Default::default() })
            .await
            .unwrap();
        serials.push(db.serials().mark_shipped_async(s.id, Uuid::new_v4()).await.unwrap().id);
    }
    let order = shipped_order(&db, &sku, 2, dec!(10), None).await;
    let ret = received_return(&db, &order, 2).await;
    let updated = db
        .returns()
        .set_item_disposition_async(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                lot_id: Some(lot.id),
                serial_ids: serials.clone(),
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .await
        .expect("restock with serials and lot");
    assert_eq!(updated.serial_ids, serials);
    assert_eq!(updated.lot_id, Some(lot.id));
    for id in &serials {
        let serial = db.serials().get_async(*id).await.unwrap().unwrap();
        assert_eq!(serial.status, SerialStatus::Available);
        assert_eq!(serial.current_location_id, Some(wh));
    }
    let lot_after = db.lots().get_async(lot.id).await.unwrap().unwrap();
    assert_eq!(lot_after.quantity_remaining, lot.quantity_remaining + dec!(2));
    let reloaded = db.returns().get_async(ret.id.into_uuid()).await.unwrap().unwrap();
    assert_eq!(reloaded.items[0].serial_ids, serials);

    // Serial count mismatch rolls everything back.
    let order = shipped_order(&db, &sku, 2, dec!(10), None).await;
    let ret = received_return(&db, &order, 2).await;
    let err = db
        .returns()
        .set_item_disposition_async(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                serial_ids: vec![serials[0]],
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .await
        .expect_err("count mismatch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(on_hand(&db, &sku, wh).await, (dec!(2), dec!(0)));
}

#[tokio::test]
async fn typed_wrong_status_errors_and_transactional_delete() {
    let db = require_pg!();
    let order = shipped_order(&db, &unique_sku("SKU-TYPED"), 2, dec!(10), None).await;
    let ret = db.returns().create_async(return_request(&order, 1)).await.unwrap();
    let err = db.returns().complete_async(ret.id.into_uuid()).await.expect_err("complete");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
    db.returns().approve_async(ret.id.into_uuid()).await.unwrap();
    let err = db.returns().approve_async(ret.id.into_uuid()).await.expect_err("approve twice");
    assert!(matches!(err, CommerceError::ReturnCannotBeApproved(_)), "got {err:?}");
    let err =
        db.returns().reject_async(ret.id.into_uuid(), "x").await.expect_err("reject approved");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");

    // Atomic batch update is guarded and rolls back as a unit.
    let other = db.returns().create_async(return_request(&order, 1)).await.unwrap();
    let err = db
        .returns()
        .update_batch_atomic_async(vec![
            (other.id, UpdateReturn { status: Some(ReturnStatus::Approved), ..Default::default() }),
            (ret.id, UpdateReturn { status: Some(ReturnStatus::Completed), ..Default::default() }),
        ])
        .await
        .expect_err("illegal transition in batch");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(
        db.returns().get_async(other.id.into_uuid()).await.unwrap().unwrap().status,
        ReturnStatus::Requested
    );

    // Delete removes header and items together.
    db.returns().delete_batch_atomic_async(vec![other.id]).await.unwrap();
    assert!(db.returns().get_async(other.id.into_uuid()).await.unwrap().is_none());
    assert!(db.returns().get_items_async(other.id.into_uuid()).await.unwrap().is_empty());
}
