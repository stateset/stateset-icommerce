//! Activity log operations (append-only subject history).

use stateset_core::{ActivityLogEntry, ActivityLogFilter, ActivityLogId, RecordActivity, Result};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;
use uuid::Uuid;

/// Activity log operations.
pub struct ActivityLogs {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for ActivityLogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActivityLogs").finish_non_exhaustive()
    }
}

impl ActivityLogs {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether activity logs are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::ActivityLogs)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::ActivityLogs)
    }

    /// Record a new activity log entry.
    pub fn record(&self, input: RecordActivity) -> Result<ActivityLogEntry> {
        self.ensure()?;
        self.db.activity_logs().record(input)
    }

    /// Get an entry by ID.
    pub fn get(&self, id: ActivityLogId) -> Result<Option<ActivityLogEntry>> {
        self.ensure()?;
        self.db.activity_logs().get(id)
    }

    /// List entries with filter (most recent first).
    pub fn list(&self, filter: ActivityLogFilter) -> Result<Vec<ActivityLogEntry>> {
        self.ensure()?;
        self.db.activity_logs().list(filter)
    }

    /// List the full history for a single subject, most recent first.
    pub fn history_for_subject(
        &self,
        subject_type: &str,
        subject_id: Uuid,
    ) -> Result<Vec<ActivityLogEntry>> {
        self.ensure()?;
        self.db.activity_logs().history_for_subject(subject_type, subject_id)
    }
}
