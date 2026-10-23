//! Dynamic segment membership on Postgres (see `support/dynamic_segments.rs`).
//! Requires `POSTGRES_URL` or `DATABASE_URL`.

#![cfg(feature = "postgres")]
#[path = "support/dynamic_segments.rs"]
mod support;

use stateset_db::PostgresDatabase;

#[test]
fn postgres_dynamic_segments_evaluate_rules_at_read_time() {
    let url = stateset_test_utils::postgres_url();
    let Some(url) = url else { return };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = runtime.block_on(PostgresDatabase::connect(url)).expect("connect and migrate");
    support::dynamic_segments(&db.segments(), &db.customers(), &db.orders());
}
