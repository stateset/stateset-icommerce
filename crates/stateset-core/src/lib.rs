//! # StateSet Core
//!
//! Pure domain models and business logic for commerce operations.
//! This crate has no I/O dependencies - just data structures and validation.
//!
//! ## Core Domains
//!
//! - **Orders**: Order management with line items
//! - **Inventory**: Stock tracking, reservations, adjustments
//! - **Customers**: Customer profiles and addresses
//! - **Products**: Product catalog with variants
//! - **Returns**: Return processing and refunds
//! - **Manufacturing**: Bill of Materials (BOM) and Work Orders
//!
//! ## Example
//!
//! ```rust
//! use stateset_core::prelude::*;
//! use rust_decimal_macros::dec;
//!
//! // Create an order input
//! let order = CreateOrder {
//!     customer_id: uuid::Uuid::new_v4(),
//!     items: vec![CreateOrderItem {
//!         sku: "SKU-001".to_string(),
//!         name: "Widget".to_string(),
//!         quantity: 2,
//!         unit_price: dec!(29.99),
//!         ..Default::default()
//!     }],
//!     ..Default::default()
//! };
//! ```

#![warn(missing_docs)]

pub mod errors;
pub mod events;
pub mod models;
pub mod traits;

#[cfg(feature = "embeddings")]
pub mod services;

#[cfg(feature = "metrics")]
pub mod metrics;

pub use errors::*;
pub use events::*;
pub use models::*;
pub use traits::*;

#[cfg(feature = "embeddings")]
pub use services::*;

/// Re-export common types for convenience
pub mod prelude {
    pub use crate::errors::*;
    pub use crate::events::*;
    pub use crate::models::*;
    pub use crate::traits::*;
}
