#![cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
use uuid::Uuid;

#[tokio::test]
async fn postgres_expired_and_reused_worker_claims_cannot_settle_new_attempts() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        return;
    };
    let db = PostgresDatabase::connect(url).await.unwrap();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO kernel_outbox (id, event_type, aggregate_type, aggregate_id, payload, created_at) VALUES ($1, 'fencing.test', 'test', $2, '{}'::jsonb, '0001-01-01T00:00:00Z')")
        .bind(id).bind(id.to_string()).execute(db.pool()).await.unwrap();
    let repo = db.kernel_outbox();
    let a = repo.claim_pending_async("worker", 1, 300).await.unwrap().remove(0);
    assert_eq!(a.id, id);
    let old = a.lease_owner.unwrap();
    assert!(!repo.mark_published_by_async(id, "worker").await.unwrap());
    assert!(repo.mark_published_async(id).await.is_err());
    assert!(repo.record_failure_async(id, "bypass").await.is_err());
    sqlx::query("UPDATE kernel_outbox SET lease_expires_at = '2000-01-01T00:00:00Z' WHERE id = $1")
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(!repo.mark_published_by_async(id, &old).await.unwrap());
    assert!(!repo.record_failure_by_async(id, &old, "expired", 0, 1).await.unwrap());
    let b = repo.claim_pending_async("worker", 1, 300).await.unwrap().remove(0);
    assert_eq!(b.id, id);
    let current = b.lease_owner.unwrap();
    assert_ne!(old, current);
    assert!(!repo.mark_published_by_async(id, &old).await.unwrap());
    assert!(!repo.record_failure_by_async(id, &old, "stale", 0, 1).await.unwrap());
    assert!(repo.record_failure_by_async(id, &current, "retry", 0, 2).await.unwrap());
    assert!(repo.mark_published_async(id).await.is_err());
    assert!(repo.record_failure_async(id, "bypass retry").await.is_err());
    let c = repo.claim_pending_async("worker", 1, 300).await.unwrap().remove(0);
    assert_eq!(c.id, id);
    let final_token = c.lease_owner.unwrap();
    assert_ne!(current, final_token);
    assert_eq!(c.attempts, 1);
    assert!(!repo.mark_published_by_async(id, &current).await.unwrap());
    assert!(repo.record_failure_by_async(id, &final_token, "dead", 0, 2).await.unwrap());
    assert!(repo.mark_published_async(id).await.is_err());
    assert!(repo.record_failure_async(id, "bypass dead letter").await.is_err());
    assert!(repo.redrive_dead_letter_async(id, true).await.unwrap());
    assert!(repo.mark_published_async(id).await.is_err());
    let d = repo.claim_pending_async("worker", 1, 300).await.unwrap().remove(0);
    assert_eq!(d.id, id);
    let redriven = d.lease_owner.unwrap();
    assert!(!repo.mark_published_by_async(id, &final_token).await.unwrap());
    assert!(repo.mark_published_by_async(id, &redriven).await.unwrap());
    assert!(!repo.record_failure_by_async(id, &redriven, "late", 0, 1).await.unwrap());
    assert!(repo.record_failure_async(id, "late legacy").await.is_err());
    assert!(repo.mark_published_async(id).await.is_err());
    let state: (i32, Option<String>) =
        sqlx::query_as("SELECT attempts, last_error FROM kernel_outbox WHERE id = $1")
            .bind(id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(state, (0, None));

    let mut expected = std::collections::HashSet::new();
    for _ in 0..4 {
        let probe = Uuid::new_v4();
        expected.insert(probe);
        sqlx::query("INSERT INTO kernel_outbox (id, event_type, aggregate_type, aggregate_id, payload, created_at) VALUES ($1, 'fencing.test', 'test', $2, '{}'::jsonb, '0001-01-01T00:00:00Z')")
            .bind(probe).bind(probe.to_string()).execute(db.pool()).await.unwrap();
    }
    let (a, b, c, d) = tokio::join!(
        repo.claim_pending_async("same-label", 1, 300),
        repo.claim_pending_async("same-label", 1, 300),
        repo.claim_pending_async("same-label", 1, 300),
        repo.claim_pending_async("same-label", 1, 300),
    );
    let rows: Vec<_> = [a, b, c, d].into_iter().map(|row| row.unwrap().remove(0)).collect();
    let actual: std::collections::HashSet<_> = rows.iter().map(|row| row.id).collect();
    let tokens: std::collections::HashSet<_> =
        rows.iter().map(|row| row.lease_owner.as_ref().unwrap()).collect();
    assert_eq!(actual, expected);
    assert_eq!(tokens.len(), 4);
    assert!(repo.claim_pending_async("same-label", 0, 300).await.unwrap().is_empty());
    assert!(
        !repo
            .mark_published_by_async(rows[0].id, rows[1].lease_owner.as_deref().unwrap())
            .await
            .unwrap()
    );
    for row in rows {
        assert!(
            repo.mark_published_by_async(row.id, row.lease_owner.as_deref().unwrap())
                .await
                .unwrap()
        );
    }
}
