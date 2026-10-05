#![cfg(feature = "sqlite")]
use stateset_core::{CreatePayment, PaymentRepository};
use stateset_db::SqliteDatabase;

#[test]
fn expired_and_reused_worker_claims_cannot_settle_new_attempts() {
    let db = SqliteDatabase::in_memory().unwrap();
    db.payments()
        .create(CreatePayment { amount: rust_decimal::Decimal::ONE, ..Default::default() })
        .unwrap();
    let repo = db.kernel_outbox();
    let a = repo.claim_pending("worker", 1, 300).unwrap().remove(0);
    let id = a.id;
    let old = a.lease_owner.unwrap();
    assert!(!repo.mark_published_by(id, "worker").unwrap());
    assert!(repo.mark_published(id).is_err());
    assert!(repo.record_failure(id, "bypass").is_err());
    db.conn()
        .unwrap()
        .execute(
            "UPDATE kernel_outbox SET lease_expires_at = '2000-01-01T00:00:00Z' WHERE id = ?",
            [id.to_string()],
        )
        .unwrap();
    assert!(!repo.mark_published_by(id, &old).unwrap());
    assert!(!repo.record_failure_by(id, &old, "expired", 0, 1).unwrap());
    let b = repo.claim_pending("worker", 1, 300).unwrap().remove(0);
    let current = b.lease_owner.unwrap();
    assert_ne!(old, current);
    assert!(!repo.mark_published_by(id, &old).unwrap());
    assert!(!repo.record_failure_by(id, &old, "stale", 0, 1).unwrap());
    assert!(repo.record_failure_by(id, &current, "retry", 0, 2).unwrap());
    assert!(repo.mark_published(id).is_err());
    assert!(repo.record_failure(id, "bypass retry").is_err());
    let c = repo.claim_pending("worker", 1, 300).unwrap().remove(0);
    let final_token = c.lease_owner.unwrap();
    assert_ne!(current, final_token);
    assert_eq!(c.attempts, 1);
    assert!(!repo.mark_published_by(id, &current).unwrap());
    assert!(repo.record_failure_by(id, &final_token, "dead", 0, 2).unwrap());
    assert!(repo.mark_published(id).is_err());
    assert!(repo.record_failure(id, "bypass dead letter").is_err());
    assert!(repo.redrive_dead_letter(id, true).unwrap());
    assert!(repo.mark_published(id).is_err());
    let d = repo.claim_pending("worker", 1, 300).unwrap().remove(0);
    let redriven = d.lease_owner.unwrap();
    assert!(!repo.mark_published_by(id, &final_token).unwrap());
    assert!(repo.mark_published_by(id, &redriven).unwrap());
    assert!(!repo.record_failure_by(id, &redriven, "late", 0, 1).unwrap());
    assert!(repo.record_failure(id, "late legacy").is_err());
    assert!(repo.mark_published(id).is_err());
    let state: (i64, Option<String>) = db
        .conn()
        .unwrap()
        .query_row(
            "SELECT attempts, last_error FROM kernel_outbox WHERE id = ?",
            [id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(state, (0, None));
}

#[test]
fn concurrent_claims_with_identical_labels_have_distinct_tokens_and_disjoint_rows() {
    use std::sync::{Arc, Barrier};
    let db = Arc::new(SqliteDatabase::in_memory().unwrap());
    for _ in 0..4 {
        db.payments()
            .create(CreatePayment { amount: rust_decimal::Decimal::ONE, ..Default::default() })
            .unwrap();
    }
    let barrier = Arc::new(Barrier::new(4));
    let tasks: Vec<_> = (0..4)
        .map(|_| {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                db.kernel_outbox().claim_pending("same-label", 1, 300).unwrap().remove(0)
            })
        })
        .collect();
    let rows: Vec<_> = tasks.into_iter().map(|task| task.join().unwrap()).collect();
    let ids: std::collections::HashSet<_> = rows.iter().map(|row| row.id).collect();
    let tokens: std::collections::HashSet<_> =
        rows.iter().map(|row| row.lease_owner.as_ref().unwrap()).collect();
    assert_eq!(ids.len(), 4);
    assert_eq!(tokens.len(), 4);
    assert!(db.kernel_outbox().claim_pending("same-label", 0, 300).unwrap().is_empty());
    assert!(
        !db.kernel_outbox()
            .mark_published_by(rows[0].id, rows[1].lease_owner.as_deref().unwrap())
            .unwrap()
    );
    for row in rows {
        assert!(
            db.kernel_outbox()
                .mark_published_by(row.id, row.lease_owner.as_deref().unwrap())
                .unwrap()
        );
    }
}
