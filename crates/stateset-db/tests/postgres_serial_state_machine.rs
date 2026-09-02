//! Postgres guards for the serial state machine and reservation lifecycle.
//!
//! * S1 — `reserve_async` read the serial with a plain `SELECT` (no `FOR
//!   UPDATE`), counted active reservations in application code, then inserted
//!   and flipped the status. Under READ COMMITTED two orders could both see
//!   "available, 0 reservations" and both end up holding one physical unit. The
//!   serial row is now locked, the status write is conditional, and a unique
//!   index on `serial_reservations.active_key` (migration 086) is the backstop.
//! * S2 — every status write goes through `SerialStatus::allowed_transitions`;
//!   a scrapped unit can no longer be shipped.
//! * S3 — a sale/shipment consumes the open reservation, so releasing it later
//!   cannot flip the unit back to `available`; expired reservations are swept.
//! * S4 — lot-level quarantine helpers only touch `available`/`reserved` units.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use chrono::Utc;
use sqlx::postgres::PgPoolOptions;
use stateset_core::{
    ChangeSerialStatus, CommerceError, CreateSerialNumber, ReserveSerialNumber, SerialStatus,
    UpdateSerialNumber,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn make_serial(db: &PostgresDatabase, sku: &str, lot_id: Option<Uuid>) -> Uuid {
    db.serials()
        .create_async(CreateSerialNumber {
            serial: Some(format!("SN-{}", Uuid::new_v4().simple())),
            sku: sku.to_string(),
            lot_id,
            ..Default::default()
        })
        .await
        .expect("create serial")
        .id
}

fn reserve_input(serial_id: Uuid) -> ReserveSerialNumber {
    ReserveSerialNumber {
        serial_id,
        reference_type: "order".into(),
        reference_id: Uuid::new_v4(),
        reserved_by: None,
        expires_in_seconds: None,
    }
}

async fn raw_pool(url: &str) -> sqlx::PgPool {
    PgPoolOptions::new().max_connections(2).connect(url).await.expect("raw pool")
}

async fn force_status(pool: &sqlx::PgPool, id: Uuid, status: SerialStatus) {
    sqlx::query("UPDATE serial_numbers SET status = $1 WHERE id = $2")
        .bind(status.to_string())
        .bind(id)
        .execute(pool)
        .await
        .expect("force status");
}

async fn status_of(db: &PostgresDatabase, id: Uuid) -> SerialStatus {
    db.serials().get_async(id).await.expect("get").expect("exists").status
}

/// S1: many concurrent reservations of ONE serial — exactly one wins, exactly
/// one open reservation row exists, the serial is `reserved`.
#[tokio::test]
async fn postgres_concurrent_reserve_admits_exactly_one() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let serial_id = make_serial(&db, "SKU-RACE", None).await;

    let mut handles = Vec::new();
    for _ in 0..12 {
        let db = Arc::clone(&db);
        handles.push(tokio::spawn(async move {
            db.serials().reserve_async(reserve_input(serial_id)).await
        }));
    }
    let mut ok = 0;
    for handle in handles {
        match handle.await.expect("task") {
            Ok(_) => ok += 1,
            Err(CommerceError::Conflict(_)) => {}
            Err(other) => panic!("unexpected error: {other:?}"),
        }
    }
    assert_eq!(ok, 1, "exactly one reservation may win the race");

    let pool = raw_pool(&url).await;
    let open: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM serial_reservations WHERE serial_id = $1 AND released_at IS NULL",
    )
    .bind(serial_id)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(open, 1, "one physical unit, one open reservation");
    assert_eq!(status_of(&db, serial_id).await, SerialStatus::Reserved);
}

/// S1 backstop: a writer bypassing the repository cannot open a second
/// reservation on a serial — the unique index on `active_key` refuses it.
#[tokio::test]
async fn postgres_active_key_unique_index_is_the_backstop() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let serial_id = make_serial(&db, "SKU-BACKSTOP", None).await;
    db.serials().reserve_async(reserve_input(serial_id)).await.expect("reserve");

    let pool = raw_pool(&url).await;
    let err = sqlx::query(
        "INSERT INTO serial_reservations (id, serial_id, reference_type, reference_id, active_key)
         VALUES ($1, $2, 'order', $3, $4)",
    )
    .bind(Uuid::new_v4())
    .bind(serial_id)
    .bind(Uuid::new_v4())
    .bind(serial_id.to_string())
    .execute(&pool)
    .await
    .expect_err("duplicate open reservation must violate the unique index");
    assert!(err.to_string().contains("idx_serial_reservations_active_key"), "{err}");
}

