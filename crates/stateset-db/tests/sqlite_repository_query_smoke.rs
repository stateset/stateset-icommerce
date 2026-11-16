#![cfg(feature = "sqlite")]

#[path = "support/repository_query_smoke.rs"]
mod smoke;

#[test]
fn fresh_store_read_queries() {
    smoke::run(&stateset_db::SqliteDatabase::in_memory().expect("migrate fresh SQLite store"));
}
