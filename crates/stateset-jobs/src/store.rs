//! Pluggable storage backend for job instances.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
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
/// Share one open store through clones; independent writers to the same path
/// are not coordinated. Mutations use unique temporary files and atomic
/// replacement, syncing the file and (on Unix) its containing directory.
/// Pre-replacement failures leave memory and the previous snapshot unchanged.
/// A directory-sync failure after replacement reports an error but retains the
/// new memory state, matching the visible snapshot; crash durability is uncertain.
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
        // Only NotFound means a new store. Permission and other lookup failures
        // must not be mistaken for permission to replace an existing snapshot.
        let (map, create) = match fs::read_to_string(&path) {
            Ok(content) => (Self::parse_snapshot(&content)?, false),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (HashMap::new(), true),
            Err(error) => {
                return Err(JobError::StoreError(format!("read snapshot failed: {error}")));
            }
        };
        let store = Self { path, inner: Arc::new(Mutex::new(map)) };
        if create {
            store.mutate(|_| Ok(()))?;
        }
        Ok(store)
    }

    fn parse_snapshot(content: &str) -> Result<HashMap<Uuid, JobInstance>, JobError> {
        let map: HashMap<Uuid, JobInstance> = serde_json::from_str(content)
            .map_err(|e| JobError::StoreError(format!("parse snapshot failed: {e}")))?;
        for (id, job) in &map {
            if *id != job.id {
                return Err(JobError::StoreError(format!(
                    "snapshot key {id} does not match job id {}",
                    job.id
                )));
            }
        }
        Ok(map)
    }

    fn mutate<T>(
        &self,
        edit: impl FnOnce(&mut HashMap<Uuid, JobInstance>) -> Result<T, JobError>,
    ) -> Result<T, JobError> {
        self.mutate_with_directory_sync(edit, |parent| {
            #[cfg(unix)]
            fs::File::open(parent)?.sync_all()?;
            #[cfg(not(unix))]
            let _ = parent;
            Ok(())
        })
    }

    fn mutate_with_directory_sync<T>(
        &self,
        edit: impl FnOnce(&mut HashMap<Uuid, JobInstance>) -> Result<T, JobError>,
        sync_directory: impl FnOnce(&Path) -> std::io::Result<()>,
    ) -> Result<T, JobError> {
        let mut map =
            self.inner.lock().map_err(|e| JobError::StoreError(format!("lock poisoned: {e}")))?;
        let mut candidate = map.clone();
        let result = edit(&mut candidate)?;
        let serialized = serde_json::to_vec_pretty(&candidate)
            .map_err(|e| JobError::StoreError(format!("serialize snapshot failed: {e}")))?;
        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|e| JobError::StoreError(format!("create store directory failed: {e}")))?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| JobError::StoreError(format!("create snapshot failed: {e}")))?;
        temporary
            .write_all(&serialized)
            .map_err(|e| JobError::StoreError(format!("write snapshot failed: {e}")))?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|e| JobError::StoreError(format!("sync snapshot failed: {e}")))?;
        temporary
            .persist(&self.path)
            .map_err(|e| JobError::StoreError(format!("replace snapshot failed: {e}")))?;
        // Rename is the visibility commit point. Never roll memory back after
        // it, even if syncing the directory reports uncertain crash durability.
        *map = candidate;
        sync_directory(parent).map_err(|e| {
            JobError::StoreError(format!("sync snapshot directory failed after replacement: {e}"))
        })?;
        Ok(result)
    }
}

impl JobStore for FileJobStore {
    fn save(&self, job: &JobInstance) -> Result<(), JobError> {
        self.mutate(|map| {
            map.insert(job.id, job.clone());
            Ok(())
        })
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
        self.mutate(|map| match map.get_mut(id) {
            Some(job) => job.transition_to(status),
            None => Err(JobError::NotFound(*id)),
        })
    }

