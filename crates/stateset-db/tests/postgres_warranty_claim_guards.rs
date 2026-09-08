//! Postgres twin of `sqlite_warranty_claim_guards`.
//!
//! Every claim mutator ran its `ensure_claim_can_*` guard against a row read
//! on the pool and then wrote with a bare `WHERE id = $1`, so approve and deny
//! could both succeed on one claim and an approve could overwrite a cancel it
//! had never seen. `transfer_async` had the same shape, and
//! `create_claim_async`'s in-transaction increment guarded only `max_claims`,
//! never the warranty's status or expiry.
//!
//! The repair reads the row inside the transaction (`FOR UPDATE` for
//! `create_claim_async` / `update_claim_async`) and puts the allowed status set
//! into the UPDATE's own `WHERE`, reporting `Conflict` when it matches
//! nothing.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use sqlx::postgres::PgPoolOptions;
use stateset_core::{
    ClaimResolution, ClaimStatus, CommerceError, CreateCustomer, CreateWarranty,
    CreateWarrantyClaim, CustomerId, UpdateWarrantyClaim, Warranty, WarrantyClaim, WarrantyStatus,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<Arc<PostgresDatabase>> {
    let url = postgres_url()?;
    Some(Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate")))
}

async fn customer(db: &PostgresDatabase) -> CustomerId {
    let unique = uuid::Uuid::new_v4().to_string();
    db.customers()
        .create_async(CreateCustomer {
            email: format!("warr-guard-{}@example.com", &unique[..12]),
            first_name: "Warr".into(),
            last_name: "Anty".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer")
        .id
}

async fn warranty(db: &PostgresDatabase) -> Warranty {
    let customer_id = customer(db).await;
    db.warranties()
        .create_async(CreateWarranty {
            customer_id,
            duration_months: Some(24),
            max_coverage_amount: Some(dec!(1000)),
            ..Default::default()
        })
        .await
        .expect("create warranty")
}

async fn claim(db: &PostgresDatabase, warranty: &Warranty) -> WarrantyClaim {
    db.warranties()
        .create_claim_async(CreateWarrantyClaim {
            warranty_id: warranty.id,
            issue_description: "Screen cracked".into(),
            ..Default::default()
        })
        .await
        .expect("create claim")
}

async fn status(db: &PostgresDatabase, claim: &WarrantyClaim) -> ClaimStatus {
    db.warranties().get_claim_async(claim.id).await.expect("get").expect("exists").status
}

// ---------------------------------------------------------------------------
// Sequential conflicts
// ---------------------------------------------------------------------------

#[tokio::test]
async fn postgres_terminal_claims_reject_every_further_transition_with_a_conflict() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let w = warranty(&db).await;
    let repo = db.warranties();

    let cancelled = claim(&db, &w).await;
    repo.cancel_claim_async(cancelled.id).await.expect("cancel");
    let err = repo.approve_claim_async(cancelled.id).await.expect_err("approve after cancel");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(status(&db, &cancelled).await, ClaimStatus::Cancelled, "the cancel must stand");

    let approved = claim(&db, &w).await;
    repo.approve_claim_async(approved.id).await.expect("approve");
    let err = repo.deny_claim_async(approved.id, "too late").await.expect_err("deny after approve");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(status(&db, &approved).await, ClaimStatus::Approved, "the approval must stand");

    let denied = claim(&db, &w).await;
    repo.deny_claim_async(denied.id, "not covered").await.expect("deny");
    for err in [
        repo.approve_claim_async(denied.id).await.unwrap_err(),
        repo.deny_claim_async(denied.id, "again").await.unwrap_err(),
        repo.complete_claim_async(denied.id, ClaimResolution::Repair).await.unwrap_err(),
        repo.cancel_claim_async(denied.id).await.unwrap_err(),
    ] {
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    }

    // A claim that was never filed is still NotFound, not a conflict.
    let err = repo.approve_claim_async(uuid::Uuid::new_v4()).await.unwrap_err();
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");

    // A bad *request* stays a validation error.
    let ready = claim(&db, &w).await;
    repo.approve_claim_async(ready.id).await.expect("approve");
    for bad in [ClaimResolution::None, ClaimResolution::Denied] {
        let err = repo.complete_claim_async(ready.id, bad).await.expect_err("bad resolution");
        assert!(matches!(err, CommerceError::ValidationError(_)), "{bad}: got {err:?}");
    }
    let err = repo
        .complete_claim_async(cancelled.id, ClaimResolution::Repair)
        .await
        .expect_err("completing a cancelled claim");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
}

#[tokio::test]
async fn postgres_claims_are_refused_on_expired_and_voided_warranties() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let repo = db.warranties();

    let expired_owner = customer(&db).await;
    let expired = repo
        .create_async(CreateWarranty {
            customer_id: expired_owner,
            start_date: Some(chrono::Utc::now() - chrono::Duration::days(400)),
            end_date: Some(chrono::Utc::now() - chrono::Duration::days(1)),
            max_claims: Some(3),
            ..Default::default()
        })
        .await
        .expect("create expired warranty");
    let err = repo
        .create_claim_async(CreateWarrantyClaim {
            warranty_id: expired.id,
            issue_description: "too late".into(),
            ..Default::default()
        })
        .await
        .expect_err("claim on an expired warranty");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(
        repo.get_async(expired.id).await.expect("get").expect("exists").claims_used,
        0,
        "a refused claim must not burn a claim slot"
    );

    let voided = warranty(&db).await;
    repo.void_async(voided.id).await.expect("void");
    let err = repo
        .create_claim_async(CreateWarrantyClaim {
            warranty_id: voided.id,
            issue_description: "voided".into(),
            ..Default::default()
        })
        .await
        .expect_err("claim on a voided warranty");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(
        repo.get_async(voided.id).await.expect("get").expect("exists").claims_used,
        0,
        "a refused claim must not burn a claim slot"
    );
}

#[tokio::test]
async fn postgres_transfer_of_a_dead_warranty_is_a_conflict() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let repo = db.warranties();

    let voided = warranty(&db).await;
    repo.void_async(voided.id).await.expect("void");
    let to = customer(&db).await;
    let err = repo.transfer_async(voided.id, to).await.expect_err("transfer voided");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let expired = warranty(&db).await;
    repo.expire_async(expired.id).await.expect("expire");
    let to = customer(&db).await;
    let err = repo.transfer_async(expired.id, to).await.expect_err("transfer expired");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    // Transferring to the current owner is a malformed request, not a conflict.
    let live = warranty(&db).await;
    let err = repo.transfer_async(live.id, live.customer_id).await.expect_err("no-op transfer");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

// ---------------------------------------------------------------------------
// Races
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_approve_and_deny_leave_exactly_one_winner() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let w = warranty(&db).await;

    for trial in 0..8 {
        let c = claim(&db, &w).await;
        let barrier = Arc::new(Barrier::new(2));

        let approver = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties().approve_claim_async(id).await
            })
        };
        let denier = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties().deny_claim_async(id, "duplicate filing").await
            })
        };

        let approved = approver.await.expect("approver task");
        let denied = denier.await.expect("denier task");
        let final_status = status(&db, &c).await;

        assert_ne!(
            approved.is_ok(),
            denied.is_ok(),
            "trial {trial}: exactly one of approve/deny may succeed \
             (approve: {approved:?}, deny: {denied:?})"
        );
        let expected = if approved.is_ok() { ClaimStatus::Approved } else { ClaimStatus::Denied };
        assert_eq!(final_status, expected, "trial {trial}: the winner's write must be on disk");
        for loser in [approved.err(), denied.err()].into_iter().flatten() {
            assert!(matches!(loser, CommerceError::Conflict(_)), "trial {trial}: got {loser:?}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_a_cancelled_claim_is_never_resurrected_by_a_concurrent_approve() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let w = warranty(&db).await;

    for trial in 0..8 {
        let c = claim(&db, &w).await;
        let barrier = Arc::new(Barrier::new(2));

        let approver = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties().approve_claim_async(id).await
            })
        };
        let canceller = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties().cancel_claim_async(id).await
            })
        };

        let approved = approver.await.expect("approver task").is_ok();
        let cancelled = canceller.await.expect("canceller task").is_ok();
        let final_status = status(&db, &c).await;

        assert!(approved || cancelled, "trial {trial}: at least one operation must land");
        if cancelled {
            assert_eq!(
                final_status,
                ClaimStatus::Cancelled,
                "trial {trial}: a successful cancel must not be overwritten by an approve"
            );
        } else {
            assert_eq!(final_status, ClaimStatus::Approved, "trial {trial}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_concurrent_partial_claim_updates_do_not_lose_fields() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let w = warranty(&db).await;

    for trial in 0..8 {
        let c = claim(&db, &w).await;
        let barrier = Arc::new(Barrier::new(2));

        let notes = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties()
                    .update_claim_async(
                        id,
                        UpdateWarrantyClaim {
                            internal_notes: Some("triaged by ops".into()),
                            ..Default::default()
                        },
                    )
                    .await
            })
        };
        let money = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties()
                    .update_claim_async(
                        id,
                        UpdateWarrantyClaim {
                            repair_cost: Some(dec!(42.50)),
                            ..Default::default()
                        },
                    )
                    .await
            })
        };

        notes.await.expect("notes task").expect("notes update");
        money.await.expect("money task").expect("money update");

        let after = db.warranties().get_claim_async(c.id).await.expect("get").expect("exists");
        assert_eq!(
            after.internal_notes.as_deref(),
            Some("triaged by ops"),
            "trial {trial}: the notes update was lost"
        );
        assert_eq!(after.repair_cost, Some(dec!(42.50)), "trial {trial}: the cost update was lost");
    }
}

