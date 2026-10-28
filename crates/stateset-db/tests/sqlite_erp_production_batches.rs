#![cfg(feature = "sqlite")]
#[path = "support/erp_production_batches.rs"]
mod batches;
#[test]
fn batch_status_follows_members() {
    batches::linked_statuses(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
