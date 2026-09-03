//! Regression tests for the Postgres transfer-order receipt race.
//!
//! `receive_line_async` used to run as four independent autocommit statements
//! on the pool: it read `(quantity, quantity_received)`, checked the over-receipt
//! cap in application code, then wrote an ABSOLUTE `quantity_received = $1`,
//! then re-read the order and wrote the derived status. Two clerks scanning
//! receipts against the same line therefore both read the same `quantity_received`,
//! both passed the cap check, and both wrote the same absolute total:
//!
//! * exact case — a 100-unit line, two 100-unit receipts: both succeed, the row
//!   still reads 100, the warehouse physically took in 200, and 100 units are
//!   untracked while the order is closed as `received`;
//! * partial case — a 20-unit line receiving 1 unit at a time: every concurrent
//!   scan writes `1`, so all but one unit silently vanish.
//!
//! Because the write was absolute rather than an increment, neither case
//! self-corrects on a later receipt. The fix locks the order head and the line
//! `FOR UPDATE` inside one transaction and writes an increment. The sibling
//! `ship_async` already used a transaction; SQLite always held the whole
//! sequence in `with_immediate_transaction` (see `sqlite_transfer_order_receipt_race`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateTransferOrder, CreateTransferOrderItem, ProductId, TransferOrder, TransferOrderStatus,
    WarehouseId,
};
use stateset_db::PostgresDatabase;
use std::sync::Arc;
use tokio::sync::Barrier;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

/// Create a transfer order with one line per quantity in `line_quantities`, then
/// ship it so the lines are in transit and ready to receive.
async fn shipped_order(db: &PostgresDatabase, line_quantities: &[Decimal]) -> TransferOrder {
    let order = db
        .transfer_orders()
        .create_async(CreateTransferOrder {
            source_warehouse_id: WarehouseId::new(),
            destination_warehouse_id: WarehouseId::new(),
            items: line_quantities
                .iter()
                .map(|q| CreateTransferOrderItem { product_id: ProductId::new(), quantity: *q })
                .collect(),
            expected_at: None,
            notes: Some("receipt race".into()),
        })
        .await
        .expect("create transfer order");
    db.transfer_orders().ship_async(order.id).await.expect("ship transfer order")
}

/// Two clerks each scan a full 100-unit receipt against a 100-unit line at the
/// same time. Exactly one may win: the line must never record fewer units than
/// were accepted, i.e. `quantity_received == 100 * successes`.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_full_receipts_do_not_double_count() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let order = shipped_order(&db, &[dec!(100)]).await;
    let item_id = order.items[0].id;

    let clerks = 6usize;
    let barrier = Arc::new(Barrier::new(clerks));
    let mut handles = Vec::with_capacity(clerks);
    for _ in 0..clerks {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.transfer_orders().receive_line_async(order_id, item_id, dec!(100)).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join receive task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "only one full receipt may be accepted against a 100-unit line");

    let stored =
        db.transfer_orders().get_async(order.id).await.expect("get order").expect("order row");
    assert_eq!(
        stored.items[0].quantity_received,
        dec!(100) * Decimal::from(successes),
        "recorded receipts must account for every accepted unit"
    );
    assert_eq!(stored.status, TransferOrderStatus::Received);
}

/// Twenty single-unit scans against a 20-unit line, all issued at once. Receipts
/// must ACCUMULATE: all twenty are within the cap, so all twenty must be
/// accepted and the line must end at exactly 20. The absolute write made each
/// scan overwrite the previous one, losing units.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_partial_receipts_accumulate() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let expected = 20u32;
    let order = shipped_order(&db, &[Decimal::from(expected)]).await;
    let item_id = order.items[0].id;

    let barrier = Arc::new(Barrier::new(expected as usize));
    let mut handles = Vec::with_capacity(expected as usize);
    for _ in 0..expected {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.transfer_orders().receive_line_async(order_id, item_id, dec!(1)).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join receive task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, expected, "every receipt within the expected quantity must be accepted");

    let stored =
        db.transfer_orders().get_async(order.id).await.expect("get order").expect("order row");
    assert_eq!(
        stored.items[0].quantity_received,
        Decimal::from(expected),
        "concurrent partial receipts must accumulate, not overwrite"
    );
    assert_eq!(stored.status, TransferOrderStatus::Received);
}

