use chrono::Utc;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn stored_reports(db: &dyn Database) {
    let snapshots = db.stock_snapshots();
    assert_eq!(
        snapshots.capture(CaptureStockSnapshot { label: None, lines: vec![] }).unwrap().total_units,
        dec!(0)
    );
    db.inventory()
        .create_item(CreateInventoryItem {
            sku: "REPORT-A".into(),
            name: "Report A".into(),
            initial_quantity: Some(dec!(10)),
            ..Default::default()
        })
        .unwrap();
    let hold = db
        .inventory()
        .reserve(ReserveInventory {
            sku: "REPORT-A".into(),
            quantity: dec!(2),
            location_id: None,
            reference_type: "report".into(),
            reference_id: "report".into(),
            expires_in_seconds: None,
        })
        .unwrap();
    let snapshot = snapshots
        .capture(CaptureStockSnapshot { label: Some("Stored".into()), lines: vec![] })
        .unwrap();
    assert_eq!((snapshot.total_units, snapshot.total_available()), (dec!(10), dec!(8)));
    assert_eq!(snapshot.lines[0].product_id, None);
    assert!(
        snapshots
            .capture(CaptureStockSnapshot {
                label: None,
                lines: vec![CaptureStockLine {
                    product_id: ProductId::new(),
                    sku: "REPORT-A".into(),
                    quantity_on_hand: dec!(999),
                    quantity_available: dec!(999),
                    location: None
                }]
            })
            .is_err()
    );
    let today = Utc::now().date_naive();
    assert!(snapshots.inventory_aging(today).is_err(), "Missing valuation layers must be explicit");
    db.cost_accounting()
        .create_cost_layer(CreateCostLayer {
            sku: "REPORT-A".into(),
            quantity: dec!(10),
            unit_cost: dec!(2.25),
            source_type: CostLayerSource::Purchase,
            source_id: None,
            lot_id: None,
            location_id: Some(1),
        })
        .unwrap();
    let report = snapshots.inventory_aging(today).unwrap();
    assert_eq!((report.total_quantity, report.total_value), (dec!(10), dec!(22.5)));
    assert_eq!(report.buckets[0].quantity, dec!(10));
    assert_eq!(
        snapshots.inventory_aging(today.pred_opt().unwrap()).unwrap().total_quantity,
        dec!(0)
    );
    db.inventory().release_reservation(hold.id).unwrap();
    db.inventory()
        .adjust(AdjustInventory {
            sku: "REPORT-A".into(),
            quantity: dec!(-3),
            location_id: None,
            reason: "shipped".into(),
            reference_type: None,
            reference_id: None,
        })
        .unwrap();
    assert!(
        snapshots.inventory_aging(today).is_err(),
        "Unposted cost movements must not produce a plausible report"
    );
    db.cost_accounting()
        .issue_fifo(IssueCostLayers {
            sku: "REPORT-A".into(),
            quantity: dec!(3),
            reference_type: None,
            reference_id: None,
            notes: None,
        })
        .unwrap();
    let report = snapshots.inventory_aging(today).unwrap();
    assert_eq!((report.total_quantity, report.total_value), (dec!(7), dec!(15.75)));
    assert_eq!(
        snapshots.get(snapshot.id).unwrap().unwrap().total_units,
        dec!(10),
        "Captured snapshot must remain immutable"
    );
    assert_eq!(
        snapshots.capture(CaptureStockSnapshot { label: None, lines: vec![] }).unwrap().total_units,
        dec!(7)
    );
}