/// S2: for every (from, to) pair the repository accepts the change iff the
/// state machine lists the edge; a refusal leaves the status untouched.
#[tokio::test]
async fn postgres_change_status_accepts_exactly_the_transition_table() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let pool = raw_pool(&url).await;
    for from in SerialStatus::ALL {
        for to in SerialStatus::ALL {
            let id = make_serial(&db, "SKU-SM", None).await;
            force_status(&pool, id, from).await;
            let result = db
                .serials()
                .change_status_async(ChangeSerialStatus {
                    serial_id: id,
                    new_status: to,
                    ..Default::default()
                })
                .await;
            assert_eq!(result.is_ok(), from.can_transition_to(to), "{from} -> {to}: {result:?}");
            if !from.can_transition_to(to) {
                assert!(matches!(result, Err(CommerceError::Conflict(_))), "{from} -> {to}");
            }
            let expected = if from.can_transition_to(to) { to } else { from };
            assert_eq!(status_of(&db, id).await, expected, "{from} -> {to}");
        }
    }
}

/// S2: the named mutations (ship/sell/return/quarantine/scrap) and `update`
/// enforce the same table.
#[tokio::test]
async fn postgres_named_mutations_enforce_the_state_machine() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let pool = raw_pool(&url).await;

    let id = make_serial(&db, "SKU-NAMED", None).await;
    db.serials().scrap_async(id, "crushed").await.expect("scrap");
    let err = db.serials().mark_shipped_async(id, Uuid::new_v4()).await.expect_err("no ship");
    match err {
        CommerceError::Conflict(msg) => {
            assert!(msg.contains("scrapped") && msg.contains("shipped"), "{msg}");
        }
        other => panic!("expected Conflict, got {other:?}"),
    }
    assert!(db.serials().mark_sold_async(id, Uuid::new_v4(), None).await.is_err());
    assert!(db.serials().mark_returned_async(id, Uuid::new_v4()).await.is_err());
    assert!(db.serials().quarantine_async(id, "qc").await.is_err());
    assert!(
        db.serials()
            .update_async(
                id,
                UpdateSerialNumber { status: Some(SerialStatus::Available), ..Default::default() }
            )
            .await
            .is_err(),
        "update cannot resurrect a scrapped serial"
    );
    assert_eq!(status_of(&db, id).await, SerialStatus::Scrapped);

    for from in SerialStatus::ALL {
        let id = make_serial(&db, "SKU-NAMED", None).await;
        force_status(&pool, id, from).await;
        let shipped = db.serials().mark_shipped_async(id, Uuid::new_v4()).await.is_ok();
        assert_eq!(shipped, from.can_transition_to(SerialStatus::Shipped), "ship from {from}");
        let id = make_serial(&db, "SKU-NAMED", None).await;
        force_status(&pool, id, from).await;
        let scrapped = db.serials().scrap_async(id, "bin").await.is_ok();
        assert_eq!(scrapped, from.can_transition_to(SerialStatus::Scrapped), "scrap from {from}");
    }
}

