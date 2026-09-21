//! Postgres regressions for engine defects surfaced by the Node binding tests
//! (the SQLite twins live inline in each `sqlite/<module>.rs`):
//!
//! 1. `get_summary` decoded `SUM(CASE ...)` over zero rows (NULL) into an
//!    integer and failed on an empty store.
//! 2. `create_backorder` accepted quantity <= 0.
//! 3. `lots.create` accepted quantity <= 0.
//! 4. `serials.create` stored `lot_number` as text without resolving `lot_id`,
//!    so a lot quarantine never reached the serial.
//! 5. `create_receipt_from_po` turned an unknown PO into an empty receipt.
//! 6. `create_wave` accepted order ids that do not exist.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise. The summary test empties the `backorders` tables to reach the
//! zero-row state, so it is not safe to run alongside a backorder test in
//! another process against the same database.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    BackorderFilter, CommerceError, CreateBackorder, CreateCustomer, CreateLot, CreateOrder,
    CreateOrderItem, CreateSerialNumber, CreateSerialNumbersBulk, CreateWarehouse, CreateWave,
    LotFilter, OrderId, ProductId, SerialFilter, SerialStatus, WarehouseType,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return None;
    };
    Some(PostgresDatabase::connect(&url).await.expect("connect + migrate"))
}

fn backorder_input(sku: &str, quantity: Decimal) -> CreateBackorder {
    CreateBackorder {
        order_id: Uuid::new_v4(),
        order_line_id: None,
        customer_id: Uuid::new_v4(),
        sku: sku.to_string(),
        quantity,
        priority: None,
        expected_date: None,
        promised_date: None,
        source_location_id: None,
        notes: None,
    }
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

// ---------------------------------------------------------------------------
// 1. backorder summary on an empty store
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_backorder_summary_is_all_zero_on_empty_store_and_after_cancel() {
    let Some(db) = connect().await else { return };
    for table in ["backorder_fulfillments", "backorder_allocations", "backorders"] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(db.pool())
            .await
            .expect("empty backorder tables");
    }

    let assert_all_zero = |summary: &stateset_core::BackorderSummary| {
        assert_eq!(summary.total_backorders, 0, "{summary:?}");
        assert_eq!(summary.total_quantity, Decimal::ZERO, "{summary:?}");
        assert_eq!(summary.pending_count, 0, "{summary:?}");
        assert_eq!(summary.allocated_count, 0, "{summary:?}");
        assert_eq!(summary.critical_count, 0, "{summary:?}");
        assert_eq!(summary.overdue_count, 0, "{summary:?}");
    };

    let empty = db.backorder().get_summary_async().await.expect("summary on an empty store");
    assert_all_zero(&empty);

    let bo = db
        .backorder()
        .create_backorder_async(backorder_input(&unique("SKU-SUM"), dec!(3)))
        .await
        .expect("create backorder");
    let open = db.backorder().get_summary_async().await.expect("summary with one open backorder");
    assert_eq!(open.total_backorders, 1);
    assert_eq!(open.pending_count, 1);
    assert_eq!(open.total_quantity, dec!(3));

    db.backorder().cancel_backorder_async(bo.id).await.expect("cancel");
    let after = db.backorder().get_summary_async().await.expect("summary after cancel");
    assert_all_zero(&after);
}

// ---------------------------------------------------------------------------
// 2. backorder quantity must be positive
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_create_backorder_rejects_non_positive_quantity() {
    let Some(db) = connect().await else { return };
    let sku = unique("SKU-BO-NEG");
    for qty in [dec!(0), dec!(-1)] {
        let err = db
            .backorder()
            .create_backorder_async(backorder_input(&sku, qty))
            .await
            .expect_err("non-positive quantity must be refused");
        assert!(matches!(err, CommerceError::ValidationError(_)), "qty {qty}: got {err:?}");
    }
    let written = db
        .backorder()
        .list_backorders_async(BackorderFilter { sku: Some(sku), ..Default::default() })
        .await
        .expect("list");
    assert!(written.is_empty(), "a refused backorder must not be written: {written:?}");
}

// ---------------------------------------------------------------------------
// 3. lot quantity must be positive
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_create_lot_rejects_non_positive_quantity() {
    let Some(db) = connect().await else { return };
    let sku = unique("SKU-LOT-NEG");
    for qty in [dec!(0), dec!(-1)] {
        let err = db
            .lots()
            .create_async(CreateLot { sku: sku.clone(), quantity: qty, ..Default::default() })
            .await
            .expect_err("non-positive quantity must be refused");
        assert!(matches!(err, CommerceError::ValidationError(_)), "qty {qty}: got {err:?}");
    }
    let written = db
        .lots()
        .list_async(LotFilter { sku: Some(sku), ..Default::default() })
        .await
        .expect("list");
    assert!(written.is_empty(), "a refused lot must not be written: {written:?}");
}

// ---------------------------------------------------------------------------
// 4. serial lot_number resolves to lot_id
// ---------------------------------------------------------------------------

fn serial_in_lot(sku: &str, serial: &str, lot_number: &str) -> CreateSerialNumber {
    CreateSerialNumber {
        serial: Some(serial.to_string()),
        sku: sku.to_string(),
        lot_id: None,
        lot_number: Some(lot_number.to_string()),
        location_id: None,
        manufactured_at: None,
        notes: None,
        attributes: None,
    }
}

