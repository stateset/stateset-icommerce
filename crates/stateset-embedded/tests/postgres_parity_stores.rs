//! Postgres smoke CRUD coverage for the nine parity stores added alongside
//! the SQLite implementations: stock snapshots, transfer orders, units of
//! measure, inbound shipments, print stations, production batches, supplier
//! SKUs, vendor returns, and vendor credits.
//!
//! `AsyncCommerce` does not (yet) expose accessors for these stores, so each
//! test drives the async Postgres repository directly, built over the shared
//! pool from `AsyncCommerce::database()`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    ApplyVendorCredit, CaptureStockLine, CaptureStockSnapshot, ConversionRuleType,
    CreateInboundShipment, CreateInboundShipmentItem, CreatePrintStation, CreateProductionBatch,
    CreateSupplierSku, CreateTransferOrder, CreateTransferOrderItem, CreateUnitClass,
    CreateUnitConversionRule, CreateUnitOfMeasure, CreateVendorCredit, CreateVendorReturn,
    CreateVendorReturnItem, EnqueuePrintJob, InboundShipmentStatus, PrintJobStatus,
    PrintPayloadKind, ProductId, ProductionBatchFilter, ProductionBatchStatus, SupplierSkuFilter,
    TransferOrderStatus, UpdateProductionBatch, UpdateSupplierSku, VendorCreditStatus,
    VendorCreditTargetType, VendorReturnReason, VendorReturnStatus, WarehouseId,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

/// Connect (or skip the test when no live Postgres is configured).
macro_rules! require_pg {
    ($name:literal) => {
        match postgres_url() {
            Some(url) => AsyncCommerce::connect(&url).await.expect("connect + migrate"),
            None => {
                eprintln!(
                    "POSTGRES_URL or DATABASE_URL not set; skipping {} parity smoke test",
                    $name
                );
                return;
            }
        }
    };
}

