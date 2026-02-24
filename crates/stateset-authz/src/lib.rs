#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! # StateSet Authorization
//!
//! IO-free, framework-agnostic authorization engine for the StateSet iCommerce
//! platform. This crate is a Rust port of the permission model in
//! `cli/src/permissions.js`, designed to be reusable across runtimes (CLI,
//! server, WASM, NAPI bindings).
//!
//! ## Features
//!
//! - **Permission levels** — `None` < `Read` < `Preview` < `Write` < `Delete` < `Admin`
//! - **Role-based access control** — built-in roles (`admin`, `operator`, `viewer`, `none`)
//!   plus a [`RoleBuilder`] for custom roles
//! - **Window-based rate limiting** — per-actor, per-resource request tracking
//! - **Audit logging** — in-memory ring buffer with filtering by actor, resource, time
//! - **Sensitive field redaction** — recursive JSON traversal + partial string masking
//! - **Access decisions** — `Allowed`, `Denied`, `RequiresApproval` with reasons
//! - **Zero IO** — no filesystem, network, or database access; bring your own persistence
//!
//! ## Quick Start
//!
//! ```rust
//! use stateset_authz::{AuthzEngineBuilder, Role, Action, Resource, RateLimitRule};
//! use std::time::Duration;
//!
//! let mut engine = AuthzEngineBuilder::new()
//!     .add_role(Role::admin())
//!     .add_role(Role::viewer())
//!     .assign_role("alice", "admin")
//!     .assign_role("bob", "viewer")
//!     .rate_limit_rule(RateLimitRule::new("orders", 100, Duration::from_secs(60)))
//!     .build();
//!
//! // Alice (admin) can create orders
//! let decision = engine.authorize("alice", &Resource::new("orders"), &Action::Create);
//! assert!(decision.is_allowed());
//!
//! // Bob (viewer) cannot create orders
//! let decision = engine.authorize("bob", &Resource::new("orders"), &Action::Create);
//! assert!(decision.is_denied());
//!
//! // Every decision is recorded in the audit log
//! assert_eq!(engine.audit_log().len(), 2);
//! ```
//!
//! ## Custom Roles
//!
//! ```rust
//! use stateset_authz::{RoleBuilder, PermissionLevel, Action};
//!
//! let order_manager = RoleBuilder::new("order-manager")
//!     .default_level(PermissionLevel::Read)
//!     .allow("orders", PermissionLevel::Admin)
//!     .allow("customers", PermissionLevel::Write)
//!     .build();
//!
//! assert!(order_manager.check("orders", &Action::Delete).is_allowed());
//! assert!(order_manager.check("customers", &Action::Create).is_allowed());
//! assert!(order_manager.check("inventory", &Action::Create).is_denied());
//! ```
//!
//! ## Redaction
//!
//! ```rust
//! use stateset_authz::{RedactionConfig, redact_value, redact_string};
//! use serde_json::json;
//!
//! let config = RedactionConfig::default();
//! let mut data = json!({ "name": "Alice", "password": "s3cr3t", "token": "abc" });
//! redact_value(&mut data, &config);
//!
//! assert_eq!(data["name"], "Alice");
//! assert_eq!(data["password"], "[REDACTED]");
//! assert_eq!(data["token"], "[REDACTED]");
//!
//! assert_eq!(redact_string("secret123"), "sec***123");
//! ```

mod audit;
mod decision;
mod engine;
mod error;
mod permission;
mod rate_limit;
mod redaction;
mod resource;
mod role;

pub use audit::*;
pub use decision::*;
pub use engine::*;
pub use error::*;
pub use permission::*;
pub use rate_limit::*;
pub use redaction::*;
pub use resource::*;
pub use role::*;
