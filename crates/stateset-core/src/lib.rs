#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet Core
//!
//! Pure domain models and business logic for commerce operations.
//! This crate has no I/O dependencies - just data structures and validation.
//!
//! ## Overview
//!
//! `stateset-core` provides the foundational types for the StateSet iCommerce platform:
//!
//! - **Domain Models**: Strongly-typed structs for all commerce entities
//! - **Repository Traits**: Abstract interfaces for data access
//! - **Error Types**: Comprehensive error handling with categorization
//! - **Validation**: Composable validation builders and traits
//! - **Events**: Domain event types for event-driven architectures
//!
//! ## Core Domains
//!
//! | Domain | Description |
//! |--------|-------------|
//! | **Orders** | Order management with line items, status tracking |
//! | **Inventory** | Stock tracking, reservations, adjustments |
//! | **Customers** | Customer profiles, addresses, contact info |
//! | **Products** | Product catalog with variants, pricing |
//! | **Returns** | Return processing, refunds, RMA |
//! | **Manufacturing** | Bill of Materials (BOM), Work Orders |
//! | **Shipments** | Shipping, tracking, carrier integration |
//! | **Payments** | Payment processing, refunds |
//! | **Subscriptions** | Recurring billing, subscription plans |
//! | **Promotions** | Discounts, coupons, promotional campaigns |
//! | **Tax** | Multi-jurisdiction tax calculation |
//! | **Currency** | Multi-currency support, exchange rates |
//!
//! ## Error Handling
//!
//! All operations return `Result<T, CommerceError>`. Errors can be categorized:
//!
//! ```rust
//! use stateset_core::CommerceError;
//!
//! fn handle_error(err: &CommerceError) {
//!     if err.is_not_found() {
//!         // Handle not found errors (404)
//!     } else if err.is_validation() {
//!         // Handle validation errors (400)
//!     } else if err.is_conflict() {
//!         // Handle conflict errors (409)
//!     } else if err.is_database() {
//!         // Handle database errors (500)
//!     } else if err.is_retryable() {
//!         // Retry the operation
//!     }
//! }
//! ```
//!
//! ## Validation
//!
//! Use `ValidationBuilder` for composable validations:
//!
//! ```rust
//! use stateset_core::{ValidationBuilder, Result};
//!
//! fn validate_order(email: &str, quantity: i32) -> Result<()> {
//!     ValidationBuilder::new()
//!         .email("email", email)
//!         .positive_i32("quantity", quantity)
//!         .build()
//! }
//! ```
//!
//! Or implement the `Validate` trait for domain models:
//!
//! ```rust
//! use stateset_core::{Validate, ValidationBuilder, Result};
//!
//! struct OrderInput {
//!     email: String,
//!     quantity: i32,
//! }
//!
//! impl Validate for OrderInput {
//!     fn validate(&self) -> Result<()> {
//!         ValidationBuilder::new()
//!             .email("email", &self.email)
//!             .positive_i32("quantity", self.quantity)
//!             .build()
//!     }
//! }
//!
//! // Use with method chaining
//! // let input = OrderInput { ... }.validated()?;
//! ```
//!
//! ## Example
//!
//! ```rust
//! use stateset_core::prelude::*;
//! use rust_decimal_macros::dec;
//!
//! // Create an order input
//! let order = CreateOrder {
//!     customer_id: CustomerId::new(),
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
//!
//! ## Feature Flags
//!
//! - `embeddings` - Enable vector search via embedding services
//! - `metrics` - Enable Prometheus metrics support

// This crate has extensive surface area; enforcing `missing_docs` across the whole
// API makes `-D warnings` builds impractical. We keep the option to enable it for
// docs builds instead.
#![cfg_attr(docsrs, warn(missing_docs))]

pub mod errors;
pub mod events;
pub mod models;
pub mod traits;
pub mod validation;

#[cfg(feature = "embeddings")]
pub mod services;

#[cfg(feature = "metrics")]
pub mod metrics;

pub use errors::*;
pub use events::*;
pub use models::*;
pub use traits::*;
pub use validation::*;

#[cfg(feature = "embeddings")]
pub use services::*;

// Re-export strongly-typed primitives so downstream crates can import from
// `stateset_core` directly without depending on `stateset-primitives`.
pub use stateset_primitives::{
    AgentId, CartId, CreditId, CurrencyCode, CustomerId, FraudRuleId, FulfillmentId, GiftCardId,
    GiftCardTransactionId, InventoryItemId, InvoiceId, LoyaltyAccountId, LoyaltyProgramId,
    LoyaltyTransactionId, Money, OrderId, OrderItemId, PaymentId, ProductId, PromotionId,
    PurchaseOrderId, ReturnId, ReviewId, RewardId, SearchConfigId, SegmentId, ShipmentId,
    ShippingMethodId, ShippingZoneId, Sku, StoreCreditId, StoreCreditTransactionId, SubscriptionId,
    WarehouseId, WarrantyId, WishlistId,
};

/// Re-export common types for convenience
pub mod prelude {
    pub use crate::errors::*;
    pub use crate::events::*;
    pub use crate::models::*;
    pub use crate::traits::*;
    pub use crate::validation::*;

    // Typed IDs and value types
    pub use stateset_primitives::{
        AgentId, CartId, CreditId, CurrencyCode, CustomerId, FraudRuleId, FulfillmentId,
        GiftCardId, GiftCardTransactionId, InventoryItemId, InvoiceId, LoyaltyAccountId,
        LoyaltyProgramId, LoyaltyTransactionId, Money, OrderId, OrderItemId, PaymentId, ProductId,
        PromotionId, PurchaseOrderId, ReturnId, ReviewId, RewardId, SearchConfigId, SegmentId,
        ShipmentId, ShippingMethodId, ShippingZoneId, Sku, StoreCreditId, StoreCreditTransactionId,
        SubscriptionId, WarehouseId, WarrantyId, WishlistId,
    };
}
