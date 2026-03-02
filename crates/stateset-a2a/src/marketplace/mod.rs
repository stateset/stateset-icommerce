//! Marketplace and RFQ (Request for Quote) module.
//!
//! Provides multi-party RFQ management with configurable scoring criteria
//! and response ranking.

pub mod scoring;
pub mod state_machine;

pub use scoring::{RfqResponse, ScoringCriteria, score_response, rank_responses};
pub use state_machine::{RfqStatus, RfqResponseStatus, RfqTransition};
