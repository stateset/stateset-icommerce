//! Event stream service types and logic.
//!
//! Provides event type filtering with wildcard and prefix matching
//! for SSE (Server-Sent Events) delivery.

pub mod filters;

pub use filters::matches_event_filter;
