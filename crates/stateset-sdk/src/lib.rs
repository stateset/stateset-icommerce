#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet SDK
//!
//! Unified facade for the StateSet iCommerce platform. This crate re-exports
//! all StateSet crates through a single dependency with feature-gated includes.
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! stateset-sdk = "0.7.5"
//! ```
//!
//! ```rust,ignore
//! use stateset::prelude::*;
//!
//! let commerce = Commerce::new("store.db")?;
//! let customer = commerce.customers().create(CreateCustomer {
//!     email: "alice@example.com".into(),
//!     first_name: "Alice".into(),
//!     last_name: "Smith".into(),
//!     ..Default::default()
//! })?;
//! ```
//!
//! ## Features
//!
//! | Feature | Description | Default |
//! |---------|-------------|---------|
//! | `core` | Primitives + Core + DB + Embedded + Observability | Yes |
//! | `crypto` | VES v1.0 cryptographic operations | No |
//! | `policy` | Declarative policy engine | No |
//! | `macros` | Proc macros (StateSetId, GenerateDto, JsonSchema) | No |
//! | `full` | All features enabled | No |

// ── Core (always available) ──────────────────────────────────────────

/// Primitive types: Money, `CurrencyCode`, Sku, typed IDs.
pub use stateset_primitives as primitives;

/// Domain models, errors, events, validation, and repository traits.
pub use stateset_core as core;

/// Database abstraction layer (SQLite, PostgreSQL).
pub use stateset_db as db;

/// High-level Commerce API wrapping core + db.
pub use stateset_embedded as embedded;

/// Metrics and observability.
pub use stateset_observability as observability;

// ── Feature-gated re-exports ─────────────────────────────────────────

/// VES v1.0 cryptographic operations (JCS, hashing, signing, encryption, Merkle trees).
#[cfg(feature = "crypto")]
pub use stateset_crypto as crypto;

/// Declarative policy engine with deny-overrides and explainable denials.
#[cfg(feature = "policy")]
pub use stateset_policy as policy;

/// Procedural macros: `#[derive(StateSetId)]`, `#[derive(GenerateDto)]`, `#[derive(JsonSchema)]`.
#[cfg(feature = "macros")]
pub use stateset_macros as macros;

// ── Prelude ──────────────────────────────────────────────────────────

/// Commonly used types, re-exported for convenience.
///
/// ```rust,ignore
/// use stateset::prelude::*;
/// ```
pub mod prelude {
    // Commerce API
    pub use stateset_embedded::Commerce;

    // Primitive types
    pub use stateset_primitives::{CurrencyCode, Money, Sku};

    // Typed IDs
    pub use stateset_core::{
        CustomerId, FulfillmentId, OrderId, OrderItemId, PaymentId, ProductId, ReturnId, ShipmentId,
    };

    // Common models
    pub use stateset_core::{
        // Cart
        AddCartItem,
        // Address
        Address,
        // Inventory
        AdjustInventory,
        Cart,
        CreateCart,
        // Customer
        CreateCustomer,
        CreateInventoryItem,
        // Order
        CreateOrder,
        CreateOrderItem,
        // Payment
        CreatePayment,
        // Product
        CreateProduct,
        // Return
        CreateReturn,
        // Shipment
        CreateShipment,
        Customer,
        CustomerFilter,
        CustomerStatus,
        InventoryItem,
        Order,
        OrderFilter,
        OrderItem,
        OrderStatus,
        Payment,
        PaymentFilter,
        PaymentStatus,
        Product,
        ProductFilter,
        ProductStatus,
        Return,
        ReturnFilter,
        ReturnStatus,
        Shipment,
    };

    // Events
    pub use stateset_core::CommerceEvent;

    // Errors
    pub use stateset_core::CommerceError;

    // Database
    pub use stateset_db::Database;

    // Observability
    pub use stateset_observability::{Metrics, MetricsConfig};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prelude_imports_compile() {
        // Verify key types are accessible through the prelude
        use crate::prelude::*;
        let _: Money = Money::zero(CurrencyCode::USD);
        let _: OrderId = OrderId::new();
    }

    #[test]
    fn core_reexport_accessible() {
        let _ = core::CommerceError::NotFound;
    }

    #[test]
    fn primitives_reexport_accessible() {
        let _id = primitives::OrderId::new();
    }

    #[test]
    fn db_reexport_accessible() {
        // DatabaseConfig is available through db re-export
        let _cfg = db::DatabaseConfig::in_memory();
    }

    #[test]
    fn observability_reexport_accessible() {
        let _cfg = observability::MetricsConfig::default();
    }
}
