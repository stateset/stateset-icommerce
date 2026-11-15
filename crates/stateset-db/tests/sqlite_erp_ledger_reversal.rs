#![cfg(feature = "sqlite")]
#[path = "support/erp_ledger_reversal.rs"]
mod reversal;
#[test]
fn reversal_across_close() {
    reversal::reversal_across_close(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
