//! Error types for the jobs crate.

use thiserror::Error;

/// Errors that can occur during job operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JobError {
    /// The job handler returned an error during execution.
    #[error("execution failed: {0}")]
    ExecutionFailed(String),

    /// The job exceeded its configured timeout.
    #[error("job timed out after {0:?}")]
    Timeout(std::time::Duration),

    /// The job has exhausted all retry attempts.
    #[error("max retries exceeded: {attempts} of {max}")]
    MaxRetriesExceeded {
        /// Number of attempts made.
        attempts: u32,
        /// Maximum allowed attempts.
        max: u32,
    },

    /// The provided cron expression or schedule is invalid.
    #[error("invalid schedule: {0}")]
    InvalidSchedule(String),

    /// The job queue has reached its maximum capacity.
    #[error("queue full: capacity {capacity}, current {current}")]
    QueueFull {
        /// Maximum allowed jobs.
        capacity: usize,
        /// Current job count.
        current: usize,
    },

    /// The requested job was not found.
    #[error("job not found: {0}")]
    NotFound(uuid::Uuid),

    /// An error occurred in the underlying store.
    #[error("store error: {0}")]
    StoreError(String),

    /// The requested state transition is not valid.
    #[error("invalid transition from {from} to {to}")]
    InvalidTransition {
        /// Current status.
        from: String,
        /// Attempted target status.
        to: String,
    },
}
