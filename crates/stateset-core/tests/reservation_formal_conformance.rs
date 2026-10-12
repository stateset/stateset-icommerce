//! The Lean and TLA+ runners independently emit this same golden table.
//! This connects their stock-holding predicate to the engine; it does not
//! establish equivalence of the entire SQL transaction implementation.

use stateset_core::ReservationStatus;
use std::collections::BTreeSet;

#[test]
fn reservation_holds_stock_matches_lean_and_tla() {
    let golden = include_str!("../../../formal/tla/inventory/holds_stock.golden");
    let mut seen = BTreeSet::new();
    for line in golden.lines() {
        let (name, held) = line.split_once(" -> ").expect("status -> boolean");
        assert!(seen.insert(name), "duplicate status: {name}");
        let status: ReservationStatus = name.parse().expect("a real reservation status");
        assert_eq!(status.to_string(), name, "use the canonical status name");
        assert_eq!(status.holds_stock(), held.parse::<bool>().expect("boolean"), "{name}");
    }
    assert_eq!(
        seen,
        BTreeSet::from([
            "allocated",
            "cancelled",
            "confirmed",
            "expired",
            "fulfilled",
            "pending",
            "released",
        ]),
        "the shared table must cover all seven reservation statuses"
    );
}