/// The over-receipt cap must still hold under concurrency: 30 single-unit scans
/// against a 20-unit line accept exactly 20 and reject the rest.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_receipts_respect_over_receipt_cap() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let expected = 20u32;
    let clerks = 30u32;
    let order = shipped_order(&db, &[Decimal::from(expected)]).await;
    let item_id = order.items[0].id;

    let barrier = Arc::new(Barrier::new(clerks as usize));
    let mut handles = Vec::with_capacity(clerks as usize);
    for _ in 0..clerks {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.transfer_orders().receive_line_async(order_id, item_id, dec!(1)).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join receive task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, expected, "exactly the expected quantity may be received");

    let stored =
        db.transfer_orders().get_async(order.id).await.expect("get order").expect("order row");
    assert_eq!(stored.items[0].quantity_received, Decimal::from(expected));
    assert_eq!(stored.status, TransferOrderStatus::Received);
}

/// Two lines, each fully received at the same time. The derived order status is
/// computed from every line, so receipts on DIFFERENT lines must serialize too:
/// otherwise each transaction sees the other line still empty and the order is
/// left `partially_received` even though nothing is outstanding.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_receipts_on_distinct_lines_close_the_order() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let order = shipped_order(&db, &[dec!(5), dec!(5)]).await;

    let barrier = Arc::new(Barrier::new(order.items.len()));
    let mut handles = Vec::with_capacity(order.items.len());
    for item in &order.items {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        let item_id = item.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.transfer_orders().receive_line_async(order_id, item_id, dec!(5)).await
        }));
    }
    for handle in handles {
        handle.await.expect("join receive task").expect("receive full line");
    }

    let stored =
        db.transfer_orders().get_async(order.id).await.expect("get order").expect("order row");
    assert_eq!(stored.total_received(), dec!(10));
    assert_eq!(
        stored.status,
        TransferOrderStatus::Received,
        "an order with nothing outstanding must close"
    );
    assert!(stored.received_at.is_some());
}

/// `cancel_async` read the current status, rejected terminal states, then wrote
/// `cancelled` — with nothing holding the row across the two. Two cancels racing
/// therefore both read a live status, both passed the terminal-state guard and
/// both wrote, so an order could be cancelled twice; the same window lets a
/// cancel land on an order that reached a terminal state after the check. The
/// terminal-state guard only means something if the row is held for the whole
/// check-then-act, so exactly one concurrent cancel may win.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_concurrent_cancels_accept_exactly_one() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));
    let order = shipped_order(&db, &[dec!(5)]).await;

    let contenders = 6usize;
    let barrier = Arc::new(Barrier::new(contenders));
    let mut handles = Vec::with_capacity(contenders);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            db.transfer_orders().cancel_async(order_id).await
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.await.expect("join cancel task").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "the terminal-state guard must admit exactly one cancel");

    let stored =
        db.transfer_orders().get_async(order.id).await.expect("get order").expect("order row");
    assert_eq!(stored.status, TransferOrderStatus::Cancelled);
}

/// A cancel racing a FULL receipt: exactly one may win, in either order.
///
/// * receipt first — the order reaches `received`, a terminal state, and the
///   cancel's guard rejects it;
/// * cancel first — the order is `cancelled` and the receipt must be refused,
///   because units cannot be booked in against a cancelled transfer.
///
/// Both halves of that only hold if each side decides under the row lock:
/// `cancel_async` used to read the status outside its write, and
/// `receive_line_async` never looked at the order status at all, so a cancel and
/// a receipt could both report success and leave a cancelled order holding
/// received stock.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn postgres_cancel_racing_a_full_receipt_admits_exactly_one() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = Arc::new(PostgresDatabase::connect(&url).await.expect("connect + migrate"));

    // Repeat: which side commits first is timing-dependent, and the invariant
    // must hold for both interleavings.
    for _ in 0..12 {
        let order = shipped_order(&db, &[dec!(5)]).await;
        let item_id = order.items[0].id;
        let barrier = Arc::new(Barrier::new(2));

        let receipt = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let order_id = order.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.transfer_orders().receive_line_async(order_id, item_id, dec!(5)).await
            })
        };
        let cancel = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let order_id = order.id;
            tokio::spawn(async move {
                barrier.wait().await;
                db.transfer_orders().cancel_async(order_id).await
            })
        };

        let received_ok = receipt.await.expect("join receipt task").is_ok();
        let cancelled_ok = cancel.await.expect("join cancel task").is_ok();
        assert!(
            received_ok ^ cancelled_ok,
            "exactly one of the receipt and the cancel may win (receipt={received_ok}, cancel={cancelled_ok})"
        );

        let stored =
            db.transfer_orders().get_async(order.id).await.expect("get order").expect("order row");
        if cancelled_ok {
            assert_eq!(stored.status, TransferOrderStatus::Cancelled);
            assert_eq!(
                stored.total_received(),
                Decimal::ZERO,
                "a cancelled transfer order must not hold received stock"
            );
        } else {
            assert_eq!(stored.status, TransferOrderStatus::Received);
            assert_eq!(stored.total_received(), dec!(5));
        }
    }
}
