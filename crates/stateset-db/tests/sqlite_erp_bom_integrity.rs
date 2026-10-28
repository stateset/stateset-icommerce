#![cfg(feature = "sqlite")]
#[path = "support/erp_bom_integrity.rs"]
mod integrity;
#[test]
fn bom_revisions_and_cycles() {
    integrity::revisions_and_cycles(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
