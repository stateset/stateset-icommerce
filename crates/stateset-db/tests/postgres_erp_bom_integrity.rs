#![cfg(feature = "postgres")]
#[path = "support/erp_bom_integrity.rs"]
mod integrity;
#[test]
fn bom_revisions_and_cycles() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let db = rt.block_on(stateset_db::PostgresDatabase::connect(url)).unwrap();
    integrity::revisions_and_cycles(&db);
}
