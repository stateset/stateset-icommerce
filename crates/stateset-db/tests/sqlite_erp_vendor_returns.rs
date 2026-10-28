#![cfg(feature = "sqlite")]
#[path = "support/erp_vendor_returns.rs"]
mod vendor_returns;

#[test]
fn vendor_returns_post_stock_and_one_credit() {
    let db = stateset_db::SqliteDatabase::in_memory().unwrap();
    vendor_returns::lifecycle(&db);
}
