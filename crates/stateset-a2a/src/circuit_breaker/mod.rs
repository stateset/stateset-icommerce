//! Circuit breaker for agent transaction safety.
//!
//! Provides a state machine (closed → open → `half_open` → closed),
//! spending limits (per-transaction, daily, monthly), and failure
//! rate tracking to protect agents from runaway transactions.

pub mod config;
pub mod limits;
pub mod state_machine;

pub use config::CircuitBreakerConfig;
pub use limits::{LimitCheckResult, SpendingLimits, check_spending_limits};
pub use state_machine::{CircuitState, CircuitTransition};
