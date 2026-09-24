//! Bulk expiry and lazy lookup must agree at the exact cutoff.
//! Requires `POSTGRES_URL` or `DATABASE_URL`; skipped otherwise.

#![cfg(feature = "postgres")]

use chrono::{DateTime, Utc};
use stateset_db::{HttpIdempotencyRecord, HttpIdempotencyRepository, PostgresDatabase};
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[test]
fn postgres_purge_expired_includes_exact_cutoff_and_frees_key() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let db = runtime.block_on(PostgresDatabase::connect(&url)).expect("connect + migrate");
    let cutoff = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).expect("timestamp");
    let key = Uuid::new_v4().to_string();
    let repo = db.http_idempotency();
    let record = HttpIdempotencyRecord {
        tenant: "expiry-test".into(),
        idempotency_key: key,
        request_fingerprint: "fingerprint".into(),
        response_status: 201,
        content_type: None,
        response_body: b"first".to_vec(),
        created_at: cutoff,
    };
    assert!(repo.put(&record).expect("put"));
    assert_eq!(repo.purge_expired(cutoff).expect("purge"), 1);
    let mut replacement = record;
    replacement.response_body = b"second".to_vec();
    replacement.created_at = Utc::now();
    assert!(repo.put(&replacement).expect("reuse key"));
}
