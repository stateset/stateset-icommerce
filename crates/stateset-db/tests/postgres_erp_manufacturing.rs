#![cfg(feature = "postgres")]
#[path = "support/erp_manufacturing.rs"]
mod manufacturing;
#[test]
fn manufacturing_posts_stock_atomically() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    manufacturing::lifecycle(&db);
}
