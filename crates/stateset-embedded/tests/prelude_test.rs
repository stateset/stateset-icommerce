#![cfg(feature = "sqlite")]

//! Locks in the contents of `stateset_embedded::prelude`.
//!
//! If a future refactor moves or renames one of the curated types, this
//! test fails at compile time. That's the signal we've broken the
//! stable surface and need to either restore the old name or cut a
//! major version bump — the whole point of having a prelude.

use stateset_embedded::prelude::*;

// --- engine + accessors are reachable from the prelude ----------------

#[test]
fn engine_accessors_are_reachable() {
    // In-memory SQLite is enough to exercise the accessors without
    // touching the filesystem. Every accessor below must be namable
    // via the prelude — the compile checks most of it; the value-level
    // calls confirm the methods still exist.
    let commerce = Commerce::new(":memory:").expect("in-memory commerce");
    let _ = commerce.customers();
    let _ = commerce.products();
    let _ = commerce.inventory();
    let _ = commerce.carts();
    let _ = commerce.orders();
    let _ = commerce.payments();
    let _ = commerce.shipments();
    let _ = commerce.returns();
    let _ = commerce.promotions();
    let _ = commerce.tax();
    let _ = CommerceBuilder::default();
}

// --- aggregates + create types can be *named* from the prelude --------
//
// We don't construct the aggregates (their shapes are domain-specific
// and change frequently) — we just prove the names resolve. A broken
// rename surfaces as E0412 at compile time.

#[allow(dead_code)]
struct PreludeTypes {
    _c: Option<Customer>,
    _p: Option<Product>,
    _i: Option<InventoryItem>,
    _cart: Option<Cart>,
    _o: Option<Order>,
    _pay: Option<Payment>,
    _s: Option<Shipment>,
    _r: Option<Return>,
    _a: Option<Address>,
    _m: Option<Money>,
    _cc: Option<CreateCustomer>,
    _cp: Option<CreateProduct>,
    _ci: Option<CreateInventoryItem>,
    _co: Option<CreateOrder>,
    _coi: Option<CreateOrderItem>,
}

// --- default-constructible create types stay default-constructible ---
//
// Embedders rely on the `..Default::default()` pattern from the README.
// If a future refactor drops `Default` from any of these, README
// examples break — which is a user-visible regression we want to catch
// here, not in a GitHub issue.

#[test]
fn create_types_implement_default() {
    let _ = CreateCustomer::default();
    let _ = CreateProduct::default();
    let _ = CreateInventoryItem::default();
    let _ = CreateOrder::default();
    let _ = CreateOrderItem::default();
}
