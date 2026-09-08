//! Regression tests for the Postgres inbound-shipment receipt race.
//!
//! `receive_line_async` used to run as four independent autocommit statements
//! on the pool: it read `(quantity_expected, quantity_received)`, checked the
//! over-receipt cap in application code, then wrote an ABSOLUTE
//! `quantity_received = $1`, then re-read the shipment and wrote the derived
//! status. It never looked at the shipment's own status either. So:
//!
//! * two receivers scanning the same ASN line both read the same
//!   `quantity_received`, both passed the cap check and both wrote the same
//!   absolute total — the dock physically took in twice the units and the
//!   system recorded one lot of them;
//! * concurrent partial receipts overwrote each other instead of accumulating,
//!   because the write was absolute rather than an increment;
//! * a receipt could be booked against a *cancelled* shipment, because the head
//!   status was never read at all.
//!
//! The fix mirrors `postgres/transfer_orders.rs`: one transaction, the shipment
//! head locked `FOR UPDATE` (which also serializes receipts against different
//! lines, so the derived status is never computed from a stale snapshot), the
//! line locked `FOR UPDATE`, and an INCREMENT with the cap enforced against the
//! locked value. `cancel_async` takes the same head lock, so a cancel and a
//! receipt cannot both succeed.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateInboundShipment, CreateInboundShipmentItem, InboundShipment,
    InboundShipmentStatus, ProductId,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

/// Create an inbound shipment with one line per quantity in `line_quantities`.
async fn shipment(db: &PostgresDatabase, line_quantities: &[Decimal]) -> InboundShipment {
    db.inbound_shipments()
        .create_async(CreateInboundShipment {
            supplier_id: Uuid::new_v4(),
            purchase_order_id: None,
            warehouse_id: None,
            carrier: Some("DHL".into()),
            tracking_number: Some("1Z-RACE".into()),
            expected_at: None,
            items: line_quantities
                .iter()
                .enumerate()
                .map(|(index, quantity)| CreateInboundShipmentItem {
                    product_id: ProductId::new(),
                    sku: format!("SKU-ASN-{index}"),
                    quantity_expected: *quantity,
                })
                .collect(),
            notes: Some("receipt race".into()),
        })
        .await
        .expect("create inbound shipment")
}

/// Six receivers each scan a full 100-unit receipt against a 100-unit line at
/// the same time. Exactly one may win, and the line must record every unit it
/// accepted.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_full_receipts_do_not_double_count() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let asn = shipment(&db, &[dec!(100)]).await;
    let item_id = asn.items[0].id;

    let receivers = 6usize;
    let barrier = Arc::new(Barrier::new(receivers));
    let mut handles = Vec::with_capacity(receivers);
    for _ in 0..receivers {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let asn_id = asn.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.inbound_shipments().receive_line_async(asn_id, item_id, dec!(100)).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join receive task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "only one full receipt may be accepted against a 100-unit line");

    let stored = db
        .inbound_shipments()
        .get_async(asn.id)
        .await
        .expect("get shipment")
        .expect("shipment row");
    assert_eq!(
        stored.items[0].quantity_received,
        dec!(100) * Decimal::from(successes),
        "recorded receipts must account for every accepted unit"
    );
    assert_eq!(stored.status, InboundShipmentStatus::Received);
}

/// Twenty single-unit scans against a 20-unit line, all issued at once. All
/// twenty are within the cap, so all twenty must be accepted and ACCUMULATE.
/// The absolute write made each scan overwrite the previous one.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_partial_receipts_accumulate() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let expected = 20u32;
    let asn = shipment(&db, &[Decimal::from(expected)]).await;
    let item_id = asn.items[0].id;

    let barrier = Arc::new(Barrier::new(expected as usize));
    let mut handles = Vec::with_capacity(expected as usize);
    for _ in 0..expected {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let asn_id = asn.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.inbound_shipments().receive_line_async(asn_id, item_id, dec!(1)).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join receive task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, expected, "every receipt within the expected quantity must be accepted");

    let stored = db
        .inbound_shipments()
        .get_async(asn.id)
        .await
        .expect("get shipment")
        .expect("shipment row");
    assert_eq!(
        stored.items[0].quantity_received,
        Decimal::from(expected),
        "concurrent partial receipts must accumulate, not overwrite"
    );
    assert_eq!(stored.status, InboundShipmentStatus::Received);
}

/// The over-receipt cap must hold under concurrency: 30 single-unit scans
/// against a 20-unit line accept exactly 20 and reject the rest.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_receipts_respect_the_over_receipt_cap() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let expected = 20u32;
    let receivers = 30u32;
    let asn = shipment(&db, &[Decimal::from(expected)]).await;
    let item_id = asn.items[0].id;

    let barrier = Arc::new(Barrier::new(receivers as usize));
    let mut handles = Vec::with_capacity(receivers as usize);
    for _ in 0..receivers {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let asn_id = asn.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.inbound_shipments().receive_line_async(asn_id, item_id, dec!(1)).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join receive task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, expected, "exactly the expected quantity may be received");

    let stored = db
        .inbound_shipments()
        .get_async(asn.id)
        .await
        .expect("get shipment")
        .expect("shipment row");
    assert_eq!(stored.items[0].quantity_received, Decimal::from(expected));
    assert_eq!(stored.status, InboundShipmentStatus::Received);
}

