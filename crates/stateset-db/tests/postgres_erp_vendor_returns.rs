#![cfg(feature = "postgres")]
#[path = "support/erp_vendor_returns.rs"]
mod vendor_returns;

#[test]
fn vendor_returns_post_stock_and_one_credit() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    vendor_returns::lifecycle(&db);
}
