//! Pluggable storage backend for job instances.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::JobError;
use crate::state::{JobInstance, JobStatus};

/// Trait for persisting and querying job instances.
pub trait JobStore: Send + Sync {
    /// Save (insert or update) a job instance.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::StoreError`] on storage failures.
    fn save(&self, job: &JobInstance) -> Result<(), JobError>;

    /// Retrieve a job by its unique ID.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::StoreError`] on storage failures.
    fn get(&self, id: &Uuid) -> Result<Option<JobInstance>, JobError>;

    /// List all jobs with the given status.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::StoreError`] on storage failures.
    fn list_by_status(&self, status: JobStatus) -> Result<Vec<JobInstance>, JobError>;

    /// Update the status of a job.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if the job does not exist, or
    /// [`JobError::StoreError`] on storage failures.
    fn update_status(&self, id: &Uuid, status: JobStatus) -> Result<(), JobError>;

    /// Delete all completed jobs older than the given cutoff.
    ///
    /// Returns the number of deleted records.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::StoreError`] on storage failures.
    fn delete_completed_before(&self, before: DateTime<Utc>) -> Result<u64, JobError>;
}

// ---------------------------------------------------------------------------
// InMemoryJobStore
// ---------------------------------------------------------------------------

/// An in-memory [`JobStore`] implementation, primarily for testing.
#[derive(Debug, Clone)]
pub struct InMemoryJobStore {
    inner: Arc<Mutex<HashMap<Uuid, JobInstance>>>,
}

impl InMemoryJobStore {
    /// Create a new empty in-memory store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns the total number of stored jobs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.lock().expect("lock poisoned").len()
    }

    /// Returns `true` if the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.lock().expect("lock poisoned").is_empty()
    }
}

impl Default for InMemoryJobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl JobStore for InMemoryJobStore {
    fn save(&self, job: &JobInstance) -> Result<(), JobError> {
        let mut map = self.inner.lock().map_err(|e| {
            JobError::StoreError(format!("lock poisoned: {e}"))
        })?;
        map.insert(job.id, job.clone());
        Ok(())
    }

    fn get(&self, id: &Uuid) -> Result<Option<JobInstance>, JobError> {
        let map = self.inner.lock().map_err(|e| {
            JobError::StoreError(format!("lock poisoned: {e}"))
        })?;
        Ok(map.get(id).cloned())
    }

    fn list_by_status(&self, status: JobStatus) -> Result<Vec<JobInstance>, JobError> {
        let map = self.inner.lock().map_err(|e| {
            JobError::StoreError(format!("lock poisoned: {e}"))
        })?;
        Ok(map
            .values()
            .filter(|j| j.status == status)
            .cloned()
            .collect())
    }

    fn update_status(&self, id: &Uuid, status: JobStatus) -> Result<(), JobError> {
        let mut map = self.inner.lock().map_err(|e| {
            JobError::StoreError(format!("lock poisoned: {e}"))
        })?;
        match map.get_mut(id) {
            Some(job) => {
                job.status = status;
                Ok(())
            }
            None => Err(JobError::NotFound(*id)),
        }
    }

    fn delete_completed_before(&self, before: DateTime<Utc>) -> Result<u64, JobError> {
        let mut map = self.inner.lock().map_err(|e| {
            JobError::StoreError(format!("lock poisoned: {e}"))
        })?;
        let initial_len = map.len();
        map.retain(|_, job| {
            !(job.status == JobStatus::Completed
                && job.completed_at.is_some_and(|t| t < before))
        });
        Ok((initial_len - map.len()) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_instance(name: &str) -> JobInstance {
        JobInstance::new(name)
    }

    fn make_completed_instance(name: &str, completed_at: DateTime<Utc>) -> JobInstance {
        let mut inst = JobInstance::new(name);
        inst.status = JobStatus::Running;
        inst.mark_completed(crate::state::JobOutput::new("done")).unwrap();
        // Override completed_at for testing
        inst.completed_at = Some(completed_at);
        inst
    }

    #[test]
    fn store_new_is_empty() {
        let store = InMemoryJobStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn store_default_is_empty() {
        let store = InMemoryJobStore::default();
        assert!(store.is_empty());
    }

    #[test]
    fn save_and_get() {
        let store = InMemoryJobStore::new();
        let inst = make_instance("test");
        let id = inst.id;
        store.save(&inst).unwrap();

        let retrieved = store.get(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().definition_name, "test");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let store = InMemoryJobStore::new();
        assert!(store.get(&Uuid::new_v4()).unwrap().is_none());
    }

    #[test]
    fn save_overwrites() {
        let store = InMemoryJobStore::new();
        let mut inst = make_instance("test");
        let id = inst.id;
        store.save(&inst).unwrap();

        inst.status = JobStatus::Running;
        store.save(&inst).unwrap();

        let retrieved = store.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.status, JobStatus::Running);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn list_by_status() {
        let store = InMemoryJobStore::new();

        let pending = make_instance("pending");
        store.save(&pending).unwrap();

        let mut running = make_instance("running");
        running.status = JobStatus::Running;
        store.save(&running).unwrap();

        let mut another_pending = make_instance("pending2");
        another_pending.status = JobStatus::Pending;
        store.save(&another_pending).unwrap();

        let results = store.list_by_status(JobStatus::Pending).unwrap();
        assert_eq!(results.len(), 2);

        let results = store.list_by_status(JobStatus::Running).unwrap();
        assert_eq!(results.len(), 1);

        let results = store.list_by_status(JobStatus::Completed).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn update_status() {
        let store = InMemoryJobStore::new();
        let inst = make_instance("test");
        let id = inst.id;
        store.save(&inst).unwrap();

        store.update_status(&id, JobStatus::Running).unwrap();
        let retrieved = store.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.status, JobStatus::Running);
    }

    #[test]
    fn update_status_not_found() {
        let store = InMemoryJobStore::new();
        let result = store.update_status(&Uuid::new_v4(), JobStatus::Running);
        assert!(result.is_err());
    }

    #[test]
    fn delete_completed_before() {
        let store = InMemoryJobStore::new();
        let now = Utc::now();
        let old = now - Duration::hours(2);
        let recent = now - Duration::minutes(5);

        let old_job = make_completed_instance("old", old);
        let recent_job = make_completed_instance("recent", recent);
        let pending_job = make_instance("pending");

        store.save(&old_job).unwrap();
        store.save(&recent_job).unwrap();
        store.save(&pending_job).unwrap();

        // Delete completed before 1 hour ago
        let cutoff = now - Duration::hours(1);
        let deleted = store.delete_completed_before(cutoff).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn delete_completed_before_no_match() {
        let store = InMemoryJobStore::new();
        let inst = make_instance("test");
        store.save(&inst).unwrap();

        let deleted = store.delete_completed_before(Utc::now()).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn store_clone_shares_data() {
        let store = InMemoryJobStore::new();
        let inst = make_instance("test");
        let id = inst.id;
        store.save(&inst).unwrap();

        let clone = store.clone();
        assert!(clone.get(&id).unwrap().is_some());
        assert_eq!(clone.len(), 1);
    }
}