/// Two lines fully received at the same time. The derived head status is
/// computed from every line, so receipts on DIFFERENT lines must serialize:
/// otherwise each transaction sees the other line still empty and the shipment
/// is left `partially_received` with nothing outstanding.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_receipts_on_distinct_lines_close_the_shipment() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let asn = shipment(&db, &[dec!(5), dec!(5)]).await;

    let barrier = Arc::new(Barrier::new(asn.items.len()));
    let mut handles = Vec::with_capacity(asn.items.len());
    for item in &asn.items {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let asn_id = asn.id;
        let item_id = item.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.inbound_shipments().receive_line_async(asn_id, item_id, dec!(5)).await
        }));
    }
    for handle in handles {
        handle.await.expect("join receive task").expect("receive full line");
    }

    let stored = db
        .inbound_shipments()
        .get_async(asn.id)
        .await
        .expect("get shipment")
        .expect("shipment row");
    assert_eq!(stored.total_received(), dec!(10));
    assert_eq!(
        stored.status,
        InboundShipmentStatus::Received,
        "a shipment with nothing outstanding must close"
    );
    assert!(stored.received_at.is_some());
}

/// A cancelled shipment must not take in stock. `receive_line_async` never read
/// the head status, so units could be booked against a cancelled ASN — the
/// receipt then re-derived the status and quietly reopened it as
/// `partially_received` / `received`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_receiving_against_a_cancelled_shipment_is_refused() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let asn = shipment(&db, &[dec!(5)]).await;
    let item_id = asn.items[0].id;

    let cancelled = db.inbound_shipments().cancel_async(asn.id).await.expect("cancel shipment");
    assert_eq!(cancelled.status, InboundShipmentStatus::Cancelled);

    let error = db
        .inbound_shipments()
        .receive_line_async(asn.id, item_id, dec!(1))
        .await
        .expect_err("a cancelled shipment must refuse receipts");
    assert!(
        matches!(error, CommerceError::ValidationError(_)),
        "expected a validation error, got {error:?}"
    );

    let stored = db
        .inbound_shipments()
        .get_async(asn.id)
        .await
        .expect("get shipment")
        .expect("shipment row");
    assert_eq!(stored.status, InboundShipmentStatus::Cancelled);
    assert_eq!(stored.total_received(), Decimal::ZERO);
}

/// A cancel racing a FULL receipt: exactly one may win, in either order.
///
/// * receipt first — the shipment reaches `received`, a terminal state, and the
///   cancel is refused;
/// * cancel first — the shipment is `cancelled` and the receipt is refused,
///   because units cannot be booked against a cancelled ASN.
///
/// Both halves only hold if each side decides under the head row lock:
/// `cancel_async` read the status outside the write that cancelled, and
/// `receive_line_async` never looked at the status at all, so both could report
/// success and leave a cancelled shipment holding received stock.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_cancel_racing_a_full_receipt_admits_exactly_one() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));

    // Which side commits first is timing-dependent; the invariant must hold for
    // both interleavings, so repeat until both have been observed enough.
    for round in 0..12 {
        let asn = shipment(&db, &[dec!(5)]).await;
        let item_id = asn.items[0].id;
        let barrier = Arc::new(Barrier::new(2));

        let receipt = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let asn_id = asn.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.inbound_shipments().receive_line_async(asn_id, item_id, dec!(5)).await
            })
        };
        let cancel = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let asn_id = asn.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.inbound_shipments().cancel_async(asn_id).await
            })
        };

        let received_ok = receipt.await.expect("join receipt task").is_ok();
        let cancelled_ok = cancel.await.expect("join cancel task").is_ok();
        assert!(
            received_ok ^ cancelled_ok,
            "round {round}: exactly one of the receipt and the cancel may win \
             (receipt={received_ok}, cancel={cancelled_ok})"
        );

        let stored = db
            .inbound_shipments()
            .get_async(asn.id)
            .await
            .expect("get shipment")
            .expect("shipment row");
        if cancelled_ok {
            assert_eq!(stored.status, InboundShipmentStatus::Cancelled);
            assert_eq!(
                stored.total_received(),
                Decimal::ZERO,
                "a cancelled inbound shipment must not hold received stock"
            );
        } else {
            assert_eq!(stored.status, InboundShipmentStatus::Received);
            assert_eq!(stored.total_received(), dec!(5));
        }
    }
}

/// Concurrent cancels: the terminal-state guard only means something if the row
/// is held across the check and the write, so exactly one may win.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_cancels_accept_exactly_one() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let asn = shipment(&db, &[dec!(5)]).await;

    let contenders = 6usize;
    let barrier = Arc::new(Barrier::new(contenders));
    let mut handles = Vec::with_capacity(contenders);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let asn_id = asn.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.inbound_shipments().cancel_async(asn_id).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join cancel task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "the terminal-state guard must admit exactly one cancel");

    let stored = db
        .inbound_shipments()
        .get_async(asn.id)
        .await
        .expect("get shipment")
        .expect("shipment row");
    assert_eq!(stored.status, InboundShipmentStatus::Cancelled);
}

