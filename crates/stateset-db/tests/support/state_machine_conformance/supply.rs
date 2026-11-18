//! Supply-chain machines: vendor returns, transfer orders, inbound
//! shipments, production batches, EDI documents, backorders.

use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

use super::harness::{Conformance, Report, SetStatus};

/// A unique SKU with a product, a warehouse and a location holding `stock`.
pub(crate) struct Stocked {
    pub(crate) sku: String,
    pub(crate) product: Product,
    pub(crate) warehouse: Warehouse,
    pub(crate) location: Location,
}

/// A product with one default variant whose SKU is `sku`.
pub(crate) fn product(db: &dyn Database, sku: &str) -> Product {
    db.products()
        .create(CreateProduct {
            name: sku.to_owned(),
            variants: Some(vec![CreateProductVariant {
                sku: sku.to_owned(),
                price: dec!(10),
                is_default: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap()
}

pub(crate) fn stocked(db: &dyn Database, stock: rust_decimal::Decimal) -> Stocked {
    let sku = format!("SM-{}", Uuid::new_v4().simple());
    let product = product(db, &sku);
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: sku.clone(),
            name: sku.clone(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location.id,
            sku: sku.clone(),
            lot_id: None,
            quantity: stock,
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    Stocked { sku, product, warehouse, location }
}

pub(crate) fn vendor_return(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let stock = stocked(db, dec!(1000));
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: stock.sku.clone(), ..Default::default() })
        .unwrap();
    let repo = db.vendor_returns();
    let create = || {
        repo.create(CreateVendorReturn {
            supplier_id: supplier.id,
            warehouse_id: stock.warehouse.id,
            purchase_order_id: None,
            currency: Some(CurrencyCode::USD),
            items: vec![CreateVendorReturnItem {
                product_id: stock.product.id,
                sku: stock.sku.clone(),
                source_location_id: stock.location.id,
                lot_id: None,
                quantity: dec!(1),
                unit_cost: dec!(10),
                reason: VendorReturnReason::Defective,
            }],
            notes: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get(id).unwrap().unwrap().status.to_string();
    Conformance::new("vendor_return", create, status)
        .op("pending", "submit", |id| repo.submit(id).map(drop))
        .op("processed", "process", |id| repo.process(id, true).map(drop))
        .op("cancelled", "cancel", |id| repo.cancel(id).map(drop))
        .check()
}

pub(crate) fn transfer_order(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let source = stocked(db, dec!(100000));
    let destination = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: source.warehouse.id, ..Default::default() })
        .unwrap();
    let repo = db.transfer_orders();
    let create = || {
        repo.create(CreateTransferOrder {
            source_warehouse_id: WarehouseId::new(),
            destination_warehouse_id: WarehouseId::new(),
            source_location_id: source.location.id,
            destination_location_id: destination.id,
            items: vec![CreateTransferOrderItem {
                product_id: source.product.id,
                sku: source.sku.clone(),
                lot_id: None,
                quantity: dec!(4),
            }],
            expected_at: None,
            notes: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get(id).unwrap().unwrap().status.to_string();
    let line = |id| repo.get(id).unwrap().unwrap().items.remove(0);
    Conformance::new("transfer_order", create, status)
        .op("in_transit", "ship", |id| repo.ship(id).map(drop))
        .op("partially_received", "receive_line (one unit)", |id| {
            repo.receive_line(id, line(id).id, dec!(1)).map(drop)
        })
        .op("received", "receive_line (the rest)", |id| {
            let item = line(id);
            let rest = item.quantity - item.quantity_received;
            repo.receive_line(id, item.id, if rest > dec!(0) { rest } else { dec!(1) }).map(drop)
        })
        .op("received", "complete_receiving", |id| repo.complete_receiving(id).map(drop))
        .op("cancelled", "cancel", |id| repo.cancel(id).map(drop))
        .seeded("pending", |id| set("transfer_orders", &id.to_string(), "pending"))
        .check()
}

pub(crate) fn edi_document(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let repo = db.edi_documents();
    let create = || {
        repo.create(CreateEdiDocument {
            document_type: "850".into(),
            direction: EdiDirection::Inbound,
            partner: Some("conformance".into()),
            reference: None,
            payload: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get(id).unwrap().unwrap().status.to_string();
    let mut check = Conformance::new("edi_document", create, status);
    for (target, label, value) in [
        ("pending", "set_status pending", EdiStatus::Pending),
        ("sent", "set_status sent", EdiStatus::Sent),
        ("acknowledged", "set_status acknowledged", EdiStatus::Acknowledged),
        ("processed", "set_status processed", EdiStatus::Processed),
        ("error", "set_status error", EdiStatus::Error),
    ] {
        let repo = &repo;
        check = check.op(target, label, move |id| repo.set_status(id, value, None).map(drop));
    }
    check.check()
}

/// A warehouse (integer id) with one receivable, pickable bin.
fn wms_warehouse(db: &dyn Database) -> (i32, i32) {
    let tag = Uuid::new_v4().simple().to_string();
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("SM-{}", &tag[..12]),
            name: "conformance".into(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation {
            warehouse_id: warehouse.id,
            code: Some(format!("L-{}", &tag[..8])),
            location_type: LocationType::Bulk,
            is_pickable: Some(true),
            is_receivable: Some(true),
            ..Default::default()
        })
        .unwrap();
    (warehouse.id, location.id)
}

fn receive_line(
    db: &dyn Database,
    receipt: Uuid,
    location: i32,
    quantity: rust_decimal::Decimal,
) -> Result<()> {
    let line = db.receiving().get_receipt_items(receipt)?.remove(0).id;
    db.receiving()
        .receive_items(ReceiveItems {
            receipt_id: receipt,
            items: vec![ReceiveItemLine {
                receipt_item_id: line,
                quantity_received: quantity,
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
            receiving_location_id: Some(location),
            received_by: Some("dock".into()),
        })
        .map(drop)
}

fn new_receipt(
    db: &dyn Database,
    warehouse: i32,
    sku: &str,
    quantity: rust_decimal::Decimal,
) -> Uuid {
    db.receiving()
        .create_receipt(CreateReceipt {
            warehouse_id: warehouse,
            items: vec![CreateReceiptItem {
                sku: sku.into(),
                expected_quantity: quantity,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap()
        .id
}

pub(crate) fn receipt(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let (warehouse, location) = wms_warehouse(db);
    let sku = format!("SM-RCPT-{}", Uuid::new_v4().simple());
    let repo = db.receiving();
    let create = || new_receipt(db, warehouse, &sku, dec!(10));
    let status = |id| repo.get_receipt(id).unwrap().unwrap().status.to_string();
    let seed = |state: &'static str| move |id: Uuid| set("receipts", &id.to_string(), state);
    Conformance::new("receipt", create, status)
        .op("in_progress", "start_receiving", |id| repo.start_receiving(id).map(drop))
        .refuses_same_status()
        .op("in_progress", "receive_items (one unit)", |id| receive_line(db, id, location, dec!(1)))
        .op("received", "complete_receiving", |id| repo.complete_receiving(id).map(drop))
        .op("cancelled", "cancel_receipt", |id| repo.cancel_receipt(id).map(drop))
        .seeded("inspecting", seed("inspecting"))
        .seeded("putting_away", seed("putting_away"))
        .seeded("completed", seed("completed"))
        .check()
}

pub(crate) fn put_away(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let (warehouse, location) = wms_warehouse(db);
    let sku = format!("SM-PUT-{}", Uuid::new_v4().simple());
    let receipt = new_receipt(db, warehouse, &sku, dec!(100000));
    db.receiving().start_receiving(receipt).unwrap();
    receive_line(db, receipt, location, dec!(100000)).unwrap();
    db.receiving().complete_receiving(receipt).unwrap();
    let line = db.receiving().get_receipt_items(receipt).unwrap().remove(0).id;
    let repo = db.receiving();
    let create = || {
        repo.create_put_away(CreatePutAway {
            receipt_id: receipt,
            receipt_item_id: line,
            sku: sku.clone(),
            from_location_id: None,
            to_location_id: location,
            quantity: dec!(1),
            lot_id: None,
            assigned_to: None,
            notes: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get_put_away(id).unwrap().unwrap().status.to_string();
    Conformance::new("put_away", create, status)
        .op("assigned", "assign_put_away", |id| repo.assign_put_away(id, "carl").map(drop))
        .op("in_progress", "start_put_away", |id| repo.start_put_away(id).map(drop))
        .op("completed", "complete_put_away", |id| {
            repo.complete_put_away(CompletePutAway {
                put_away_id: id,
                actual_location_id: None,
                completed_by: Some("carl".into()),
                notes: None,
            })
            .map(drop)
        })
        .op("cancelled", "cancel_put_away", |id| repo.cancel_put_away(id).map(drop))
        .check()
}

pub(crate) fn wave(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let (warehouse, _) = wms_warehouse(db);
    let repo = db.fulfillment();
    let create = || {
        repo.create_wave(CreateWave {
            warehouse_id: warehouse,
            order_ids: Vec::new(),
            priority: None,
            notes: None,
            created_by: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get_wave(id).unwrap().unwrap().status.to_string();
    Conformance::new("wave", create, status)
        .op("released", "release_wave", |id| repo.release_wave(id).map(drop))
        .op("completed", "complete_wave (no open picks)", |id| repo.complete_wave(id).map(drop))
        .op("cancelled", "cancel_wave", |id| repo.cancel_wave(id).map(drop))
        .seeded("in_progress", |id| set("waves", &id.to_string(), "in_progress"))
        .check()
}

pub(crate) fn pick_task(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let (warehouse, location) = wms_warehouse(db);
    let sku = format!("SM-PICK-{}", Uuid::new_v4().simple());
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location,
            sku: sku.clone(),
            lot_id: None,
            quantity: dec!(100000),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    let repo = db.fulfillment();
    let create = || {
        repo.create_pick(CreatePickTask {
            wave_id: None,
            order_id: OrderId::new(),
            order_item_id: OrderItemId::new(),
            warehouse_id: warehouse,
            sku: sku.clone(),
            product_name: None,
            source_location_id: location,
            quantity_requested: dec!(2),
            lot_id: None,
            serial_number: None,
            priority: None,
            notes: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get_pick(id).unwrap().unwrap().status.to_string();
    let complete = |id, picked, short: Option<rust_decimal::Decimal>| {
        repo.complete_pick(CompletePick {
            pick_id: id,
            quantity_picked: picked,
            quantity_short: short,
            short_reason: short.map(|_| "bin empty".into()),
            lot_id: None,
            serial_number: None,
            completed_by: Some("picker".into()),
        })
        .map(drop)
    };
    Conformance::new("pick_task", create, status)
        .op("assigned", "assign_pick", |id| repo.assign_pick(id, "picker").map(drop))
        .op("in_progress", "start_pick", |id| repo.start_pick(id).map(drop))
        // complete_pick returns a finished pick unchanged, whatever outcome
        // it is asked for.
        .silent_op("completed", "complete_pick", |id| complete(id, dec!(2), None))
        .silent_op("short", "complete_pick (with a shortage)", |id| {
            complete(id, dec!(1), Some(dec!(1)))
        })
        .op("short", "report_short", |id| repo.report_short(id, dec!(1), "bin empty").map(drop))
        .refuses_same_status()
        .op("cancelled", "cancel_pick", |id| repo.cancel_pick(id).map(drop))
        .check()
}

pub(crate) fn bom(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let repo = db.bom();
    // A product per BOM: activating one obsoletes the product's other active
    // revisions.
    let create = || {
        let product = product(db, &format!("SM-BOM-{}", Uuid::new_v4().simple()));
        repo.create(CreateBom {
            product_id: product.id,
            name: "conformance".into(),
            components: Some(vec![CreateBomComponent {
                component_sku: Some(format!("SM-PART-{}", Uuid::new_v4().simple())),
                name: "part".into(),
                quantity: dec!(1),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get(id).unwrap().unwrap().status.to_string();
    let update = |id, value| {
        repo.update(id, UpdateBom { status: Some(value), ..Default::default() }).map(drop)
    };
    Conformance::new("bom", create, status)
        .op("active", "activate", |id| repo.activate(id).map(drop))
        .op("draft", "update status=draft", |id| update(id, BomStatus::Draft))
        .op("active", "update status=active", |id| update(id, BomStatus::Active))
        .op("obsolete", "update status=obsolete", |id| update(id, BomStatus::Obsolete))
        .op("obsolete", "delete", |id| repo.delete(id))
        .check()
}
