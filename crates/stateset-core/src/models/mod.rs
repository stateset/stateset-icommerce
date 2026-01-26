//! Domain models for commerce operations

pub mod accounts_payable;
pub mod accounts_receivable;
pub mod analytics;
pub mod backorder;
pub mod cost_accounting;
pub mod credit;
pub mod cart;
pub mod currency;
pub mod customer;
pub mod forecasting;
pub mod fulfillment;
pub mod general_ledger;
pub mod inventory;
pub mod invoice;
pub mod lot;
pub mod manufacturing;
pub mod metrics;
pub mod order;
pub mod payment;
pub mod product;
pub mod promotion;
pub mod purchase_order;
pub mod quality;
pub mod receiving;
pub mod returns;
pub mod serial;
pub mod shipment;
pub mod subscription;
pub mod tax;
pub mod warehouse;
pub mod warranty;
pub mod x402;
pub mod vector;

pub use accounts_payable::*;
pub use accounts_receivable::*;
pub use analytics::*;
pub use backorder::*;
pub use cost_accounting::*;
pub use credit::*;
pub use cart::*;
pub use currency::*;
pub use customer::*;
pub use forecasting::*;
pub use fulfillment::*;
pub use general_ledger::*;
pub use inventory::*;
pub use invoice::*;
pub use lot::*;
pub use manufacturing::*;
pub use metrics::*;
pub use order::*;
pub use payment::*;
pub use product::*;
pub use promotion::*;
pub use purchase_order::*;
pub use quality::*;
pub use receiving::*;
pub use returns::*;
pub use serial::*;
pub use shipment::*;
pub use subscription::*;
pub use tax::*;
pub use warehouse::*;
pub use warranty::*;
pub use x402::*;
pub use vector::*;

/// Common ID type alias
pub type Id = uuid::Uuid;

/// Decimal amount type (for backward compatibility)
/// Use `Money` struct from currency module for proper currency handling
pub type Amount = rust_decimal::Decimal;
