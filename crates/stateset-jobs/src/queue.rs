//! Time-sorted job queue.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::JobError;
use crate::state::JobInstance;

/// A priority queue for job instances, sorted by scheduled run time.
#[derive(Debug)]
pub struct JobQueue {
    /// Jobs indexed by their scheduled run time.
    jobs: BTreeMap<DateTime<Utc>, Vec<JobInstance>>,
    /// Maximum number of jobs allowed in the queue.
    max_size: usize,
    /// Current total count of jobs across all time slots.
    count: usize,
}

impl JobQueue {
    /// Create a new queue with the given maximum capacity.
    #[must_use]
    pub const fn new(max_size: usize) -> Self {
        Self { jobs: BTreeMap::new(), max_size, count: 0 }
    }

    /// Add a job to the queue at its scheduled time.
    ///
    /// If `next_run_at` is `None`, the job is scheduled for `Utc::now()`.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::QueueFull`] if the queue is at capacity.
    pub fn enqueue(&mut self, job: JobInstance) -> Result<(), JobError> {
        if self.count >= self.max_size {
            return Err(JobError::QueueFull { capacity: self.max_size, current: self.count });
        }

        let run_at = job.next_run_at.unwrap_or_else(Utc::now);
        self.jobs.entry(run_at).or_default().push(job);
        self.count += 1;
        Ok(())
    }

    /// Remove and return all jobs that are due at or before `now`.
    pub fn dequeue_ready(&mut self, now: DateTime<Utc>) -> Vec<JobInstance> {
        let mut ready = Vec::new();

        // Collect all keys <= now
        let due_keys: Vec<DateTime<Utc>> = self.jobs.range(..=now).map(|(k, _)| *k).collect();

        for key in due_keys {
            if let Some(jobs) = self.jobs.remove(&key) {
                self.count -= jobs.len();
                ready.extend(jobs);
            }
        }

        ready
    }

    /// Peek at the next scheduled time without removing anything.
    #[must_use]
    pub fn peek_next(&self) -> Option<DateTime<Utc>> {
        self.jobs.keys().next().copied()
    }

    /// The total number of jobs in the queue.
    #[must_use]
    pub const fn size(&self) -> usize {
        self.count
    }

    /// Returns `true` if the queue contains no jobs.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The maximum capacity of the queue.
    #[must_use]
    pub const fn max_size(&self) -> usize {
        self.max_size
    }

    /// Remove all jobs from the queue.
    pub fn clear(&mut self) {
        self.jobs.clear();
        self.count = 0;
    }

