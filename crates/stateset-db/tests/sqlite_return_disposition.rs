#![cfg(feature = "sqlite")]
//! Return item disposition against the live SQLite engine.
//!
//! Covers each disposition's stock effect (warehouse level and, when bins
//! exist, the returns/quarantine bin), the status precondition, idempotency
//! (a second disposition on the same item is rejected with `Conflict`), and
//! rollback when the stock effect fails.

use rust_decimal_macros::dec;
use stateset_core::{
    BinRepository, BinType, CommerceError, CreateCustomer, CreateInventoryItem, CreateOrder,
    CreateOrderItem, CreateProduct, CreateReturn, CreateReturnItem, CreateWarehouse,
    CreateWarehouseBin, CustomerRepository, InventoryRepository, OrderRepository, OrderStatus,
    ProductRepository, Return, ReturnDisposition, ReturnRepository, ReturnStatus,
    SetReturnDisposition, UpdateOrder, UpdateReturn, WarehouseAddress, WarehouseRepository,
    WarehouseType,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("in-memory sqlite")
}

/// Customer + product + order with one line of `quantity` units of `sku`.
fn order_with_item(db: &SqliteDatabase, sku: &str, quantity: i32) -> stateset_core::Order {
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
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("create order");
    // Returns require shipped goods; move the order through to Shipped.
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .expect("advance order status");
    }
    db.orders().get(order.id).expect("get order").expect("order exists")
}

/// A return of `quantity` units, advanced to `received`.
fn received_return(db: &SqliteDatabase, sku: &str, quantity: i32) -> Return {
    let order = order_with_item(db, sku, quantity);
    let ret = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity,
                condition: None,
            }],
            ..Default::default()
        })
        .expect("create return");
    db.returns().approve(ret.id).expect("approve");
    for status in [ReturnStatus::InTransit, ReturnStatus::Received] {
        db.returns()
            .update(ret.id, UpdateReturn { status: Some(status), ..Default::default() })
            .expect("advance status");
    }
    db.returns().get(ret.id).expect("get").expect("present")
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

fn bin(db: &SqliteDatabase, wh: i32, code: &str, bin_type: BinType) -> i32 {
    db.bins()
        .create_bin(CreateWarehouseBin {
            warehouse_id: wh,
            code: code.into(),
            bin_type,
            ..Default::default()
        })
        .expect("create bin")
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

fn on_hand(
    db: &SqliteDatabase,
    sku: &str,
    wh: i32,
) -> (rust_decimal::Decimal, rust_decimal::Decimal) {
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

#[test]
fn restock_without_bins_increments_warehouse_on_hand() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-RESTOCK";
    item(&db, sku);
    let ret = received_return(&db, sku, 3);
    let item_id = ret.items[0].id;

    let updated = db
        .returns()
        .set_item_disposition(ret.id, item_id, disposition(ReturnDisposition::Restock, wh))
        .expect("restock");
    assert_eq!(updated.disposition, Some(ReturnDisposition::Restock));
    assert!(updated.disposition_at.is_some());
    assert_eq!(updated.disposition_by.as_deref(), Some("inspector"));
    assert_eq!(on_hand(&db, sku, wh), (dec!(3), dec!(0)));

    // Persisted on the return's items.
    let reloaded = db.returns().get(ret.id).unwrap().unwrap();
    assert_eq!(reloaded.items[0].disposition, Some(ReturnDisposition::Restock));
}

#[test]
fn restock_with_bins_lands_in_returns_bin_and_keeps_invariant() {
    let db = db();
    let wh = warehouse(&db);
    let _pick = bin(&db, wh, "PICK", BinType::Pick);
    let returns_bin = bin(&db, wh, "RET", BinType::Returns);
    let sku = "SKU-RESTOCK-BIN";
    item(&db, sku);
    let ret = received_return(&db, sku, 2);

    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Restock, wh))
        .expect("restock");
    let levels = db.bins().get_bin_levels(returns_bin).unwrap();
    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].quantity_on_hand, dec!(2));
    assert_eq!(on_hand(&db, sku, wh), (dec!(2), dec!(0)));
    assert!(db.bins().reconcile(wh, sku).unwrap().is_balanced());
}

#[test]
fn restock_with_explicit_bin_uses_it() {
    let db = db();
    let wh = warehouse(&db);
    let _returns_bin = bin(&db, wh, "RET", BinType::Returns);
    let explicit = bin(&db, wh, "A-1", BinType::Pick);
    let sku = "SKU-EXPLICIT";
    item(&db, sku);
    let ret = received_return(&db, sku, 1);
    db.returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            SetReturnDisposition {
                bin_id: Some(explicit),
                ..disposition(ReturnDisposition::Restock, wh)
            },
        )
        .unwrap();
    assert_eq!(db.bins().get_bin_levels(explicit).unwrap()[0].quantity_on_hand, dec!(1));
}

