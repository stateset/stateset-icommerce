#![cfg(feature = "sqlite")]
#[path = "support/erp_costing.rs"]
mod costing;

#[test]
fn valuation_and_recursive_rollups() {
    let db = stateset_db::SqliteDatabase::in_memory().unwrap();
    costing::valuation(&db, |item_id| {
        let conn = db.conn().unwrap();
        conn.execute(
            "INSERT INTO inventory_locations (id, name, code) VALUES (2, 'Other', 'OTHER')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO inventory_balances (item_id, location_id, quantity_on_hand, quantity_available, reorder_point) VALUES (?, 2, '0.2', '0.2', '2')", [item_id]).unwrap();
    });
    costing::rollups(&db);
}
