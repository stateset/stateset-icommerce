//! Error types for commerce operations.
//!
//! This module provides comprehensive error handling for all commerce operations.
//! Errors are organized into a two-level hierarchy:
//!
//! - **[`CommerceError`]** — top-level error that all operations return. It composes
//!   domain-specific sub-errors via `#[from]` conversions.
//! - **Domain errors** — [`OrderError`], [`InventoryError`], [`CustomerError`],
//!   [`ProductError`], [`ReturnError`], [`PaymentError`], [`ShippingError`] —
//!   finer-grained errors for use within domain-specific code.
//!
//! # Using domain errors
//!
//! New code should prefer returning domain-specific errors when possible. They
//! convert into `CommerceError` automatically via `From` impls:
//!
//! ```rust
//! use stateset_core::errors::{OrderError, CommerceError};
//! use uuid::Uuid;
//!
//! fn find_order(id: Uuid) -> Result<(), CommerceError> {
//!     Err(OrderError::not_found(id).into())
//! }
//! ```
//!
//! # Generic state transition errors
//!
//! All state machines share [`StateTransitionError<S>`] for invalid transitions:
//!
//! ```rust
//! use stateset_core::errors::StateTransitionError;
//!
//! #[derive(Debug, Clone, Copy)]
//! enum Light { Red, Green }
//! impl std::fmt::Display for Light {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         write!(f, "{:?}", self)
//!     }
//! }
//!
//! let err = StateTransitionError::new(Light::Red, Light::Green);
//! assert!(err.to_string().contains("Red"));
//! ```
//!
//! # Error categorization
//!
//! ```rust
//! use stateset_core::{CommerceError, Result};
//!
//! fn process_order(id: uuid::Uuid) -> Result<()> {
//!     Err(CommerceError::OrderNotFound(id))
//! }
//!
//! match process_order(uuid::Uuid::new_v4()) {
//!     Err(e) if e.is_not_found() => println!("Not found: {}", e),
//!     Err(e) if e.is_database() => println!("Database error: {}", e),
//!     Err(e) => println!("Other error: {}", e),
//!     Ok(()) => println!("Success"),
//! }
//! ```

// Domain sub-error modules (kept private; types are re-exported below).
mod customer;
mod inventory;
mod order;
mod payment;
mod product;
mod returns;
mod shipping;
pub mod transition;

pub use customer::CustomerError;
pub use inventory::InventoryError;
pub use order::OrderError;
pub use payment::PaymentError;
pub use product::ProductError;
pub use returns::ReturnError;
pub use shipping::ShippingError;
pub use transition::{GotExpected, StateTransitionError};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

// ============================================================================
// Compile-Time Size Assertions (reth pattern)
// ============================================================================

/// Assert at compile time that a type has the expected size in bytes.
///
/// Adapted from the reth project. A size mismatch causes a compile error,
/// preventing accidental error enum bloat. Only active on 64-bit targets.
#[cfg(target_pointer_width = "64")]
macro_rules! static_assert_size {
    ($ty:ty, $size:expr) => {
        const _: [(); $size] = [(); ::std::mem::size_of::<$ty>()];
    };
}

// Pin error enum sizes to detect accidental bloat.
// If you intentionally add large variants, update the expected size here.
#[cfg(target_pointer_width = "64")]
mod _size_assertions {
    use super::{
        CommerceError, CustomerError, DbError, InventoryError, OrderError, PaymentError,
        ProductError, ReturnError, ShippingError,
    };
    static_assert_size!(CommerceError, 80);
    static_assert_size!(DbError, 64);
    static_assert_size!(OrderError, 48);
    static_assert_size!(InventoryError, 72);
    static_assert_size!(CustomerError, 24);
    static_assert_size!(ProductError, 24);
    static_assert_size!(ReturnError, 48);
    static_assert_size!(PaymentError, 56);
    static_assert_size!(ShippingError, 48);
}

// ============================================================================
// Database Error Types (Enhanced)
// ============================================================================

/// Specific database error types with context.
///
/// This enum provides detailed, typed database errors that preserve context
/// about what operation failed and why. Use [`CommerceError::as_db_error()`]
/// to extract the underlying database error for detailed handling.
#[derive(Error, Debug, Clone)]
#[non_exhaustive]
pub enum DbError {
    /// Failed to establish database connection.
    #[error("Connection failed to {url}: {message}")]
    ConnectionFailed {
        /// The database URL or connection string.
        url: String,
        /// Detailed error message.
        message: String,
    },

    /// Query execution failed.
    #[error("Query failed on {table}: {message}")]
    QueryFailed {
        /// The table being queried.
        table: &'static str,
        /// The operation (e.g., "insert", "update", "select").
        operation: &'static str,
        /// Detailed error message.
        message: String,
    },

    /// Constraint violation (unique, foreign key, check).
    #[error("Constraint violation on {table}: {constraint} - {message}")]
    ConstraintViolation {
        /// The table with the constraint.
        table: &'static str,
        /// The constraint name or type.
        constraint: String,
        /// Detailed error message.
        message: String,
    },

    /// Database migration failed.
    #[error("Migration {version} failed: {message}")]
    MigrationFailed {
        /// The migration version that failed.
        version: i32,
        /// Detailed error message.
        message: String,
    },

    /// Transaction error.
    #[error("Transaction failed: {message}")]
    TransactionFailed {
        /// Detailed error message.
        message: String,
    },

