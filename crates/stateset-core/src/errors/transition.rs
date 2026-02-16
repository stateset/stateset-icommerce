//! Generic state transition error.
//!
//! Commerce entities move through well-defined state machines (e.g. an order goes
//! from `Pending` → `Confirmed` → `Shipped` → `Delivered`). When an invalid
//! transition is attempted, this type captures both the current and requested
//! states in a type-safe way.

use std::fmt;

/// Error indicating an invalid state transition.
///
/// The type parameter `S` is the status enum (e.g. `OrderStatus`, `ReturnStatus`).
/// This keeps transition errors strongly-typed while reusing a single generic
/// error structure across all domain state machines.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::StateTransitionError;
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum Light { Red, Yellow, Green }
///
/// impl std::fmt::Display for Light {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         write!(f, "{:?}", self)
///     }
/// }
///
/// let err = StateTransitionError::new(Light::Red, Light::Green);
/// assert_eq!(err.to_string(), "invalid state transition from Red to Green");
/// assert_eq!(err.from, Light::Red);
/// assert_eq!(err.to, Light::Green);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateTransitionError<S> {
    /// The current state.
    pub from: S,
    /// The requested (invalid) target state.
    pub to: S,
}

impl<S> StateTransitionError<S> {
    /// Create a new state transition error.
    #[inline]
    pub const fn new(from: S, to: S) -> Self {
        Self { from, to }
    }
}

impl<S: fmt::Display> fmt::Display for StateTransitionError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid state transition from {} to {}", self.from, self.to)
    }
}

impl<S: fmt::Debug> fmt::Debug for StateTransitionError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StateTransitionError")
            .field("from", &self.from)
            .field("to", &self.to)
            .finish()
    }
}

impl<S: fmt::Debug + fmt::Display> std::error::Error for StateTransitionError<S> {}

/// A mismatch between an expected value and the actual value found.
///
/// Inspired by reth's `GotExpected<T>` pattern — useful for version conflicts,
/// quantity mismatches, and other "expected X but got Y" scenarios.
///
/// # Example
///
/// ```rust
/// use stateset_core::errors::GotExpected;
///
/// let mismatch = GotExpected { got: 3, expected: 5 };
/// assert_eq!(mismatch.to_string(), "expected 5, got 3");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GotExpected<T> {
    /// The value that was actually found.
    pub got: T,
    /// The value that was expected.
    pub expected: T,
}

impl<T> GotExpected<T> {
    /// Create a new `GotExpected` mismatch.
    #[inline]
    pub const fn new(got: T, expected: T) -> Self {
        Self { got, expected }
    }
}

impl<T: fmt::Display> fmt::Display for GotExpected<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {}, got {}", self.expected, self.got)
    }
}

impl<T: fmt::Debug + fmt::Display> std::error::Error for GotExpected<T> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Status {
        Pending,
        Active,
        Closed,
    }

    impl fmt::Display for Status {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Pending => write!(f, "pending"),
                Self::Active => write!(f, "active"),
                Self::Closed => write!(f, "closed"),
            }
        }
    }

    #[test]
    fn transition_error_display() {
        let err = StateTransitionError::new(Status::Pending, Status::Closed);
        assert_eq!(err.to_string(), "invalid state transition from pending to closed");
    }

    #[test]
    fn transition_error_debug() {
        let err = StateTransitionError::new(Status::Active, Status::Pending);
        let debug = format!("{:?}", err);
        assert!(debug.contains("StateTransitionError"));
        assert!(debug.contains("Active"));
        assert!(debug.contains("Pending"));
    }

    #[test]
    fn transition_error_fields() {
        let err = StateTransitionError::new(Status::Active, Status::Closed);
        assert_eq!(err.from, Status::Active);
        assert_eq!(err.to, Status::Closed);
    }

    #[test]
    fn got_expected_display() {
        let ge = GotExpected { got: 3, expected: 5 };
        assert_eq!(ge.to_string(), "expected 5, got 3");
    }

    #[test]
    fn got_expected_with_strings() {
        let ge = GotExpected { got: "foo".to_string(), expected: "bar".to_string() };
        assert_eq!(ge.to_string(), "expected bar, got foo");
    }
}
