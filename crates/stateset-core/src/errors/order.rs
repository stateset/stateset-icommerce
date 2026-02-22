//! Order-domain errors.

use super::StateTransitionError;
use std::fmt;
use uuid::Uuid;

/// Errors specific to order operations.
///
/// Use [`OrderError`] in domain logic that only touches orders. It converts
/// into [`CommerceError`](super::CommerceError) via `#[from]` so callers
/// working with the top-level result type get seamless interop.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::OrderError;
/// use uuid::Uuid;
///
/// let err = OrderError::not_found(Uuid::nil());
/// assert!(err.to_string().contains("not found"));
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OrderError {
    /// Order with the given ID was not found.
    #[error("order not found: {0}")]
    NotFound(Uuid),

    /// Order cannot be cancelled in its current status.
    #[error("order cannot be cancelled in status: {status}")]
    CannotCancel {
        /// The current order status that prevents cancellation.
        status: String,
    },

    /// Order cannot be refunded.
    #[error("order cannot be refunded: {reason}")]
    CannotRefund {
        /// The reason the refund was rejected.
        reason: String,
    },

    /// Invalid status transition for an order.
    #[error("{0}")]
    InvalidTransition(StateTransitionError<OrderStatusLabel>),

    /// Extensibility — domain callers can wrap arbitrary errors.
    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send + Sync>),
}

impl OrderError {
    /// Convenience constructor for `NotFound`.
    #[inline]
    #[track_caller]
    pub const fn not_found(id: Uuid) -> Self {
        Self::NotFound(id)
    }

    /// Convenience constructor for `CannotCancel`.
    #[track_caller]
    pub fn cannot_cancel(status: impl fmt::Display) -> Self {
        Self::CannotCancel { status: status.to_string() }
    }

    /// Convenience constructor for `CannotRefund`.
    #[track_caller]
    pub fn cannot_refund(reason: impl Into<String>) -> Self {
        Self::CannotRefund { reason: reason.into() }
    }

    /// Create an invalid transition error from display-able status values.
    #[track_caller]
    pub fn invalid_transition(from: impl fmt::Display, to: impl fmt::Display) -> Self {
        Self::InvalidTransition(StateTransitionError::new(
            OrderStatusLabel(from.to_string()),
            OrderStatusLabel(to.to_string()),
        ))
    }
}

/// Thin wrapper so `StateTransitionError` can display order status strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderStatusLabel(pub String);

impl fmt::Display for OrderStatusLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_display() {
        let id = Uuid::nil();
        let err = OrderError::not_found(id);
        assert!(err.to_string().contains("not found"));
        assert!(err.to_string().contains(&id.to_string()));
    }

    #[test]
    fn cannot_cancel_display() {
        let err = OrderError::cannot_cancel("shipped");
        assert!(err.to_string().contains("shipped"));
    }

    #[test]
    fn invalid_transition_display() {
        let err = OrderError::invalid_transition("pending", "delivered");
        let msg = err.to_string();
        assert!(msg.contains("pending"));
        assert!(msg.contains("delivered"));
    }

    #[test]
    fn converts_to_commerce_error() {
        use crate::errors::CommerceError;

        let order_err = OrderError::not_found(Uuid::nil());
        let commerce_err: CommerceError = order_err.into();
        assert!(commerce_err.is_not_found());
    }

    #[test]
    fn other_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "disk full");
        let err = OrderError::Other(Box::new(io_err));
        assert!(err.to_string().contains("disk full"));
    }
}
