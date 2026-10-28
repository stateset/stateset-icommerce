#![cfg(feature = "sqlite")]
#[path = "support/erp_purchase_controls.rs"]
mod controls;
#[test]
fn approved_lines_are_immutable() {
    controls::approved_lines_are_immutable(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn receipt_serials_and_line_statuses() {
    controls::receipt_serials_and_line_statuses(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn po_backorders_consume_received_stock() {
    controls::po_backorders_consume_received_stock(
        &stateset_db::SqliteDatabase::in_memory().unwrap(),
    );
}
