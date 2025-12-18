#![cfg(feature = "sqlite")]

use rust_decimal::Decimal;
use stateset_core::{
    CreateInvoice, CreateInvoiceItem, CreatePurchaseOrder, CreatePurchaseOrderItem,
    CreateShipment, CreateShipmentItem, CreateSupplier, InvoiceRepository,
    PurchaseOrderRepository, ShipmentRepository,
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

