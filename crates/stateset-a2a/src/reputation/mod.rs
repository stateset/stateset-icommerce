//! Reputation and trust scoring for A2A agents.
//!
//! Provides dimension-based scoring (reliability, quality, speed, communication),
//! overall score aggregation, and trust tier promotion based on transaction history.

pub mod scoring;
pub mod tiers;

pub use scoring::{DimensionScores, FeedbackEntry, ReputationSummary, ScoreDimension};
pub use tiers::{TierRequirements, TransactionType, TrustTier};
