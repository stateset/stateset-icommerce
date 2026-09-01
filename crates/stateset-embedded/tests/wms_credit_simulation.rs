//! Randomized WMS + credit burn-in simulation against the SQLite backend.
//!
//! Drives ~250 random warehouse and credit operations — location-inventory
//! adjustments in both directions, inter-location moves, credit reservations,
//! reservation releases and charges — and asserts after EVERY operation:
//!
//!   * every location/SKU cell matches an independently tracked shadow model
//!     (this is what catches a lost update: an adjustment that the engine
//!     accepts but silently drops, or a move that credits one side only),
//!   * `quantity_on_hand >= 0`, `quantity_reserved >= 0`, and
//!     `quantity_available == quantity_on_hand - quantity_reserved`,
//!   * every credit account satisfies the engine's own documented invariant
//!     `available_credit == credit_limit - current_balance - hold_amount`,
//!     with balance and holds non-negative and never exceeding the limit.
//!
//! Moves are quantity-conserving, so the sum of on-hand across every cell must
//! always equal the net of all accepted adjustments — checked at the end as a
//! global conservation law.
//!
//! Operations that legitimately hit an engine guard (an adjustment that would
//! drive a cell negative, a move with insufficient stock, a charge beyond the
//! credit limit) are tolerated and counted, not failed.
//!
//! Reproducibility: the operation stream is driven by a seeded deterministic
//! PRNG. Override the seed with the `WMS_SIM_SEED` env var (u64) to explore
//! other trajectories; the default is fixed so CI runs are stable.

// Uses the sync `Commerce` engine, which only exists with the sqlite backend.
#![cfg(feature = "sqlite")]

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    AdjustLocationInventory, Commerce, CommerceError, CreateCreditAccount, CreateCustomer,
    CreateLocation, CreateWarehouse, CustomerId, LocationType, MoveInventory, OrderId,
    WarehouseType,
};

const DEFAULT_SEED: u64 = 0x5EED_11A5_2026_0901;
const OPERATIONS: usize = 250;
const SKUS: [&str; 3] = ["SIM-WIDGET", "SIM-GADGET", "SIM-DOODAD"];
const CREDIT_LIMIT: Decimal = dec!(1000);

/// Deterministic splitmix64-style PRNG — no external dependency, fully
/// reproducible from the seed (same generator as `ap_ar_simulation.rs`).
struct Rng(u64);

impl Rng {
    const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `[0, bound)`.
    const fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }

    /// Uniform value in `[lo, hi]` (inclusive).
    const fn between(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }
}

/// A guard rejection is an expected outcome, not a failure; anything else is.
fn tolerate(label: &str, result: Result<(), CommerceError>, rejected: &mut usize) {
    match result {
        Ok(()) => {}
        Err(CommerceError::ValidationError(_) | CommerceError::Conflict(_)) => *rejected += 1,
        Err(CommerceError::NotFound) => *rejected += 1,
        Err(other) => panic!("{label} failed with an unexpected error: {other:?}"),
    }
}

struct Sim {
    commerce: Commerce,
    locations: Vec<i32>,
    customers: Vec<CustomerId>,
    /// Expected on-hand per (`location_id`, sku).
    shadow: BTreeMap<(i32, String), Decimal>,
    /// Net of every accepted adjustment; moves conserve quantity.
    net_adjusted: Decimal,
}

