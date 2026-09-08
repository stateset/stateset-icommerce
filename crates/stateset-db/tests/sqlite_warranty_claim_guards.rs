//! SQLite warranty transfer / claim-lifecycle guards.
//!
//! Every claim mutator used to run as
//!
//! ```text
//!   let claim = self.get_claim(id)?;      // pooled connection A
//!   Self::ensure_claim_can_*(&claim)?;    // decision on a row nobody holds
//!   conn.execute("UPDATE warranty_claims SET status = ... WHERE id = ?");
//! ```
//!
//! so two operators acting at once both passed their guard against the same
//! `submitted` snapshot and both wrote: approve and deny could each report
//! success on one claim, and an approve could resurrect a claim that was
//! already cancelled. `transfer` had the same shape against `warranties`, and
//! `create_claim`'s in-transaction increment guarded only `max_claims`, so a
//! claim could still be filed against a warranty that was voided or expired
//! while the claim waited for the write lock.
//!
//! The repair puts the status set each `ensure_*` allows into the UPDATE's own
//! `WHERE`, and reports `Conflict` when it matches no row. These tests pin
//! both halves: the sequential conflicts and the concurrent ones.

#![cfg(feature = "sqlite")]

use rust_decimal_macros::dec;
use stateset_core::{
    ClaimResolution, ClaimStatus, CommerceError, CreateCustomer, CreateWarranty,
    CreateWarrantyClaim, CustomerId, CustomerRepository, UpdateWarrantyClaim, Warranty,
    WarrantyClaim, WarrantyRepository, WarrantyStatus,
};
use stateset_db::{DatabaseConfig, SqliteDatabase};
use std::sync::{Arc, Barrier};

/// A pool wide enough that the contending threads actually overlap instead of
/// queueing on connection checkout.
fn test_db() -> Arc<SqliteDatabase> {
    Arc::new(
        SqliteDatabase::new(&DatabaseConfig { url: ":memory:".into(), max_connections: 8 })
            .expect("in-memory db"),
    )
}