    /// Connection pool exhausted.
    #[error("Connection pool exhausted after {timeout_ms}ms")]
    PoolExhausted {
        /// The timeout in milliseconds before giving up.
        timeout_ms: u64,
    },

    /// Serialization/deserialization error.
    #[error("Serialization error for {field}: {message}")]
    SerializationError {
        /// The field being serialized/deserialized.
        field: String,
        /// Detailed error message.
        message: String,
    },

    /// Generic database error (fallback).
    #[error("Database error: {0}")]
    Other(String),
}

impl DbError {
    /// Create a query failed error.
    #[track_caller]
    pub fn query_failed(
        table: &'static str,
        operation: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::QueryFailed { table, operation, message: message.into() }
    }

    /// Create a constraint violation error.
    #[track_caller]
    pub fn constraint_violation(
        table: &'static str,
        constraint: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::ConstraintViolation { table, constraint: constraint.into(), message: message.into() }
    }

    /// Create a connection failed error.
    #[track_caller]
    pub fn connection_failed(url: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ConnectionFailed { url: url.into(), message: message.into() }
    }

    /// Create a transaction failed error.
    #[track_caller]
    pub fn transaction_failed(message: impl Into<String>) -> Self {
        Self::TransactionFailed { message: message.into() }
    }

    /// Create a serialization error.
    #[track_caller]
    pub fn serialization_error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SerializationError { field: field.into(), message: message.into() }
    }
}

// ============================================================================
// Main Commerce Error Type
// ============================================================================

