//! Shipping-domain errors.

use uuid::Uuid;

/// Errors specific to shipping and shipment operations.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::ShippingError;
///
/// let err = ShippingError::not_found(uuid::Uuid::nil());
/// assert!(err.to_string().contains("not found"));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShippingError {
    /// Shipment with the given ID was not found.
    #[error("shipment not found: {0}")]
    NotFound(Uuid),

    /// Carrier rejected the shipment request.
    #[error("carrier error: {reason}")]
    CarrierError {
        /// The carrier name.
        carrier: String,
        /// The reason for rejection.
        reason: String,
    },

    /// Invalid tracking number format.
    #[error("invalid tracking number: {0}")]
    InvalidTrackingNumber(String),

    /// Extensibility.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl ShippingError {
    /// Convenience constructor for `NotFound`.
    #[inline]
    #[track_caller]
    pub const fn not_found(id: Uuid) -> Self {
        Self::NotFound(id)
    }

    /// Convenience constructor for `CarrierError`.
    #[track_caller]
    pub fn carrier_error(carrier: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::CarrierError { carrier: carrier.into(), reason: reason.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let err = ShippingError::not_found(Uuid::nil());
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn carrier_error_display() {
        let err = ShippingError::carrier_error("FedEx", "address invalid");
        assert!(err.to_string().contains("address invalid"));
    }

    #[test]
    fn converts_to_commerce_error() {
        use crate::errors::CommerceError;

        let ship_err = ShippingError::not_found(Uuid::nil());
        let commerce_err: CommerceError = ship_err.into();
        assert!(commerce_err.is_not_found());
    }
}
