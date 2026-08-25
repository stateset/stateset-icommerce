#![cfg(feature = "sqlite")]
//! Bin-level inventory (warehouse bins) against the live SQLite engine.
//!
//! Invariant under test: for every `(warehouse, sku)`,
//! `Σ inventory_bin_levels.quantity_on_hand == inventory_balances.quantity_on_hand`
//! (warehouse-level, `location_id = warehouse_id`). Adjustments mirror their
//! delta to the warehouse level in the same transaction; moves are neutral.

use rust_decimal_macros::dec;
use stateset_core::{
    AdjustBinLevel, BinRepository, BinType, CommerceError, CreateInventoryItem, CreateWarehouse,
    CreateWarehouseBin, InventoryRepository, MoveBetweenBins, UpdateWarehouseBin, WarehouseAddress,
    WarehouseBinFilter, WarehouseRepository, WarehouseType,
};
use stateset_db::SqliteDatabase;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("in-memory sqlite")
}

fn warehouse(db: &SqliteDatabase, code: &str) -> i32 {
    db.warehouse()
        .create_warehouse(CreateWarehouse {
            code: code.into(),
            name: format!("WH {code}"),
            warehouse_type: WarehouseType::Distribution,
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
            zone: Some("A".into()),
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

fn adjust(
    db: &SqliteDatabase,
    bin_id: i32,
    sku: &str,
    qty: rust_decimal::Decimal,
) -> stateset_core::Result<stateset_core::BinLevel> {
    db.bins().adjust_bin_level(AdjustBinLevel {
        bin_id,
        sku: sku.into(),
        quantity: qty,
        reason: "test".into(),
        reference_type: None,
        reference_id: None,
        performed_by: None,
    })
}

#[test]
fn bin_crud_and_code_unique_per_warehouse() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let wh2 = warehouse(&db, "WH2");
    let a = bin(&db, wh, "A-01-01", BinType::Pick);

    let dup = db.bins().create_bin(CreateWarehouseBin {
        warehouse_id: wh,
        code: "A-01-01".into(),
        bin_type: BinType::Bulk,
        ..Default::default()
    });
    assert!(matches!(dup, Err(CommerceError::Conflict(_))), "got {dup:?}");
    // Same code in another warehouse is fine.
    bin(&db, wh2, "A-01-01", BinType::Pick);

    let fetched = db.bins().get_bin_by_code(wh, "A-01-01").unwrap().unwrap();
    assert_eq!(fetched.id, a);
    assert_eq!(fetched.bin_type, BinType::Pick);
    assert!(fetched.is_active);

    let updated = db
        .bins()
        .update_bin(
            a,
            UpdateWarehouseBin {
                bin_type: Some(BinType::Bulk),
                capacity: Some(Some(dec!(50))),
                is_active: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(updated.bin_type, BinType::Bulk);
    assert_eq!(updated.capacity, Some(dec!(50)));
    assert!(!updated.is_active);

    let listed = db
        .bins()
        .list_bins(WarehouseBinFilter { warehouse_id: Some(wh), ..Default::default() })
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(db.bins().count_bins(WarehouseBinFilter::default()).unwrap(), 2);

    db.bins().delete_bin(a).unwrap();
    assert!(db.bins().get_bin(a).unwrap().is_none());
    assert!(matches!(db.bins().get_bin(9999), Ok(None)));
    assert!(matches!(
        db.bins().update_bin(9999, UpdateWarehouseBin::default()),
        Err(CommerceError::NotFound)
    ));
}

#[test]
fn adjust_requires_inventory_item_and_rejects_zero() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let a = bin(&db, wh, "A", BinType::Pick);
    assert!(matches!(adjust(&db, a, "SKU-X", dec!(0)), Err(CommerceError::ValidationError(_))));
    let err = adjust(&db, a, "SKU-X", dec!(5)).expect_err("no inventory item");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    // Nothing written at either level.
    assert!(db.bins().get_bin_levels(a).unwrap().is_empty());
    let rec = db.bins().reconcile(wh, "SKU-X").unwrap();
    assert_eq!(rec.bin_on_hand, dec!(0));
    assert_eq!(rec.warehouse_on_hand, dec!(0));
}

#[test]
fn sum_of_bins_equals_warehouse_on_hand_invariant() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let a = bin(&db, wh, "A", BinType::Pick);
    let b = bin(&db, wh, "B", BinType::Bulk);
    item(&db, "SKU-1");

    adjust(&db, a, "SKU-1", dec!(10.5)).unwrap();
    adjust(&db, b, "SKU-1", dec!(4)).unwrap();
    adjust(&db, a, "SKU-1", dec!(-0.5)).unwrap();
    db.bins()
        .move_between_bins(MoveBetweenBins {
            from_bin_id: a,
            to_bin_id: b,
            sku: "SKU-1".into(),
            quantity: dec!(3),
            reason: Some("replen".into()),
            performed_by: Some("tester".into()),
        })
        .unwrap();

    let rec = db.bins().reconcile(wh, "SKU-1").unwrap();
    assert_eq!(rec.bin_on_hand, dec!(14));
    assert_eq!(rec.warehouse_on_hand, dec!(14));
    assert!(rec.is_balanced(), "{rec:?}");

    // Warehouse-level stock is visible through the inventory repository at
    // location_id = warehouse_id.
    let stock = db.inventory().get_stock("SKU-1").unwrap().unwrap();
    assert_eq!(stock.total_on_hand, dec!(14));
    assert!(stock.locations.iter().any(|l| l.location_id == wh && l.on_hand == dec!(14)));

    let levels = db.bins().get_bin_levels_for_sku(wh, "SKU-1").unwrap();
    assert_eq!(levels.len(), 2);
    let a_level = levels.iter().find(|l| l.bin_id == a).unwrap();
    let b_level = levels.iter().find(|l| l.bin_id == b).unwrap();
    assert_eq!(a_level.quantity_on_hand, dec!(7));
    assert_eq!(b_level.quantity_on_hand, dec!(7));
    assert_eq!(a_level.quantity_available, dec!(7));
}

#[test]
fn move_rejects_insufficient_source_and_is_atomic() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let a = bin(&db, wh, "A", BinType::Pick);
    let b = bin(&db, wh, "B", BinType::Bulk);
    item(&db, "SKU-1");
    adjust(&db, a, "SKU-1", dec!(5)).unwrap();

    let err = db
        .bins()
        .move_between_bins(MoveBetweenBins {
            from_bin_id: a,
            to_bin_id: b,
            sku: "SKU-1".into(),
            quantity: dec!(6),
            reason: None,
            performed_by: None,
        })
        .expect_err("over-move must fail");
    assert!(matches!(err, CommerceError::InsufficientStock { .. }), "got {err:?}");

    // Empty source: also insufficient.
    let err = db
        .bins()
        .move_between_bins(MoveBetweenBins {
            from_bin_id: b,
            to_bin_id: a,
            sku: "SKU-1".into(),
            quantity: dec!(1),
            reason: None,
            performed_by: None,
        })
        .expect_err("empty source must fail");
    assert!(matches!(err, CommerceError::InsufficientStock { .. }), "got {err:?}");

    // Bad destination: source untouched.
    let err = db
        .bins()
        .move_between_bins(MoveBetweenBins {
            from_bin_id: a,
            to_bin_id: 9999,
            sku: "SKU-1".into(),
            quantity: dec!(2),
            reason: None,
            performed_by: None,
        })
        .expect_err("missing destination");
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
    let a_level = &db.bins().get_bin_levels(a).unwrap()[0];
    assert_eq!(a_level.quantity_on_hand, dec!(5));
    assert!(db.bins().get_bin_levels(b).unwrap().is_empty());
    assert!(db.bins().reconcile(wh, "SKU-1").unwrap().is_balanced());
}

#[test]
fn move_rejects_cross_warehouse_and_inactive_destination() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let wh2 = warehouse(&db, "WH2");
    let a = bin(&db, wh, "A", BinType::Pick);
    let other = bin(&db, wh2, "A", BinType::Pick);
    let inactive = bin(&db, wh, "Z", BinType::Bulk);
    db.bins()
        .update_bin(inactive, UpdateWarehouseBin { is_active: Some(false), ..Default::default() })
        .unwrap();
    item(&db, "SKU-1");
    adjust(&db, a, "SKU-1", dec!(5)).unwrap();

    for to in [other, inactive] {
        let err = db
            .bins()
            .move_between_bins(MoveBetweenBins {
                from_bin_id: a,
                to_bin_id: to,
                sku: "SKU-1".into(),
                quantity: dec!(1),
                reason: None,
                performed_by: None,
            })
            .expect_err("must be rejected");
        assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    }
}

