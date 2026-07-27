#![deny(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/stateset.png",
    html_favicon_url = "https://raw.githubusercontent.com/stateset/stateset-icommerce/main/assets/favicon.ico",
    issue_tracker_base_url = "https://github.com/stateset/stateset-icommerce/issues/"
)]
//! # StateSet Jobs
//!
//! A background work system for the StateSet iCommerce engine.
//!
//! **Status: standalone library.** No other workspace crate depends on this
//! crate yet — the engine does not schedule jobs implicitly. Embed it
//! directly (`cargo add stateset-jobs`) and register your own jobs against
//! your `Commerce` instance.
//!
//! This crate provides a Tokio-based async job scheduler supporting:
//!
//! - **Cron scheduling** — standard 5-field cron expressions
//! - **Interval scheduling** — repeat every *N* seconds/minutes
//! - **One-shot jobs** — run immediately, once
//! - **Event-triggered jobs** — fired by an event type string
//! - **Retry with backoff** — fixed, exponential, or linear strategies
//! - **State machine** — validated transitions for job lifecycle
//! - **Pluggable storage** — [`JobStore`] trait with [`InMemoryJobStore`] and [`FileJobStore`]
//! - **Built-in job types** — billing, webhook retry, retention, low stock, subscription renewal
//!
//! ## Quick Start
//!
//! ```rust
//! use stateset_jobs::{
//!     JobDefinition, Schedule, BackoffStrategy, JobContext, JobOutput,
//!     JobHandler, JobError, Scheduler, InMemoryJobStore,
//! };
//! use stateset_jobs::job::BoxFuture;
//! use std::time::Duration;
//!
//! struct MyJob;
//!
//! impl JobHandler for MyJob {
//!     fn name(&self) -> &str { "my_job" }
//!     fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
//!         Box::pin(async { Ok(JobOutput::new("done")) })
//!     }
//! }
//!
//! let def = JobDefinition::new("my_job", Schedule::Once, Box::new(MyJob))
//!     .with_timeout(Duration::from_secs(30))
//!     .with_max_retries(3)
//!     .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(5)));
//! ```
//!
//! ## Modules
//!
//! - [`job`] — [`JobDefinition`], [`Schedule`], [`BackoffStrategy`], [`JobHandler`]
//! - [`state`] — [`JobStatus`], [`JobInstance`], [`JobOutput`]
//! - [`context`] — [`JobContext`] passed to handlers
//! - [`queue`] — Time-sorted [`JobQueue`]
//! - [`store`] — [`JobStore`] trait, [`InMemoryJobStore`], and [`FileJobStore`]
//! - [`builtins`] — Pre-defined commerce job types
//! - [`scheduler`] — [`Scheduler`] orchestrator and [`TickAction`]
//! - [`error`] — [`JobError`] error type

pub mod builtins;
pub mod context;
pub mod error;
pub mod job;
pub mod queue;
pub mod scheduler;
pub mod state;
pub mod store;

// Re-exports for convenience
pub use builtins::{
    BillingTickJob, EventRetentionJob, LowStockAlertJob, SubscriptionRenewalJob, WebhookRetryJob,
};
pub use context::JobContext;
pub use error::JobError;
pub use job::{BackoffStrategy, JobDefinition, JobHandler, Schedule};
pub use queue::JobQueue;
pub use scheduler::{Scheduler, SchedulerStatus, TickAction};
pub use state::{JobInstance, JobOutput, JobStatus};
pub use store::{FileJobStore, InMemoryJobStore, JobStore};

/// Compiles the code examples in `README.md` as doctests, so the crates.io
/// landing page can never drift from the real API.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
