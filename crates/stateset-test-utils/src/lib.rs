#![deny(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]

//! Shared test fixtures and helpers for StateSet iCommerce.
//!
//! This crate consolidates common test data builders that were previously
//! duplicated across 25+ test files. It provides deterministic, composable
//! fixtures for domain objects.
//!
//! # Usage
//!
//! Add to your crate's `[dev-dependencies]`:
//!
//! ```toml
//! [dev-dependencies]
//! stateset-test-utils = { path = "../stateset-test-utils" }
//! ```
//!
//! Then use the builders in tests:
//!
//! ```rust
//! use stateset_test_utils::fixtures;
//!
//! let customer = fixtures::create_customer_input();
//! assert!(!customer.email.is_empty());
//!
//! let order = fixtures::create_order_input(stateset_core::CustomerId::new());
//! assert_eq!(order.items.len(), 1);
//! ```

pub mod assertions;
pub mod fixtures;

// Re-export commonly used items for convenience.
pub use fixtures::*;