/// S3: reserve -> confirm -> sell consumes the reservation; a release
/// afterwards is refused and the unit stays sold. Expired reservations are
/// swept back to stock.
#[tokio::test]
async fn postgres_reservation_lifecycle_is_coherent() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");

    let id = make_serial(&db, "SKU-LIFE", None).await;
    let res = db.serials().reserve_async(reserve_input(id)).await.expect("reserve");
    db.serials().confirm_reservation_async(res.id).await.expect("confirm");
    db.serials().confirm_reservation_async(res.id).await.expect("confirm is idempotent");
    let confirmed = db.serials().get_reservation_async(res.id).await.unwrap().unwrap();
    assert!(confirmed.is_confirmed() && confirmed.is_open());
    assert_eq!(status_of(&db, id).await, SerialStatus::Reserved);

    db.serials().mark_sold_async(id, Uuid::new_v4(), None).await.expect("sell");
    let consumed = db.serials().get_reservation_async(res.id).await.unwrap().unwrap();
    assert!(consumed.released_at.is_some(), "sale must consume the reservation");
    let err = db.serials().release_reservation_async(res.id).await.expect_err("consumed");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");
    assert_eq!(status_of(&db, id).await, SerialStatus::Sold);

    // Shipping consumes too.
    let id = make_serial(&db, "SKU-LIFE", None).await;
    let res = db.serials().reserve_async(reserve_input(id)).await.expect("reserve");
    db.serials().mark_shipped_async(id, Uuid::new_v4()).await.expect("ship");
    assert!(
        db.serials().get_reservation_async(res.id).await.unwrap().unwrap().released_at.is_some()
    );
    assert!(db.serials().release_reservation_async(res.id).await.is_err());
    assert_eq!(status_of(&db, id).await, SerialStatus::Shipped);

    // Release while reserved returns the unit to stock; a second release is refused.
    let id = make_serial(&db, "SKU-LIFE", None).await;
    let res = db.serials().reserve_async(reserve_input(id)).await.expect("reserve");
    db.serials().release_reservation_async(res.id).await.expect("release");
    assert_eq!(status_of(&db, id).await, SerialStatus::Available);
    assert!(db.serials().release_reservation_async(res.id).await.is_err());
    assert!(db.serials().confirm_reservation_async(res.id).await.is_err());

    // Expiry sweep: expired+unconfirmed is swept, live and confirmed are not.
    let expired_id = make_serial(&db, "SKU-LIFE", None).await;
    let live_id = make_serial(&db, "SKU-LIFE", None).await;
    let expired = db
        .serials()
        .reserve_async(ReserveSerialNumber {
            expires_in_seconds: Some(-1),
            ..reserve_input(expired_id)
        })
        .await
        .expect("reserve expired");
    let live = db
        .serials()
        .reserve_async(ReserveSerialNumber {
            expires_in_seconds: Some(3600),
            ..reserve_input(live_id)
        })
        .await
        .expect("reserve live");
    let err = db.serials().confirm_reservation_async(expired.id).await.expect_err("expired");
    assert!(matches!(err, CommerceError::Conflict(_)), "got {err:?}");

    let swept = db.serials().release_expired_reservations_async(Utc::now()).await.expect("sweep");
    assert!(swept >= 1, "at least our expired reservation is swept (got {swept})");
    assert_eq!(status_of(&db, expired_id).await, SerialStatus::Available);
    assert!(
        db.serials()
            .get_reservation_async(expired.id)
            .await
            .unwrap()
            .unwrap()
            .released_at
            .is_some()
    );
    assert_eq!(status_of(&db, live_id).await, SerialStatus::Reserved);
    assert!(db.serials().get_reservation_async(live.id).await.unwrap().unwrap().is_open());

    // Lazy expiry: reserving a serial held by a stale reservation succeeds.
    let stale_id = make_serial(&db, "SKU-LIFE", None).await;
    let stale = db
        .serials()
        .reserve_async(ReserveSerialNumber {
            expires_in_seconds: Some(-1),
            ..reserve_input(stale_id)
        })
        .await
        .expect("stale reserve");
    let fresh = db.serials().reserve_async(reserve_input(stale_id)).await.expect("re-reserve");
    assert_ne!(stale.id, fresh.id);
    assert!(
        db.serials().get_reservation_async(stale.id).await.unwrap().unwrap().released_at.is_some()
    );
    assert_eq!(status_of(&db, stale_id).await, SerialStatus::Reserved);
}

/// S4: lot-level quarantine only touches available/reserved serials of that
/// lot and closes their reservations; release returns the quarantined ones.
#[tokio::test]
async fn postgres_lot_quarantine_helpers() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let pool = raw_pool(&url).await;
    let lot_id = db
        .lots()
        .create_async(stateset_core::CreateLot {
            sku: format!("LOT-{}", Uuid::new_v4().simple()),
            quantity: rust_decimal::Decimal::from(10),
            ..Default::default()
        })
        .await
        .expect("create lot")
        .id;

    let available = make_serial(&db, "SKU-LOTQ", Some(lot_id)).await;
    let reserved = make_serial(&db, "SKU-LOTQ", Some(lot_id)).await;
    let res = db.serials().reserve_async(reserve_input(reserved)).await.expect("reserve");
    let sold = make_serial(&db, "SKU-LOTQ", Some(lot_id)).await;
    force_status(&pool, sold, SerialStatus::Sold).await;
    let other = make_serial(&db, "SKU-LOTQ", None).await;

    let n = db.serials().quarantine_for_lot_async(lot_id, "supplier recall").await.expect("q");
    assert_eq!(n, 2);
    assert_eq!(status_of(&db, available).await, SerialStatus::Quarantined);
    assert_eq!(status_of(&db, reserved).await, SerialStatus::Quarantined);
    assert_eq!(status_of(&db, sold).await, SerialStatus::Sold);
    assert_eq!(status_of(&db, other).await, SerialStatus::Available);
    assert!(
        db.serials().get_reservation_async(res.id).await.unwrap().unwrap().released_at.is_some()
    );
    assert!(db.serials().release_reservation_async(res.id).await.is_err());

    let released = db.serials().release_quarantine_for_lot_async(lot_id).await.expect("release");
    assert_eq!(released, 2);
    assert_eq!(status_of(&db, available).await, SerialStatus::Available);
    assert_eq!(status_of(&db, reserved).await, SerialStatus::Available);
    assert_eq!(db.serials().release_quarantine_for_lot_async(lot_id).await.expect("none"), 0);
}