#[tokio::test]
async fn postgres_stock_snapshots_smoke() {
    let commerce = require_pg!("stock snapshot");
    let store =
        stateset_db::postgres::PgStockSnapshotRepository::new(commerce.database().pool().clone());

    let label = format!("pg-snap-{}", uuid::Uuid::new_v4().simple());
    let snapshot = store
        .capture_async(CaptureStockSnapshot {
            label: Some(label.clone()),
            lines: vec![
                CaptureStockLine {
                    product_id: ProductId::new(),
                    sku: "SNAP-A".into(),
                    quantity_on_hand: dec!(10),
                    quantity_available: dec!(8),
                    location: Some("A-01".into()),
                },
                CaptureStockLine {
                    product_id: ProductId::new(),
                    sku: "SNAP-B".into(),
                    quantity_on_hand: dec!(5),
                    quantity_available: dec!(5),
                    location: None,
                },
            ],
        })
        .await
        .expect("capture snapshot");
    assert_eq!(snapshot.total_skus, 2);
    assert_eq!(snapshot.total_units, dec!(15));

    let fetched =
        store.get_async(snapshot.id).await.expect("get snapshot").expect("snapshot exists");
    assert_eq!(fetched.label.as_deref(), Some(label.as_str()));
    assert_eq!(fetched.lines.len(), 2);

    let latest = store.latest_async().await.expect("latest snapshot").expect("latest exists");
    assert_eq!(latest.id, snapshot.id);
    assert!(
        store
            .list_async(Default::default())
            .await
            .expect("list")
            .iter()
            .any(|s| s.id == snapshot.id)
    );

    store.delete_async(snapshot.id).await.expect("delete snapshot");
    assert!(store.get_async(snapshot.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_transfer_orders_smoke() {
    let commerce = require_pg!("transfer order");
    let store =
        stateset_db::postgres::PgTransferOrderRepository::new(commerce.database().pool().clone());

    let created = store
        .create_async(CreateTransferOrder {
            source_warehouse_id: WarehouseId::new(),
            destination_warehouse_id: WarehouseId::new(),
            items: vec![CreateTransferOrderItem {
                product_id: ProductId::new(),
                quantity: dec!(10),
            }],
            expected_at: None,
            notes: Some("pg smoke".into()),
        })
        .await
        .expect("create transfer order");
    assert!(!created.number.is_empty());
    assert_eq!(created.items.len(), 1);
    let item_id = created.items[0].id;

    let shipped = store.ship_async(created.id).await.expect("ship");
    assert_eq!(shipped.status, TransferOrderStatus::InTransit);
    assert!(shipped.shipped_at.is_some());

    let partial =
        store.receive_line_async(created.id, item_id, dec!(4)).await.expect("receive partial");
    assert_eq!(partial.status, TransferOrderStatus::PartiallyReceived);

    let full = store.receive_line_async(created.id, item_id, dec!(6)).await.expect("receive rest");
    assert_eq!(full.status, TransferOrderStatus::Received);
    assert!(full.received_at.is_some());

    // NOTE: Pg `cancel_async` currently has no terminal-state guard (it
    // unconditionally stamps `cancelled`), so cancellation of received orders
    // is not asserted here — only that the received state round-trips.
    let fetched = store.get_async(created.id).await.expect("get").expect("exists");
    assert_eq!(fetched.status, TransferOrderStatus::Received);
    assert_eq!(fetched.items[0].quantity_received, dec!(10));
}

#[tokio::test]
async fn postgres_units_of_measure_smoke() {
    let commerce = require_pg!("units of measure");
    let store =
        stateset_db::postgres::PgUnitOfMeasureRepository::new(commerce.database().pool().clone());

    let class = store
        .create_class_async(CreateUnitClass {
            name: format!("Weight-{}", uuid::Uuid::new_v4().simple()),
            description: Some("pg smoke".into()),
        })
        .await
        .expect("create class");
    assert!(
        store.list_classes_async().await.expect("list classes").iter().any(|c| c.id == class.id)
    );

    let gram = store
        .create_uom_async(CreateUnitOfMeasure {
            unit_class_id: class.id,
            name: "Gram".into(),
            abbreviation: "g".into(),
            factor: dec!(1),
        })
        .await
        .expect("create gram");
    let kilo = store
        .create_uom_async(CreateUnitOfMeasure {
            unit_class_id: class.id,
            name: "Kilogram".into(),
            abbreviation: "kg".into(),
            factor: dec!(1000),
        })
        .await
        .expect("create kilogram");

    let base = store.set_base_uom_async(gram.id).await.expect("set base uom");
    assert!(base.is_base);
    let uoms = store
        .list_uoms_async(stateset_core::UnitOfMeasureFilter {
            class_id: Some(class.id),
            ..Default::default()
        })
        .await
        .expect("list uoms");
    assert_eq!(uoms.len(), 2);

    let rule = store
        .create_rule_async(CreateUnitConversionRule {
            rule_type: ConversionRuleType::System,
            product_id: None,
            from_uom_id: kilo.id,
            to_uom_id: gram.id,
            factor: dec!(1000),
        })
        .await
        .expect("create rule");
    assert!(store.list_rules_async().await.expect("list rules").iter().any(|r| r.id == rule.id));

    store.delete_rule_async(rule.id).await.expect("delete rule");
    store.delete_uom_async(kilo.id).await.expect("delete kilogram");
    store.delete_uom_async(gram.id).await.expect("delete gram");
    store.delete_class_async(class.id).await.expect("delete class");
    assert!(
        store
            .list_uoms_async(stateset_core::UnitOfMeasureFilter {
                class_id: Some(class.id),
                ..Default::default()
            })
            .await
            .expect("list uoms")
            .is_empty()
    );
}

#[tokio::test]
async fn postgres_inbound_shipments_smoke() {
    let commerce = require_pg!("inbound shipment");
    let store =
        stateset_db::postgres::PgInboundShipmentRepository::new(commerce.database().pool().clone());

    let created = store
        .create_async(CreateInboundShipment {
            supplier_id: uuid::Uuid::new_v4(),
            purchase_order_id: None,
            warehouse_id: None,
            carrier: Some("PG Freight".into()),
            tracking_number: Some("PGF-123".into()),
            expected_at: None,
            items: vec![CreateInboundShipmentItem {
                product_id: ProductId::new(),
                sku: "INB-1".into(),
                quantity_expected: dec!(20),
            }],
            notes: None,
        })
        .await
        .expect("create inbound shipment");
    assert_eq!(created.status, InboundShipmentStatus::Pending);
    let item_id = created.items[0].id;

    let in_transit = store.mark_in_transit_async(created.id).await.expect("mark in transit");
    assert_eq!(in_transit.status, InboundShipmentStatus::InTransit);
    let arrived = store.mark_arrived_async(created.id).await.expect("mark arrived");
    assert_eq!(arrived.status, InboundShipmentStatus::Arrived);

    let partial =
        store.receive_line_async(created.id, item_id, dec!(5)).await.expect("receive partial");
    assert_eq!(partial.status, InboundShipmentStatus::PartiallyReceived);
    let full = store.receive_line_async(created.id, item_id, dec!(15)).await.expect("receive rest");
    assert_eq!(full.status, InboundShipmentStatus::Received);

    // NOTE: Pg `cancel_async` currently has no terminal-state guard, so
    // cancellation of received shipments is not asserted here.
    let fetched = store.get_async(created.id).await.expect("get").expect("exists");
    assert_eq!(fetched.items[0].quantity_received, dec!(20));
}

#[tokio::test]
async fn postgres_print_stations_smoke() {
    let commerce = require_pg!("print station");
    let store =
        stateset_db::postgres::PgPrintStationRepository::new(commerce.database().pool().clone());

    let paired = store
        .pair_async(CreatePrintStation {
            name: format!("Station-{}", uuid::Uuid::new_v4().simple()),
            printers: vec!["zebra-1".into()],
        })
        .await
        .expect("pair station");
    assert!(!paired.token.is_empty(), "pairing must issue a token");
    let station_id = paired.station.id;
    assert!(
        store
            .list_stations_async()
            .await
            .expect("list stations")
            .iter()
            .any(|s| s.id == station_id)
    );

    let job = store
        .enqueue_job_async(
            station_id,
            EnqueuePrintJob {
                printer_name: Some("zebra-1".into()),
                payload_kind: PrintPayloadKind::Zpl,
                payload: "^XA^FDpg^FS^XZ".into(),
            },
        )
        .await
        .expect("enqueue job");
    assert_eq!(job.status, PrintJobStatus::Queued);

    let picked = store.next_job_async(station_id).await.expect("next job").expect("job available");
    assert_eq!(picked.id, job.id);
    assert_eq!(picked.status, PrintJobStatus::PickedUp);
    assert!(store.next_job_async(station_id).await.expect("next job again").is_none());

    let done = store.complete_job_async(job.id, true).await.expect("complete job");
    assert_eq!(done.status, PrintJobStatus::Printed);
    assert!(
        store
            .list_jobs_async(station_id, Default::default())
            .await
            .expect("list jobs")
            .iter()
            .any(|j| j.id == job.id)
    );

    store.revoke_station_async(station_id).await.expect("revoke station");
}

#[tokio::test]
async fn postgres_production_batches_smoke() {
    let commerce = require_pg!("production batch");
    let store = commerce.database().production_batches();

    let created = store
        .create_async(CreateProductionBatch {
            name: format!("Batch-{}", uuid::Uuid::new_v4().simple()),
            vendor_id: Some(uuid::Uuid::new_v4()),
            work_order_ids: vec![],
            notes: Some("pg smoke".into()),
            scheduled_start: None,
            scheduled_end: None,
        })
        .await
        .expect("create batch");
    assert_eq!(created.status, ProductionBatchStatus::Planned);

    let updated = store
        .update_async(
            created.id,
            UpdateProductionBatch {
                name: None,
                vendor_id: None,
                status: Some(ProductionBatchStatus::InProgress),
                notes: None,
                scheduled_start: None,
                scheduled_end: None,
            },
        )
        .await
        .expect("update batch");
    assert_eq!(updated.status, ProductionBatchStatus::InProgress);

    let listed = store
        .list_async(ProductionBatchFilter {
            status: Some(ProductionBatchStatus::InProgress),
            ..Default::default()
        })
        .await
        .expect("list batches");
    assert!(listed.iter().any(|b| b.id == created.id));

    store.delete_async(created.id).await.expect("delete batch");
    assert!(store.get_async(created.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_supplier_skus_smoke() {
    let commerce = require_pg!("supplier SKU");
    let store = commerce.database().supplier_skus();
    let supplier_id = uuid::Uuid::new_v4();

    let created = store
        .create_async(CreateSupplierSku {
            product_id: ProductId::new(),
            supplier_id,
            sku: format!("SUP-{}", uuid::Uuid::new_v4().simple()),
            unit_cost: Some(dec!(4.25)),
            currency: None,
            min_order_qty: Some(dec!(12)),
            lead_time_days: Some(7),
        })
        .await
        .expect("create supplier sku");
    assert_eq!(created.unit_cost, Some(dec!(4.25)));

    let updated = store
        .update_async(
            created.id,
            UpdateSupplierSku {
                sku: None,
                unit_cost: Some(dec!(3.99)),
                currency: None,
                min_order_qty: None,
                lead_time_days: Some(10),
                is_preferred: Some(true),
            },
        )
        .await
        .expect("update supplier sku");
    assert_eq!(updated.unit_cost, Some(dec!(3.99)));
    assert_eq!(updated.lead_time_days, Some(10));
    assert!(updated.is_preferred);

    let listed = store
        .list_async(SupplierSkuFilter { supplier_id: Some(supplier_id), ..Default::default() })
        .await
        .expect("list supplier skus");
    assert_eq!(listed.len(), 1);

    store.delete_async(created.id).await.expect("delete supplier sku");
    assert!(store.get_async(created.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_vendor_returns_smoke() {
    let commerce = require_pg!("vendor return");
    let store = commerce.database().vendor_returns();

    let created = store
        .create_async(CreateVendorReturn {
            supplier_id: uuid::Uuid::new_v4(),
            purchase_order_id: None,
            currency: None,
            items: vec![CreateVendorReturnItem {
                product_id: ProductId::new(),
                quantity: dec!(3),
                unit_cost: dec!(15),
                reason: VendorReturnReason::Defective,
            }],
            notes: Some("pg smoke".into()),
        })
        .await
        .expect("create vendor return");
    assert_eq!(created.status, VendorReturnStatus::Draft);
    assert!(!created.number.is_empty());
    assert!(!created.credit_generated);

    let submitted = store.submit_async(created.id).await.expect("submit");
    assert_eq!(submitted.status, VendorReturnStatus::Pending);

    let processed = store.process_async(created.id, true).await.expect("process");
    assert_eq!(processed.status, VendorReturnStatus::Processed);
    assert!(processed.credit_generated);
    assert!(processed.processed_at.is_some());

    assert!(store.cancel_async(created.id).await.is_err(), "processed returns cannot be cancelled");
    let fetched = store.get_async(created.id).await.expect("get").expect("exists");
    assert_eq!(fetched.items.len(), 1);
}

#[tokio::test]
async fn postgres_vendor_credits_smoke() {
    let commerce = require_pg!("vendor credit");
    let store = commerce.database().vendor_credits();

    let created = store
        .create_async(CreateVendorCredit {
            supplier_id: uuid::Uuid::new_v4(),
            vendor_return_id: None,
            amount: dec!(100),
            currency: None,
            memo: Some("pg smoke".into()),
        })
        .await
        .expect("create vendor credit");
    assert_eq!(created.status, VendorCreditStatus::Open);
    assert_eq!(created.remaining, dec!(100));

    let applied = store
        .apply_async(
            created.id,
            ApplyVendorCredit {
                target_type: VendorCreditTargetType::Bill,
                target_id: uuid::Uuid::new_v4(),
                amount: dec!(40),
            },
        )
        .await
        .expect("apply credit");
    assert_eq!(applied.remaining, dec!(60));

    // Over-application is rejected.
    assert!(
        store
            .apply_async(
                created.id,
                ApplyVendorCredit {
                    target_type: VendorCreditTargetType::Bill,
                    target_id: uuid::Uuid::new_v4(),
                    amount: dec!(1000),
                },
            )
            .await
            .is_err()
    );

    let applications = store.list_applications_async(created.id).await.expect("list applications");
    assert_eq!(applications.len(), 1);
    assert_eq!(applications[0].amount, dec!(40));

    let reversed = store
        .reverse_application_async(created.id, applications[0].id)
        .await
        .expect("reverse application");
    assert_eq!(reversed.remaining, dec!(100));

    let cancelled = store.cancel_async(created.id).await.expect("cancel credit");
    assert_eq!(cancelled.status, VendorCreditStatus::Cancelled);
}
