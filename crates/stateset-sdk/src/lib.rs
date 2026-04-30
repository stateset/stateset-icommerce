#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet SDK
//!
//! Rust-facing facade for the StateSet iCommerce platform.
//!
//! This crate provides a curated set of feature-gated re-exports for Rust
//! consumers who want a single dependency for the core commerce engine and
//! optional sync, crypto, policy, and macro surfaces.
//!
//! ## Quick Start
//!
//! ```toml
//! [dependencies]
//! stateset-sdk = "1.0.1"
//! ```
//!
//! ```rust,ignore
//! use stateset_sdk::prelude::*;
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
//! | `macros` | Proc macros (`StateSetId`, `GenerateDto`, `JsonSchema`) | No |
//! | `sync` | Outbox-driven sync engine and sequencer transport facade | No |
//! | `full` | All features enabled | No |
//!
//! With `sync` enabled, [`SyncRuntime`] and [`SyncRuntimeConfig`] bundle the
//! sync engine, sequencer HTTP transport, and runtime auth/options into a
//! single Rust-facing helper surface for config loading, sync operations,
//! JSON-ready runtime snapshots, kernel receipt queries, and local
//! confirmation/dead-letter inspection. [`SyncRuntimeConfig`] can also load
//! from JSON or environment variables via `from_file`, `from_json_str`, and
//! `from_env`.

// ── Core (always available) ──────────────────────────────────────────

/// Primitive types: Money, `CurrencyCode`, Sku, typed IDs.
#[cfg(feature = "core")]
pub use stateset_primitives as primitives;

/// Domain models, errors, events, validation, and repository traits.
#[cfg(feature = "core")]
pub use stateset_core as core;

/// Database abstraction layer (SQLite, PostgreSQL).
#[cfg(feature = "core")]
pub use stateset_db as db;

/// High-level Commerce API wrapping core + db.
#[cfg(feature = "core")]
pub use stateset_embedded as embedded;

/// Metrics and observability.
#[cfg(feature = "core")]
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

/// Outbox-driven sync engine, transport abstraction, and sequencer HTTP client.
#[cfg(feature = "sync")]
pub use stateset_sync as sync;

#[cfg(feature = "sync")]
pub mod sync_runtime;

#[cfg(feature = "sync")]
pub use sync_runtime::{SyncRuntime, SyncRuntimeAuth, SyncRuntimeConfig, SyncRuntimeSnapshot};

// ── Prelude ──────────────────────────────────────────────────────────

/// Commonly used types, re-exported for convenience.
///
/// ```rust,ignore
/// use stateset_sdk::prelude::*;
/// ```
#[cfg(feature = "core")]
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

    // Errors and Result
    pub use stateset_core::{CommerceError, Result};

    // Database
    pub use stateset_db::Database;

    // Observability
    pub use stateset_observability::{Metrics, MetricsConfig};

    // Sync
    #[cfg(feature = "sync")]
    pub use crate::{SyncRuntime, SyncRuntimeAuth, SyncRuntimeConfig, SyncRuntimeSnapshot};

    #[cfg(feature = "sync")]
    pub use stateset_sync::{
        AttestationError, BudgetAuthorization, BudgetCheckpoint, CommandAttestation,
        CommandConvergence, CommandInclusionProof, CommitmentManifest, CommitmentTrustPolicy,
        CounterpartyCommitment, CounterpartyConvergenceStatus, DeadLetter, KernelExecutionError,
        KernelMetadata, KernelReceipt, KernelReceiptStatus, KernelTransaction,
        ManifestVerificationError, PolicyCheckpoint, PolicyDecision, PullResult, PushConfirmation,
        PushResult, RemoteHead, SequenceAuthority, SequencerHttpTransport, SyncConfig, SyncEngine,
        SyncError, SyncEvent, SyncStatus, VerifiedCommitmentManifest,
        compute_commitment_manifest_hash, sign_commitment_manifest, verify_commitment_manifest,
        verify_commitment_manifest_against_state,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "core")]
    #[test]
    fn prelude_imports_compile() {
        // Verify key types are accessible through the prelude
        use crate::prelude::*;
        let _: Money = Money::zero(CurrencyCode::USD);
        let _: OrderId = OrderId::new();

        #[cfg(feature = "sync")]
        {
            let _: SyncConfig = SyncConfig::new("agent-1", "tenant-1", "store-1");
            let _: SequenceAuthority = SequenceAuthority::LocalOutbox;
            let runtime = SyncRuntimeConfig::new(
                "https://sequencer.stateset.com",
                SyncConfig::new("agent-1", "tenant-1", "store-1"),
            )
            .with_agent_key_id(5)
            .build()
            .unwrap();
            let _: SyncRuntimeSnapshot = runtime.snapshot();
            assert_eq!(runtime.transport().agent_id(), "agent-1");
            assert_eq!(runtime.transport().agent_key_id(), 5);
        }
    }

    #[cfg(feature = "core")]
    #[test]
    fn core_reexport_accessible() {
        let _ = core::CommerceError::NotFound;
    }

    #[cfg(feature = "core")]
    #[test]
    fn primitives_reexport_accessible() {
        let _id = primitives::OrderId::new();
    }

    #[cfg(feature = "core")]
    #[test]
    fn db_reexport_accessible() {
        // DatabaseConfig is available through db re-export
        let _cfg = db::DatabaseConfig::in_memory();
    }

    #[cfg(feature = "core")]
    #[test]
    fn observability_reexport_accessible() {
        let _cfg = observability::MetricsConfig::default();
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn crypto_reexport_accessible() {
        let _ = crypto::ZERO_HASH;
    }

    #[cfg(feature = "sync")]
    #[test]
    fn sync_reexport_accessible() {
        let runtime = SyncRuntimeConfig::new(
            "https://sequencer.stateset.com",
            sync::SyncConfig::new("agent-1", "tenant-1", "store-1"),
        )
        .with_api_key("ss_example_key")
        .with_agent_key_id(11)
        .build()
        .unwrap();
        assert_eq!(runtime.transport().agent_key_id(), 11);
    }

    #[cfg(feature = "policy")]
    #[test]
    fn policy_reexport_accessible() {
        let _ = policy::PolicyEngine::new();
    }

    #[cfg(feature = "macros")]
    #[test]
    fn macros_reexport_accessible() {
        #[allow(unused_imports)]
        use crate::macros as _;
    }
}
