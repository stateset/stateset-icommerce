#![cfg(feature = "sqlite")]
#[path = "support/erp_receivables.rs"]
mod receivables;
#[test]
fn funding_guards() {
    receivables::funding_guards(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn partial_writeoffs() {
    receivables::partial_writeoffs(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn terminal_unapply() {
    receivables::terminal_unapply(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn allocation_race() {
    receivables::allocation_race(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
#[test]
fn refund_allocation_race() {
    receivables::refund_allocation_race(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn draft_edits() {
    receivables::draft_edits(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn issue_edit_race() {
    receivables::issue_edit_race(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn statements() {
    receivables::statements(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn invoice_writeoff() {
    receivables::invoice_writeoff(&stateset_db::SqliteDatabase::in_memory().unwrap());
}

#[test]
fn credit_filters() {
    receivables::credit_filters(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
