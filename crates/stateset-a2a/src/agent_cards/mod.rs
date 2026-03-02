//! Agent card validation and discovery filtering.
//!
//! Provides types for agent identity cards, discovery filters,
//! and validation logic for the A2A agent registry.

pub mod types;

pub use types::{AgentCard, AgentSkill, DiscoveryFilter, validate_agent_card};
