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
fn postgres_pending_inspection_is_tenant_scoped_bounded_and_nanosecond_exact() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let db = runtime.block_on(PostgresDatabase::connect(&url)).unwrap();
    let tenant = format!("pending-test-{}", Uuid::new_v4());
    let repo = db.http_idempotency();
    let first = HttpIdempotencyRecord {
        tenant: tenant.clone(),
        idempotency_key: "first".into(),
        request_fingerprint: "f".repeat(64),
        response_status: 0,
        content_type: None,
        response_body: vec![],
        created_at: DateTime::from_timestamp(1_700_000_000, 100).unwrap(),
    };
    let mut later = first.clone();
    later.idempotency_key = "later".into();
    later.created_at += chrono::Duration::nanoseconds(1);
    let mut other = first.clone();
    other.tenant = format!("{tenant}-other");
    let mut completed = first.clone();
    completed.idempotency_key = "completed".into();
    completed.response_status = 201;
    for record in [&first, &later, &other, &completed] {
        assert!(repo.put(record).unwrap());
    }
    let rows = repo.list_pending(&tenant, first.created_at, 100).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].idempotency_key, "first");
    assert_eq!(rows[0].reserved_at, first.created_at);
    let rows = repo.list_pending(&tenant, Utc::now(), 1).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].idempotency_key, "first");
    assert_eq!(repo.list_pending(&other.tenant, Utc::now(), 100).unwrap().len(), 1);
    for limit in [0, 1001, u32::MAX] {
        assert!(repo.list_pending(&tenant, Utc::now(), limit).is_err());
    }
    assert!(
        repo.get(&tenant, "completed", first.created_at - chrono::Duration::hours(1))
            .unwrap()
            .is_some(),
        "inspection does not expire completed rows"
    );
}

#[test]
fn postgres_unresolved_reservation_survives_expiry_and_completes_once() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let db = runtime.block_on(PostgresDatabase::connect(&url)).expect("connect + migrate");
    let repo = db.http_idempotency();
    let key = Uuid::new_v4().to_string();
    let mut record = HttpIdempotencyRecord {
        tenant: "reservation-test".into(),
        idempotency_key: key.clone(),
        request_fingerprint: "fingerprint".into(),
        response_status: 0,
        content_type: None,
        response_body: Vec::new(),
        created_at: DateTime::<Utc>::from_timestamp(1_600_000_000, 0).unwrap(),
    };
    assert!(repo.put(&record).unwrap());
    assert!(!repo.put(&record).unwrap());
    // Use a cutoff before the other tests' rows so this sweep is isolated.
    let cutoff = record.created_at + chrono::Duration::days(1);
    repo.purge_expired(cutoff).unwrap();
    assert_eq!(repo.get(&record.tenant, &key, cutoff).unwrap().unwrap(), record);
    record.response_status = 201;
    record.response_body = b"completed".to_vec();
    record.created_at = Utc::now();
    record.request_fingerprint = "mismatch".into();
    assert!(!repo.complete(&record).unwrap());
    record.request_fingerprint = "fingerprint".into();
    assert!(repo.complete(&record).unwrap());
    assert!(!repo.complete(&record).unwrap());
    assert_eq!(repo.get(&record.tenant, &key, cutoff).unwrap().unwrap(), record);
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
