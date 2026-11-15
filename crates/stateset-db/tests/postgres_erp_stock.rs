#![cfg(feature = "postgres")]
#[path = "support/erp_stock_custody.rs"]
mod custody;
#[path = "support/erp_lot_stock.rs"]
mod lot_stock;
#[path = "support/erp_stock.rs"]
mod stock;
#[path = "support/erp_transfer.rs"]
mod transfers;

#[test]
fn cycle_counts_use_stored_balances() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL stock test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    stock::cycle_counts(&db);
    stock::put_away_identity(&db);
    stock::purchase_receiving(&db);
    stock::quality_guards(&db);
    transfers::lifecycle(&db);
    custody::lifecycle(&db);
    lot_stock::lifecycle(&db);
    lot_stock::mixed_bins_and_order_warehouses(&db);
}
