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

#[test]
fn postgres_same_microsecond_cutoff_keeps_later_response() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let db = runtime.block_on(PostgresDatabase::connect(&url)).expect("connect + migrate");
    let repo = db.http_idempotency();
    // Keep these rows ahead of the other test's 2023 bulk-purge cutoff.
    let cutoff = DateTime::<Utc>::from_timestamp(1_900_000_000, 100).expect("cutoff");
    let created = DateTime::<Utc>::from_timestamp(1_900_000_000, 900).expect("created");
    let key = Uuid::new_v4().to_string();
    let record = HttpIdempotencyRecord {
        tenant: "expiry-test".into(),
        idempotency_key: key.clone(),
        request_fingerprint: "fingerprint".into(),
        response_status: 201,
        content_type: None,
        response_body: b"first".to_vec(),
        created_at: created,
    };
    assert!(repo.put(&record).expect("put"));
    let loaded = repo.get("expiry-test", &key, cutoff).expect("get").expect("live");
    assert_eq!(loaded.created_at, created);
    assert!(repo.get("expiry-test", &key, created).expect("get").is_none());
}

#[test]
fn postgres_legacy_row_waits_for_unambiguous_expiry() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let db = runtime.block_on(PostgresDatabase::connect(&url)).expect("connect + migrate");
    let repo = db.http_idempotency();
    let created = DateTime::<Utc>::from_timestamp(1_900_000_001, 0).expect("created");
    let key = Uuid::new_v4().to_string();
    let record = HttpIdempotencyRecord {
        tenant: "expiry-test".into(),
        idempotency_key: key.clone(),
        request_fingerprint: "fingerprint".into(),
        response_status: 201,
        content_type: None,
        response_body: b"legacy".to_vec(),
        created_at: created,
    };
    assert!(repo.put(&record).expect("put"));
    runtime
        .block_on(
            sqlx::query(
                "UPDATE http_idempotency_keys SET created_at_epoch_seconds = NULL,
                 created_at_subsec_ns = NULL WHERE tenant = $1 AND idempotency_key = $2",
            )
            .bind("expiry-test")
            .bind(&key)
            .execute(db.pool()),
        )
        .expect("simulate pre-migration row");

    assert!(repo.get("expiry-test", &key, created).expect("get").is_some());
    assert!(
        repo.get("expiry-test", &key, created + chrono::Duration::microseconds(1))
            .expect("get")
            .is_none()
    );
}