impl Sim {
    fn new() -> Self {
        let commerce = Commerce::new(":memory:").expect("create in-memory Commerce");

        let warehouse = commerce
            .warehouse()
            .create_warehouse(CreateWarehouse {
                code: "WH-SIM".into(),
                name: "Simulation Warehouse".into(),
                warehouse_type: WarehouseType::Distribution,
                ..Default::default()
            })
            .expect("create warehouse");

        let locations = (0..3)
            .map(|i| {
                commerce
                    .warehouse()
                    .create_location(CreateLocation {
                        warehouse_id: warehouse.id,
                        location_type: LocationType::Pick,
                        zone: Some("A".into()),
                        aisle: Some(format!("{i:02}")),
                        rack: Some("01".into()),
                        bin: Some("01".into()),
                        ..Default::default()
                    })
                    .expect("create location")
                    .id
            })
            .collect::<Vec<_>>();

        let customers = (0..2)
            .map(|i| {
                let customer_id = commerce
                    .customers()
                    .create(CreateCustomer {
                        email: format!("wms-sim-{i}-{}@example.com", uuid::Uuid::new_v4()),
                        first_name: "Sim".into(),
                        last_name: "Customer".into(),
                        ..Default::default()
                    })
                    .expect("create customer")
                    .id;
                commerce
                    .credit()
                    .create_credit_account(CreateCreditAccount {
                        customer_id,
                        credit_limit: CREDIT_LIMIT,
                        currency: None,
                        payment_terms: None,
                        risk_rating: None,
                        notes: None,
                    })
                    .expect("create credit account");
                customer_id
            })
            .collect::<Vec<CustomerId>>();

        Self {
            commerce,
            locations,
            customers,
            shadow: BTreeMap::new(),
            net_adjusted: Decimal::ZERO,
        }
    }

    /// Every invariant, re-checked after every single operation.
    fn assert_invariants(&self, after: &str) {
        for &location_id in &self.locations {
            let rows = self
                .commerce
                .warehouse()
                .get_location_inventory(location_id)
                .expect("read location inventory");

            for row in &rows {
                assert!(
                    row.quantity_on_hand >= Decimal::ZERO,
                    "after {after}: location {location_id} sku {} went negative on hand ({})",
                    row.sku,
                    row.quantity_on_hand
                );
                assert!(
                    row.quantity_reserved >= Decimal::ZERO,
                    "after {after}: location {location_id} sku {} has negative reserved ({})",
                    row.sku,
                    row.quantity_reserved
                );
                assert_eq!(
                    row.quantity_available,
                    row.quantity_on_hand - row.quantity_reserved,
                    "after {after}: location {location_id} sku {} available must be on_hand - reserved",
                    row.sku
                );

                let expected = self
                    .shadow
                    .get(&(location_id, row.sku.clone()))
                    .copied()
                    .unwrap_or(Decimal::ZERO);
                assert_eq!(
                    row.quantity_on_hand, expected,
                    "after {after}: location {location_id} sku {} on-hand {} != shadow {expected} \
                     (a lost update or a half-applied move)",
                    row.sku, row.quantity_on_hand
                );
            }

            // A cell the shadow believes is non-zero must actually exist.
            for ((loc, sku), expected) in &self.shadow {
                if *loc == location_id && !expected.is_zero() {
                    assert!(
                        rows.iter().any(|r| &r.sku == sku),
                        "after {after}: location {location_id} sku {sku} expected {expected} but \
                         no inventory row exists"
                    );
                }
            }
        }

        for &customer_id in &self.customers {
            let account = self
                .commerce
                .credit()
                .get_credit_account_by_customer(customer_id)
                .expect("read credit account")
                .expect("credit account exists");

            assert!(
                account.current_balance >= Decimal::ZERO,
                "after {after}: credit balance went negative ({})",
                account.current_balance
            );
            assert!(
                account.hold_amount >= Decimal::ZERO,
                "after {after}: hold amount went negative ({})",
                account.hold_amount
            );
            assert_eq!(
                account.available_credit,
                account.credit_limit - account.current_balance - account.hold_amount,
                "after {after}: available_credit must equal limit - balance - holds"
            );
            assert!(
                account.current_balance + account.hold_amount <= account.credit_limit,
                "after {after}: balance {} + holds {} exceeded the limit {}",
                account.current_balance,
                account.hold_amount,
                account.credit_limit
            );
        }
    }
}

