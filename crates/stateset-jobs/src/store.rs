//! Pluggable storage backend for job instances.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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

    /// List active jobs that should be recovered on scheduler startup.
    ///
    /// The default implementation unions jobs in `Pending`, `Scheduled`,
    /// `Retrying`, and `Running` states.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::StoreError`] on storage failures.
    fn list_active(&self) -> Result<Vec<JobInstance>, JobError> {
        let mut combined: HashMap<Uuid, JobInstance> = HashMap::new();
        for status in
            [JobStatus::Pending, JobStatus::Scheduled, JobStatus::Retrying, JobStatus::Running]
        {
            for job in self.list_by_status(status)? {
                combined.insert(job.id, job);
            }
        }
        Ok(combined.into_values().collect())
    }
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
        Self { inner: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn lock_map_unpoisoned(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, JobInstance>> {
        match self.inner.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// Returns the total number of stored jobs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock_map_unpoisoned().len()
    }

    /// Returns `true` if the store is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock_map_unpoisoned().is_empty()
    }
}

impl Default for InMemoryJobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl JobStore for InMemoryJobStore {
    fn save(&self, job: &JobInstance) -> Result<(), JobError> {
        let mut map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        map.insert(job.id, job.clone());
        Ok(())
    }

    fn get(&self, id: &Uuid) -> Result<Option<JobInstance>, JobError> {
        let map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        Ok(map.get(id).cloned())
    }

    fn list_by_status(&self, status: JobStatus) -> Result<Vec<JobInstance>, JobError> {
        let map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        Ok(map.values().filter(|j| j.status == status).cloned().collect())
    }

    fn update_status(&self, id: &Uuid, status: JobStatus) -> Result<(), JobError> {
        let mut map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        match map.get_mut(id) {
            Some(job) => {
                job.transition_to(status)?;
                Ok(())
            }
            None => Err(JobError::NotFound(*id)),
        }
    }

    fn delete_completed_before(&self, before: DateTime<Utc>) -> Result<u64, JobError> {
        let mut map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        let initial_len = map.len();
        map.retain(|_, job| {
            !(job.status == JobStatus::Completed && job.completed_at.is_some_and(|t| t < before))
        });
        Ok((initial_len - map.len()) as u64)
    }
}

// ---------------------------------------------------------------------------
// FileJobStore
// ---------------------------------------------------------------------------

/// A file-backed [`JobStore`] implementation using JSON snapshots.
///
/// Designed for single-process durability and restart recovery.
#[derive(Debug, Clone)]
pub struct FileJobStore {
    path: PathBuf,
    inner: Arc<Mutex<HashMap<Uuid, JobInstance>>>,
}

impl FileJobStore {
    /// Open or create a file-backed job store at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::StoreError`] if loading fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, JobError> {
        let path = path.as_ref().to_path_buf();
        let map = if path.exists() { Self::read_snapshot(&path)? } else { HashMap::new() };

        let store = Self { path, inner: Arc::new(Mutex::new(map)) };
        if !store.path.exists() {
            store.persist_map(&store.lock_map_unpoisoned())?;
        }
        Ok(store)
    }

    fn lock_map_unpoisoned(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, JobInstance>> {
        match self.inner.lock() {
            Ok(map) => map,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn read_snapshot(path: &Path) -> Result<HashMap<Uuid, JobInstance>, JobError> {
        let content = fs::read_to_string(path)
            .map_err(|e| JobError::StoreError(format!("read snapshot failed: {e}")))?;
        if content.trim().is_empty() {
            return Ok(HashMap::new());
        }
        serde_json::from_str(&content)
            .map_err(|e| JobError::StoreError(format!("parse snapshot failed: {e}")))
    }

    fn persist_map(&self, map: &HashMap<Uuid, JobInstance>) -> Result<(), JobError> {
        let serialized = serde_json::to_string_pretty(map)
            .map_err(|e| JobError::StoreError(format!("serialize snapshot failed: {e}")))?;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| JobError::StoreError(format!("create store directory failed: {e}")))?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serialized)
            .map_err(|e| JobError::StoreError(format!("write snapshot failed: {e}")))?;
        fs::rename(&tmp, &self.path)
            .map_err(|e| JobError::StoreError(format!("replace snapshot failed: {e}")))?;
        Ok(())
    }
}

impl JobStore for FileJobStore {
    fn save(&self, job: &JobInstance) -> Result<(), JobError> {
        let mut map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        map.insert(job.id, job.clone());
        self.persist_map(&map)
    }

    fn get(&self, id: &Uuid) -> Result<Option<JobInstance>, JobError> {
        let map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        Ok(map.get(id).cloned())
    }

    fn list_by_status(&self, status: JobStatus) -> Result<Vec<JobInstance>, JobError> {
        let map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        Ok(map.values().filter(|j| j.status == status).cloned().collect())
    }

    fn update_status(&self, id: &Uuid, status: JobStatus) -> Result<(), JobError> {
        let mut map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        match map.get_mut(id) {
            Some(job) => {
                job.transition_to(status)?;
                self.persist_map(&map)
            }
            None => Err(JobError::NotFound(*id)),
        }
    }

    fn delete_completed_before(&self, before: DateTime<Utc>) -> Result<u64, JobError> {
        let mut map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        let initial_len = map.len();
        map.retain(|_, job| {
            !(job.status == JobStatus::Completed && job.completed_at.is_some_and(|t| t < before))
        });
        let deleted = (initial_len - map.len()) as u64;
        self.persist_map(&map)?;
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use tempfile::tempdir;

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
    fn update_status_rejects_invalid_transition() {
        let store = InMemoryJobStore::new();
        let inst = make_instance("test");
        let id = inst.id;
        store.save(&inst).unwrap();

        let result = store.update_status(&id, JobStatus::Completed);
        assert!(matches!(result, Err(JobError::InvalidTransition { .. })));
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

        let clone = store;
        assert!(clone.get(&id).unwrap().is_some());
        assert_eq!(clone.len(), 1);
    }

    #[test]
    fn file_store_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let store = FileJobStore::open(&path).unwrap();

        let inst = make_instance("file-roundtrip");
        let id = inst.id;
        store.save(&inst).unwrap();

        let reopened = FileJobStore::open(&path).unwrap();
        let retrieved = reopened.get(&id).unwrap().unwrap();
        assert_eq!(retrieved.definition_name, "file-roundtrip");
    }

    #[test]
    fn file_store_list_active_includes_running() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jobs-active.json");
        let store = FileJobStore::open(&path).unwrap();

        let mut running = make_instance("running");
        running.status = JobStatus::Running;
        running.started_at = Some(Utc::now());
        store.save(&running).unwrap();

        let mut completed = make_instance("completed");
        completed.status = JobStatus::Completed;
        completed.completed_at = Some(Utc::now());
        store.save(&completed).unwrap();

        let active = store.list_active().unwrap();
        assert!(active.iter().any(|job| job.id == running.id));
        assert!(!active.iter().any(|job| job.id == completed.id));
    }
}
