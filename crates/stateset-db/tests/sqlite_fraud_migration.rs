#![cfg(feature = "sqlite")]

//! Regression: the SQLite `fraud_assessments` / `fraud_rules` tables were never
//! provisioned by the embedded migration set, so every fraud operation crashed at
//! runtime on the default SQLite backend with a "no such table" error (Postgres has
//! `048_fraud.sql`). The fraud test module masked this by creating the
//! tables inline under `#[cfg(test)]`. This test builds the database through the real
//! migration path and exercises the fraud repository directly.

use stateset_core::{CreateFraudAssessment, FraudRepository};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_fraud_tables_are_provisioned_by_migrations() {
    // The real migration path — NOT the inline `#[cfg(test)]` DDL in sqlite/fraud.rs.
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");

    let order_id = uuid::Uuid::new_v4().into();
    let assessment = db
        .fraud()
        .create_assessment(CreateFraudAssessment { order_id, signals: vec![] })
        .expect("create_assessment must succeed (fraud tables must be migrated)");
    assert_eq!(assessment.order_id, order_id);

    let fetched = db
        .fraud()
        .get_assessment(order_id)
        .expect("get_assessment must succeed")
        .expect("the created assessment must be found");
    assert_eq!(fetched.order_id, order_id);
}