/// Main error type for commerce operations.
///
/// This is the top-level error that all public APIs return. Domain-specific
/// sub-errors ([`OrderError`], [`InventoryError`], etc.) convert into this
/// type via `From` impls, so callers can work with a uniform result type
/// while domain code uses precise error types internally.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CommerceError {
    // ========================================================================
    // Order errors
    // ========================================================================
    /// Order with given ID was not found.
    #[error("Order not found: {0}")]
    OrderNotFound(Uuid),

    /// Order cannot be cancelled in its current status.
    #[error("Order cannot be cancelled in status: {0}")]
    OrderCannotBeCancelled(String),

    /// Order cannot be refunded.
    #[error("Order cannot be refunded: {0}")]
    OrderCannotBeRefunded(String),

    /// Invalid status transition for an order.
    #[error("Invalid order status transition from {from} to {to}")]
    InvalidOrderStatusTransition {
        /// The current status.
        from: String,
        /// The requested new status.
        to: String,
    },

    // ========================================================================
    // Inventory errors
    // ========================================================================
    /// Inventory item not found by SKU or ID.
    #[error("Inventory item not found: {0}")]
    InventoryItemNotFound(String),

    /// Insufficient stock for the requested quantity.
    #[error("Insufficient stock for SKU {sku}: requested {requested}, available {available}")]
    InsufficientStock {
        /// The SKU that has insufficient stock.
        sku: String,
        /// The quantity that was requested.
        requested: String,
        /// The quantity that is available.
        available: String,
    },

    /// A shipment line requested more units than remain unshipped on the order line.
    #[error(
        "Shipment exceeds ordered quantity for order item {order_item_id}: requested {requested}, remaining {remaining}"
    )]
    ShipmentExceedsOrdered {
        /// The order line being shipped.
        order_item_id: Uuid,
        /// Units the caller tried to ship.
        requested: i32,
        /// Units still unshipped on the line (`quantity - shipped_quantity`).
        remaining: i32,
    },

    /// Inventory reservation not found.
    #[error("Inventory reservation not found: {0}")]
    ReservationNotFound(Uuid),

    /// Inventory reservation has expired.
    #[error("Inventory reservation expired: {0}")]
    ReservationExpired(Uuid),

    /// Duplicate SKU already exists.
    #[error("Duplicate SKU: {0}")]
    DuplicateSku(String),

    // ========================================================================
    // Customer errors
    // ========================================================================
    /// Customer with given ID was not found.
    #[error("Customer not found: {0}")]
    CustomerNotFound(Uuid),

    /// Email address is already registered.
    #[error("Email already exists: {0}")]
    EmailAlreadyExists(String),

    /// Customer account is not active.
    #[error("Customer is not active")]
    CustomerNotActive,

    // ========================================================================
    // Product errors
    // ========================================================================
    /// Product with given ID was not found.
    #[error("Product not found: {0}")]
    ProductNotFound(Uuid),

    /// Product variant with given ID was not found.
    #[error("Product variant not found: {0}")]
    ProductVariantNotFound(Uuid),

    /// Duplicate product slug already exists.
    #[error("Duplicate product slug: {0}")]
    DuplicateSlug(String),

    /// Product is not available for purchase.
    #[error("Product is not purchasable")]
    ProductNotPurchasable,

    // ========================================================================
    // Return errors
    // ========================================================================
    /// Return with given ID was not found.
    #[error("Return not found: {0}")]
    ReturnNotFound(Uuid),

    /// Return cannot be approved in its current status.
    #[error("Return cannot be approved in status: {0}")]
    ReturnCannotBeApproved(String),

    /// Return period has expired.
    #[error("Return period expired")]
    ReturnPeriodExpired,

    /// Item is not eligible for return.
    #[error("Item not eligible for return")]
    ItemNotEligibleForReturn,

    // ========================================================================
    // Validation errors
    // ========================================================================
    /// General validation error.
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Invalid input for a specific field.
    #[error("Invalid input: {field} - {message}")]
    InvalidInput {
        /// The field that has invalid input.
        field: String,
        /// The validation error message.
        message: String,
    },

    // ========================================================================
    // Database/storage errors
    // ========================================================================
    /// Legacy database error (for backwards compatibility).
    #[error("Database error: {0}")]
    DatabaseError(String),

    /// Typed database error with context.
    #[error(transparent)]
    Database(#[from] DbError),

    /// Generic record not found.
    #[error("Record not found")]
    NotFound,

    /// Conflict during operation.
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Optimistic locking failure.
    #[error("Optimistic lock failure: record was modified")]
    OptimisticLockFailure,

    /// Version conflict during update.
    #[error("Version conflict on {entity} {id}: expected version {expected_version}")]
    VersionConflict {
        /// The entity type (e.g., "order", "customer").
        entity: String,
        /// The entity ID.
        id: String,
        /// The expected version that was not found.
        expected_version: i32,
    },

    // ========================================================================
    // External service errors
    // ========================================================================
    /// External service (payment, shipping, etc.) failed.
    #[error("External service error: {0}")]
    ExternalServiceError(String),

    // ========================================================================
    // Domain sub-errors (composed via #[from])
    // ========================================================================
    /// Order-domain error.
    #[error(transparent)]
    Order(#[from] OrderError),

    /// Inventory-domain error.
    #[error(transparent)]
    Inventory(#[from] InventoryError),

    /// Customer-domain error.
    #[error(transparent)]
    Customer(#[from] CustomerError),

    /// Product-domain error.
    #[error(transparent)]
    Product(#[from] ProductError),

    /// Return-domain error.
    #[error(transparent)]
    Return(#[from] ReturnError),

    /// Payment-domain error.
    #[error(transparent)]
    Payment(#[from] PaymentError),

    /// Shipping-domain error.
    #[error(transparent)]
    Shipping(#[from] ShippingError),

    // ========================================================================
    // General errors
    // ========================================================================
    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Operation not permitted.
    #[error("Operation not permitted: {0}")]
    NotPermitted(String),
}

/// Result type alias for commerce operations.
pub type Result<T> = std::result::Result<T, CommerceError>;

/// Per-domain result type aliases for focused error handling.
pub mod result {
    /// Result alias for order operations.
    pub type OrderResult<T> = std::result::Result<T, super::OrderError>;
    /// Result alias for inventory operations.
    pub type InventoryResult<T> = std::result::Result<T, super::InventoryError>;
    /// Result alias for customer operations.
    pub type CustomerResult<T> = std::result::Result<T, super::CustomerError>;
    /// Result alias for product operations.
    pub type ProductResult<T> = std::result::Result<T, super::ProductError>;
    /// Result alias for return operations.
    pub type ReturnResult<T> = std::result::Result<T, super::ReturnError>;
    /// Result alias for payment operations.
    pub type PaymentResult<T> = std::result::Result<T, super::PaymentError>;
    /// Result alias for shipping operations.
    pub type ShippingResult<T> = std::result::Result<T, super::ShippingError>;
    /// Result alias for database operations.
    pub type DbResult<T> = std::result::Result<T, super::DbError>;
}

impl CommerceError {
    /// Check if error is a not found error.
    #[must_use]
    pub const fn is_not_found(&self) -> bool {
        match self {
            Self::NotFound
            | Self::OrderNotFound(_)
            | Self::CustomerNotFound(_)
            | Self::ProductNotFound(_)
            | Self::ProductVariantNotFound(_)
            | Self::ReturnNotFound(_)
            | Self::InventoryItemNotFound(_)
            | Self::ReservationNotFound(_) => true,
            // Domain sub-errors
            Self::Order(OrderError::NotFound(_)) => true,
            Self::Inventory(
                InventoryError::ItemNotFound(_) | InventoryError::ReservationNotFound(_),
            ) => true,
            Self::Customer(CustomerError::NotFound(_)) => true,
            Self::Product(ProductError::NotFound(_) | ProductError::VariantNotFound(_)) => true,
            Self::Return(ReturnError::NotFound(_)) => true,
            Self::Payment(PaymentError::NotFound(_)) => true,
            Self::Shipping(ShippingError::NotFound(_)) => true,
            _ => false,
        }
    }

    /// Check if error is a validation error.
    #[must_use]
    pub const fn is_validation(&self) -> bool {
        matches!(self, Self::ValidationError(_) | Self::InvalidInput { .. })
    }

    /// Check if error is a conflict error.
    #[must_use]
    pub const fn is_conflict(&self) -> bool {
        match self {
            Self::Conflict(_)
            | Self::OptimisticLockFailure
            | Self::VersionConflict { .. }
            | Self::DuplicateSku(_)
            | Self::DuplicateSlug(_)
            | Self::EmailAlreadyExists(_) => true,
            // Domain sub-errors
            Self::Inventory(InventoryError::DuplicateSku(_)) => true,
            Self::Customer(CustomerError::EmailAlreadyExists(_)) => true,
            Self::Product(ProductError::DuplicateSlug(_)) => true,
            _ => false,
        }
    }

    /// Check if error is a database error.
    #[must_use]
    pub const fn is_database(&self) -> bool {
        matches!(self, Self::DatabaseError(_) | Self::Database(_))
    }

    /// Check if error is an external service error.
    #[must_use]
    pub const fn is_external_service(&self) -> bool {
        matches!(self, Self::ExternalServiceError(_))
    }

    /// Check if error is retryable.
    ///
    /// Retryable errors include:
    /// - Connection failures
    /// - Pool exhaustion
    /// - Transaction failures (some)
    /// - Optimistic lock failures
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        match self {
            Self::OptimisticLockFailure => true,
            Self::Database(db_err) => matches!(
                db_err,
                DbError::ConnectionFailed { .. }
                    | DbError::PoolExhausted { .. }
                    | DbError::TransactionFailed { .. }
            ),
            _ => false,
        }
    }

    /// Check if error is transient (temporary failures that may resolve on retry).
    ///
    /// This is a superset of [`is_retryable`](Self::is_retryable) — it also includes
    /// external service failures which may recover after a delay.
    #[must_use]
    pub const fn is_transient(&self) -> bool {
        self.is_retryable() || self.is_external_service()
    }

    /// Check if error is a client error (bad input from the caller).
    ///
    /// Client errors include not-found, validation, conflict, and permission errors.
    #[must_use]
    pub const fn is_client_error(&self) -> bool {
        self.is_not_found() || self.is_validation() || self.is_conflict() || self.is_not_permitted()
    }

    /// Check if error is a server error (internal / infrastructure failures).
    ///
    /// Server errors include database errors, internal errors, and external service
    /// failures.
    #[must_use]
    pub const fn is_server_error(&self) -> bool {
        self.is_database() || matches!(self, Self::Internal(_)) || self.is_external_service()
    }

    /// Check if this is a permission-denied error.
    #[must_use]
    pub const fn is_not_permitted(&self) -> bool {
        matches!(self, Self::NotPermitted(_))
    }

    /// Suggest an HTTP status code for this error.
    ///
    /// Useful for API layers that need to map domain errors to HTTP responses.
    ///
    /// # Example
    ///
    /// ```rust
    /// use stateset_core::CommerceError;
    ///
    /// let err = CommerceError::NotFound;
    /// assert_eq!(err.suggested_status_code(), 404);
    ///
    /// let err = CommerceError::ValidationError("bad".into());
    /// assert_eq!(err.suggested_status_code(), 400);
    /// ```
    #[must_use]
    pub const fn suggested_status_code(&self) -> u16 {
        if self.is_not_found() {
            404
        } else if self.is_validation() {
            400
        } else if self.is_conflict() {
            409
        } else if self.is_not_permitted() {
            403
        } else if self.is_external_service() {
            502
        } else {
            500
        }
    }

    /// Get the underlying database error if this is a database error.
    #[must_use]
    pub const fn as_db_error(&self) -> Option<&DbError> {
        match self {
            Self::Database(e) => Some(e),
            _ => None,
        }
    }

    /// Get the underlying order error if this is an order error.
    #[must_use]
    pub const fn as_order_error(&self) -> Option<&OrderError> {
        match self {
            Self::Order(e) => Some(e),
            _ => None,
        }
    }

    /// Get the underlying inventory error if this is an inventory error.
    #[must_use]
    pub const fn as_inventory_error(&self) -> Option<&InventoryError> {
        match self {
            Self::Inventory(e) => Some(e),
            _ => None,
        }
    }

    /// Get the underlying customer error if this is a customer error.
    #[must_use]
    pub const fn as_customer_error(&self) -> Option<&CustomerError> {
        match self {
            Self::Customer(e) => Some(e),
            _ => None,
        }
    }

    /// Get the underlying product error if this is a product error.
    #[must_use]
    pub const fn as_product_error(&self) -> Option<&ProductError> {
        match self {
            Self::Product(e) => Some(e),
            _ => None,
        }
    }

    /// Create a database error from a typed `DbError`.
    #[track_caller]
    #[must_use]
    pub const fn db(error: DbError) -> Self {
        Self::Database(error)
    }

    /// Create a query failed error with context.
    #[track_caller]
    pub fn query_failed(
        table: &'static str,
        operation: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::Database(DbError::query_failed(table, operation, message))
    }

    /// Create a constraint violation error.
    #[track_caller]
    pub fn constraint_violation(
        table: &'static str,
        constraint: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Database(DbError::constraint_violation(table, constraint, message))
    }

    /// Create a connection failed error.
    #[track_caller]
    pub fn connection_failed(url: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Database(DbError::connection_failed(url, message))
    }
}

// ============================================================================
// Batch Operation Types
// ============================================================================

/// Maximum items allowed per batch operation.
pub const MAX_BATCH_SIZE: usize = 1000;

/// Categorized batch error codes for programmatic handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, Serialize, Deserialize)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum BatchErrorCode {
    /// Entity was not found.
    NotFound,
    /// Input validation failed.
    ValidationError,
    /// Duplicate key constraint violation.
    DuplicateKey,
    /// Optimistic locking version conflict.
    VersionConflict,
    /// Database-level error.
    DatabaseError,
    /// Unclassified internal error.
    InternalError,
}

impl From<&CommerceError> for BatchErrorCode {
    fn from(err: &CommerceError) -> Self {
        if err.is_not_found() {
            return Self::NotFound;
        }
        if err.is_validation() {
            return Self::ValidationError;
        }
        if err.is_conflict() {
            return Self::DuplicateKey;
        }

        match err {
            CommerceError::VersionConflict { .. } | CommerceError::OptimisticLockFailure => {
                Self::VersionConflict
            }

            CommerceError::DatabaseError(_) | CommerceError::Database(_) => Self::DatabaseError,

            // Also catch constraint violations from DbError as duplicate key
            _ if matches!(err.as_db_error(), Some(DbError::ConstraintViolation { .. })) => {
                Self::DuplicateKey
            }

            _ => Self::InternalError,
        }
    }
}

/// Error information for a single item in a batch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchError {
    /// Index in the original batch (for create/update operations).
    pub index: usize,
    /// ID of the entity (for update/delete/get operations, if available).
    pub id: Option<String>,
    /// Human-readable error message.
    pub error: String,
    /// Error code for programmatic handling.
    pub code: BatchErrorCode,
}

impl BatchError {
    /// Create a new `BatchError` from an index and `CommerceError`.
    #[must_use]
    pub fn from_error(index: usize, id: Option<String>, err: &CommerceError) -> Self {
        Self { index, id, error: err.to_string(), code: BatchErrorCode::from(err) }
    }
}

/// Result of a batch operation that allows partial success.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult<T> {
    /// Successfully processed items.
    pub succeeded: Vec<T>,
    /// Failed operations with their errors.
    pub failed: Vec<BatchError>,
    /// Total items attempted.
    pub total_attempted: usize,
    /// Count of successful operations.
    pub success_count: usize,
    /// Count of failed operations.
    pub failure_count: usize,
}

impl<T> BatchResult<T> {
    /// Create a new empty `BatchResult`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            succeeded: Vec::new(),
            failed: Vec::new(),
            total_attempted: 0,
            success_count: 0,
            failure_count: 0,
        }
    }

    /// Create a `BatchResult` with pre-allocated capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            succeeded: Vec::with_capacity(capacity),
            failed: Vec::new(),
            total_attempted: 0,
            success_count: 0,
            failure_count: 0,
        }
    }

    /// Record a successful operation.
    pub fn record_success(&mut self, item: T) {
        self.succeeded.push(item);
        self.success_count += 1;
        self.total_attempted += 1;
    }

    /// Record a failed operation.
    pub fn record_failure(&mut self, index: usize, id: Option<String>, err: &CommerceError) {
        self.failed.push(BatchError::from_error(index, id, err));
        self.failure_count += 1;
        self.total_attempted += 1;
    }

    /// Check if all operations succeeded.
    #[must_use]
    pub const fn all_succeeded(&self) -> bool {
        self.failure_count == 0
    }

    /// Check if all operations failed.
    #[must_use]
    pub const fn all_failed(&self) -> bool {
        self.success_count == 0 && self.total_attempted > 0
    }

    /// Check if some operations succeeded and some failed.
    #[must_use]
    pub const fn partial_success(&self) -> bool {
        self.success_count > 0 && self.failure_count > 0
    }

    /// Check if the batch was empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total_attempted == 0
    }
}