/// `mark_in_transit_async` / `mark_arrived_async` wrote the status with no
/// precondition at all, so `cancel -> mark_arrived -> receive_line` walked
/// straight around the cancelled-shipment refusal above and put stock on a
/// cancelled ASN. Both writes are now guarded on `status <> 'cancelled'`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_status_advances_are_refused_on_a_cancelled_shipment() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let asn = shipment(&db, &[dec!(5)]).await;
    let item_id = asn.items[0].id;
    db.inbound_shipments().cancel_async(asn.id).await.expect("cancel shipment");

    let arrived = db
        .inbound_shipments()
        .mark_arrived_async(asn.id)
        .await
        .expect_err("cannot arrive a cancelled shipment");
    assert!(matches!(arrived, CommerceError::Conflict(_)), "expected Conflict, got {arrived:?}");

    let transit = db
        .inbound_shipments()
        .mark_in_transit_async(asn.id)
        .await
        .expect_err("cannot ship a cancelled shipment");
    assert!(matches!(transit, CommerceError::Conflict(_)), "expected Conflict, got {transit:?}");

    // …and the receipt is still refused, i.e. the bypass is closed.
    db.inbound_shipments()
        .receive_line_async(asn.id, item_id, dec!(1))
        .await
        .expect_err("a cancelled shipment must refuse receipts");

    let stored = db
        .inbound_shipments()
        .get_async(asn.id)
        .await
        .expect("get shipment")
        .expect("shipment row");
    assert_eq!(stored.status, InboundShipmentStatus::Cancelled);
    assert_eq!(stored.total_received(), Decimal::ZERO);
}

/// A successful status advance must return the row it actually wrote.
///
/// `set_status` ran its guarded UPDATE on the pool and then re-read the
/// shipment through `require_full` on a *different* pooled connection, with no
/// transaction spanning the two. A cancel committing in that window made
/// `mark_arrived_async` return `Ok` carrying a **cancelled** shipment: the
/// response contradicted the write it was reporting, and any caller that
/// trusted the returned row (the HTTP layer serialises it straight back) saw a
/// status nobody had asked for. The write and its read now share one
/// transaction, so the returned row is the written row.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_a_successful_status_advance_returns_the_status_it_wrote() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));

    for trial in 0..24 {
        let asn = shipment(&db, &[dec!(5)]).await;
        let barrier = Arc::new(Barrier::new(2));

        let advancing = {
            let (db, barrier, id) = (Arc::clone(&db), Arc::clone(&barrier), asn.id);
            tokio::spawn(async move {
                barrier.wait().await;
                db.inbound_shipments().mark_arrived_async(id).await
            })
        };
        let cancelling = {
            let (db, barrier, id) = (Arc::clone(&db), Arc::clone(&barrier), asn.id);
            tokio::spawn(async move {
                barrier.wait().await;
                db.inbound_shipments().cancel_async(id).await
            })
        };

        let advanced = advancing.await.expect("advance task");
        let cancelled = cancelling.await.expect("cancel task");

        if let Ok(returned) = &advanced {
            assert_eq!(
                returned.status,
                InboundShipmentStatus::Arrived,
                "trial {trial}: a successful advance must return the status it wrote, \
                 not whatever a racing cancel left behind"
            );
        } else {
            assert!(
                matches!(advanced, Err(CommerceError::Conflict(_))),
                "trial {trial}: a refused advance must be a conflict: {advanced:?}"
            );
        }
        if let Ok(returned) = &cancelled {
            assert_eq!(
                returned.status,
                InboundShipmentStatus::Cancelled,
                "trial {trial}: a successful cancel must return the status it wrote"
            );
        }

        // Whatever order they landed in, the stored row is one of the two and
        // never a mixture.
        let stored = db
            .inbound_shipments()
            .get_async(asn.id)
            .await
            .expect("get shipment")
            .expect("shipment row");
        assert!(
            matches!(
                stored.status,
                InboundShipmentStatus::Arrived | InboundShipmentStatus::Cancelled
            ),
            "trial {trial}: unexpected stored status {:?}",
            stored.status
        );
    }
}

/// A guard that never lets the happy path through is an outage: the normal
/// `pending` -> `in_transit` -> `arrived` -> `received` walk must still work.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn postgres_status_advances_still_work_on_a_live_shipment() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let asn = shipment(&db, &[dec!(5)]).await;

    assert_eq!(
        db.inbound_shipments().mark_in_transit_async(asn.id).await.expect("in transit").status,
        InboundShipmentStatus::InTransit
    );
    assert_eq!(
        db.inbound_shipments().mark_arrived_async(asn.id).await.expect("arrived").status,
        InboundShipmentStatus::Arrived
    );
    let received = db
        .inbound_shipments()
        .receive_line_async(asn.id, asn.items[0].id, dec!(5))
        .await
        .expect("receive the line");
    assert_eq!(received.status, InboundShipmentStatus::Received);
    assert_eq!(received.total_received(), dec!(5));
}