    /// Cancel a job by ID, marking it as [`JobStatus::Cancelled`].
    ///
    /// Returns `true` if the job was found and cancelled.
    pub fn cancel(&mut self, job_id: Uuid) -> bool {
        let keys: Vec<DateTime<Utc>> = self.jobs.keys().copied().collect();
        for key in keys {
            let mut should_remove_bucket = false;
            let mut found = false;
            if let Some(jobs) = self.jobs.get_mut(&key) {
                if let Some(idx) = jobs.iter().position(|job| job.id == job_id) {
                    let mut job = jobs.remove(idx);
                    let _ = job.mark_cancelled();
                    self.count = self.count.saturating_sub(1);
                    should_remove_bucket = jobs.is_empty();
                    found = true;
                }
            }
            if should_remove_bucket {
                self.jobs.remove(&key);
            }
            if found {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn make_instance(name: &str, run_at: DateTime<Utc>) -> JobInstance {
        JobInstance::new_scheduled(name, run_at)
    }

    #[test]
    fn new_queue_is_empty() {
        let q = JobQueue::new(10);
        assert!(q.is_empty());
        assert_eq!(q.size(), 0);
        assert_eq!(q.max_size(), 10);
        assert!(q.peek_next().is_none());
    }

    #[test]
    fn enqueue_and_size() {
        let mut q = JobQueue::new(10);
        let now = Utc::now();
        q.enqueue(make_instance("a", now)).unwrap();
        assert_eq!(q.size(), 1);
        assert!(!q.is_empty());
    }

    #[test]
    fn enqueue_full_returns_error() {
        let mut q = JobQueue::new(1);
        let now = Utc::now();
        q.enqueue(make_instance("a", now)).unwrap();
        let result = q.enqueue(make_instance("b", now));
        assert!(result.is_err());
    }

    #[test]
    fn dequeue_ready_returns_due_jobs() {
        let mut q = JobQueue::new(10);
        let now = Utc::now();
        let past = now - Duration::seconds(10);
        let future = now + Duration::seconds(60);

        q.enqueue(make_instance("past", past)).unwrap();
        q.enqueue(make_instance("now", now)).unwrap();
        q.enqueue(make_instance("future", future)).unwrap();

        let ready = q.dequeue_ready(now);
        assert_eq!(ready.len(), 2);
        assert_eq!(q.size(), 1); // only the future one remains
    }

    #[test]
    fn dequeue_ready_empty_when_nothing_due() {
        let mut q = JobQueue::new(10);
        let future = Utc::now() + Duration::hours(1);
        q.enqueue(make_instance("future", future)).unwrap();

        let ready = q.dequeue_ready(Utc::now());
        assert!(ready.is_empty());
        assert_eq!(q.size(), 1);
    }

    #[test]
    fn peek_next_returns_earliest() {
        let mut q = JobQueue::new(10);
        let now = Utc::now();
        let later = now + Duration::seconds(30);
        let earliest = now - Duration::seconds(10);

        q.enqueue(make_instance("later", later)).unwrap();
        q.enqueue(make_instance("earliest", earliest)).unwrap();

        assert_eq!(q.peek_next(), Some(earliest));
    }

    #[test]
    fn clear_removes_all() {
        let mut q = JobQueue::new(10);
        let now = Utc::now();
        q.enqueue(make_instance("a", now)).unwrap();
        q.enqueue(make_instance("b", now)).unwrap();

        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.size(), 0);
    }

    #[test]
    fn cancel_marks_job_cancelled() {
        let mut q = JobQueue::new(10);
        let now = Utc::now();
        let inst = make_instance("test", now);
        let id = inst.id;
        q.enqueue(inst).unwrap();

        assert!(q.cancel(id));
        assert_eq!(q.size(), 0);
    }

    #[test]
    fn cancel_frees_queue_capacity() {
        let mut q = JobQueue::new(1);
        let now = Utc::now();
        let inst = make_instance("test", now);
        let id = inst.id;
        q.enqueue(inst).unwrap();
        assert!(q.cancel(id));
        assert!(q.enqueue(make_instance("replacement", now)).is_ok());
    }

    #[test]
    fn cancel_nonexistent_returns_false() {
        let mut q = JobQueue::new(10);
        assert!(!q.cancel(Uuid::new_v4()));
    }

    #[test]
    fn enqueue_without_next_run_at() {
        let mut q = JobQueue::new(10);
        let inst = JobInstance::new("no_time");
        q.enqueue(inst).unwrap();
        assert_eq!(q.size(), 1);
    }

    #[test]
    fn dequeue_preserves_order_within_same_time() {
        let mut q = JobQueue::new(10);
        let now = Utc::now();

        let a = make_instance("a", now);
        let b = make_instance("b", now);
        let a_id = a.id;

        q.enqueue(a).unwrap();
        q.enqueue(b).unwrap();

        let ready = q.dequeue_ready(now);
        assert_eq!(ready.len(), 2);
        assert_eq!(ready[0].id, a_id); // FIFO within same slot
    }

    #[test]
    fn multiple_dequeue_calls() {
        let mut q = JobQueue::new(10);
        let t1 = Utc::now();
        let t2 = t1 + Duration::seconds(10);

        q.enqueue(make_instance("a", t1)).unwrap();
        q.enqueue(make_instance("b", t2)).unwrap();

        let first = q.dequeue_ready(t1);
        assert_eq!(first.len(), 1);
        assert_eq!(q.size(), 1);

        let second = q.dequeue_ready(t2);
        assert_eq!(second.len(), 1);
        assert_eq!(q.size(), 0);
    }
}