impl<T> Default for BatchResult<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate batch size against maximum limit.
pub fn validate_batch_size<T>(items: &[T]) -> Result<()> {
    if items.len() > MAX_BATCH_SIZE {
        return Err(CommerceError::ValidationError(format!(
            "Batch size {} exceeds maximum of {}",
            items.len(),
            MAX_BATCH_SIZE
        )));
    }
    Ok(())
}

/// Validate a required text field for non-empty content and length.
pub fn validate_required_text(field: &str, value: &str, max_len: usize) -> Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CommerceError::InvalidInput {
            field: field.to_string(),
            message: "cannot be empty".into(),
        });
    }

    if trimmed.len() > max_len {
        return Err(CommerceError::InvalidInput {
            field: field.to_string(),
            message: format!("cannot exceed {max_len} characters"),
        });
    }

    Ok(())
}

/// Validate that a UUID is not the nil (all-zero) value.
pub fn validate_required_uuid(field: &str, value: Uuid) -> Result<()> {
    if value.is_nil() {
        return Err(CommerceError::InvalidInput {
            field: field.to_string(),
            message: "cannot be nil".into(),
        });
    }

    Ok(())
}

/// Validate an email address format.
///
/// Performs basic email validation checking for:
/// - Non-empty string
/// - Contains exactly one @ symbol
/// - Has non-empty local and domain parts
/// - Domain contains at least one dot
/// - No whitespace characters
///
/// # Example
///
/// ```
/// use stateset_core::validate_email;
///
/// assert!(validate_email("user@example.com").is_ok());
/// assert!(validate_email("invalid").is_err());
/// assert!(validate_email("").is_err());
/// ```
pub fn validate_email(email: &str) -> Result<()> {
    let email = email.trim();

    if email.is_empty() {
        return Err(CommerceError::ValidationError("Email cannot be empty".into()));
    }

    if email.len() > 254 {
        return Err(CommerceError::ValidationError("Email cannot exceed 254 characters".into()));
    }

    if email.contains(char::is_whitespace) {
        return Err(CommerceError::ValidationError("Email cannot contain whitespace".into()));
    }

    if email.chars().any(char::is_control) {
        return Err(CommerceError::ValidationError(
            "Email cannot contain control characters".into(),
        ));
    }

    if !email.is_ascii() {
        return Err(CommerceError::ValidationError(
            "Email must use ASCII or punycode characters".into(),
        ));
    }

    let Some((local, domain)) = email.rsplit_once('@') else {
        return Err(CommerceError::ValidationError(
            "Email must contain exactly one @ symbol".into(),
        ));
    };
    if local.contains('@') || domain.contains('@') {
        return Err(CommerceError::ValidationError(
            "Email must contain exactly one @ symbol".into(),
        ));
    }

    if local.is_empty() {
        return Err(CommerceError::ValidationError(
            "Email local part (before @) cannot be empty".into(),
        ));
    }

    if domain.is_empty() {
        return Err(CommerceError::ValidationError(
            "Email domain (after @) cannot be empty".into(),
        ));
    }

    if local.len() > 64 {
        return Err(CommerceError::ValidationError(
            "Email local part cannot exceed 64 characters".into(),
        ));
    }

    if domain.len() > 253 {
        return Err(CommerceError::ValidationError(
            "Email domain cannot exceed 253 characters".into(),
        ));
    }

    if local.starts_with('.') || local.ends_with('.') {
        return Err(CommerceError::ValidationError(
            "Email local part cannot start or end with a dot".into(),
        ));
    }

    if local.contains("..") {
        return Err(CommerceError::ValidationError(
            "Email local part cannot contain consecutive dots".into(),
        ));
    }

    if !local.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '!' | '#'
                    | '$'
                    | '%'
                    | '&'
                    | '\''
                    | '*'
                    | '+'
                    | '-'
                    | '/'
                    | '='
                    | '?'
                    | '^'
                    | '_'
                    | '`'
                    | '{'
                    | '|'
                    | '}'
                    | '~'
                    | '.'
            )
    }) {
        return Err(CommerceError::ValidationError(
            "Email local part contains unsupported characters".into(),
        ));
    }

    if !domain.contains('.') {
        return Err(CommerceError::ValidationError(
            "Email domain must contain at least one dot".into(),
        ));
    }

    // Check domain doesn't start or end with a dot
    if domain.starts_with('.') || domain.ends_with('.') {
        return Err(CommerceError::ValidationError(
            "Email domain cannot start or end with a dot".into(),
        ));
    }

    if domain.contains("..") {
        return Err(CommerceError::ValidationError(
            "Email domain cannot contain consecutive dots".into(),
        ));
    }

    for label in domain.split('.') {
        if label.is_empty() {
            return Err(CommerceError::ValidationError(
                "Email domain cannot contain empty labels".into(),
            ));
        }
        if label.len() > 63 {
            return Err(CommerceError::ValidationError(
                "Email domain labels cannot exceed 63 characters".into(),
            ));
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(CommerceError::ValidationError(
                "Email domain labels cannot start or end with a hyphen".into(),
            ));
        }
        if !label.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-') {
            return Err(CommerceError::ValidationError(
                "Email domain contains unsupported characters".into(),
            ));
        }
    }

    Ok(())
}

