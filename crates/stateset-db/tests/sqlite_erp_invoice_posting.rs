#![cfg(feature = "sqlite")]
#[path = "support/erp_invoice_posting.rs"]
mod posting;
#[test]
fn invoice_posting_accounts() {
    posting::invoice_posting(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
