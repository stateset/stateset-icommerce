#![deny(unsafe_code)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

//! Test helpers for cross-boundary integration tests.
//!
//! This crate provides utilities for testing full Commerce pipelines
//! that span multiple internal crates (embedded, db, core, events, policy).

// These dependencies are used by integration test files in `tests/`, not by
// `lib.rs` itself. Silence the `unused_crate_dependencies` lint.
use chrono as _;
use rust_decimal as _;
use rust_decimal_macros as _;
use serde as _;
use serde_json as _;
use stateset_core as _;
use stateset_crypto as _;
use stateset_db as _;
use stateset_observability as _;
use stateset_policy as _;
use stateset_primitives as _;
use stateset_test_utils as _;
use tokio as _;
use uuid as _;

use stateset_embedded::Commerce;
use tempfile::TempDir;

/// Create a Commerce instance backed by a temporary SQLite database.
///
/// Returns both the `Commerce` instance and the `TempDir` guard.
/// The temporary directory (and database) is cleaned up when the
/// `TempDir` is dropped, so callers must keep it alive for the
/// duration of the test.
#[must_use] 
pub fn create_test_commerce() -> (Commerce, TempDir) {
    let dir = TempDir::new().expect("temp dir");
    let db_path = dir.path().join("test.db");
    let commerce = Commerce::new(db_path.to_str().unwrap()).expect("commerce");
    (commerce, dir)
}

/// Create a Commerce instance using an in-memory SQLite database.
///
/// Simpler than [`create_test_commerce`] but the database cannot be
/// inspected on disk after the test.
#[must_use] 
pub fn create_in_memory_commerce() -> Commerce {
    Commerce::in_memory().expect("in-memory commerce")
}
