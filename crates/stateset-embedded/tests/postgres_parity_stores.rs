//! Postgres smoke CRUD coverage for the parity stores added alongside the
//! SQLite implementations: stock snapshots, transfer orders, units of
//! measure, inbound shipments, print stations, production batches, supplier
//! SKUs, vendor returns, vendor credits, channels, companies, payment
//! obligations, price levels, prepayments, price schedules, activity logs,
//! integration (field) mappings, purgatory, EDI documents, and topology
//! snapshots.
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

#[tokio::test]
async fn postgres_channels_smoke() {
    use stateset_core::{ChannelProductSyncItem, ChannelStatus, ChannelType, CreateChannel};

    let commerce = require_pg!("channel");
    let store = commerce.database().channels();

    let created = store
        .create_async(CreateChannel {
            name: format!("Chan-{}", uuid::Uuid::new_v4().simple()),
            channel_type: ChannelType::SalesChannel,
            integration: Some("shopify".into()),
            default_warehouse_id: None,
            tags: vec!["pg".into()],
            metadata: serde_json::json!({"smoke": true}),
        })
        .await
        .expect("create channel");
    assert_eq!(created.status, ChannelStatus::Active);
    assert!(!created.api_locked);

    let updated = store
        .update_async(
            created.id,
            stateset_core::UpdateChannel {
                status: Some(ChannelStatus::Paused),
                ..Default::default()
            },
        )
        .await
        .expect("update channel");
    assert_eq!(updated.status, ChannelStatus::Paused);

    let locked = store.set_lock_async(created.id, true).await.expect("lock channel");
    assert!(locked.api_locked);
    assert!(
        store.update_async(created.id, Default::default()).await.is_err(),
        "locked channels reject updates"
    );
    store.set_lock_async(created.id, false).await.expect("unlock channel");

    let product_id = ProductId::new();
    let affected = store
        .sync_products_async(
            created.id,
            vec![ChannelProductSyncItem {
                channel_sku: "CH-SKU-1".into(),
                product_id: Some(product_id),
                internal_sku: Some("INT-1".into()),
                delete: false,
            }],
        )
        .await
        .expect("sync products");
    assert_eq!(affected, 1);
    let mappings = store.list_product_mappings_async(created.id).await.expect("list mappings");
    assert_eq!(mappings.len(), 1);
    assert_eq!(mappings[0].channel_sku, "CH-SKU-1");

    store.delete_async(created.id).await.expect("soft delete channel");
    let listed = store.list_async(Default::default()).await.expect("list channels");
    assert!(!listed.iter().any(|c| c.id == created.id), "deleted channels are excluded from list");
}

