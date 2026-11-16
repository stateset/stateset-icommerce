#![cfg(feature = "postgres")]
#[path = "support/erp_production_batches.rs"]
mod batches;
#[test]
fn batch_status_follows_members() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    batches::linked_statuses(&db);
}
