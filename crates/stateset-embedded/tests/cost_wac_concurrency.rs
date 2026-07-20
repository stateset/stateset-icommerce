//! Concurrency regression for weighted-average-cost updates (SQLite).
//!
//! `update_average_cost` is a read-modify-write of `item_costs.average_cost`
//! (read current avg → compute new weighted average → write). Without
//! serialization two concurrent receipts for the same SKU both read the same
//! `average_cost` and one overwrites the other, corrupting the WAC. Because the
//! receipts here are identical, the correct result is order-independent, so the
//! concurrent final average must equal the sequential (one-at-a-time) result.

#![cfg(feature = "sqlite")]

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{Commerce, CreateInventoryItem, SetItemCost};
use std::sync::{Arc, Barrier};
use std::thread;

/// Create an inventory item with `on_hand` units and seed its cost row at $0
/// (so `average_cost` starts at zero).
fn setup(commerce: &Commerce, sku: &str, on_hand: Decimal) {
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: sku.into(),
            name: "WAC widget".into(),
            initial_quantity: Some(on_hand),
            ..Default::default()
        })
        .expect("create inventory item");
    commerce
        .cost_accounting()
        .set_item_cost(SetItemCost {
            sku: sku.into(),
            standard_cost: Some(dec!(0)),
            ..Default::default()
        })
        .expect("seed item cost");
}

#[test]
fn wac_concurrent_receipts_match_sequential() {
    let n = 10usize;
    let on_hand = dec!(1000);
    let qty = dec!(100);
    let unit_cost = dec!(10);

    // Sequential reference: apply the identical receipt n times, one at a time.
    let seq = Commerce::new(":memory:").expect("commerce");
    setup(&seq, "WAC", on_hand);
    for _ in 0..n {
        seq.cost_accounting().update_average_cost("WAC", qty, unit_cost).expect("seq update");
    }
    let expected =
        seq.cost_accounting().get_item_cost("WAC").expect("get").expect("cost").average_cost;

    // Concurrent run: the same n identical receipts land simultaneously.
    let db = Arc::new(Commerce::new(":memory:").expect("commerce"));
    setup(&db, "WAC", on_hand);
    let barrier = Arc::new(Barrier::new(n));
    let mut handles = Vec::new();
    for _ in 0..n {
        let db = Arc::clone(&db);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            db.cost_accounting().update_average_cost("WAC", qty, unit_cost)
        }));
    }
    let results: Vec<_> = handles.into_iter().map(|h| h.join().expect("thread")).collect();
    // A failure is acceptable only if it is a transient write-lock (the caller
    // retries); a lost update is never acceptable. Count how many committed.
    assert!(
        results.iter().all(|r| {
            r.is_ok() || format!("{:?}", r.as_ref().unwrap_err()).to_lowercase().contains("lock")
        }),
        "only transient lock failures are acceptable: {results:?}"
    );
    let committed = results.iter().filter(|r| r.is_ok()).count();
    assert!(
        committed >= 2,
        "at least two updates must commit to exercise concurrency: {committed}"
    );

    // The concurrent result must equal applying exactly `committed` identical
    // receipts one at a time — i.e. every committed update's contribution is
    // preserved (no lost update). The `expected` from n sequential receipts only
    // matches if all n committed; otherwise recompute the reference for the
    // number that actually committed.
    let expected = if committed == n {
        expected
    } else {
        let seq_ref = Commerce::new(":memory:").expect("commerce");
        setup(&seq_ref, "WAC", on_hand);
        for _ in 0..committed {
            seq_ref.cost_accounting().update_average_cost("WAC", qty, unit_cost).expect("seq");
        }
        seq_ref.cost_accounting().get_item_cost("WAC").expect("get").expect("cost").average_cost
    };

    let got = db.cost_accounting().get_item_cost("WAC").expect("get").expect("cost").average_cost;
    assert_eq!(
        got, expected,
        "concurrent weighted-average of {committed} committed receipts must equal the serialized \
         result (lost update otherwise)"
    );
}

/// `update_average_cost` computes the weighted-average of existing on-hand stock
/// and the incoming receipt. (It previously queried `quantity_on_hand` from
/// `inventory_items`, which has no such column, so it errored on every call.)
#[test]
fn update_average_cost_computes_weighted_average() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    // 100 units on hand valued at $10 each.
    commerce
        .inventory()
        .create_item(CreateInventoryItem {
            sku: "WAC-FN".into(),
            name: "WAC widget".into(),
            initial_quantity: Some(dec!(100)),
            ..Default::default()
        })
        .expect("create inventory item");
    commerce
        .cost_accounting()
        .set_item_cost(SetItemCost {
            sku: "WAC-FN".into(),
            standard_cost: Some(dec!(10)),
            ..Default::default()
        })
        .expect("seed item cost");

    // Receive 100 more units at $20 → WAC = (10·100 + 20·100) / 200 = 15.
    let cost = commerce
        .cost_accounting()
        .update_average_cost("WAC-FN", dec!(100), dec!(20))
        .expect("update average cost");
    assert_eq!(cost.average_cost, dec!(15), "weighted-average cost must blend old and new");
    assert_eq!(cost.last_cost, dec!(20));
}
