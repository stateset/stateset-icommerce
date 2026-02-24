#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]
//! # StateSet Policy Engine
//!
//! A declarative, condition-based rule system for enforcing business logic,
//! access control, and data transformations in commerce operations.
//!
//! This crate is a faithful Rust port of the JS policy engine
//! (`cli/src/policies/engine.js`, 1,138 lines).
//!
//! ## Features
//!
//! - **20 operators** for condition evaluation (comparison, string, collection, type, numeric)
//! - **Deny-overrides precedence** -- any deny action overrides all allow actions
//! - **Explainable denials** -- per-condition breakdown of why a request was denied
//! - **Transform audit trail** -- tracks before/after values for policy transforms
//! - **YAML/JSON policy loading** from filesystem (YAML requires the `yaml` feature)
//! - **Dry-run evaluation** -- test policies without recording history
//! - **5 pre-built templates** for returns, inventory, fraud, promotions, subscriptions
//!
//! ## Quick Start
//!
//! ```rust
//! use stateset_policy::{
//!     PolicyEngine, PolicySet, PolicyRule, PolicyAction,
//!     ConditionGroup, ConditionNode, Condition, Operator, Logic,
//! };
//! use serde_json::json;
//!
//! let mut engine = PolicyEngine::new();
//!
//! // Create a rule that denies orders over $10,000
//! let rule = PolicyRule::new("high-value-review", "Require review for high-value orders")
//!     .with_priority(10)
//!     .with_conditions(ConditionGroup::new(Logic::And, vec![
//!         ConditionNode::Leaf(Condition::new("order.total", Operator::Gt, json!(10000))),
//!     ]))
//!     .with_action(PolicyAction::deny(
//!         "Order exceeds $10,000 limit",
//!         "Request manager approval",
//!     ));
//!
//! let policy_set = PolicySet::new("order-limits", "orders").with_rule(rule);
//! engine.register_policy_set(policy_set);
//!
//! let context = json!({
//!     "order": { "total": 15000, "customer": { "tier": "standard" } }
//! });
//!
//! let result = engine.evaluate("orders", &context);
//! assert!(result.should_deny);
//! assert_eq!(result.explanations.len(), 1);
//! ```
//!
//! ## Policy Templates
//!
//! Ready-to-use templates for common commerce scenarios:
//!
//! ```rust
//! use stateset_policy::templates;
//!
//! let returns_policy = templates::auto_approve_returns_template();
//! let fraud_policy = templates::order_fraud_detection_template();
//! let promo_policy = templates::promotion_eligibility_template();
//! ```
//!
//! ## Loading from Files
//!
//! ```rust,no_run
//! use stateset_policy::PolicyEngine;
//! use std::path::Path;
//!
//! let mut engine = PolicyEngine::new();
//! let count = engine.load_from_dir(Path::new("./policies")).unwrap();
//! println!("Loaded {count} policy sets");
//! ```

mod action;
mod condition;
mod context;
mod engine;
mod error;
mod explanation;
mod loader;
mod operator;
mod policy_set;
mod rule;
pub mod templates;

pub use action::*;
pub use condition::*;
pub use context::*;
pub use engine::*;
pub use error::*;
pub use explanation::*;
pub use loader::*;
pub use operator::*;
pub use policy_set::*;
pub use rule::*;