#[test]
fn quarantine_with_bin_holds_stock_as_allocated() {
    let db = db();
    let wh = warehouse(&db);
    let q = bin(&db, wh, "QUAR", BinType::Quarantine);
    let sku = "SKU-QUAR";
    item(&db, sku);
    let ret = received_return(&db, sku, 4);
    db.returns()
        .set_item_disposition(
            ret.id,
            ret.items[0].id,
            disposition(ReturnDisposition::Quarantine, wh),
        )
        .unwrap();
    let level = &db.bins().get_bin_levels(q).unwrap()[0];
    assert_eq!(level.quantity_on_hand, dec!(4));
    assert_eq!(level.quantity_allocated, dec!(4));
    assert_eq!(level.quantity_available, dec!(0));
    assert_eq!(on_hand(&db, sku, wh), (dec!(4), dec!(4)));
    assert!(db.bins().reconcile(wh, sku).unwrap().is_balanced());
}

/// Without a quarantine bin the hold is still recorded at warehouse level (on
/// hand + allocated), so received units never vanish from stock tracking.
#[test]
fn quarantine_without_bins_holds_stock_at_warehouse() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-QUAR-NOBIN";
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
    assert_eq!(on_hand(&db, sku, wh), (dec!(4), dec!(4)));
}

#[test]
fn scrap_return_to_vendor_and_refurbish_do_not_touch_stock() {
    let db = db();
    let wh = warehouse(&db);
    let _ret_bin = bin(&db, wh, "RET", BinType::Returns);
    for d in
        [ReturnDisposition::Scrap, ReturnDisposition::ReturnToVendor, ReturnDisposition::Refurbish]
    {
        let sku = format!("SKU-{d}");
        item(&db, &sku);
        let ret = received_return(&db, &sku, 2);
        let updated =
            db.returns().set_item_disposition(ret.id, ret.items[0].id, disposition(d, wh)).unwrap();
        assert_eq!(updated.disposition, Some(d));
        assert_eq!(on_hand(&db, &sku, wh), (dec!(0), dec!(0)));
        assert!(db.bins().get_bin_levels_for_sku(wh, &sku).unwrap().is_empty());
    }
}

#[test]
fn second_disposition_on_same_item_is_rejected() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-TWICE";
    item(&db, sku);
    let ret = received_return(&db, sku, 1);
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Scrap, wh))
        .unwrap();
    let err = db
        .returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Restock, wh))
        .expect_err("second disposition");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    // The rejected Restock did not add stock.
    assert_eq!(on_hand(&db, sku, wh), (dec!(0), dec!(0)));
}

#[test]
fn disposition_requires_received_status() {
    let db = db();
    let wh = warehouse(&db);
    let sku = "SKU-STATUS";
    item(&db, sku);
    let order = order_with_item(&db, sku, 1);
    let ret = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                condition: None,
            }],
            ..Default::default()
        })
        .unwrap();
    let err = db
        .returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Restock, wh))
        .expect_err("not received yet");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");

    let err = db
        .returns()
        .set_item_disposition(ret.id, Uuid::new_v4(), disposition(ReturnDisposition::Restock, wh))
        .expect_err("unknown item");
    assert!(
        matches!(err, CommerceError::NotPermitted(_) | CommerceError::ValidationError(_)),
        "got {err:?}"
    );
    let err = db
        .returns()
        .set_item_disposition(
            stateset_core::ReturnId::new(),
            ret.items[0].id,
            disposition(ReturnDisposition::Restock, wh),
        )
        .expect_err("unknown return");
    assert!(matches!(err, CommerceError::ReturnNotFound(_)), "got {err:?}");
}

#[test]
fn failed_stock_effect_rolls_back_disposition() {
    let db = db();
    let wh = warehouse(&db);
    // No inventory item for the SKU: the warehouse-level restock fails.
    let sku = "SKU-NO-ITEM";
    let ret = received_return(&db, sku, 1);
    let err = db
        .returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Restock, wh))
        .expect_err("restock must fail without inventory item");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    let reloaded = db.returns().get(ret.id).unwrap().unwrap();
    assert_eq!(reloaded.items[0].disposition, None);
    assert!(reloaded.items[0].disposition_at.is_none());
    // Retrying after fixing the cause succeeds (nothing was half-written).
    item(&db, sku);
    db.returns()
        .set_item_disposition(ret.id, ret.items[0].id, disposition(ReturnDisposition::Restock, wh))
        .expect("retry succeeds");
    assert_eq!(on_hand(&db, sku, wh), (dec!(1), dec!(0)));
}

#[test]
fn return_state_machine_and_outbox_are_atomic() {
    let db = db();
    let ret = received_return(&db, "SKU-KERNEL-RETURN", 1);

    let before = db.kernel_outbox().pending(100).expect("events");
    let lifecycle_before: Vec<_> =
        before.iter().filter(|event| event.aggregate_id == ret.id.to_string()).collect();
    assert_eq!(lifecycle_before.len(), 4);
    assert_eq!(lifecycle_before[0].payload["refund_amount"], "10");

    let error = db
        .returns()
        .update(ret.id, UpdateReturn { status: Some(ReturnStatus::Approved), ..Default::default() })
        .expect_err("received -> approved must be rejected");
    assert!(matches!(error, CommerceError::ValidationError(_)));

    let after = db.kernel_outbox().pending(100).expect("events after rejection");
    let lifecycle_after =
        after.iter().filter(|event| event.aggregate_id == ret.id.to_string()).count();
    assert_eq!(lifecycle_after, lifecycle_before.len());
    assert_eq!(db.returns().get(ret.id).unwrap().unwrap().status, ReturnStatus::Received);
}