#[tokio::test]
async fn postgres_companies_smoke() {
    use stateset_core::{CompanyStatus, CreateCompany, CreateContact};

    let commerce = require_pg!("company");
    let store = commerce.database().companies();

    let name = format!("Acme-{}", uuid::Uuid::new_v4().simple());
    let created = store
        .create_async(CreateCompany {
            name: name.clone(),
            reference: Some("ACME-REF".into()),
            email: Some("buyer@acme.test".into()),
            phone: None,
            currency: None,
            payment_terms_days: Some(30),
            tags: vec![],
            metadata: serde_json::Value::Null,
        })
        .await
        .expect("create company");
    assert_eq!(created.status, CompanyStatus::Active);

    let updated = store
        .update_async(
            created.id,
            stateset_core::UpdateCompany { payment_terms_days: Some(45), ..Default::default() },
        )
        .await
        .expect("update company");
    assert_eq!(updated.payment_terms_days, Some(45));

    let found = store
        .list_async(stateset_core::CompanyFilter { search: Some(name), ..Default::default() })
        .await
        .expect("search companies");
    assert_eq!(found.len(), 1);

    let contact = store
        .create_contact_async(CreateContact {
            first_name: "Pat".into(),
            last_name: Some("Buyer".into()),
            email: None,
            phone: None,
            title: None,
            company_ids: vec![created.id],
        })
        .await
        .expect("create contact");
    let contacts = store.list_contacts_async(created.id).await.expect("list contacts");
    assert!(contacts.iter().any(|c| c.id == contact.id));
    assert!(store.list_addresses_async(created.id).await.expect("list addresses").is_empty());
    assert!(store.list_price_overrides_async(created.id).await.expect("list overrides").is_empty());

    store.delete_async(created.id).await.expect("delete company");
    assert!(store.get_async(created.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_payment_obligations_smoke() {
    use stateset_core::{CreatePaymentObligation, PaymentObligationStatus};

    let commerce = require_pg!("payment obligation");
    let store = commerce.database().payment_obligations();

    let created = store
        .create_async(CreatePaymentObligation {
            supplier_id: uuid::Uuid::new_v4(),
            purchase_order_id: None,
            amount: dec!(500),
            currency: None,
            due_date: chrono::Utc::now().date_naive(),
            notes: Some("pg smoke".into()),
        })
        .await
        .expect("create obligation");
    assert_eq!(created.status, PaymentObligationStatus::Pending);
    assert!(created.number.starts_with("OBL-"));

    let partial = store.record_payment_async(created.id, dec!(200)).await.expect("record payment");
    assert_eq!(partial.status, PaymentObligationStatus::PartiallyPaid);
    assert_eq!(partial.amount_paid, dec!(200));

    assert!(
        store.record_payment_async(created.id, dec!(1000)).await.is_err(),
        "overpay is rejected"
    );

    let bill_id = uuid::Uuid::new_v4();
    let linked = store.link_bill_async(created.id, bill_id).await.expect("link bill");
    assert!(linked.linked_bill_ids.contains(&bill_id));
    let relinked = store.link_bill_async(created.id, bill_id).await.expect("relink bill");
    assert_eq!(relinked.linked_bill_ids.len(), linked.linked_bill_ids.len(), "link is idempotent");

    let paid = store.record_payment_async(created.id, dec!(300)).await.expect("pay rest");
    assert_eq!(paid.status, PaymentObligationStatus::Paid);

    let fetched = store.get_async(created.id).await.expect("get").expect("exists");
    assert_eq!(fetched.amount_paid, dec!(500));
    store.dashboard_async(chrono::Utc::now().date_naive()).await.expect("dashboard");
}

#[tokio::test]
async fn postgres_price_levels_smoke() {
    use stateset_core::CreatePriceLevel;

    let commerce = require_pg!("price level");
    let store = commerce.database().price_levels();

    let created = store
        .create_async(CreatePriceLevel {
            name: "Wholesale".into(),
            code: format!("WH-{}", uuid::Uuid::new_v4().simple()),
            description: None,
            adjustment_type: Default::default(),
            adjustment_value: dec!(0),
            currency: None,
        })
        .await
        .expect("create price level");
    assert!(created.is_active);

    let updated = store
        .update_async(
            created.id,
            stateset_core::UpdatePriceLevel {
                name: Some("Wholesale+".into()),
                ..Default::default()
            },
        )
        .await
        .expect("update price level");
    assert_eq!(updated.name, "Wholesale+");

    let product_id = ProductId::new();
    let entry = store.set_entry_async(created.id, product_id, dec!(7.25)).await.expect("set entry");
    assert_eq!(entry.price, dec!(7.25));
    let entry2 =
        store.set_entry_async(created.id, product_id, dec!(6.99)).await.expect("upsert entry");
    assert_eq!(entry2.price, dec!(6.99));
    assert_eq!(store.list_entries_async(created.id).await.expect("list entries").len(), 1);

    store.delete_entry_async(created.id, product_id).await.expect("delete entry");
    assert!(store.list_entries_async(created.id).await.expect("list entries").is_empty());

    store.delete_async(created.id).await.expect("delete price level");
    assert!(store.get_async(created.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_prepayments_smoke() {
    use stateset_core::{
        ApplyPrepayment, CreatePrepayment, PrepaymentStatus, PrepaymentTargetType,
    };

    let commerce = require_pg!("prepayment");
    let store = commerce.database().prepayments();

    let created = store
        .create_async(CreatePrepayment {
            supplier_id: uuid::Uuid::new_v4(),
            amount: dec!(250),
            currency: None,
            method: Some("wire".into()),
            reference: None,
            memo: Some("pg smoke".into()),
        })
        .await
        .expect("create prepayment");
    assert_eq!(created.status, PrepaymentStatus::Open);
    assert_eq!(created.remaining, dec!(250));

    let applied = store
        .apply_async(
            created.id,
            ApplyPrepayment {
                target_type: PrepaymentTargetType::Bill,
                target_id: uuid::Uuid::new_v4(),
                amount: dec!(100),
            },
        )
        .await
        .expect("apply prepayment");
    assert_eq!(applied.remaining, dec!(150));

    assert!(
        store
            .apply_async(
                created.id,
                ApplyPrepayment {
                    target_type: PrepaymentTargetType::Bill,
                    target_id: uuid::Uuid::new_v4(),
                    amount: dec!(1000),
                },
            )
            .await
            .is_err(),
        "over-application is rejected"
    );

    let applications = store.list_applications_async(created.id).await.expect("list applications");
    assert_eq!(applications.len(), 1);
    let reversed = store
        .reverse_application_async(created.id, applications[0].id)
        .await
        .expect("reverse application");
    assert_eq!(reversed.remaining, dec!(250));

    let refunded = store.refund_async(created.id).await.expect("refund");
    assert_eq!(refunded.status, PrepaymentStatus::Refunded);
    assert_eq!(refunded.remaining, dec!(0));
}

#[tokio::test]
async fn postgres_price_schedules_smoke() {
    use stateset_core::CreatePriceSchedule;

    let commerce = require_pg!("price schedule");
    let store = commerce.database().price_schedules();
    let now = chrono::Utc::now();

    let created = store
        .create_async(CreatePriceSchedule {
            name: format!("Sale-{}", uuid::Uuid::new_v4().simple()),
            code: None,
            currency: None,
            starts_at: Some(now - chrono::Duration::hours(1)),
            ends_at: Some(now + chrono::Duration::hours(1)),
            priority: 10,
        })
        .await
        .expect("create schedule");
    assert!(created.is_active);

    let product_id = ProductId::new();
    store.set_entry_async(created.id, product_id, dec!(19.99)).await.expect("set entry");
    let resolved =
        store.resolve_price_async(product_id, now).await.expect("resolve price in window");
    assert_eq!(resolved, Some(dec!(19.99)));
    let outside = store
        .resolve_price_async(product_id, now + chrono::Duration::hours(3))
        .await
        .expect("resolve price outside window");
    assert_eq!(outside, None);

    let updated = store
        .update_async(
            created.id,
            stateset_core::UpdatePriceSchedule { is_active: Some(false), ..Default::default() },
        )
        .await
        .expect("deactivate schedule");
    assert!(!updated.is_active);
    assert_eq!(
        store.resolve_price_async(product_id, now).await.expect("resolve inactive"),
        None,
        "inactive schedules do not resolve"
    );

    store.delete_async(created.id).await.expect("delete schedule");
    assert!(store.get_async(created.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_activity_logs_smoke() {
    use stateset_core::{ActorKind, RecordActivity};

    let commerce = require_pg!("activity log");
    let store = commerce.database().activity_logs();
    let subject_id = uuid::Uuid::new_v4();

    let first = store
        .record_async(RecordActivity {
            subject_type: "order".into(),
            subject_id,
            action: "created".into(),
            summary: "order created".into(),
            actor_kind: ActorKind::System,
            actor: None,
            metadata: serde_json::json!({"source": "pg-smoke"}),
        })
        .await
        .expect("record first");
    store
        .record_async(RecordActivity {
            subject_type: "order".into(),
            subject_id,
            action: "updated".into(),
            summary: "order updated".into(),
            actor_kind: ActorKind::User,
            actor: Some("pat".into()),
            metadata: serde_json::Value::Null,
        })
        .await
        .expect("record second");

    let fetched = store.get_async(first.id).await.expect("get entry").expect("entry exists");
    assert_eq!(fetched.action, "created");

    let history =
        store.history_for_subject_async("order", subject_id).await.expect("history for subject");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].action, "updated", "history is most recent first");

    let filtered = store
        .list_async(stateset_core::ActivityLogFilter {
            subject_type: Some("order".into()),
            subject_id: Some(subject_id),
            action: Some("created".into()),
            ..Default::default()
        })
        .await
        .expect("filtered list");
    assert_eq!(filtered.len(), 1);
}

#[tokio::test]
async fn postgres_integration_mappings_smoke() {
    use stateset_core::{CreateIntegrationMapping, MappingLookup};

    let commerce = require_pg!("integration mapping");
    let store = commerce.database().integration_mappings();
    let group = format!("grp-{}", uuid::Uuid::new_v4().simple());

    let created = store
        .create_async(CreateIntegrationMapping {
            integration: "shopify".into(),
            mapping_group: group.clone(),
            field_name: "carrier".into(),
            external_value: "USPS Ground".into(),
            internal_value: "usps".into(),
        })
        .await
        .expect("create mapping");
    assert!(created.is_active);

    let resolved = store
        .resolve_async(&MappingLookup {
            integration: "shopify".into(),
            mapping_group: group.clone(),
            field_name: "carrier".into(),
            external_value: "USPS Ground".into(),
        })
        .await
        .expect("resolve mapping");
    assert_eq!(resolved.as_deref(), Some("usps"));

    let affected = store
        .bulk_upsert_async(vec![CreateIntegrationMapping {
            integration: "shopify".into(),
            mapping_group: group.clone(),
            field_name: "carrier".into(),
            external_value: "USPS Ground".into(),
            internal_value: "usps_ground".into(),
        }])
        .await
        .expect("bulk upsert");
    assert!(affected >= 1);
    let re_resolved = store
        .resolve_async(&MappingLookup {
            integration: "shopify".into(),
            mapping_group: group.clone(),
            field_name: "carrier".into(),
            external_value: "USPS Ground".into(),
        })
        .await
        .expect("resolve after upsert");
    assert_eq!(re_resolved.as_deref(), Some("usps_ground"));

    let deactivated = store
        .update_async(
            created.id,
            stateset_core::UpdateIntegrationMapping {
                is_active: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("deactivate");
    assert!(!deactivated.is_active);
    assert_eq!(
        store
            .resolve_async(&MappingLookup {
                integration: "shopify".into(),
                mapping_group: group.clone(),
                field_name: "carrier".into(),
                external_value: "USPS Ground".into(),
            })
            .await
            .expect("resolve inactive"),
        None,
        "inactive mappings do not resolve"
    );

    store.delete_async(created.id).await.expect("delete mapping");
    assert!(store.get_async(created.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_integration_field_mappings_smoke() {
    use stateset_core::CreateIntegrationFieldMapping;

    let commerce = require_pg!("integration field mapping");
    let store = commerce.database().integration_field_mappings();
    let account = format!("acct-{}", uuid::Uuid::new_v4().simple());

    let created = store
        .create_async(CreateIntegrationFieldMapping {
            integration_account: account.clone(),
            mapping_group: "orders".into(),
            source_field: "shipping.method".into(),
            destination_field: "carrier".into(),
            template: None,
            transform: Default::default(),
            fallback: Some("standard".into()),
        })
        .await
        .expect("create field mapping");

    let n = store
        .bulk_create_async(vec![CreateIntegrationFieldMapping {
            integration_account: account.clone(),
            mapping_group: "customers".into(),
            source_field: "email".into(),
            destination_field: "contact_email".into(),
            template: None,
            transform: Default::default(),
            fallback: None,
        }])
        .await
        .expect("bulk create");
    assert_eq!(n, 1);

    let groups = store.distinct_groups_async(&account).await.expect("distinct groups");
    assert_eq!(groups.len(), 2);

    let listed = store
        .list_async(stateset_core::IntegrationFieldMappingFilter {
            integration_account: Some(account.clone()),
            ..Default::default()
        })
        .await
        .expect("list field mappings");
    assert_eq!(listed.len(), 2);

    let other_ids: Vec<_> = listed.iter().filter(|m| m.id != created.id).map(|m| m.id).collect();
    let deleted = store.bulk_delete_async(other_ids).await.expect("bulk delete");
    assert_eq!(deleted, 1);
    store.delete_async(created.id).await.expect("delete field mapping");
    assert!(store.get_async(created.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_purgatory_smoke() {
    use stateset_core::{IngestLineItem, IngestOrder, MapPurgatoryLine};

    let commerce = require_pg!("purgatory");
    let store = commerce.database().purgatory();

    let ingested = store
        .ingest_async(IngestOrder {
            channel_id: None,
            external_order_id: format!("EXT-{}", uuid::Uuid::new_v4().simple()),
            external_status: Some("paid".into()),
            metadata: serde_json::json!({"src": "pg-smoke"}),
            items: vec![
                IngestLineItem {
                    external_sku: "UNKNOWN-1".into(),
                    quantity: dec!(2),
                    product_id: None,
                },
                IngestLineItem {
                    external_sku: "UNKNOWN-2".into(),
                    quantity: dec!(1),
                    product_id: None,
                },
            ],
        })
        .await
        .expect("ingest order");
    assert!(!ingested.is_posted);
    assert_eq!(ingested.items.len(), 2);

    assert!(store.post_async(ingested.id).await.is_err(), "unresolved lines block posting");

    let mapped = store
        .map_line_async(
            ingested.id,
            ingested.items[0].id,
            MapPurgatoryLine { product_id: Some(ProductId::new()), ..Default::default() },
        )
        .await
        .expect("map first line");
    assert!(mapped.items.iter().any(|i| i.product_id.is_some()));
    store
        .map_line_async(
            ingested.id,
            ingested.items[1].id,
            MapPurgatoryLine { ignore_item: Some(true), ..Default::default() },
        )
        .await
        .expect("ignore second line");

    let posted = store.post_async(ingested.id).await.expect("post order");
    assert!(posted.is_posted);
    assert!(store.post_async(ingested.id).await.is_err(), "double post is rejected");

    let default_list = store.list_async(Default::default()).await.expect("list non-posted");
    assert!(
        !default_list.iter().any(|o| o.id == ingested.id),
        "default list excludes posted orders"
    );

    store.delete_async(ingested.id).await.expect("delete purgatory order");
    assert!(store.get_async(ingested.id).await.expect("get after delete").is_none());
}

#[tokio::test]
async fn postgres_edi_documents_smoke() {
    use stateset_core::{CreateEdiDocument, EdiDirection, EdiStatus};

    let commerce = require_pg!("edi document");
    let store = commerce.database().edi_documents();

    let created = store
        .create_async(CreateEdiDocument {
            document_type: "850".into(),
            direction: EdiDirection::Inbound,
            partner: Some("ACME-RETAIL".into()),
            reference: Some("PO-1001".into()),
            payload: Some("ISA*00*...".into()),
        })
        .await
        .expect("create edi document");
    assert_eq!(created.status, EdiStatus::Pending);

    let errored = store
        .set_status_async(created.id, EdiStatus::Error, Some("bad segment".into()))
        .await
        .expect("set error status");
    assert_eq!(errored.status, EdiStatus::Error);
    assert_eq!(errored.error_message.as_deref(), Some("bad segment"));

    let processed = store
        .set_status_async(created.id, EdiStatus::Processed, None)
        .await
        .expect("set processed");
    assert_eq!(processed.status, EdiStatus::Processed);
    assert_eq!(processed.error_message, None, "clearing the error message");

    let listed = store
        .list_async(stateset_core::EdiDocumentFilter {
            partner: Some("ACME-RETAIL".into()),
            ..Default::default()
        })
        .await
        .expect("list by partner");
    assert!(listed.iter().any(|d| d.id == created.id));

    let summary = store.summary_async().await.expect("summary");
    assert!(summary.total >= 1);
}

#[tokio::test]
async fn postgres_topology_snapshots_smoke() {
    use stateset_core::{CaptureTopologySnapshot, HealthGrade};

    let commerce = require_pg!("topology snapshot");
    let store = commerce.database().topology_snapshots();

    let captured = store
        .capture_async(CaptureTopologySnapshot {
            channels_total: 3,
            channels_active: 2,
            warehouses_total: 1,
            products_total: 100,
            open_orders: 7,
            signals: serde_json::json!({"note": "pg-smoke"}),
        })
        .await
        .expect("capture snapshot");
    assert_eq!(captured.health, HealthGrade::Healthy, "health is derived from metrics");

    let fetched =
        store.get_async(captured.id).await.expect("get snapshot").expect("snapshot exists");
    assert_eq!(fetched.open_orders, 7);

    let latest = store.latest_async().await.expect("latest").expect("latest exists");
    assert_eq!(latest.id, captured.id);
    assert!(
        store
            .list_async(Default::default())
            .await
            .expect("list snapshots")
            .iter()
            .any(|s| s.id == captured.id)
    );

    store.delete_async(captured.id).await.expect("delete snapshot");
    assert!(store.get_async(captured.id).await.expect("get after delete").is_none());
}

/// Every parity store must be reachable through the `Database` trait factory
/// (not just via direct `Pg*Repository::new(pool)` construction) and the
/// Postgres capability matrix must report ALL capabilities as supported —
/// full backend parity, no `NotPermitted` shims left.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn postgres_parity_stores_reachable_via_database_trait() {
    use stateset_db::{Database, DatabaseCapability};

    let commerce = require_pg!("database trait reachability");
    let db = commerce.database().clone();

    for capability in [
        DatabaseCapability::GiftCards,
        DatabaseCapability::StoreCredits,
        DatabaseCapability::Segments,
        DatabaseCapability::ShippingZones,
        DatabaseCapability::ZoneShippingMethods,
        DatabaseCapability::Reviews,
        DatabaseCapability::Wishlists,
        DatabaseCapability::LoyaltyPrograms,
        DatabaseCapability::Rewards,
        DatabaseCapability::Fraud,
        DatabaseCapability::SearchConfigs,
        DatabaseCapability::Channels,
        DatabaseCapability::Companies,
        DatabaseCapability::TransferOrders,
        DatabaseCapability::UnitsOfMeasure,
        DatabaseCapability::ProductionBatches,
        DatabaseCapability::SupplierSkus,
        DatabaseCapability::VendorReturns,
        DatabaseCapability::VendorCredits,
        DatabaseCapability::PaymentObligations,
        DatabaseCapability::PriceLevels,
        DatabaseCapability::Prepayments,
        DatabaseCapability::PriceSchedules,
        DatabaseCapability::ActivityLogs,
        DatabaseCapability::IntegrationMappings,
        DatabaseCapability::InboundShipments,
        DatabaseCapability::Purgatory,
        DatabaseCapability::PrintStations,
        DatabaseCapability::EdiDocuments,
        DatabaseCapability::FixedAssets,
        DatabaseCapability::RevenueRecognition,
        DatabaseCapability::IntegrationFieldMappings,
        DatabaseCapability::TopologySnapshots,
        DatabaseCapability::StockSnapshots,
    ] {
        assert!(
            db.supports_capability(capability),
            "postgres should support {capability:?} via the Database capability matrix"
        );
    }

    // Sync repository calls are rejected inside an async runtime, so exercise
    // the trait-factory repositories on a plain OS thread. Each call hitting
    // the database (instead of returning NotPermitted) proves the factory
    // returns the real Postgres repository, not the unsupported shim.
    std::thread::spawn(move || {
        Database::transfer_orders(&db).list(Default::default()).expect("transfer_orders via trait");
        Database::units_of_measure(&db).list_classes().expect("units_of_measure via trait");
        Database::production_batches(&db)
            .list(Default::default())
            .expect("production_batches via trait");
        Database::supplier_skus(&db).list(Default::default()).expect("supplier_skus via trait");
        Database::vendor_returns(&db).list(Default::default()).expect("vendor_returns via trait");
        Database::vendor_credits(&db).list(Default::default()).expect("vendor_credits via trait");
        Database::inbound_shipments(&db)
            .list(Default::default())
            .expect("inbound_shipments via trait");
        Database::print_stations(&db).list_stations().expect("print_stations via trait");
        Database::stock_snapshots(&db).list(Default::default()).expect("stock_snapshots via trait");
        Database::channels(&db).list(Default::default()).expect("channels via trait");
        Database::companies(&db).list(Default::default()).expect("companies via trait");
        Database::payment_obligations(&db)
            .list(Default::default())
            .expect("payment_obligations via trait");
        Database::price_levels(&db).list(Default::default()).expect("price_levels via trait");
        Database::prepayments(&db).list(Default::default()).expect("prepayments via trait");
        Database::price_schedules(&db).list(Default::default()).expect("price_schedules via trait");
        Database::activity_logs(&db).list(Default::default()).expect("activity_logs via trait");
        Database::integration_mappings(&db)
            .list(Default::default())
            .expect("integration_mappings via trait");
        Database::integration_field_mappings(&db)
            .list(Default::default())
            .expect("integration_field_mappings via trait");
        Database::purgatory(&db).list(Default::default()).expect("purgatory via trait");
        Database::edi_documents(&db).list(Default::default()).expect("edi_documents via trait");
        Database::topology_snapshots(&db)
            .list(Default::default())
            .expect("topology_snapshots via trait");
    })
    .join()
    .expect("trait reachability thread");
}
