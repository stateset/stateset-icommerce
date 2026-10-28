#![cfg(feature = "sqlite")]
#[path = "support/erp_manufacturing.rs"]
mod manufacturing;
#[test]
fn manufacturing_posts_stock_atomically() {
    let db = stateset_db::SqliteDatabase::in_memory().unwrap();
    manufacturing::lifecycle(&db);
}
