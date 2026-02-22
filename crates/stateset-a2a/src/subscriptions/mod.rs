//! Subscription service types and logic.
//!
//! Supports trial periods, pause/resume, graceful cancellation,
//! and automated billing cycle processing.

pub mod billing;
pub mod state_machine;

pub use billing::{BillingInterval, compute_next_billing_date};
pub use state_machine::{SubscriptionStatus, SubscriptionTransition};