/// Validate a SKU format.
///
/// SKUs must:
/// - Be non-empty
/// - Be 1-100 characters
/// - Contain only alphanumeric characters, hyphens, and underscores
///
/// # Example
///
/// ```
/// use stateset_core::validate_sku;
///
/// assert!(validate_sku("SKU-001").is_ok());
/// assert!(validate_sku("WIDGET_BLUE_XL").is_ok());
/// assert!(validate_sku("").is_err());
/// assert!(validate_sku("sku with spaces").is_err());
/// ```
pub fn validate_sku(sku: &str) -> Result<()> {
    let sku = sku.trim();

    if sku.is_empty() {
        return Err(CommerceError::ValidationError("SKU cannot be empty".into()));
    }

    if sku.len() > 100 {
        return Err(CommerceError::ValidationError("SKU cannot exceed 100 characters".into()));
    }

    if !sku.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(CommerceError::ValidationError(
            "SKU can only contain alphanumeric characters, hyphens, and underscores".into(),
        ));
    }

    Ok(())
}

/// Validate a phone number format (basic validation).
///
/// This performs basic phone number validation:
/// - Non-empty
/// - Contains only digits, spaces, parentheses, hyphens, and plus sign
/// - Has at least 7 digits (minimum for local numbers)
/// - Has at most 15 digits (ITU-T E.164 standard)
///
/// # Example
///
/// ```
/// use stateset_core::validate_phone;
///
/// assert!(validate_phone("+1 (555) 123-4567").is_ok());
/// assert!(validate_phone("5551234567").is_ok());
/// assert!(validate_phone("123").is_err()); // Too short
/// assert!(validate_phone("").is_err());
/// ```
pub fn validate_phone(phone: &str) -> Result<()> {
    let phone = phone.trim();

    if phone.is_empty() {
        return Err(CommerceError::ValidationError("Phone number cannot be empty".into()));
    }

    // Check for valid characters
    if !phone
        .chars()
        .all(|c| c.is_ascii_digit() || c == ' ' || c == '-' || c == '(' || c == ')' || c == '+')
    {
        return Err(CommerceError::ValidationError(
            "Phone number contains invalid characters".into(),
        ));
    }

    // Count digits
    let digit_count = phone.chars().filter(char::is_ascii_digit).count();

    if digit_count < 7 {
        return Err(CommerceError::ValidationError(
            "Phone number must have at least 7 digits".into(),
        ));
    }

    if digit_count > 15 {
        return Err(CommerceError::ValidationError("Phone number cannot exceed 15 digits".into()));
    }

    Ok(())
}

