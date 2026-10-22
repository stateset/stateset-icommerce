//! `count(f) == list(f).len()` for every filter field of the inventory, gift
//! card, fixed asset and integration mapping repositories (SQLite).

#![cfg(feature = "sqlite")]
#[path = "support/list_count_parity.rs"]
mod support;

use stateset_db::SqliteDatabase;

#[test]
fn sqlite_gift_card_count_matches_list() {
    let db = SqliteDatabase::in_memory().unwrap();
    support::gift_card_parity(&db.gift_cards(), None);
}

#[test]
fn sqlite_inventory_count_matches_list() {
    let db = SqliteDatabase::in_memory().unwrap();
    let pool = db.pool().clone();
    support::inventory_parity(&db.inventory(), None, |sku| {
        pool.get()
            .unwrap()
            .execute("UPDATE inventory_items SET is_active = 0 WHERE sku = ?", [sku])
            .unwrap();
    });
}

#[test]
fn sqlite_fixed_asset_count_matches_list() {
    let db = SqliteDatabase::in_memory().unwrap();
    support::fixed_asset_parity(&db.fixed_assets(), None);
}

#[test]
fn sqlite_integration_mapping_count_matches_list() {
    let db = SqliteDatabase::in_memory().unwrap();
    support::integration_mapping_parity(&db.integration_mappings(), None);
}

#[test]
fn sqlite_lot_count_matches_list() {
    let db = SqliteDatabase::in_memory().unwrap();
    // SQLite `list` returns at most 100 lots without a limit.
    support::lot_parity(&db.lots(), Some(100));
}

#[test]
fn sqlite_warranty_count_matches_list() {
    let db = SqliteDatabase::in_memory().unwrap();
    support::warranty_parity(&db.warranties(), &db.customers(), None);
}

#[test]
fn sqlite_backorder_filters_apply() {
    let db = SqliteDatabase::in_memory().unwrap();
    support::backorder_filters(&db.backorder());
}

#[test]
fn sqlite_location_capacity_filter_is_refused() {
    let db = SqliteDatabase::in_memory().unwrap();
    support::location_capacity_refused(&db.warehouse());
}
