//! Inventory-domain errors.

use uuid::Uuid;

/// Errors specific to inventory operations.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::InventoryError;
///
/// let err = InventoryError::insufficient_stock("SKU-001", "10", "3");
/// assert!(err.to_string().contains("SKU-001"));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum InventoryError {
    /// Inventory item not found by SKU or ID.
    #[error("inventory item not found: {0}")]
    ItemNotFound(String),

    /// Insufficient stock for the requested quantity.
    #[error("insufficient stock for {sku}: requested {requested}, available {available}")]
    InsufficientStock {
        /// The SKU that has insufficient stock.
        sku: String,
        /// The quantity that was requested.
        requested: String,
        /// The quantity that is available.
        available: String,
    },

    /// Inventory reservation not found.
    #[error("reservation not found: {0}")]
    ReservationNotFound(Uuid),

    /// Inventory reservation has expired.
    #[error("reservation expired: {0}")]
    ReservationExpired(Uuid),

    /// Duplicate SKU already exists.
    #[error("duplicate SKU: {0}")]
    DuplicateSku(String),

    /// Extensibility.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl InventoryError {
    /// Convenience constructor for `ItemNotFound`.
    #[track_caller]
    pub fn item_not_found(id: impl Into<String>) -> Self {
        Self::ItemNotFound(id.into())
    }

    /// Convenience constructor for `InsufficientStock`.
    #[track_caller]
    pub fn insufficient_stock(
        sku: impl Into<String>,
        requested: impl Into<String>,
        available: impl Into<String>,
    ) -> Self {
        Self::InsufficientStock {
            sku: sku.into(),
            requested: requested.into(),
            available: available.into(),
        }
    }

    /// Convenience constructor for `ReservationNotFound`.
    #[inline]
    #[track_caller]
    pub fn reservation_not_found(id: Uuid) -> Self {
        Self::ReservationNotFound(id)
    }

    /// Convenience constructor for `ReservationExpired`.
    #[inline]
    #[track_caller]
    pub fn reservation_expired(id: Uuid) -> Self {
        Self::ReservationExpired(id)
    }

    /// Convenience constructor for `DuplicateSku`.
    #[track_caller]
    pub fn duplicate_sku(sku: impl Into<String>) -> Self {
        Self::DuplicateSku(sku.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_not_found_display() {
        let err = InventoryError::item_not_found("SKU-999");
        assert_eq!(err.to_string(), "inventory item not found: SKU-999");
    }

    #[test]
    fn insufficient_stock_display() {
        let err = InventoryError::insufficient_stock("WIDGET", "10", "3");
        let msg = err.to_string();
        assert!(msg.contains("WIDGET"));
        assert!(msg.contains("10"));
        assert!(msg.contains("3"));
    }

    #[test]
    fn reservation_not_found_display() {
        let id = Uuid::nil();
        let err = InventoryError::reservation_not_found(id);
        assert!(err.to_string().contains(&id.to_string()));
    }

    #[test]
    fn converts_to_commerce_error() {
        use crate::errors::CommerceError;

        let inv_err = InventoryError::item_not_found("SKU-001");
        let commerce_err: CommerceError = inv_err.into();
        assert!(commerce_err.is_not_found());
    }

    #[test]
    fn duplicate_sku_display() {
        let err = InventoryError::duplicate_sku("DUPE-001");
        assert_eq!(err.to_string(), "duplicate SKU: DUPE-001");
    }
}