#[tokio::test]
async fn postgres_serial_create_resolves_lot_number_and_follows_lot_quarantine() {
    let Some(db) = connect().await else { return };
    let sku = unique("SKU-LN");
    let lot_number = unique("LOT-LN");
    let lot = db
        .lots()
        .create_async(CreateLot {
            sku: sku.clone(),
            lot_number: Some(lot_number.clone()),
            quantity: dec!(10),
            ..Default::default()
        })
        .await
        .expect("create lot");

    let serial = db
        .serials()
        .create_async(serial_in_lot(&sku, &unique("SN-LN"), &lot_number))
        .await
        .expect("create serial");
    assert_eq!(serial.lot_id, Some(lot.id), "lot_number must resolve to the lot's id");
    assert_eq!(serial.lot_number.as_deref(), Some(lot_number.as_str()));

    let bulk = db
        .serials()
        .create_bulk_async(CreateSerialNumbersBulk {
            sku: sku.clone(),
            quantity: 2,
            prefix: Some(unique("BLK-LN")),
            lot_id: None,
            lot_number: Some(lot_number.clone()),
            location_id: None,
            manufactured_at: None,
        })
        .await
        .expect("bulk");
    assert!(bulk.iter().all(|s| s.lot_id == Some(lot.id)), "bulk create must resolve too");

    db.lots().quarantine_async(lot.id, "recall").await.expect("quarantine lot");
    for id in std::iter::once(serial.id).chain(bulk.iter().map(|s| s.id)) {
        let after = db.serials().get_async(id).await.expect("get").expect("exists");
        assert_eq!(after.status, SerialStatus::Quarantined, "serial {id} did not follow its lot");
    }
}

#[tokio::test]
async fn postgres_serial_create_with_unknown_lot_number_is_not_found() {
    let Some(db) = connect().await else { return };
    let sku = unique("SKU-LN-X");
    let err = db
        .serials()
        .create_async(serial_in_lot(&sku, &unique("SN-LN-X"), &unique("LOT-MISSING")))
        .await
        .expect_err("unknown lot number must be refused");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    let err = db
        .serials()
        .create_bulk_async(CreateSerialNumbersBulk {
            sku: sku.clone(),
            quantity: 2,
            prefix: Some(unique("BLK-X")),
            lot_id: None,
            lot_number: Some(unique("LOT-MISSING")),
            location_id: None,
            manufactured_at: None,
        })
        .await
        .expect_err("unknown lot number must be refused in bulk");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    let count = db
        .serials()
        .count_async(SerialFilter { sku: Some(sku), ..Default::default() })
        .await
        .expect("count");
    assert_eq!(count, 0, "a refused serial must not be written");
}

// ---------------------------------------------------------------------------
// 5. receipt from an unknown PO
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_create_receipt_from_unknown_po_is_not_found() {
    let Some(db) = connect().await else { return };
    let po_id = Uuid::new_v4();
    let err = db
        .receiving()
        .create_receipt_from_po_async(po_id, 1)
        .await
        .expect_err("unknown PO must be refused");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    let written: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM receipts WHERE reference_id = $1")
        .bind(po_id)
        .fetch_one(db.pool())
        .await
        .expect("count");
    assert_eq!(written, 0, "no receipt may be written for an unknown PO");
}

// ---------------------------------------------------------------------------
// 6. wave with an unknown order
// ---------------------------------------------------------------------------

async fn seed_order(db: &PostgresDatabase) -> OrderId {
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: format!("{}@example.com", unique("wave")),
            first_name: "Wave".into(),
            last_name: "Order".into(),
            ..Default::default()
        })
        .await
        .expect("create customer");
    db.orders()
        .create_async(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: unique("SKU-WAVE"),
                name: "Widget".into(),
                quantity: 1,
                unit_price: dec!(10.00),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .expect("create order")
        .id
}

#[tokio::test]
async fn postgres_create_wave_rejects_unknown_order_and_writes_nothing() {
    let Some(db) = connect().await else { return };
    let warehouse = db
        .warehouse()
        .create_warehouse_async(CreateWarehouse {
            code: unique("WH"),
            name: "Wave Test".into(),
            warehouse_type: WarehouseType::Distribution,
            ..Default::default()
        })
        .await
        .expect("create warehouse");

    let known = seed_order(&db).await;
    let unknown = OrderId::new();
    let err = db
        .fulfillment()
        .create_wave_async(CreateWave {
            warehouse_id: warehouse.id,
            order_ids: vec![known, unknown],
            priority: None,
            notes: None,
            created_by: None,
        })
        .await
        .expect_err("an unknown order must be refused");
    assert!(
        matches!(err, CommerceError::OrderNotFound(id) if id == unknown.into_uuid()),
        "got {err:?}"
    );

    for order in [known, unknown] {
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM wave_orders WHERE order_id = $1")
            .bind(order.into_uuid())
            .fetch_one(db.pool())
            .await
            .expect("count");
        assert_eq!(rows, 0, "no wave_orders row may be written for {order}");
    }

    // A wave of real orders still goes through.
    let wave = db
        .fulfillment()
        .create_wave_async(CreateWave {
            warehouse_id: warehouse.id,
            order_ids: vec![known],
            priority: None,
            notes: None,
            created_by: None,
        })
        .await
        .expect("wave of existing orders");
    let orders = db.fulfillment().get_wave_orders_async(wave.id.into_uuid()).await.expect("orders");
    assert_eq!(orders, vec![known.into_uuid()]);
}
