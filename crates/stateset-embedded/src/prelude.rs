//! Stable 1.0 surface for direct `stateset-embedded` consumers.
//!
//! This module re-exports the minimum set of types an embedder needs for
//! the core commerce lifecycle: create a customer, put products in
//! inventory, build a cart, place an order, take payment, fulfill, and
//! handle returns. Everything in here is the surface `stateset-embedded`
//! intends to stand behind with a SemVer-stable commitment at 1.0 —
//! breaking changes to anything below require a major-version bump.
//!
//! Most users should prefer `stateset_sdk::prelude` — it's the same
//! commitment served through the facade crate and adds typed IDs,
//! filters, and the error/result types. This module exists for
//! consumers (like `stateset-icp-handler`) that depend on
//! `stateset-embedded` directly without going through the SDK, and the
//! two preludes are kept consistent.
//!
//! The rest of the crate's public API (subscriptions, A2A, loyalty,
//! accounting, analytics, etc.) remains accessible at
//! `stateset_embedded::*` and is useful in production today, but is
//! **experimental** in the sense that its shape may still evolve
//! between minor releases. Promote to the prelude once you'd sign a
//! multi-year support contract on it.
//!
//! ## Usage
//!
//! ```ignore
//! use stateset_embedded::prelude::*;
//! use rust_decimal_macros::dec;
//!
//! let commerce = Commerce::new("./store.db")?;
//!
//! let customer = commerce.customers().create(CreateCustomer {
//!     email: "alice@example.com".into(),
//!     first_name: "Alice".into(),
//!     last_name: "Smith".into(),
//!     ..Default::default()
//! })?;
//!
//! commerce.inventory().create_item(CreateInventoryItem {
//!     sku: "WIDGET-001".into(),
//!     name: "Widget".into(),
//!     initial_quantity: Some(dec!(100)),
//!     ..Default::default()
//! })?;
//! ```
//!
//! ## What's in here
//!
//! * **Engine handle** — [`Commerce`], [`CommerceBuilder`], [`CommerceHealth`].
//! * **Domain accessors** — the zero-arg methods you call on
//!   `commerce` ([`Customers`], [`Products`], [`Inventory`], [`Carts`],
//!   [`Orders`], [`Payments`], [`Shipments`], [`Returns`],
//!   [`Promotions`], [`Tax`]).
//! * **Primitive value types** — [`Money`], [`Address`].
//! * **Aggregates** — one per domain ([`Customer`], [`Product`],
//!   [`InventoryItem`], [`Cart`], [`Order`], [`Payment`],
//!   [`Shipment`], [`Return`]).
//! * **Common request types** — the `CreateXxx` shapes every embedder
//!   uses to drive the buy lifecycle ([`CreateCustomer`],
//!   [`CreateProduct`], [`CreateInventoryItem`], [`CreateOrder`],
//!   [`CreateOrderItem`]).
//!
//! ## What's not in here (and why)
//!
//! * **Subscriptions, A2A, loyalty, gift cards, warranties, segments**
//!   — available at `stateset_embedded::*`, still shape-iterating.
//! * **Accounting/ERP surfaces** (general ledger, journal entries,
//!   accounts payable/receivable, cost accounting) — these are real
//!   and useful, but embedders running a storefront rarely touch them;
//!   keeping them out of the prelude keeps the "SQLite-small" surface
//!   promise honest.
//! * **`update_*` and filter types** — derive-friendly but numerous;
//!   available on each accessor, just not re-exported here to keep the
//!   prelude glance-able.

// --- engine ---------------------------------------------------------------
pub use crate::commerce::{Commerce, CommerceBuilder, CommerceHealth};

// --- domain accessors ---------------------------------------------------
pub use crate::{
    Carts, Customers, Inventory, Orders, Payments, Products, Promotions, Returns, Shipments, Tax,
};

// --- primitives + aggregates from stateset-core -------------------------
//
// These are already re-exported at `stateset_embedded::*` via the big
// `pub use stateset_core::{…}` block in lib.rs. The prelude imports them
// from the facade crate's own root to keep the prelude path short for
// embedders and insulated from any future reorganization of the core
// crate's module tree.
pub use crate::{
    Address, Cart, Customer, InventoryItem, Money, Order, Payment, Product, Return, Shipment,
};

// --- common request types ------------------------------------------------
pub use crate::{CreateCustomer, CreateInventoryItem, CreateOrder, CreateOrderItem, CreateProduct};
