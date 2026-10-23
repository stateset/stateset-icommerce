//! Dynamic segment membership on SQLite (see `support/dynamic_segments.rs`).

#![cfg(feature = "sqlite")]
#[path = "support/dynamic_segments.rs"]
mod support;

use stateset_db::SqliteDatabase;

#[test]
fn sqlite_dynamic_segments_evaluate_rules_at_read_time() {
    let db = SqliteDatabase::in_memory().unwrap();
    support::dynamic_segments(&db.segments(), &db.customers(), &db.orders());
}
