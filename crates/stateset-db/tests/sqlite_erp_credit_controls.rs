#![cfg(feature = "sqlite")]
#[path = "support/erp_credit_controls.rs"]
mod credit;

#[test]
fn eligibility() {
    credit::eligibility(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn documents() {
    credit::documents(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn currency_and_manual_overlap() {
    credit::currency_and_manual_overlap(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn race() {
    credit::race(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
