//! SQLite twin of `postgres_transfer_order_receipt_race`.
//!
//! The Postgres `receive_line_async` ran its read / cap-check / write / status
//! update as four separate autocommit statements and wrote an ABSOLUTE
//! `quantity_received`, so concurrent receipts against one line overwrote each
//! other (see that file for the full mechanism). SQLite has always held the whole
//! sequence inside a single `with_immediate_transaction`, which serializes
//! read-modify-write against the same line, so these are regression guards
//! rather than reproducers: they pin the behaviour the Postgres backend now
//! matches, using real OS threads rather than tasks.

#![cfg(feature = "sqlite")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CreateTransferOrder, CreateTransferOrderItem, ProductId, TransferOrder,
    TransferOrderRepository, TransferOrderStatus, WarehouseId,
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

fn shipped_order(db: &SqliteDatabase, line_quantities: &[Decimal]) -> TransferOrder {
    let order = db
        .transfer_orders()
        .create(CreateTransferOrder {
            source_warehouse_id: WarehouseId::new(),
            destination_warehouse_id: WarehouseId::new(),
            items: line_quantities
                .iter()
                .map(|q| CreateTransferOrderItem { product_id: ProductId::new(), quantity: *q })
                .collect(),
            expected_at: None,
            notes: Some("receipt race".into()),
        })
        .expect("create transfer order");
    db.transfer_orders().ship(order.id).expect("ship transfer order")
}

/// Two clerks scanning a full 100-unit receipt against a 100-unit line: exactly
/// one may win, and the line must record every accepted unit.
#[test]
fn sqlite_concurrent_full_receipts_do_not_double_count() {
    let db = test_db();
    let order = shipped_order(&db, &[dec!(100)]);
    let item_id = order.items[0].id;

    let clerks = 6usize;
    let barrier = Arc::new(Barrier::new(clerks));
    let mut handles = Vec::with_capacity(clerks);
    for _ in 0..clerks {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            db.transfer_orders().receive_line(order_id, item_id, dec!(100))
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.join().expect("join receive thread").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "only one full receipt may be accepted against a 100-unit line");

    let stored = db.transfer_orders().get(order.id).expect("get order").expect("order row");
    assert_eq!(
        stored.items[0].quantity_received,
        dec!(100) * Decimal::from(successes),
        "recorded receipts must account for every accepted unit"
    );
    assert_eq!(stored.status, TransferOrderStatus::Received);
}

/// Twenty single-unit scans against a 20-unit line, all issued at once: every
/// one is within the cap, so all must be accepted and accumulate to exactly 20.
#[test]
fn sqlite_concurrent_partial_receipts_accumulate() {
    let db = test_db();
    let expected = 20u32;
    let order = shipped_order(&db, &[Decimal::from(expected)]);
    let item_id = order.items[0].id;

    let barrier = Arc::new(Barrier::new(expected as usize));
    let mut handles = Vec::with_capacity(expected as usize);
    for _ in 0..expected {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            db.transfer_orders().receive_line(order_id, item_id, dec!(1))
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.join().expect("join receive thread").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, expected, "every receipt within the expected quantity must be accepted");

    let stored = db.transfer_orders().get(order.id).expect("get order").expect("order row");
    assert_eq!(
        stored.items[0].quantity_received,
        Decimal::from(expected),
        "concurrent partial receipts must accumulate, not overwrite"
    );
    assert_eq!(stored.status, TransferOrderStatus::Received);
}

/// The over-receipt cap holds under concurrency: 30 single-unit scans against a
/// 20-unit line accept exactly 20 and reject the rest.
#[test]
fn sqlite_concurrent_receipts_respect_over_receipt_cap() {
    let db = test_db();
    let expected = 20u32;
    let clerks = 30u32;
    let order = shipped_order(&db, &[Decimal::from(expected)]);
    let item_id = order.items[0].id;

    let barrier = Arc::new(Barrier::new(clerks as usize));
    let mut handles = Vec::with_capacity(clerks as usize);
    for _ in 0..clerks {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            db.transfer_orders().receive_line(order_id, item_id, dec!(1))
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.join().expect("join receive thread").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, expected, "exactly the expected quantity may be received");

    let stored = db.transfer_orders().get(order.id).expect("get order").expect("order row");
    assert_eq!(stored.items[0].quantity_received, Decimal::from(expected));
    assert_eq!(stored.status, TransferOrderStatus::Received);
}