#[test]
fn capacity_is_enforced_on_adjust_and_move() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let small = db
        .bins()
        .create_bin(CreateWarehouseBin {
            warehouse_id: wh,
            code: "SMALL".into(),
            bin_type: BinType::Pick,
            capacity: Some(dec!(3)),
            ..Default::default()
        })
        .unwrap()
        .id;
    let big = bin(&db, wh, "BIG", BinType::Bulk);
    item(&db, "SKU-1");
    let err = adjust(&db, small, "SKU-1", dec!(4)).expect_err("over capacity");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    // Rolled back at warehouse level too.
    assert_eq!(db.bins().reconcile(wh, "SKU-1").unwrap().warehouse_on_hand, dec!(0));

    adjust(&db, big, "SKU-1", dec!(10)).unwrap();
    let err = db
        .bins()
        .move_between_bins(MoveBetweenBins {
            from_bin_id: big,
            to_bin_id: small,
            sku: "SKU-1".into(),
            quantity: dec!(4),
            reason: None,
            performed_by: None,
        })
        .expect_err("over capacity move");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(db.bins().get_bin_levels(big).unwrap()[0].quantity_on_hand, dec!(10));
    assert!(matches!(
        db.bins().create_bin(CreateWarehouseBin {
            warehouse_id: wh,
            code: "NEG".into(),
            capacity: Some(dec!(-1)),
            ..Default::default()
        }),
        Err(CommerceError::ValidationError(_))
    ));
}

