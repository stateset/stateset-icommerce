#![cfg(feature = "sqlite")]
#[path = "support/erp_cost_filters.rs"]
mod filters;
#[test]
fn cost_filters() {
    filters::filters(&stateset_db::SqliteDatabase::in_memory().unwrap());
}
