#![cfg(feature = "sqlite")]
#[path = "support/erp_supplier_settlements.rs"]
mod settlements;
#[test]
fn settlements() {
    settlements::settlements(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn invalid_targets() {
    settlements::invalid_targets(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn disputes() {
    settlements::disputes(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn currencies() {
    settlements::currencies(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn refund_and_dashboard() {
    settlements::refund_and_dashboard(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn bill_matching() {
    settlements::bill_matching(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn maker_checker() {
    settlements::maker_checker(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
