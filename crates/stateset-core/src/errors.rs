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

/// Validate an email address format
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

    if email.contains(char::is_whitespace) {
        return Err(CommerceError::ValidationError("Email cannot contain whitespace".into()));
    }

    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return Err(CommerceError::ValidationError(
            "Email must contain exactly one @ symbol".into()
        ));
    }

    let (local, domain) = (parts[0], parts[1]);

    if local.is_empty() {
        return Err(CommerceError::ValidationError(
            "Email local part (before @) cannot be empty".into()
        ));
    }

    if domain.is_empty() {
        return Err(CommerceError::ValidationError(
            "Email domain (after @) cannot be empty".into()
        ));
    }

    if !domain.contains('.') {
        return Err(CommerceError::ValidationError(
            "Email domain must contain at least one dot".into()
        ));
    }

    // Check domain doesn't start or end with a dot
    if domain.starts_with('.') || domain.ends_with('.') {
        return Err(CommerceError::ValidationError(
            "Email domain cannot start or end with a dot".into()
        ));
    }

    Ok(())
}

/// Validate a SKU format
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
        return Err(CommerceError::ValidationError(
            "SKU cannot exceed 100 characters".into()
        ));
    }

    if !sku.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(CommerceError::ValidationError(
            "SKU can only contain alphanumeric characters, hyphens, and underscores".into()
        ));
    }

    Ok(())
}

/// Validate a phone number format (basic validation)
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
    if !phone.chars().all(|c| c.is_ascii_digit() || c == ' ' || c == '-' || c == '(' || c == ')' || c == '+') {
        return Err(CommerceError::ValidationError(
            "Phone number contains invalid characters".into()
        ));
    }

    // Count digits
    let digit_count = phone.chars().filter(|c| c.is_ascii_digit()).count();

    if digit_count < 7 {
        return Err(CommerceError::ValidationError(
            "Phone number must have at least 7 digits".into()
        ));
    }

    if digit_count > 15 {
        return Err(CommerceError::ValidationError(
            "Phone number cannot exceed 15 digits".into()
        ));
    }

    Ok(())
}

/// Validate a currency code (ISO 4217 format)
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
            "Currency code must be exactly 3 characters".into()
        ));
    }

    if !code.chars().all(|c| c.is_ascii_uppercase()) {
        return Err(CommerceError::ValidationError(
            "Currency code must be uppercase letters only".into()
        ));
    }

    Ok(())
}

/// Validate a postal/ZIP code format (basic validation)
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
            "Postal code must be at least 3 characters".into()
        ));
    }

    if code.len() > 10 {
        return Err(CommerceError::ValidationError(
            "Postal code cannot exceed 10 characters".into()
        ));
    }

    if !code.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-') {
        return Err(CommerceError::ValidationError(
            "Postal code contains invalid characters".into()
        ));
    }

    Ok(())
}

/// Validate a quantity value
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
        return Err(CommerceError::ValidationError(
            "Quantity must be greater than zero".into()
        ));
    }
    Ok(())
}

/// Validate a price/amount value
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
        return Err(CommerceError::ValidationError(
            "Price cannot be negative".into()
        ));
    }
    Ok(())
}
