//! Error types for commerce operations

use serde::{Deserialize, Serialize};
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

// ============================================================================
// Batch Operation Types
// ============================================================================

/// Maximum items allowed per batch operation
pub const MAX_BATCH_SIZE: usize = 1000;

/// Categorized batch error codes for programmatic handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchErrorCode {
    /// Entity was not found
    NotFound,
    /// Input validation failed
    ValidationError,
    /// Duplicate key constraint violation
    DuplicateKey,
    /// Optimistic locking version conflict
    VersionConflict,
    /// Database-level error
    DatabaseError,
    /// Unclassified internal error
    InternalError,
}

impl From<&CommerceError> for BatchErrorCode {
    fn from(err: &CommerceError) -> Self {
        match err {
            CommerceError::NotFound
            | CommerceError::OrderNotFound(_)
            | CommerceError::CustomerNotFound(_)
            | CommerceError::ProductNotFound(_)
            | CommerceError::ProductVariantNotFound(_)
            | CommerceError::ReturnNotFound(_)
            | CommerceError::InventoryItemNotFound(_)
            | CommerceError::ReservationNotFound(_) => BatchErrorCode::NotFound,

            CommerceError::ValidationError(_) | CommerceError::InvalidInput { .. } => {
                BatchErrorCode::ValidationError
            }

            CommerceError::DuplicateSku(_)
            | CommerceError::DuplicateSlug(_)
            | CommerceError::EmailAlreadyExists(_) => BatchErrorCode::DuplicateKey,

            CommerceError::VersionConflict { .. } | CommerceError::OptimisticLockFailure => {
                BatchErrorCode::VersionConflict
            }

            CommerceError::DatabaseError(_) => BatchErrorCode::DatabaseError,

            _ => BatchErrorCode::InternalError,
        }
    }
}

/// Error information for a single item in a batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchError {
    /// Index in the original batch (for create/update operations)
    pub index: usize,
    /// ID of the entity (for update/delete/get operations, if available)
    pub id: Option<String>,
    /// Human-readable error message
    pub error: String,
    /// Error code for programmatic handling
    pub code: BatchErrorCode,
}

impl BatchError {
    /// Create a new BatchError from an index and CommerceError
    pub fn from_error(index: usize, id: Option<String>, err: &CommerceError) -> Self {
        Self {
            index,
            id,
            error: err.to_string(),
            code: BatchErrorCode::from(err),
        }
    }
}

/// Result of a batch operation that allows partial success
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult<T> {
    /// Successfully processed items
    pub succeeded: Vec<T>,
    /// Failed operations with their errors
    pub failed: Vec<BatchError>,
    /// Total items attempted
    pub total_attempted: usize,
    /// Count of successful operations
    pub success_count: usize,
    /// Count of failed operations
    pub failure_count: usize,
}

impl<T> BatchResult<T> {
    /// Create a new empty BatchResult
    pub fn new() -> Self {
        Self {
            succeeded: Vec::new(),
            failed: Vec::new(),
            total_attempted: 0,
            success_count: 0,
            failure_count: 0,
        }
    }

    /// Create a BatchResult with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            succeeded: Vec::with_capacity(capacity),
            failed: Vec::new(),
            total_attempted: 0,
            success_count: 0,
            failure_count: 0,
        }
    }

    /// Record a successful operation
    pub fn record_success(&mut self, item: T) {
        self.succeeded.push(item);
        self.success_count += 1;
        self.total_attempted += 1;
    }

    /// Record a failed operation
    pub fn record_failure(&mut self, index: usize, id: Option<String>, err: &CommerceError) {
        self.failed.push(BatchError::from_error(index, id, err));
        self.failure_count += 1;
        self.total_attempted += 1;
    }

    /// Check if all operations succeeded
    pub fn all_succeeded(&self) -> bool {
        self.failure_count == 0
    }

    /// Check if all operations failed
    pub fn all_failed(&self) -> bool {
        self.success_count == 0 && self.total_attempted > 0
    }

    /// Check if some operations succeeded and some failed
    pub fn partial_success(&self) -> bool {
        self.success_count > 0 && self.failure_count > 0
    }

    /// Check if the batch was empty
    pub fn is_empty(&self) -> bool {
        self.total_attempted == 0
    }
}

impl<T> Default for BatchResult<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate batch size against maximum limit
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