#[test]
fn randomized_wms_credit_simulation_keeps_inventory_and_credit_invariants() {
    let seed = std::env::var("WMS_SIM_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SEED);
    let mut rng = Rng(seed);
    let mut sim = Sim::new();
    let mut rejected = 0usize;

    // A small pool of order ids so reservations and releases can pair up.
    let orders: Vec<OrderId> = (0..4).map(|_| OrderId::new()).collect();

    sim.assert_invariants("setup");

    for op in 0..OPERATIONS {
        let location_id = sim.locations[rng.below(sim.locations.len() as u64) as usize];
        let sku = SKUS[rng.below(SKUS.len() as u64) as usize].to_string();
        let customer_id = sim.customers[rng.below(sim.customers.len() as u64) as usize];
        let order_id = orders[rng.below(orders.len() as u64) as usize];

        match rng.below(14) {
            // Positive adjustment — always legal, always shadowed.
            0..=3 => {
                let qty = Decimal::from(rng.between(1, 20));
                sim.commerce
                    .warehouse()
                    .adjust_inventory(AdjustLocationInventory {
                        location_id,
                        sku: sku.clone(),
                        lot_id: None,
                        quantity: qty,
                        reason: "simulation receipt".into(),
                        reference_type: None,
                        reference_id: None,
                        performed_by: Some("sim".into()),
                    })
                    .expect("positive adjustment must succeed");
                *sim.shadow.entry((location_id, sku)).or_insert(Decimal::ZERO) += qty;
                sim.net_adjusted += qty;
            }
            // Negative adjustment — rejected when it would go below zero.
            4..=6 => {
                let qty = Decimal::from(rng.between(1, 12));
                let result = sim
                    .commerce
                    .warehouse()
                    .adjust_inventory(AdjustLocationInventory {
                        location_id,
                        sku: sku.clone(),
                        lot_id: None,
                        quantity: -qty,
                        reason: "simulation issue".into(),
                        reference_type: None,
                        reference_id: None,
                        performed_by: Some("sim".into()),
                    })
                    .map(|_| ());
                if result.is_ok() {
                    *sim.shadow.entry((location_id, sku)).or_insert(Decimal::ZERO) -= qty;
                    sim.net_adjusted -= qty;
                }
                tolerate("negative adjustment", result, &mut rejected);
            }
            // Move between two distinct locations — quantity conserving.
            7..=8 => {
                let to_location_id = sim.locations[rng.below(sim.locations.len() as u64) as usize];
                if to_location_id == location_id {
                    continue;
                }
                let qty = Decimal::from(rng.between(1, 10));
                let result = sim
                    .commerce
                    .warehouse()
                    .move_inventory(MoveInventory {
                        from_location_id: location_id,
                        to_location_id,
                        sku: sku.clone(),
                        lot_id: None,
                        quantity: qty,
                        reason: Some("simulation move".into()),
                        performed_by: Some("sim".into()),
                    })
                    .map(|_| ());
                if result.is_ok() {
                    *sim.shadow.entry((location_id, sku.clone())).or_insert(Decimal::ZERO) -= qty;
                    *sim.shadow.entry((to_location_id, sku)).or_insert(Decimal::ZERO) += qty;
                }
                tolerate("move inventory", result, &mut rejected);
            }
            // Reserve credit — rejected beyond the limit.
            9..=10 => {
                let amount = Decimal::from(rng.between(10, 400));
                let result =
                    sim.commerce.credit().reserve_credit(customer_id, order_id, amount).map(|_| ());
                tolerate("reserve credit", result, &mut rejected);
            }
            // Release a reservation — NotFound when nothing is reserved.
            11 => {
                let result = sim
                    .commerce
                    .credit()
                    .release_credit_reservation(customer_id, order_id)
                    .map(|_| ());
                tolerate("release reservation", result, &mut rejected);
            }
            // Charge credit — rejected beyond the limit.
            _ => {
                let amount = Decimal::from(rng.between(10, 300));
                let result =
                    sim.commerce.credit().charge_credit(customer_id, order_id, amount).map(|_| ());
                tolerate("charge credit", result, &mut rejected);
            }
        }

        sim.assert_invariants(&format!("operation {op} (seed {seed})"));
    }

    // Global conservation: moves shuffle stock between cells but never create
    // or destroy it, so the total on hand is exactly the net of adjustments.
    let total_on_hand: Decimal = sim
        .locations
        .iter()
        .flat_map(|&location_id| {
            sim.commerce
                .warehouse()
                .get_location_inventory(location_id)
                .expect("read location inventory")
        })
        .map(|row| row.quantity_on_hand)
        .sum();
    assert_eq!(
        total_on_hand, sim.net_adjusted,
        "total on hand must equal the net of every accepted adjustment (seed {seed})"
    );

    // The run must actually exercise the guards, or it proves nothing.
    assert!(
        rejected > 0,
        "expected some operations to hit engine guards (seed {seed}); the simulation is not \
         exercising the money paths"
    );
}