/// Validate a currency code (ISO 4217 format).
///
/// Currency codes must be exactly 3 uppercase letters.
///
/// # Example
///
/// ```
/// use stateset_core::validate_currency_code;
///
/// assert!(validate_currency_code("USD").is_ok());
/// assert!(validate_currency_code("EUR").is_ok());
/// assert!(validate_currency_code("usd").is_err()); // lowercase
/// assert!(validate_currency_code("US").is_err()); // too short
/// assert!(validate_currency_code("USDD").is_err()); // too long
/// ```
pub fn validate_currency_code(code: &str) -> Result<()> {
    if code.len() != 3 {
        return Err(CommerceError::ValidationError(
            "Currency code must be exactly 3 characters".into(),
        ));
    }

    if !code.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(CommerceError::ValidationError(
            "Currency code must be uppercase letters only".into(),
        ));
    }

    Ok(())
}

/// Validate a postal/ZIP code format (basic validation).
///
/// This performs basic postal code validation:
/// - Non-empty
/// - 3-10 characters
/// - Contains only alphanumeric characters, spaces, and hyphens
///
/// Note: This is a generic validator. For country-specific validation,
/// use dedicated validators.
///
/// # Example
///
/// ```
/// use stateset_core::validate_postal_code;
///
/// assert!(validate_postal_code("12345").is_ok());
/// assert!(validate_postal_code("12345-6789").is_ok());
/// assert!(validate_postal_code("SW1A 1AA").is_ok()); // UK format
/// assert!(validate_postal_code("").is_err());
/// ```
pub fn validate_postal_code(code: &str) -> Result<()> {
    let code = code.trim();

    if code.is_empty() {
        return Err(CommerceError::ValidationError("Postal code cannot be empty".into()));
    }

    if code.len() < 3 {
        return Err(CommerceError::ValidationError(
            "Postal code must be at least 3 characters".into(),
        ));
    }

    if code.len() > 10 {
        return Err(CommerceError::ValidationError(
            "Postal code cannot exceed 10 characters".into(),
        ));
    }

    if !code.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-') {
        return Err(CommerceError::ValidationError(
            "Postal code contains invalid characters".into(),
        ));
    }

    Ok(())
}

