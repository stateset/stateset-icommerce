#![cfg(feature = "sqlite")]
#[path = "support/erp_ledger_sources.rs"]
mod sources;
#[test]
fn source_lifecycle() {
    sources::source_lifecycle(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