    fn delete_completed_before(&self, before: DateTime<Utc>) -> Result<u64, JobError> {
        self.mutate(|map| {
            let initial_len = map.len();
            map.retain(|_, job| {
                !(job.status == JobStatus::Completed
                    && job.completed_at.is_some_and(|t| t < before))
            });
            Ok((initial_len - map.len()) as u64)
        })
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
    fn file_store_failed_mutations_preserve_memory_and_last_snapshot() {
        let dir = tempdir().unwrap();
        let live = dir.path().join("live");
        let offline = dir.path().join("offline");
        let path = live.join("jobs.json");
        let store = FileJobStore::open(&path).unwrap();
        let peer = store.clone();
        let pending = make_instance("pending");
        let completed = make_completed_instance("completed", Utc::now() - Duration::hours(2));
        store.save(&pending).unwrap();
        store.save(&completed).unwrap();
        let before = fs::read(&path).unwrap();

        // Make the parent unusable without relying on chmod (which root can
        // bypass). The previous snapshot remains available at its moved path.
        fs::rename(&live, &offline).unwrap();
        fs::write(&live, "parent is unavailable").unwrap();
        let refused = make_instance("refused");
        assert!(store.save(&refused).is_err());
        let mut updated = pending.clone();
        updated.definition_name = "uncommitted update".into();
        assert!(store.save(&updated).is_err());
        assert!(store.update_status(&pending.id, JobStatus::Running).is_err());
        assert!(store.delete_completed_before(Utc::now()).is_err());
        for reader in [&store, &peer] {
            assert!(reader.get(&refused.id).unwrap().is_none());
            let retained = reader.get(&pending.id).unwrap().unwrap();
            assert_eq!(retained.definition_name, "pending");
            assert_eq!(retained.status, pending.status);
            assert!(reader.get(&completed.id).unwrap().is_some());
        }
        assert_eq!(fs::read(offline.join("jobs.json")).unwrap(), before);
        fs::remove_file(&live).unwrap();
        fs::rename(&offline, &live).unwrap();
        let reopened = FileJobStore::open(&path).unwrap();
        assert_eq!(reopened.get(&pending.id).unwrap().unwrap().status, pending.status);
        assert!(reopened.get(&refused.id).unwrap().is_none());
        assert!(reopened.get(&completed.id).unwrap().is_some());
        // A later successful commit must not accidentally flush failed edits.
        store.save(&refused).unwrap();
        let reopened = FileJobStore::open(&path).unwrap();
        assert_eq!(reopened.get(&pending.id).unwrap().unwrap().definition_name, "pending");
        assert!(reopened.get(&completed.id).unwrap().is_some());
        assert!(reopened.get(&refused.id).unwrap().is_some());
    }

    #[test]
    fn file_store_failed_replacement_cleans_temporary_snapshot() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let original = dir.path().join("original.json");
        let store = FileJobStore::open(&path).unwrap();
        let job = make_instance("pending");
        store.save(&job).unwrap();
        fs::rename(&path, &original).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(store.update_status(&job.id, JobStatus::Running).is_err());
        assert_eq!(store.get(&job.id).unwrap().unwrap().status, job.status);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
        fs::remove_dir(&path).unwrap();
        fs::rename(&original, &path).unwrap();
        assert_eq!(
            FileJobStore::open(&path).unwrap().get(&job.id).unwrap().unwrap().status,
            job.status
        );
        store.update_status(&job.id, JobStatus::Running).unwrap();
        assert_eq!(
            FileJobStore::open(&path).unwrap().get(&job.id).unwrap().unwrap().status,
            JobStatus::Running
        );
    }

    #[test]
    fn file_store_directory_sync_error_keeps_visible_commit_in_memory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let store = FileJobStore::open(&path).unwrap();
        let peer = store.clone();
        let job = make_instance("committed but durability uncertain");
        let error = store
            .mutate_with_directory_sync(
                |map| {
                    map.insert(job.id, job.clone());
                    Ok(())
                },
                |_| Err(std::io::Error::other("injected directory sync failure")),
            )
            .unwrap_err();
        assert!(error.to_string().contains("after replacement"));
        assert!(store.get(&job.id).unwrap().is_some());
        assert!(peer.get(&job.id).unwrap().is_some());
        assert!(FileJobStore::open(&path).unwrap().get(&job.id).unwrap().is_some());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn file_store_does_not_overwrite_a_neighboring_tmp_file() {
        let dir = tempdir().unwrap();
        let neighbor = dir.path().join("jobs.tmp");
        fs::write(&neighbor, "owned by another application").unwrap();
        let store = FileJobStore::open(dir.path().join("jobs.json")).unwrap();
        store.save(&make_instance("pending")).unwrap();
        assert_eq!(fs::read_to_string(neighbor).unwrap(), "owned by another application");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn file_store_rejects_damaged_snapshots_without_replacing_them() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        for content in ["", "  \n", "{", "[]", "null"] {
            fs::write(&path, content).unwrap();
            assert!(matches!(FileJobStore::open(&path), Err(JobError::StoreError(_))));
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
        let wrong_key = Uuid::new_v4();
        let content =
            serde_json::to_string(&HashMap::from([(wrong_key, make_instance("mismatched"))]))
                .unwrap();
        fs::write(&path, &content).unwrap();
        assert!(matches!(FileJobStore::open(&path), Err(JobError::StoreError(_))));
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
        fs::write(&path, "{}").unwrap();
        assert!(FileJobStore::open(&path).unwrap().list_active().unwrap().is_empty());
    }

    #[test]
    fn file_store_cloned_writers_preserve_every_committed_job() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let store = FileJobStore::open(&path).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let writers: Vec<_> = (0..4)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    (0..8)
                        .map(|_| {
                            let job = make_instance("concurrent");
                            store.save(&job).unwrap();
                            job.id
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let ids: Vec<_> = writers.into_iter().flat_map(|writer| writer.join().unwrap()).collect();
        let reopened = FileJobStore::open(path).unwrap();
        assert_eq!(reopened.list_active().unwrap().len(), ids.len());
        for id in ids {
            assert!(store.get(&id).unwrap().is_some());
            assert!(reopened.get(&id).unwrap().is_some());
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
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
