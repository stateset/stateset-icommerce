#![cfg(feature = "postgres")]
#[path = "support/erp_purchase_controls.rs"]
mod controls;
#[test]
fn approved_lines_are_immutable() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    controls::approved_lines_are_immutable(&db);
}
#[test]
fn receipt_serials_and_line_statuses() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    controls::receipt_serials_and_line_statuses(&db);
}
#[test]
fn po_backorders_consume_received_stock() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    controls::po_backorders_consume_received_stock(&db);
}
