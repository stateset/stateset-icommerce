//! Execution context passed to job handlers.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Context provided to a [`JobHandler`](crate::job::JobHandler) during execution.
///
/// Contains metadata about the current job instance and attempt.
#[derive(Debug, Clone)]
pub struct JobContext {
    /// The unique identifier of the job instance being executed.
    pub job_id: Uuid,
    /// The current attempt number (0 on the first try).
    pub attempt: u32,
    /// When this execution was scheduled to run.
    pub scheduled_at: DateTime<Utc>,
    /// Arbitrary key-value metadata.
    pub metadata: HashMap<String, String>,
}

impl JobContext {
    /// Create a new job context.
    #[must_use]
    pub fn new(job_id: Uuid, attempt: u32, scheduled_at: DateTime<Utc>) -> Self {
        Self { job_id, attempt, scheduled_at, metadata: HashMap::new() }
    }

    /// Add a metadata entry.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get a metadata value by key.
    #[must_use]
    pub fn get_metadata(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_new() {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let ctx = JobContext::new(id, 0, now);
        assert_eq!(ctx.job_id, id);
        assert_eq!(ctx.attempt, 0);
        assert_eq!(ctx.scheduled_at, now);
        assert!(ctx.metadata.is_empty());
    }

    #[test]
    fn context_with_metadata() {
        let ctx = JobContext::new(Uuid::new_v4(), 1, Utc::now())
            .with_metadata("region", "us-east-1")
            .with_metadata("priority", "high");
        assert_eq!(ctx.get_metadata("region"), Some("us-east-1"));
        assert_eq!(ctx.get_metadata("priority"), Some("high"));
        assert_eq!(ctx.get_metadata("missing"), None);
    }

    #[test]
    fn context_metadata_overwrite() {
        let ctx = JobContext::new(Uuid::new_v4(), 0, Utc::now())
            .with_metadata("key", "v1")
            .with_metadata("key", "v2");
        assert_eq!(ctx.get_metadata("key"), Some("v2"));
    }

    #[test]
    fn context_debug() {
        let ctx = JobContext::new(Uuid::new_v4(), 0, Utc::now());
        let debug = format!("{ctx:?}");
        assert!(debug.contains("JobContext"));
    }
}