/// Validate a quantity value.
///
/// Quantities must be positive (greater than zero).
///
/// # Example
///
/// ```
/// use stateset_core::validate_quantity;
/// use rust_decimal_macros::dec;
///
/// assert!(validate_quantity(dec!(1)).is_ok());
/// assert!(validate_quantity(dec!(0.5)).is_ok());
/// assert!(validate_quantity(dec!(0)).is_err());
/// assert!(validate_quantity(dec!(-1)).is_err());
/// ```
pub fn validate_quantity(qty: rust_decimal::Decimal) -> Result<()> {
    if qty <= rust_decimal::Decimal::ZERO {
        return Err(CommerceError::ValidationError("Quantity must be greater than zero".into()));
    }
    Ok(())
}

/// Validate a price/amount value.
///
/// Prices must be non-negative (zero or greater).
///
/// # Example
///
/// ```
/// use stateset_core::validate_price;
/// use rust_decimal_macros::dec;
///
/// assert!(validate_price(dec!(0)).is_ok());
/// assert!(validate_price(dec!(99.99)).is_ok());
/// assert!(validate_price(dec!(-1)).is_err());
/// ```
pub fn validate_price(price: rust_decimal::Decimal) -> Result<()> {
    if price < rust_decimal::Decimal::ZERO {
        return Err(CommerceError::ValidationError("Price cannot be negative".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_required_text_rejects_empty() {
        let result = validate_required_text("field", "   ", 10);
        assert!(result.is_err());
    }

    #[test]
    fn validate_required_text_rejects_too_long() {
        let result = validate_required_text("field", "toolong", 3);
        assert!(result.is_err());
    }

    #[test]
    fn validate_required_text_accepts_trimmed() {
        let result = validate_required_text("field", "  ok  ", 10);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_required_uuid_rejects_nil() {
        let result = validate_required_uuid("id", Uuid::nil());
        assert!(result.is_err());
    }

    #[test]
    fn validate_required_uuid_accepts_non_nil() {
        let result = validate_required_uuid("id", Uuid::new_v4());
        assert!(result.is_ok());
    }

    #[test]
    fn validate_email_accepts_common_production_formats() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("user.name+tag@example.co.uk").is_ok());
        assert!(validate_email("ops_team-42@xn--bcher-kva.example").is_ok());
    }

    #[test]
    fn validate_email_rejects_common_invalid_formats() {
        assert!(validate_email("alice..bob@example.com").is_err());
        assert!(validate_email(".alice@example.com").is_err());
        assert!(validate_email("alice@example..com").is_err());
        assert!(validate_email("alice@-example.com").is_err());
        assert!(validate_email("alice@example-.com").is_err());
        assert!(validate_email("alice@[127.0.0.1]").is_err());
    }

    // ====================================================================
    // Domain sub-error conversion tests
    // ====================================================================

    #[test]
    fn order_error_converts_to_commerce_error() {
        let err: CommerceError = OrderError::not_found(Uuid::nil()).into();
        assert!(err.is_not_found());
        assert!(err.as_order_error().is_some());
    }

    #[test]
    fn inventory_error_converts_to_commerce_error() {
        let err: CommerceError = InventoryError::item_not_found("SKU").into();
        assert!(err.is_not_found());
        assert!(err.as_inventory_error().is_some());
    }

    #[test]
    fn customer_error_converts_to_commerce_error() {
        let err: CommerceError = CustomerError::not_found(Uuid::nil()).into();
        assert!(err.is_not_found());
        assert!(err.as_customer_error().is_some());
    }

    #[test]
    fn product_error_converts_to_commerce_error() {
        let err: CommerceError = ProductError::not_found(Uuid::nil()).into();
        assert!(err.is_not_found());
        assert!(err.as_product_error().is_some());
    }

    #[test]
    fn return_error_converts_to_commerce_error() {
        let err: CommerceError = ReturnError::not_found(Uuid::nil()).into();
        assert!(err.is_not_found());
    }

    #[test]
    fn payment_error_converts_to_commerce_error() {
        let err: CommerceError = PaymentError::not_found(Uuid::nil()).into();
        assert!(err.is_not_found());
    }

    #[test]
    fn shipping_error_converts_to_commerce_error() {
        let err: CommerceError = ShippingError::not_found(Uuid::nil()).into();
        assert!(err.is_not_found());
    }

    #[test]
    fn conflict_detection_domain_errors() {
        let err: CommerceError = InventoryError::duplicate_sku("X").into();
        assert!(err.is_conflict());

        let err: CommerceError = CustomerError::email_already_exists("x@y.com").into();
        assert!(err.is_conflict());

        let err: CommerceError = ProductError::duplicate_slug("slug").into();
        assert!(err.is_conflict());
    }

    #[test]
    fn batch_error_code_from_domain_errors() {
        let err: CommerceError = OrderError::not_found(Uuid::nil()).into();
        assert_eq!(BatchErrorCode::from(&err), BatchErrorCode::NotFound);

        let err: CommerceError = InventoryError::duplicate_sku("X").into();
        assert_eq!(BatchErrorCode::from(&err), BatchErrorCode::DuplicateKey);
    }

    #[test]
    fn state_transition_error_in_order() {
        let err = OrderError::invalid_transition("pending", "delivered");
        let commerce_err: CommerceError = err.into();
        let msg = commerce_err.to_string();
        assert!(msg.contains("pending"));
        assert!(msg.contains("delivered"));
    }

    #[test]
    fn got_expected_basic() {
        let ge = GotExpected { got: 2_i32, expected: 5_i32 };
        assert_eq!(ge.to_string(), "expected 5, got 2");
    }

    #[test]
    fn non_exhaustive_allows_future_variants() {
        // This test verifies that match arms require a wildcard,
        // proving #[non_exhaustive] is in effect.
        let err = CommerceError::NotFound;
        let _ = match err {
            CommerceError::NotFound => "ok",
            _ => "other",
        };
    }

    // ====================================================================
    // Size reporting (run with `--nocapture` to see sizes)
    // ====================================================================

    #[test]
    fn print_error_enum_sizes() {
        use std::mem::size_of;
        println!("--- Error Enum Sizes (bytes) ---");
        println!("CommerceError:  {}", size_of::<CommerceError>());
        println!("DbError:        {}", size_of::<DbError>());
        println!("OrderError:     {}", size_of::<OrderError>());
        println!("InventoryError: {}", size_of::<InventoryError>());
        println!("CustomerError:  {}", size_of::<CustomerError>());
        println!("ProductError:   {}", size_of::<ProductError>());
        println!("ReturnError:    {}", size_of::<ReturnError>());
        println!("PaymentError:   {}", size_of::<PaymentError>());
        println!("ShippingError:  {}", size_of::<ShippingError>());
    }

    // ====================================================================
    // Error classification tests (Phase 0D)
    // ====================================================================

    #[test]
    fn is_transient_includes_retryable() {
        let err = CommerceError::OptimisticLockFailure;
        assert!(err.is_transient());
        assert!(err.is_retryable());
    }

    #[test]
    fn is_transient_includes_external_service() {
        let err = CommerceError::ExternalServiceError("timeout".into());
        assert!(err.is_transient());
        assert!(!err.is_retryable()); // external is transient but not auto-retryable
    }

    #[test]
    fn is_client_error_variants() {
        assert!(CommerceError::NotFound.is_client_error());
        assert!(CommerceError::ValidationError("bad".into()).is_client_error());
        assert!(CommerceError::DuplicateSku("X".into()).is_client_error());
        assert!(CommerceError::NotPermitted("denied".into()).is_client_error());
    }

    #[test]
    fn is_server_error_variants() {
        assert!(CommerceError::Internal("oops".into()).is_server_error());
        assert!(CommerceError::DatabaseError("fail".into()).is_server_error());
        assert!(CommerceError::ExternalServiceError("timeout".into()).is_server_error());
    }

    #[test]
    fn is_not_permitted() {
        assert!(CommerceError::NotPermitted("admin only".into()).is_not_permitted());
        assert!(!CommerceError::NotFound.is_not_permitted());
    }

    #[test]
    fn suggested_status_codes() {
        assert_eq!(CommerceError::NotFound.suggested_status_code(), 404);
        assert_eq!(CommerceError::OrderNotFound(Uuid::nil()).suggested_status_code(), 404);
        assert_eq!(CommerceError::ValidationError("bad".into()).suggested_status_code(), 400);
        assert_eq!(CommerceError::DuplicateSku("X".into()).suggested_status_code(), 409);
        assert_eq!(CommerceError::NotPermitted("no".into()).suggested_status_code(), 403);
        assert_eq!(CommerceError::ExternalServiceError("t".into()).suggested_status_code(), 502);
        assert_eq!(CommerceError::Internal("x".into()).suggested_status_code(), 500);
    }

    #[test]
    fn client_vs_server_mutually_exclusive_for_not_found() {
        let err = CommerceError::NotFound;
        assert!(err.is_client_error());
        assert!(!err.is_server_error());
    }

    #[test]
    fn got_expected_const_new() {
        // Verify the new const constructor works identically to struct literal.
        let a = GotExpected::new(2, 5);
        let b = GotExpected { got: 2, expected: 5 };
        assert_eq!(a, b);
    }
}