fn customer(db: &SqliteDatabase) -> CustomerId {
    let unique = uuid::Uuid::new_v4().to_string();
    db.customers()
        .create(CreateCustomer {
            email: format!("warr-{}@example.com", &unique[..8]),
            first_name: "Warr".into(),
            last_name: "Anty".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .expect("create customer")
        .id
}

fn warranty(db: &SqliteDatabase) -> Warranty {
    let customer_id = customer(db);
    db.warranties()
        .create(CreateWarranty {
            customer_id,
            duration_months: Some(24),
            max_coverage_amount: Some(dec!(1000)),
            ..Default::default()
        })
        .expect("create warranty")
}

fn claim(db: &SqliteDatabase, warranty: &Warranty) -> WarrantyClaim {
    db.warranties()
        .create_claim(CreateWarrantyClaim {
            warranty_id: warranty.id,
            issue_description: "Screen cracked".into(),
            ..Default::default()
        })
        .expect("create claim")
}

fn status(db: &SqliteDatabase, claim: &WarrantyClaim) -> ClaimStatus {
    db.warranties().get_claim(claim.id).expect("get").expect("exists").status
}

// ---------------------------------------------------------------------------
// Sequential conflicts
// ---------------------------------------------------------------------------

/// A cancelled claim is terminal: approving it is a state conflict, not a
/// malformed request.
#[test]
fn sqlite_approve_after_cancel_is_a_conflict() {
    let db = test_db();
    let w = warranty(&db);
    let c = claim(&db, &w);
    db.warranties().cancel_claim(c.id).expect("cancel");

    let err = db.warranties().approve_claim(c.id).expect_err("approve a cancelled claim");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(status(&db, &c), ClaimStatus::Cancelled, "the cancel must stand");
}

/// Denying an already-approved claim is a conflict, and must not move it.
#[test]
fn sqlite_deny_after_approve_is_a_conflict() {
    let db = test_db();
    let w = warranty(&db);
    let c = claim(&db, &w);
    db.warranties().approve_claim(c.id).expect("approve");

    let err = db.warranties().deny_claim(c.id, "too late").expect_err("deny an approved claim");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(status(&db, &c), ClaimStatus::Approved, "the approval must stand");
}

/// The other terminal edges of the lifecycle report the same variant.
#[test]
fn sqlite_terminal_claims_reject_every_further_transition_with_a_conflict() {
    let db = test_db();
    let w = warranty(&db);

    let denied = claim(&db, &w);
    db.warranties().deny_claim(denied.id, "not covered").expect("deny");
    for err in [
        db.warranties().approve_claim(denied.id).unwrap_err(),
        db.warranties().deny_claim(denied.id, "again").unwrap_err(),
        db.warranties().complete_claim(denied.id, ClaimResolution::Repair).unwrap_err(),
        db.warranties().cancel_claim(denied.id).unwrap_err(),
    ] {
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    }

    let completed = claim(&db, &w);
    db.warranties().approve_claim(completed.id).expect("approve");
    db.warranties().complete_claim(completed.id, ClaimResolution::Repair).expect("complete");
    for err in [
        db.warranties().approve_claim(completed.id).unwrap_err(),
        db.warranties().complete_claim(completed.id, ClaimResolution::Refund).unwrap_err(),
        db.warranties().cancel_claim(completed.id).unwrap_err(),
    ] {
        assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    }

    // A claim that was never filed is still NotFound, not a conflict.
    let err = db.warranties().approve_claim(uuid::Uuid::new_v4()).unwrap_err();
    assert!(matches!(err, CommerceError::NotFound), "got {err:?}");
}

/// `complete_claim` still separates a bad *request* (no resolution) from a bad
/// *state* (the claim was never approved).
#[test]
fn sqlite_complete_claim_distinguishes_bad_input_from_bad_state() {
    let db = test_db();
    let w = warranty(&db);

    let submitted = claim(&db, &w);
    let err = db
        .warranties()
        .complete_claim(submitted.id, ClaimResolution::Repair)
        .expect_err("completing a submitted claim");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let approved = claim(&db, &w);
    db.warranties().approve_claim(approved.id).expect("approve");
    for bad in [ClaimResolution::None, ClaimResolution::Denied] {
        let err = db.warranties().complete_claim(approved.id, bad).expect_err("bad resolution");
        assert!(matches!(err, CommerceError::ValidationError(_)), "{bad}: got {err:?}");
    }
}

/// A warranty that is no longer active cannot be claimed against, and the
/// rejection must not consume one of its claims.
#[test]
fn sqlite_claims_are_refused_on_expired_and_voided_warranties() {
    let db = test_db();

    let expired_customer = customer(&db);
    let expired = db
        .warranties()
        .create(CreateWarranty {
            customer_id: expired_customer,
            start_date: Some(chrono::Utc::now() - chrono::Duration::days(400)),
            end_date: Some(chrono::Utc::now() - chrono::Duration::days(1)),
            max_claims: Some(3),
            ..Default::default()
        })
        .expect("create expired warranty");
    let err = db
        .warranties()
        .create_claim(CreateWarrantyClaim {
            warranty_id: expired.id,
            issue_description: "too late".into(),
            ..Default::default()
        })
        .expect_err("claim on an expired warranty");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(
        db.warranties().get(expired.id).expect("get").expect("exists").claims_used,
        0,
        "a refused claim must not burn a claim slot"
    );

    let voided = warranty(&db);
    db.warranties().void(voided.id).expect("void");
    let err = db
        .warranties()
        .create_claim(CreateWarrantyClaim {
            warranty_id: voided.id,
            issue_description: "voided".into(),
            ..Default::default()
        })
        .expect_err("claim on a voided warranty");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
    assert_eq!(
        db.warranties().get(voided.id).expect("get").expect("exists").claims_used,
        0,
        "a refused claim must not burn a claim slot"
    );
}

/// Transferring a warranty that is voided or expired is a state conflict.
#[test]
fn sqlite_transfer_of_a_dead_warranty_is_a_conflict() {
    let db = test_db();

    let voided = warranty(&db);
    db.warranties().void(voided.id).expect("void");
    let err = db.warranties().transfer(voided.id, customer(&db)).expect_err("transfer voided");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let expired = warranty(&db);
    db.warranties().expire(expired.id).expect("expire");
    let err = db.warranties().transfer(expired.id, customer(&db)).expect_err("transfer expired");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    // Transferring to the current owner is a malformed request, not a conflict.
    let live = warranty(&db);
    let err = db.warranties().transfer(live.id, live.customer_id).expect_err("no-op transfer");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");
}

// ---------------------------------------------------------------------------
// Races
// ---------------------------------------------------------------------------

/// Approve and deny are mutually exclusive: they allow the same three source
/// statuses, so whichever lands first must lock the other out. Before the
/// repair both read `submitted`, both passed their guard and both wrote — the
/// claim ended up in whichever status committed last while *both* callers had
/// been told they succeeded.
#[test]
fn sqlite_concurrent_approve_and_deny_leave_exactly_one_winner() {
    let db = test_db();
    let w = warranty(&db);

    for trial in 0..12 {
        let c = claim(&db, &w);
        let barrier = Arc::new(Barrier::new(2));

        let approver = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().approve_claim(id)
            })
        };
        let denier = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().deny_claim(id, "duplicate filing")
            })
        };

        let approved = approver.join().expect("approver thread");
        let denied = denier.join().expect("denier thread");
        let final_status = status(&db, &c);

        assert_ne!(
            approved.is_ok(),
            denied.is_ok(),
            "trial {trial}: exactly one of approve/deny may succeed \
             (approve: {approved:?}, deny: {denied:?})"
        );
        let expected = if approved.is_ok() { ClaimStatus::Approved } else { ClaimStatus::Denied };
        assert_eq!(
            final_status, expected,
            "trial {trial}: the winner's write must be the one on disk"
        );
        for loser in [approved.err(), denied.err()].into_iter().flatten() {
            assert!(matches!(loser, CommerceError::Conflict(_)), "trial {trial}: got {loser:?}");
        }
    }
}

