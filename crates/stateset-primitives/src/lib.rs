#![deny(unsafe_code)]
#![cfg_attr(all(not(test), not(feature = "std")), no_std)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet Primitives
//!
//! Strongly-typed primitive types for the StateSet iCommerce platform.
//!
//! This crate provides newtype wrappers that prevent accidental misuse of raw types
//! like `Uuid` and `String` across domain boundaries. Inspired by the type-safety
//! patterns in [alloy](https://github.com/alloy-rs/alloy) and
//! [reth](https://github.com/paradigmxyz/reth).
//!
//! ## Why Newtypes?
//!
//! Without newtypes, it's easy to accidentally pass a `CustomerId` where an `OrderId`
//! is expected — both are `Uuid` under the hood. Newtypes catch these bugs at compile
//! time.
//!
//! ```rust
//! use stateset_primitives::{OrderId, CustomerId};
//!
//! fn process_order(order_id: OrderId, customer_id: CustomerId) {
//!     // Can't accidentally swap these — the compiler will catch it!
//! }
//! ```
//!
//! ## Types
//!
//! ### Entity IDs
//!
//! All entity identifiers are `Copy` + `Eq` + `Hash` newtypes around `Uuid`:
//!
//! - [`OrderId`], [`CustomerId`], [`ProductId`], [`InvoiceId`]
//! - [`ShipmentId`], [`ReturnId`], [`WarehouseId`], [`PaymentId`]
//! - [`InventoryItemId`], [`SubscriptionId`], [`CartId`], [`FulfillmentId`]
//!
//! ### Value Types
//!
//! - [`Sku`] — validated product SKU string
//! - [`Money`] — amount + currency, preventing currency mismatches
//! - [`CurrencyCode`] — ISO 4217 3-letter currency code
//!
//! ## Feature Flags
//!
//! - `std` (default) — standard library support

extern crate alloc;

mod id;
mod money;
mod sku;

pub use id::*;
pub use money::*;
pub use sku::*;

/// Compiles the code examples in `README.md` as doctests, so the crates.io
/// landing page can never drift from the real API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
