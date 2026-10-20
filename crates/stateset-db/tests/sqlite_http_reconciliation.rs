#![cfg(feature = "sqlite")]

use chrono::{Duration, Utc};
use stateset_core::CommerceError;
use stateset_db::reconciliation::{
    HttpReceiptResolution, pending_http_receipts, reconcile_http_receipt,
};
use stateset_db::{
    DatabaseConfig, HttpIdempotencyRecord, HttpIdempotencyRepository, SqliteDatabase,
};

fn pending(db: &SqliteDatabase, key: &str) -> HttpIdempotencyRecord {
    let record = HttpIdempotencyRecord {
        tenant: "tenant:reconciliation".into(),
        idempotency_key: key.into(),
        request_fingerprint: "a".repeat(64),
        response_status: 0,
        content_type: None,
        response_body: vec![],
        created_at: Utc::now() - Duration::days(30),
    };
    assert!(db.http_idempotency().put(&record).unwrap());
    record
}

fn resolution(record: &HttpIdempotencyRecord) -> HttpReceiptResolution {
    HttpReceiptResolution {
        tenant: record.tenant.clone(),
        idempotency_key: record.idempotency_key.clone(),
        request_fingerprint: record.request_fingerprint.clone(),
        reserved_at: record.created_at,
        response_status: 201,
        content_type: Some("application/json".into()),
        response_body: br#"{"id":"verified-payment","amount":"0.10"}"#.to_vec(),
        actor: "operator:recovery".into(),
        evidence: "incident:verified-provider-receipt-123".into(),
    }
}

#[test]
fn pending_inspection_is_read_only_tenant_scoped_bounded_and_nanosecond_exact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.db");
    let db = SqliteDatabase::new(&DatabaseConfig::sqlite(path.to_str().unwrap())).unwrap();
    let mut first = pending(&db, "first");
    let mut later = first.clone();
    later.idempotency_key = "later".into();
    later.created_at += Duration::nanoseconds(1);
    assert!(db.http_idempotency().put(&later).unwrap());
    let mut other = first.clone();
    other.tenant = "tenant:other".into();
    assert!(db.http_idempotency().put(&other).unwrap());
    first.idempotency_key = "completed".into();
    first.response_status = 201;
    assert!(db.http_idempotency().put(&first).unwrap());
    let read_only =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let rows = pending_http_receipts(&read_only, &first.tenant, first.created_at, 100).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].idempotency_key, "first");
    assert_eq!(rows[0].reserved_at, first.created_at);
    let repo = db.http_idempotency();
    let rows = repo.list_pending(&first.tenant, Utc::now(), 1).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].idempotency_key, "first");
    assert_eq!(repo.list_pending("tenant:other", Utc::now(), 100).unwrap().len(), 1);
    for limit in [0, 1001, u32::MAX] {
        assert!(repo.list_pending(&first.tenant, Utc::now(), limit).is_err());
    }
    assert_eq!(
        read_only
            .query_row("SELECT COUNT(*) FROM http_idempotency_keys", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        4,
        "inspection must not expire old completed receipts"
    );
}

#[test]
fn verified_resolution_commits_response_and_audit_together_and_never_overwrites() {
    let db = SqliteDatabase::in_memory().unwrap();
    let record = pending(&db, "verified");
    let resolution = resolution(&record);
    let mut conn = db.conn().unwrap();
    let completed = reconcile_http_receipt(&mut conn, &resolution).unwrap();
    assert_eq!(
        db.http_idempotency()
            .get(&record.tenant, &record.idempotency_key, Utc::now() - Duration::hours(24))
            .unwrap()
            .unwrap(),
        completed
    );
    let (actor, details): (String, String) = conn
        .query_row(
            "SELECT actor, details FROM audit_log WHERE action='http_receipt.reconciled'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(actor, resolution.actor);
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(details["evidence"], resolution.evidence);
    assert_eq!(details["tenant"], record.tenant);
    assert_eq!(details["response_sha256"].as_str().unwrap().len(), 64);
    assert!(
        !details.to_string().contains("verified-payment"),
        "response payload is not duplicated into the audit log"
    );
    assert!(matches!(
        reconcile_http_receipt(&mut conn, &resolution),
        Err(CommerceError::Conflict(_))
    ));
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM audit_log WHERE action='http_receipt.reconciled'",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    assert!(
        db.http_idempotency().list_pending(&record.tenant, Utc::now(), 100).unwrap().is_empty()
    );
    println!("reconciliation-proof response=verified audit=atomic duplicate=refused tenant=scoped");
}

