//! Split payment service types and logic.
//!
//! Supports percentage-based and fixed-amount multi-party splits
//! with optional platform fees and rounding drift prevention.

pub mod calculation;
pub mod state_machine;

pub use calculation::{
    Recipient, SplitResult, SplitShare, SplitType, calculate_fixed_split,
    calculate_percentage_split,
};
pub use state_machine::{SplitPaymentStatus, SplitPaymentTransition};
