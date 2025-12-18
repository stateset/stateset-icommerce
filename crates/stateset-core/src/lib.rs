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

pub mod errors;
pub mod events;
pub mod models;
pub mod traits;

pub use errors::*;
pub use events::*;
pub use models::*;
pub use traits::*;

/// Re-export common types for convenience
pub mod prelude {
    pub use crate::errors::*;
    pub use crate::events::*;
    pub use crate::models::*;
    pub use crate::traits::*;
}
