//! Error types for commerce operations

use thiserror::Error;
use uuid::Uuid;

/// Main error type for commerce operations
#[derive(Error, Debug)]
pub enum CommerceError {
    // Order errors
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),

    #[error("Order cannot be cancelled in status: {0}")]
    OrderCannotBeCancelled(String),

    #[error("Order cannot be refunded: {0}")]
    OrderCannotBeRefunded(String),

    #[error("Invalid order status transition from {from} to {to}")]
    InvalidOrderStatusTransition { from: String, to: String },

    // Inventory errors
    #[error("Inventory item not found: {0}")]
    InventoryItemNotFound(String),

    #[error("Insufficient stock for SKU {sku}: requested {requested}, available {available}")]
    InsufficientStock {
        sku: String,
        requested: String,
        available: String,
    },

    #[error("Inventory reservation not found: {0}")]
    ReservationNotFound(Uuid),

    #[error("Inventory reservation expired: {0}")]
    ReservationExpired(Uuid),

    #[error("Duplicate SKU: {0}")]
    DuplicateSku(String),

    // Customer errors
    #[error("Customer not found: {0}")]
    CustomerNotFound(Uuid),

    #[error("Email already exists: {0}")]
    EmailAlreadyExists(String),

    #[error("Customer is not active")]
    CustomerNotActive,

    // Product errors
    #[error("Product not found: {0}")]
    ProductNotFound(Uuid),

    #[error("Product variant not found: {0}")]
    ProductVariantNotFound(Uuid),

    #[error("Duplicate product slug: {0}")]
    DuplicateSlug(String),

    #[error("Product is not purchasable")]
    ProductNotPurchasable,

    // Return errors
    #[error("Return not found: {0}")]
    ReturnNotFound(Uuid),

    #[error("Return cannot be approved in status: {0}")]
    ReturnCannotBeApproved(String),

    #[error("Return period expired")]
    ReturnPeriodExpired,

    #[error("Item not eligible for return")]
    ItemNotEligibleForReturn,

    // Validation errors
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid input: {field} - {message}")]
    InvalidInput { field: String, message: String },

    // Database/storage errors
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Record not found")]
    NotFound,

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Optimistic lock failure: record was modified")]
    OptimisticLockFailure,

    #[error("Version conflict on {entity} {id}: expected version {expected_version}")]
    VersionConflict {
        entity: String,
        id: String,
        expected_version: i32,
    },

    // General errors
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Operation not permitted: {0}")]
    NotPermitted(String),
}

/// Result type alias for commerce operations
pub type Result<T> = std::result::Result<T, CommerceError>;

impl CommerceError {
    /// Check if error is a not found error
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            Self::NotFound
                | Self::OrderNotFound(_)
                | Self::CustomerNotFound(_)
                | Self::ProductNotFound(_)
                | Self::ProductVariantNotFound(_)
                | Self::ReturnNotFound(_)
                | Self::InventoryItemNotFound(_)
                | Self::ReservationNotFound(_)
        )
    }

    /// Check if error is a validation error
    pub fn is_validation(&self) -> bool {
        matches!(self, Self::ValidationError(_) | Self::InvalidInput { .. })
    }

    /// Check if error is a conflict error
    pub fn is_conflict(&self) -> bool {
        matches!(
            self,
            Self::Conflict(_)
                | Self::OptimisticLockFailure
                | Self::VersionConflict { .. }
                | Self::DuplicateSku(_)
                | Self::DuplicateSlug(_)
                | Self::EmailAlreadyExists(_)
        )
    }
}