/// Cancel legitimately follows an approval, so both calls may succeed — what
/// must never happen is the reverse: a claim the repository already reported
/// as cancelled coming back as `approved` because the approver decided on a
/// pre-cancel snapshot.
#[test]
fn sqlite_a_cancelled_claim_is_never_resurrected_by_a_concurrent_approve() {
    let db = test_db();
    let w = warranty(&db);

    for trial in 0..12 {
        let c = claim(&db, &w);
        let barrier = Arc::new(Barrier::new(2));

        let approver = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().approve_claim(id)
            })
        };
        let canceller = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().cancel_claim(id)
            })
        };

        let approved = approver.join().expect("approver thread").is_ok();
        let cancelled = canceller.join().expect("canceller thread").is_ok();
        let final_status = status(&db, &c);

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

/// `update_claim` is a read-merge-write over nullable columns: it re-writes
/// every field from the snapshot it read, so two operators filling in
/// different fields at once used to clobber each other's value with the `None`
/// they had read.
#[test]
fn sqlite_concurrent_partial_claim_updates_do_not_lose_fields() {
    let db = test_db();
    let w = warranty(&db);

    for trial in 0..12 {
        let c = claim(&db, &w);
        let barrier = Arc::new(Barrier::new(2));

        let notes = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().update_claim(
                    id,
                    UpdateWarrantyClaim {
                        internal_notes: Some("triaged by ops".into()),
                        ..Default::default()
                    },
                )
            })
        };
        let money = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = c.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().update_claim(
                    id,
                    UpdateWarrantyClaim { repair_cost: Some(dec!(42.50)), ..Default::default() },
                )
            })
        };

        notes.join().expect("notes thread").expect("notes update");
        money.join().expect("money thread").expect("money update");

        let after = db.warranties().get_claim(c.id).expect("get").expect("exists");
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
#[test]
fn sqlite_a_voided_warranty_is_never_resurrected_by_a_concurrent_transfer() {
    let db = test_db();

    for trial in 0..12 {
        let w = warranty(&db);
        let new_owner = customer(&db);
        let barrier = Arc::new(Barrier::new(2));

        let transferrer = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = w.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().transfer(id, new_owner)
            })
        };
        let voider = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let id = w.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.warranties().void(id)
            })
        };

        let transferred = transferrer.join().expect("transfer thread");
        let voided = voider.join().expect("void thread").is_ok();
        let after = db.warranties().get(w.id).expect("get").expect("exists");

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

/// The deterministic version of the `create_claim` race: a writer holds the
/// database's write lock, the claim reads a live warranty and then blocks, and
/// the holder voids the warranty before letting go. The claim's increment must
/// re-check the warranty's status under the lock it finally gets — before the
/// repair it only checked `max_claims`, so the claim was filed against (and
/// burned a slot on) a warranty that had been dead for the whole write.
#[test]
fn sqlite_a_claim_blocked_on_the_write_lock_cannot_land_on_a_voided_warranty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("warranty-race.db");
    let db = Arc::new(
        SqliteDatabase::new(&DatabaseConfig {
            url: path.to_string_lossy().into_owned(),
            max_connections: 8,
        })
        .expect("file db"),
    );
    let w = warranty(&db);

    // A second connection to the same file takes the write lock and keeps it.
    let blocker = rusqlite::Connection::open(&path).expect("open blocker");
    blocker.execute_batch("PRAGMA busy_timeout = 30000;").expect("busy timeout");
    blocker.execute_batch("BEGIN IMMEDIATE;").expect("take the write lock");

    let claimer = {
        let db = Arc::clone(&db);
        let warranty_id = w.id;
        std::thread::spawn(move || {
            db.warranties().create_claim(CreateWarrantyClaim {
                warranty_id,
                issue_description: "filed while the warranty was still live".into(),
                ..Default::default()
            })
        })
    };

    // Long enough for the claimer's pre-check read to have happened and for it
    // to be parked on `BEGIN IMMEDIATE`.
    std::thread::sleep(std::time::Duration::from_millis(300));
    blocker
        .execute(
            "UPDATE warranties SET status = 'voided' WHERE id = ?1",
            rusqlite::params![w.id.to_string()],
        )
        .expect("void under the lock");
    blocker.execute_batch("COMMIT;").expect("release the write lock");
    drop(blocker);

    let result = claimer.join().expect("claimer thread");
    assert!(
        result.is_err(),
        "a claim must not be filed against a warranty voided while the claim waited \
         for the write lock: {result:?}"
    );
    let after = db.warranties().get(w.id).expect("get").expect("exists");
    assert_eq!(after.status, WarrantyStatus::Voided);
    assert_eq!(after.claims_used, 0, "the refused claim must not have burned a slot");
    assert!(
        db.warranties().get_claims(w.id).expect("claims").is_empty(),
        "no claim row may survive"
    );
}