/// Concurrent receipts on two DIFFERENT lines of one order must still leave the
/// order closed once nothing is outstanding.
#[test]
fn sqlite_concurrent_receipts_on_distinct_lines_close_the_order() {
    let db = test_db();
    let order = shipped_order(&db, &[dec!(5), dec!(5)]);

    let barrier = Arc::new(Barrier::new(order.items.len()));
    let mut handles = Vec::with_capacity(order.items.len());
    for item in &order.items {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        let item_id = item.id;
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            db.transfer_orders().receive_line(order_id, item_id, dec!(5))
        }));
    }
    for handle in handles {
        handle.join().expect("join receive thread").expect("receive full line");
    }

    let stored = db.transfer_orders().get(order.id).expect("get order").expect("order row");
    assert_eq!(stored.total_received(), dec!(10));
    assert_eq!(
        stored.status,
        TransferOrderStatus::Received,
        "an order with nothing outstanding must close"
    );
    assert!(stored.received_at.is_some());
}

/// `cancel` read the current status, rejected terminal states, then wrote
/// `cancelled` on a separate connection. The terminal-state guard only means
/// something if the row is held for the whole check-then-act, so exactly one of
/// several concurrent cancels may win.
#[test]
fn sqlite_concurrent_cancels_accept_exactly_one() {
    let db = test_db();
    let order = shipped_order(&db, &[dec!(5)]);

    let contenders = 16usize;
    let barrier = Arc::new(Barrier::new(contenders));
    let mut handles = Vec::with_capacity(contenders);
    for _ in 0..contenders {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        let order_id = order.id;
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            db.transfer_orders().cancel(order_id)
        }));
    }

    let mut successes = 0u32;
    for handle in handles {
        if handle.join().expect("join cancel thread").is_ok() {
            successes += 1;
        }
    }
    assert_eq!(successes, 1, "the terminal-state guard must admit exactly one cancel");

    let stored = db.transfer_orders().get(order.id).expect("get order").expect("order row");
    assert_eq!(stored.status, TransferOrderStatus::Cancelled);
}

/// A cancel racing a FULL receipt: exactly one may win, in either order. Receipt
/// first leaves the order `received`, which the cancel's terminal-state guard
/// rejects; cancel first leaves it `cancelled`, and units cannot be booked in
/// against a cancelled transfer.
#[test]
fn sqlite_cancel_racing_a_full_receipt_admits_exactly_one() {
    let db = test_db();

    // Repeat: which side commits first is timing-dependent, and the invariant
    // must hold for both interleavings.
    for _ in 0..12 {
        let order = shipped_order(&db, &[dec!(5)]);
        let item_id = order.items[0].id;
        let barrier = Arc::new(Barrier::new(2));

        let receipt = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let order_id = order.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.transfer_orders().receive_line(order_id, item_id, dec!(5))
            })
        };
        let cancel = {
            let db = Arc::clone(&db);
            let barrier = Arc::clone(&barrier);
            let order_id = order.id;
            std::thread::spawn(move || {
                barrier.wait();
                db.transfer_orders().cancel(order_id)
            })
        };

        let received_ok = receipt.join().expect("join receipt thread").is_ok();
        let cancelled_ok = cancel.join().expect("join cancel thread").is_ok();
        assert!(
            received_ok ^ cancelled_ok,
            "exactly one of the receipt and the cancel may win (receipt={received_ok}, cancel={cancelled_ok})"
        );

        let stored = db.transfer_orders().get(order.id).expect("get order").expect("order row");
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
