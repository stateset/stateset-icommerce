#![cfg(feature = "sqlite")]
#[path = "support/erp_reporting.rs"]
mod reporting;
#[test]
fn periods_and_earnings() {
    reporting::periods_and_earnings(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn schedule_precision() {
    reporting::schedule_precision(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn stored_reports() {
    reporting::stored_reports(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