/// Transferring a warranty twice in a row is legal, so two concurrent
/// transfers may both succeed. What must never happen is a transfer landing on
/// a warranty that has already been voided: before the repair the transfer's
/// UPDATE carried no status predicate, so it resurrected a dead warranty as
/// `transferred` with a brand-new owner.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_a_voided_warranty_is_never_resurrected_by_a_concurrent_transfer() {
    let Some(db) = connect().await else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };

    for trial in 0..8 {
        let w = warranty(&db).await;
        let new_owner = customer(&db).await;
        let barrier = Arc::new(Barrier::new(2));

        let transferrer = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = w.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties().transfer_async(id, new_owner).await
            })
        };
        let voider = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = w.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.warranties().void_async(id).await
            })
        };

        let transferred = transferrer.await.expect("transfer task");
        let voided = voider.await.expect("void task").is_ok();
        let after = db.warranties().get_async(w.id).await.expect("get").expect("exists");

        assert!(voided || transferred.is_ok(), "trial {trial}: at least one operation must land");
        if voided {
            assert_eq!(
                after.status,
                WarrantyStatus::Voided,
                "trial {trial}: a successful void must not be overwritten by a transfer"
            );
        } else {
            assert_eq!(after.status, WarrantyStatus::Transferred, "trial {trial}");
            assert_eq!(after.customer_id, new_owner, "trial {trial}");
        }
        if let Err(err) = transferred {
            assert!(matches!(err, CommerceError::Conflict(_)), "trial {trial}: got {err:?}");
        }
    }
}

