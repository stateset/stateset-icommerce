#![cfg(feature = "sqlite")]

use rust_decimal::Decimal;
use stateset_core::{
    AdjustInventory, CreateInventoryItem, CreateInvoice, CreateInvoiceItem, CreatePurchaseOrder,
    CreatePurchaseOrderItem, CreateShipment, CreateShipmentItem, CreateSupplier, InventoryRepository,
    InvoiceRepository, PurchaseOrderRepository, ReserveInventory, ShipmentRepository,
};
use stateset_db::SqliteDatabase;
use uuid::Uuid;

#[test]
fn sqlite_invoice_create_rolls_back_when_item_insert_fails() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute("DROP TABLE invoice_items", [])
            .expect("drop invoice_items");
    }

    let repo = db.invoices();
    repo.create(CreateInvoice {
        customer_id: Uuid::new_v4(),
        items: vec![CreateInvoiceItem {
            description: "Test item".to_string(),
            quantity: Decimal::ONE,
            unit_price: Decimal::ONE,
            ..Default::default()
        }],
        ..Default::default()
    })
    .expect_err("expected create invoice to fail");

    let conn = db.conn().expect("get sqlite connection");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM invoices", [], |row| row.get(0))
        .expect("count invoices");
    assert_eq!(count, 0, "invoice insert should have been rolled back");
}

#[test]
fn sqlite_purchase_order_create_rolls_back_when_item_insert_fails() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let repo = db.purchase_orders();

    let supplier = repo
        .create_supplier(CreateSupplier {
            name: "Test supplier".to_string(),
            ..Default::default()
        })
        .expect("create supplier");

    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute("DROP TABLE purchase_order_items", [])
            .expect("drop purchase_order_items");
    }

    repo.create(CreatePurchaseOrder {
        supplier_id: supplier.id,
        items: vec![CreatePurchaseOrderItem {
            sku: "SKU-1".to_string(),
            name: "Test item".to_string(),
            quantity: Decimal::ONE,
            unit_cost: Decimal::ONE,
            ..Default::default()
        }],
        ..Default::default()
    })
    .expect_err("expected create purchase order to fail");

    let conn = db.conn().expect("get sqlite connection");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM purchase_orders", [], |row| row.get(0))
        .expect("count purchase_orders");
    assert_eq!(count, 0, "purchase order insert should have been rolled back");
}

#[test]
fn sqlite_shipment_create_rolls_back_when_item_insert_fails() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute("DROP TABLE shipment_items", [])
            .expect("drop shipment_items");
    }

    let repo = db.shipments();
    repo.create(CreateShipment {
        order_id: Uuid::new_v4(),
        recipient_name: "Test recipient".to_string(),
        shipping_address: "123 Test St".to_string(),
        items: Some(vec![CreateShipmentItem {
            sku: "SKU-1".to_string(),
            name: "Test item".to_string(),
            quantity: 1,
            ..Default::default()
        }]),
        ..Default::default()
    })
    .expect_err("expected create shipment to fail");

    let conn = db.conn().expect("get sqlite connection");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM shipments", [], |row| row.get(0))
        .expect("count shipments");
    assert_eq!(count, 0, "shipment insert should have been rolled back");
}

#[test]
fn sqlite_inventory_adjust_rolls_back_when_transaction_insert_fails() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let repo = db.inventory();

    let item = repo
        .create_item(CreateInventoryItem {
            sku: "SKU-1".to_string(),
            name: "Test item".to_string(),
            initial_quantity: Some(Decimal::new(10, 0)),
            ..Default::default()
        })
        .expect("create inventory item");

    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute("DROP TABLE inventory_transactions", [])
            .expect("drop inventory_transactions");
    }

    repo.adjust(AdjustInventory {
        sku: "SKU-1".to_string(),
        quantity: Decimal::ONE,
        reason: "Test adjust".to_string(),
        location_id: None,
        reference_type: None,
        reference_id: None,
    })
    .expect_err("expected adjust to fail");

    let conn = db.conn().expect("get sqlite connection");
    let qty: String = conn
        .query_row(
            "SELECT quantity_on_hand FROM inventory_balances WHERE item_id = ?",
            [item.id],
            |row| row.get(0),
        )
        .expect("query inventory balance");
    let qty_dec: Decimal = qty.parse().expect("parse quantity_on_hand");
    assert_eq!(qty_dec, Decimal::new(10, 0), "balance should be unchanged");
}

#[test]
fn sqlite_inventory_release_reservation_rolls_back_when_balance_update_fails() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let repo = db.inventory();

    repo.create_item(CreateInventoryItem {
        sku: "SKU-2".to_string(),
        name: "Test item".to_string(),
        initial_quantity: Some(Decimal::new(5, 0)),
        ..Default::default()
    })
    .expect("create inventory item");

    let reservation = repo
        .reserve(ReserveInventory {
            sku: "SKU-2".to_string(),
            quantity: Decimal::ONE,
            reference_type: "test".to_string(),
            reference_id: "ref-1".to_string(),
            location_id: None,
            expires_in_seconds: None,
        })
        .expect("reserve inventory");

    {
        let conn = db.conn().expect("get sqlite connection");
        conn.execute("DROP TABLE inventory_balances", [])
            .expect("drop inventory_balances");
    }

    repo.release_reservation(reservation.id)
        .expect_err("expected release_reservation to fail");

    let conn = db.conn().expect("get sqlite connection");
    let status: String = conn
        .query_row(
            "SELECT status FROM inventory_reservations WHERE id = ?",
            [reservation.id.to_string()],
            |row| row.get(0),
        )
        .expect("query reservation status");
    assert_eq!(status, "pending", "reservation status should be unchanged");
}