#[test]
fn stale_identity_or_timestamp_evidence_cannot_complete_a_reservation() {
    let db = SqliteDatabase::in_memory().unwrap();
    let record = pending(&db, "stale");
    for kind in ["tenant", "key", "fingerprint", "time"] {
        let mut proposal = resolution(&record);
        match kind {
            "tenant" => proposal.tenant = "tenant:wrong".into(),
            "key" => proposal.idempotency_key = "wrong".into(),
            "fingerprint" => proposal.request_fingerprint = "b".repeat(64),
            _ => proposal.reserved_at += Duration::nanoseconds(1),
        }
        assert!(
            matches!(
                reconcile_http_receipt(&mut db.conn().unwrap(), &proposal),
                Err(CommerceError::Conflict(_))
            ),
            "{kind}"
        );
    }
    assert_eq!(
        db.http_idempotency()
            .get(&record.tenant, &record.idempotency_key, Utc::now())
            .unwrap()
            .unwrap(),
        record
    );
}

#[test]
fn required_audit_failure_rolls_back_completion_and_is_observable() {
    let db = SqliteDatabase::in_memory().unwrap();
    let record = pending(&db, "audit-fails");
    let mut conn = db.conn().unwrap();
    for (trigger, message) in [
        (
            "CREATE TRIGGER reject_reconciliation_audit BEFORE INSERT ON audit_log
             WHEN NEW.action = 'http_receipt.reconciled'
             BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END;",
            "audit unavailable",
        ),
        (
            "CREATE TRIGGER reject_reconciliation_audit BEFORE INSERT ON audit_log
             WHEN NEW.action = 'http_receipt.reconciled'
             BEGIN SELECT RAISE(IGNORE); END;",
            "0 rows",
        ),
    ] {
        conn.execute_batch(trigger).unwrap();
        let failures = stateset_db::audit::audit_write_failures();
        let error = reconcile_http_receipt(&mut conn, &resolution(&record)).unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert!(stateset_db::audit::audit_write_failures() > failures);
        assert_eq!(
            db.http_idempotency()
                .get(&record.tenant, &record.idempotency_key, Utc::now())
                .unwrap()
                .unwrap()
                .response_status,
            0
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM audit_log", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
        conn.execute_batch("DROP TRIGGER reject_reconciliation_audit").unwrap();
    }
    reconcile_http_receipt(&mut conn, &resolution(&record)).unwrap();
}

#[test]
fn competing_operator_resolutions_commit_one_response_and_one_audit() {
    let db = std::sync::Arc::new(SqliteDatabase::in_memory().unwrap());
    let record = pending(&db, "competing");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let mut workers = Vec::new();
    for status in [200, 201] {
        let db = db.clone();
        let barrier = barrier.clone();
        let mut proposal = resolution(&record);
        proposal.response_status = status;
        workers.push(std::thread::spawn(move || {
            let mut conn = db.conn().unwrap();
            barrier.wait();
            reconcile_http_receipt(&mut conn, &proposal)
        }));
    }
    let outcomes: Vec<_> = workers.into_iter().map(|worker| worker.join().unwrap()).collect();
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes.iter().filter(|result| matches!(result, Err(CommerceError::Conflict(_)))).count(),
        1
    );
    let count: i64 = db
        .conn()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM audit_log WHERE action='http_receipt.reconciled'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn legacy_reservation_can_be_reconciled_at_its_stored_millisecond() {
    let db = SqliteDatabase::in_memory().unwrap();
    let mut record = pending(&db, "legacy");
    record.created_at =
        chrono::DateTime::from_timestamp_millis(record.created_at.timestamp_millis()).unwrap();
    let mut conn = db.conn().unwrap();
    conn.execute(
        "UPDATE http_idempotency_keys SET created_at_sub_ms_ns=NULL WHERE idempotency_key='legacy'",
        [],
    )
    .unwrap();
    reconcile_http_receipt(&mut conn, &resolution(&record)).unwrap();
}

#[test]
fn resolution_rejects_invalid_responses_and_missing_evidence_before_writing() {
    let db = SqliteDatabase::in_memory().unwrap();
    let record = pending(&db, "invalid");
    for kind in ["status", "body", "actor", "evidence", "header", "fingerprint"] {
        let mut proposal = resolution(&record);
        match kind {
            "status" => proposal.response_status = 0,
            "body" => proposal.response_body = vec![0; 1024 * 1024 + 1],
            "actor" => proposal.actor = " ".into(),
            "evidence" => proposal.evidence.clear(),
            "header" => {
                proposal.content_type = Some("application/json\r\nx-secret: injected".into())
            }
            _ => proposal.request_fingerprint = "invalid".into(),
        }
        assert!(reconcile_http_receipt(&mut db.conn().unwrap(), &proposal).is_err(), "{kind}");
    }
    assert_eq!(
        db.http_idempotency()
            .get(&record.tenant, &record.idempotency_key, Utc::now())
            .unwrap()
            .unwrap(),
        record
    );
}