/// The deterministic version of the `create_claim_async` race: a separate
/// transaction holds the warranty row `FOR UPDATE`, the claim opens its own
/// transaction and blocks on that lock, and the holder voids the warranty
/// before committing. When the lock is released the claim must re-decide on
/// the row it now owns — before the repair it only re-checked `max_claims`, so
/// the claim was filed against a warranty that had been dead for the whole
/// write.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_a_claim_blocked_on_the_row_lock_cannot_land_on_a_voided_warranty() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let w = warranty(&db).await;

    let raw = PgPoolOptions::new().max_connections(2).connect(&url).await.expect("raw pool");
    let mut blocker = raw.begin().await.expect("begin blocker");
    sqlx::query("SELECT id FROM warranties WHERE id = $1 FOR UPDATE")
        .bind(w.id.into_uuid())
        .fetch_one(blocker.as_mut())
        .await
        .expect("lock the warranty row");

    let claimer = {
        let db = Arc::clone(&db);
        let warranty_id = w.id;
        tokio::spawn(async move {
            db.warranties()
                .create_claim_async(CreateWarrantyClaim {
                    warranty_id,
                    issue_description: "filed while the warranty was still live".into(),
                    ..Default::default()
                })
                .await
        })
    };

    // Long enough for the claimer to have read the live warranty and parked on
    // the row lock.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    sqlx::query("UPDATE warranties SET status = 'voided' WHERE id = $1")
        .bind(w.id.into_uuid())
        .execute(blocker.as_mut())
        .await
        .expect("void under the lock");
    blocker.commit().await.expect("release the row lock");

    let result = claimer.await.expect("claimer task");
    assert!(
        result.is_err(),
        "a claim must not be filed against a warranty voided while the claim waited \
         for the row lock: {result:?}"
    );
    let after = db.warranties().get_async(w.id).await.expect("get").expect("exists");
    assert_eq!(after.status, WarrantyStatus::Voided);
    assert_eq!(after.claims_used, 0, "the refused claim must not have burned a slot");
    assert!(
        db.warranties().get_claims_async(w.id).await.expect("claims").is_empty(),
        "no claim row may survive"
    );
    raw.close().await;
}
