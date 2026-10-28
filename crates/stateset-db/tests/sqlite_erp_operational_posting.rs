#![cfg(feature = "sqlite")]
#[path = "support/erp_operational_posting.rs"]
mod posting;
#[test]
fn cash_refunds() {
    posting::refunds(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn stock() {
    posting::stock(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn manufacturing() {
    posting::manufacturing(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn restock() {
    posting::restock(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn methods() {
    posting::methods(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn transit() {
    posting::transit(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn foreign_stock() {
    posting::foreign_stock(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn specific_lots() {
    posting::specific_lots(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn unconfigured_cost_currency() {
    posting::unconfigured_cost_currency(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn sales_reporting() {
    posting::sales_reporting(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
