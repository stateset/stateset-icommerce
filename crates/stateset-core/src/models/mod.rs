//! Domain models for commerce operations

pub mod analytics;
pub mod cart;
pub mod currency;
pub mod customer;
pub mod inventory;
pub mod invoice;
pub mod manufacturing;
pub mod order;
pub mod payment;
pub mod product;
pub mod purchase_order;
pub mod returns;
pub mod shipment;
pub mod warranty;

pub use analytics::*;
pub use cart::*;
pub use currency::*;
pub use customer::*;
pub use inventory::*;
pub use invoice::*;
pub use manufacturing::*;
pub use order::*;
pub use payment::*;
pub use product::*;
pub use purchase_order::*;
pub use returns::*;
pub use shipment::*;
pub use warranty::*;

/// Common ID type alias
pub type Id = uuid::Uuid;

/// Decimal amount type (for backward compatibility)
/// Use `Money` struct from currency module for proper currency handling
pub type Amount = rust_decimal::Decimal;