#[test]
fn negative_adjust_cannot_take_bin_below_zero() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let a = bin(&db, wh, "A", BinType::Pick);
    item(&db, "SKU-1");
    adjust(&db, a, "SKU-1", dec!(2)).unwrap();
    let err = adjust(&db, a, "SKU-1", dec!(-3)).expect_err("below zero");
    assert!(matches!(err, CommerceError::InsufficientStock { .. }), "got {err:?}");
    assert!(db.bins().reconcile(wh, "SKU-1").unwrap().is_balanced());
}

#[test]
fn delete_bin_with_stock_is_rejected() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let a = bin(&db, wh, "A", BinType::Pick);
    item(&db, "SKU-1");
    adjust(&db, a, "SKU-1", dec!(1)).unwrap();
    let err = db.bins().delete_bin(a).expect_err("holds stock");
    assert!(matches!(err, CommerceError::NotPermitted(_)), "got {err:?}");
    adjust(&db, a, "SKU-1", dec!(-1)).unwrap();
    db.bins().delete_bin(a).unwrap();
    assert!(matches!(db.bins().delete_bin(a), Err(CommerceError::NotFound)));
}

#[test]
fn reconcile_reports_variance_when_warehouse_level_changes_outside_bins() {
    let db = db();
    let wh = warehouse(&db, "WH1");
    let a = bin(&db, wh, "A", BinType::Pick);
    item(&db, "SKU-1");
    adjust(&db, a, "SKU-1", dec!(5)).unwrap();
    // A warehouse-level adjustment (receipt not yet put away) creates drift
    // that reconcile surfaces rather than hides.
    db.inventory()
        .adjust(stateset_core::AdjustInventory {
            sku: "SKU-1".into(),
            location_id: Some(wh),
            quantity: dec!(2),
            reason: "receipt".into(),
            reference_type: None,
            reference_id: None,
        })
        .unwrap();
    let rec = db.bins().reconcile(wh, "SKU-1").unwrap();
    assert_eq!(rec.bin_on_hand, dec!(5));
    assert_eq!(rec.warehouse_on_hand, dec!(7));
    assert_eq!(rec.variance, dec!(2));
    assert!(!rec.is_balanced());
}
