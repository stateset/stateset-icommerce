//! Application state shared across all route handlers.

use std::sync::Arc;

use stateset_embedded::Commerce;

/// Shared application state injected into every route handler via
/// [`axum::extract::State`].
#[derive(Debug, Clone)]
pub struct AppState {
    commerce: Arc<Commerce>,
}

impl AppState {
    /// Create a new `AppState` wrapping a [`Commerce`] instance.
    #[must_use]
    pub fn new(commerce: Commerce) -> Self {
        Self {
            commerce: Arc::new(commerce),
        }
    }

    /// Access the underlying [`Commerce`] engine.
    #[must_use]
    pub fn commerce(&self) -> &Commerce {
        &self.commerce
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_commerce() -> Commerce {
        Commerce::new(":memory:").expect("in-memory Commerce")
    }

    #[test]
    fn state_debug_impl() {
        let state = AppState::new(test_commerce());
        let dbg = format!("{state:?}");
        assert!(dbg.contains("AppState"));
    }

    #[test]
    fn state_clone() {
        let state = AppState::new(test_commerce());
        let cloned = state.clone();
        // Both point to the same Arc
        assert!(Arc::ptr_eq(&state.commerce, &cloned.commerce));
    }

    #[test]
    fn state_accessor() {
        let state = AppState::new(test_commerce());
        // Just verify it doesn't panic
        let _commerce = state.commerce();
    }
}
