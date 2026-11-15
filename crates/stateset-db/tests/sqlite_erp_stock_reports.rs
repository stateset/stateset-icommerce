#![cfg(feature = "sqlite")]
#[path = "support/erp_stock_reports.rs"]
mod reports;
#[test]
fn reports_use_stored_stock() {
    reports::stored_reports(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
