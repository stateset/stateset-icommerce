#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]
//! # stateset-a2a
//!
//! Agent-to-Agent (A2A) commerce service layer for the StateSet iCommerce platform.
//!
//! This crate provides the business logic for multi-party split payments,
//! recurring subscriptions, conditional escrow, HMAC-signed webhooks,
//! SSRF protection, SSE event stream filtering, dispute resolution,
//! reputation scoring, circuit breakers, SLA compliance, marketplace RFQs,
//! and agent card management.
//!
//! ## Modules
//!
//! - [`splits`] — Multi-party payment splitting with rounding drift prevention.
//! - [`subscriptions`] — Recurring billing with trial periods and state machine.
//! - [`escrow`] — Conditional fund holding with four condition types.
//! - [`notifications`] — HMAC-SHA256 webhook signing and SSRF URL validation.
//! - [`events`] — Event type filtering with wildcard/prefix matching.
//! - [`disputes`] — Dispute resolution with evidence hashing and deadline management.
//! - [`reputation`] — Trust scoring with dimension-based evaluation and tier promotion.
//! - [`circuit_breaker`] — Transaction safety with spending limits and failure rate tracking.
//! - [`sla`] — Service level agreements with compliance checking and penalty calculation.
//! - [`marketplace`] — Multi-party RFQ with scoring and response ranking.
//! - [`agent_cards`] — Agent card validation and discovery filtering.
//!
//! ## Example: Percentage Split
//!
//! ```
//! use rust_decimal_macros::dec;
//! use stateset_a2a::splits::{Recipient, calculate_percentage_split};
//!
//! let recipients = vec![
//!     Recipient { address: "0xAlice".into(), percent: Some(dec!(50)), amount: None },
//!     Recipient { address: "0xBob".into(), percent: Some(dec!(30)), amount: None },
//!     Recipient { address: "0xCharlie".into(), percent: Some(dec!(20)), amount: None },
//! ];
//!
//! let result = calculate_percentage_split(dec!(100), dec!(2.5), &recipients).unwrap();
//! assert_eq!(result.total_distributed, dec!(100));
//! ```
//!
//! ## Example: HMAC Webhook
//!
//! ```
//! use stateset_a2a::notifications::{sign_webhook, verify_webhook};
//!
//! let signature = sign_webhook(b"whsec_abc123", b"{\"event\":\"payment.completed\"}");
//! assert!(verify_webhook(b"whsec_abc123", b"{\"event\":\"payment.completed\"}", &signature));
//! ```

pub mod agent_cards;
pub mod circuit_breaker;
pub mod disputes;
pub mod error;
pub mod escrow;
pub mod events;
pub mod marketplace;
pub mod notifications;
pub mod reputation;
pub mod sla;
pub mod splits;
pub mod subscriptions;

// Re-export top-level error type for convenience.
pub use error::{A2AError, A2AResult};
