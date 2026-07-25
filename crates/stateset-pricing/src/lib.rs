//! Deterministic, WASM-compatible pricing engine for StateSet Commerce.
//!
//! This crate provides pure, side-effect-free functions for all pricing
//! calculations: line items, order totals, promotions, currency conversion,
//! tax computation, and monetary rounding.
//!
//! # Design Goals
//!
//! - **Pure functions** — no database, no network, no I/O.
//! - **Deterministic** — same inputs always produce same outputs.
//! - **WASM-compatible** — no system dependencies.
//! - **Configurable rounding** — supports different currency minor units.
//!
//! # Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`line_item`] | Per-line subtotal, discount, tax, and total |
//! | [`order_total`] | Aggregate order totals with shipping and fees |
//! | [`promotions`] | Rule-based promotion evaluation |
//! | [`currency`] | Exchange rates, conversion, triangulation |
//! | [`tax`] | Multi-jurisdiction, compound tax calculation |
//! | [`rounding`] | Configurable rounding policies per currency |
//! | [`error`] | Pricing error types |
//!
//! # Quick Start
//!
//! ```rust
//! use stateset_pricing::{
//!     LineItem, LineDiscount, Fee, OrderTotalInput, RoundingPolicy,
//!     compute_order_total,
//! };
//! use rust_decimal_macros::dec;
//!
//! let input = OrderTotalInput {
//!     items: vec![LineItem {
//!         sku: "WIDGET-001".into(),
//!         name: "Blue Widget".into(),
//!         unit_price: dec!(25.00),
//!         quantity: 4,
//!         discount: Some(LineDiscount::Percentage(dec!(0.10))),
//!         tax_rate: Some(dec!(0.08)),
//!     }],
//!     shipping_cost: dec!(5.99),
//!     shipping_tax_rate: Some(dec!(0.08)),
//!     order_discount: None,
//!     fees: vec![Fee { name: "Handling".into(), amount: dec!(2.00) }],
//!     rounding: RoundingPolicy::usd(),
//! };
//!
//! let total = compute_order_total(&input);
//! assert!(total.grand_total > dec!(0));
//! ```

#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_docs)]

pub mod currency;
pub mod error;
pub mod line_item;
pub mod order_total;
pub mod promotions;
pub mod rounding;
pub mod tax;
mod validation;

// Re-export primary types for convenience.
pub use currency::{ConversionResult, CurrencyConverter, ExchangeRate};
pub use error::{PricingError, PricingResult};
pub use line_item::{LineDiscount, LineItem};
pub use order_total::{
    Fee, OrderTotal, OrderTotalInput, compute_order_total, try_compute_order_total,
};
pub use promotions::{
    AppliedPromotion, Promotion, PromotionContext, PromotionResult, PromotionRule,
    RejectedPromotion, RejectionReason, evaluate_promotions, try_evaluate_promotions,
};
pub use rounding::{RoundingMode, RoundingPolicy, minor_units_for_currency, round};
pub use tax::{
    TaxAppliesTo, TaxContext, TaxLine, TaxResult, TaxRule, TaxableItem, calculate_tax,
    try_calculate_tax,
};

/// Compiles the code examples in `README.md` as doctests, so the crates.io
/// landing page can never drift from the real API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
