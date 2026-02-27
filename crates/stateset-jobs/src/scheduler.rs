//! The main scheduler orchestrator.
//!
//! The scheduler manages job definitions, a time-sorted queue, and a pluggable
//! store. The core scheduling logic is synchronous (via [`Scheduler::tick`]),
//! which returns a list of [`TickAction`]s. This design makes the scheduler
//! fully testable without an async runtime.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::context::JobContext;
use crate::error::JobError;
use crate::job::JobDefinition;
#[cfg(test)]
use crate::job::Schedule;
use crate::queue::JobQueue;
use crate::state::{JobInstance, JobOutput, JobStatus};
use crate::store::JobStore;

// ---------------------------------------------------------------------------
// Default constants
// ---------------------------------------------------------------------------

/// Default maximum concurrent jobs.
const DEFAULT_MAX_CONCURRENT: usize = 5;

/// Default maximum queue size.
const DEFAULT_MAX_QUEUE_SIZE: usize = 10_000;

// ---------------------------------------------------------------------------
// TickAction
// ---------------------------------------------------------------------------

/// An action to be taken as a result of a scheduler tick.
///
/// Returned by [`Scheduler::tick`] so the caller can execute async work
/// outside the synchronous scheduling logic.
#[derive(Debug)]
#[non_exhaustive]
pub enum TickAction {
    /// Execute the job with the given ID and context.
    Execute {
        /// The job instance ID.
        job_id: Uuid,
        /// The definition name (to look up the handler).
        definition_name: String,
        /// The context to pass to the handler.
        context: JobContext,
    },
    /// A job has timed out and should be marked accordingly.
    Timeout {
        /// The job instance ID.
        job_id: Uuid,
    },
    /// A job should be retried.
    Retry {
        /// The job instance ID.
        job_id: Uuid,
        /// When to retry.
        retry_at: DateTime<Utc>,
    },
}

// ---------------------------------------------------------------------------
// SchedulerStatus
// ---------------------------------------------------------------------------

/// A snapshot of the scheduler's current state.
#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    /// Number of registered job definitions.
    pub registered_definitions: usize,
    /// Number of jobs currently running.
    pub running_jobs: usize,
    /// Number of jobs in the queue.
    pub queued_jobs: usize,
    /// Maximum concurrent jobs allowed.
    pub max_concurrent: usize,
    /// When the next job is due (if any).
    pub next_run_at: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

/// Orchestrates job scheduling, execution tracking, and retries.
///
/// The scheduler does not own an async runtime. Instead, [`tick`](Self::tick)
/// returns [`TickAction`]s that the caller is responsible for executing.
#[derive(Debug)]
pub struct Scheduler {
    definitions: HashMap<String, JobDefinition>,
    queue: JobQueue,
    store: Box<dyn JobStore>,
    running: HashMap<Uuid, RunningJob>,
    max_concurrent: usize,
}

/// Metadata about a currently running job.
#[derive(Debug, Clone)]
struct RunningJob {
    #[allow(dead_code)]
    definition_name: String,
    started_at: DateTime<Utc>,
    timeout: std::time::Duration,
}

impl Scheduler {
    /// Create a new scheduler with the given store.
    #[must_use]
    pub fn new(store: Box<dyn JobStore>) -> Self {
        Self {
            definitions: HashMap::new(),
            queue: JobQueue::new(DEFAULT_MAX_QUEUE_SIZE),
            store,
            running: HashMap::new(),
            max_concurrent: DEFAULT_MAX_CONCURRENT,
        }
    }

    /// Set the maximum number of concurrently running jobs.
    #[must_use]
    pub const fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Set the maximum queue size.
    #[must_use]
    pub fn with_max_queue_size(mut self, max: usize) -> Self {
        self.queue = JobQueue::new(max);
        self
    }

    /// Register a job definition.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidSchedule`] if the definition's schedule is invalid.
    pub fn register(&mut self, definition: JobDefinition) -> Result<(), JobError> {
        definition.validate()?;
        self.definitions.insert(definition.name.clone(), definition);
        Ok(())
    }

    /// Schedule a job for execution.
    ///
    /// Creates a new [`JobInstance`], saves it to the store, and enqueues it.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if the definition name is not registered,
    /// or [`JobError::QueueFull`] if the queue is at capacity.
    pub fn schedule(
        &mut self,
        definition_name: &str,
        now: DateTime<Utc>,
    ) -> Result<Uuid, JobError> {
        if !self.definitions.contains_key(definition_name) {
            return Err(JobError::NotFound(Uuid::nil()));
        }

        let instance = JobInstance::new_scheduled(definition_name, now);
        let id = instance.id;
        self.store.save(&instance)?;
        self.queue.enqueue(instance)?;

        Ok(id)
    }

    /// Process one scheduler tick.
    ///
    /// This method is intentionally synchronous. It:
    /// 1. Dequeues all jobs due at or before `now`.
    /// 2. Checks for running jobs that have timed out.
    /// 3. Returns a list of [`TickAction`]s for the caller to execute.
    pub fn tick(&mut self, now: DateTime<Utc>) -> Vec<TickAction> {
        let mut actions = Vec::new();

        // --- Check for timeouts on running jobs ---
        let timed_out: Vec<(Uuid, RunningJob)> = self
            .running
            .iter()
            .filter(|(_, rj)| {
                let elapsed = now
                    .signed_duration_since(rj.started_at)
                    .to_std()
                    .unwrap_or(std::time::Duration::ZERO);
                elapsed >= rj.timeout
            })
            .map(|(id, rj)| (*id, rj.clone()))
            .collect();

        for (id, timeout_meta) in timed_out {
            if self.handle_timeout(id, now, &timeout_meta, &mut actions) {
                self.running.remove(&id);
            }
        }

        // --- Dequeue ready jobs ---
        let available_slots = self.max_concurrent.saturating_sub(self.running.len());
        if available_slots == 0 {
            return actions;
        }

        let ready = self.queue.dequeue_ready(now);
        let mut to_run = Vec::with_capacity(available_slots.min(ready.len()));
        let mut deferred = Vec::new();
        for instance in ready {
            if instance.status == JobStatus::Cancelled {
                continue;
            }
            if to_run.len() < available_slots {
                to_run.push(instance);
            } else {
                deferred.push(instance);
            }
        }

        // Keep overflow jobs scheduled instead of dropping them.
        for instance in deferred {
            if let Err(err) = self.queue.enqueue(instance) {
                eprintln!("failed to re-enqueue deferred job: {err}");
            }
        }

        for mut instance in to_run {
            if instance.status == JobStatus::Cancelled {
                continue;
            }

            let scheduled_instance = instance.clone();
            let def_name = instance.definition_name.clone();

            let timeout = self
                .definitions
                .get(&def_name)
                .map(|d| d.timeout)
                .unwrap_or(std::time::Duration::from_secs(300));

            if instance.mark_running().is_err() {
                continue;
            }
            if let Err(err) = self.store.save(&instance) {
                eprintln!("failed to persist running job state: {err}");
                if let Err(requeue_err) = self.queue.enqueue(scheduled_instance) {
                    eprintln!("failed to re-enqueue job after save failure: {requeue_err}");
                }
                continue;
            }

            let ctx = JobContext::new(instance.id, instance.attempt, now);

            self.running.insert(
                instance.id,
                RunningJob { definition_name: def_name.clone(), started_at: now, timeout },
            );

            actions.push(TickAction::Execute {
                job_id: instance.id,
                definition_name: def_name,
                context: ctx,
            });
        }

        actions
    }

    fn handle_timeout(
        &mut self,
        job_id: Uuid,
        now: DateTime<Utc>,
        timeout_meta: &RunningJob,
        actions: &mut Vec<TickAction>,
    ) -> bool {
        let mut instance = match self.store.get(&job_id) {
            Ok(Some(instance)) => instance,
            Ok(None) => return true,
            Err(err) => {
                eprintln!("failed to load job state for timeout handling: {err}");
                return false;
            }
        };

        if instance.status != JobStatus::Running {
            return true;
        }

        let elapsed = now
            .signed_duration_since(timeout_meta.started_at)
            .to_std()
            .unwrap_or(std::time::Duration::ZERO);
        let timeout_msg = format!(
            "timed out after {}s (limit {}s)",
            elapsed.as_secs(),
            timeout_meta.timeout.as_secs()
        );

        if instance.mark_timed_out().is_err() {
            return false;
        }
        instance.completed_at = Some(now);
        instance.error = Some(timeout_msg.clone());
        if self.store.save(&instance).is_err() {
            return false;
        }
        actions.push(TickAction::Timeout { job_id });

        if let Some(def) = self.definitions.get(&instance.definition_name) {
            if instance.should_retry(def.max_retries) {
                let retry_at = instance.next_retry_at(&def.retry_backoff, now);
                if instance.mark_retrying(retry_at).is_ok() {
                    if self.store.save(&instance).is_err() {
                        return false;
                    }
                    if self.queue.enqueue(instance.clone()).is_err() {
                        return false;
                    }
                    actions.push(TickAction::Retry { job_id, retry_at });
                    return true;
                }
            }
        }

        if instance.mark_failed(format!("{timeout_msg}; retries exhausted")).is_ok() {
            instance.completed_at = Some(now);
            if self.store.save(&instance).is_err() {
                return false;
            }
        }
        if self.reschedule_recurring(&instance, now).is_err() {
            return false;
        }
        true
    }

    /// Mark a job as completed after successful execution.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if the job is not in the store.
    pub fn complete(&mut self, job_id: Uuid, output: JobOutput) -> Result<(), JobError> {
        self.complete_at(job_id, output, Utc::now())
    }

    /// Mark a job as completed after successful execution, using an explicit
    /// timestamp for deterministic testing and time-travel simulation.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if the job is not in the store.
    pub fn complete_at(
        &mut self,
        job_id: Uuid,
        output: JobOutput,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let mut instance = self.store.get(&job_id)?.ok_or(JobError::NotFound(job_id))?;

        instance.mark_completed(output)?;
        self.store.save(&instance)?;
        self.running.remove(&job_id);

        self.reschedule_recurring(&instance, now)?;

        Ok(())
    }

    /// Mark a job as failed after an execution error.
    ///
    /// If the job is eligible for retry, a [`Retry`](TickAction::Retry) action
    /// is returned.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if the job is not in the store.
    pub fn fail(&mut self, job_id: Uuid, error: &str) -> Result<Option<TickAction>, JobError> {
        self.fail_at(job_id, error, Utc::now())
    }

    /// Mark a job as failed after an execution error at an explicit time.
    ///
    /// Useful for deterministic retry scheduling in tests.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if the job is not in the store.
    pub fn fail_at(
        &mut self,
        job_id: Uuid,
        error: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<TickAction>, JobError> {
        let mut instance = self.store.get(&job_id)?.ok_or(JobError::NotFound(job_id))?;

        instance.mark_failed(error)?;
        self.store.save(&instance)?;
        self.running.remove(&job_id);

        let def = self.definitions.get(&instance.definition_name);
        if let Some(def) = def {
            if instance.should_retry(def.max_retries) {
                let retry_at = instance.next_retry_at(&def.retry_backoff, now);
                instance.mark_retrying(retry_at)?;
                self.store.save(&instance)?;
                self.queue.enqueue(instance)?;

                return Ok(Some(TickAction::Retry { job_id, retry_at }));
            }
        }

        self.reschedule_recurring(&instance, now)?;

        Ok(None)
    }

    /// Cancel a queued or running job.
    ///
    /// # Errors
    ///
    /// Returns [`JobError`] on store failures.
    pub fn cancel(&mut self, job_id: Uuid) -> Result<(), JobError> {
        self.running.remove(&job_id);
        self.queue.cancel(job_id);

        if let Some(mut instance) = self.store.get(&job_id)? {
            if !instance.status.is_terminal() {
                instance.mark_cancelled()?;
                self.store.save(&instance)?;
            }
        }

        Ok(())
    }

    /// Get the current scheduler status.
    #[must_use]
    pub fn status(&self) -> SchedulerStatus {
        SchedulerStatus {
            registered_definitions: self.definitions.len(),
            running_jobs: self.running.len(),
            queued_jobs: self.queue.size(),
            max_concurrent: self.max_concurrent,
            next_run_at: self.queue.peek_next(),
        }
    }

    /// Access the job store.
    #[must_use]
    pub fn store(&self) -> &dyn JobStore {
        &*self.store
    }

    /// Re-schedule a recurring (interval/cron) job for its next run.
    fn reschedule_recurring(
        &mut self,
        instance: &JobInstance,
        now: DateTime<Utc>,
    ) -> Result<(), JobError> {
        let def = match self.definitions.get(&instance.definition_name) {
            Some(d) => d,
            None => return Ok(()),
        };

        let next_run = def.schedule.next_run_after(now);

        if let Some(run_at) = next_run {
            let new_instance = JobInstance::new_scheduled(&instance.definition_name, run_at);
            self.store.save(&new_instance)?;
            self.queue.enqueue(new_instance)?;
        }

        Ok(())
    }
}

// Debug impl for Box<dyn JobStore>
impl std::fmt::Debug for dyn JobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("dyn JobStore")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::{BackoffStrategy, BoxFuture, JobHandler};
    use crate::store::InMemoryJobStore;
    use chrono::{Duration, TimeZone, Timelike};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingHandler {
        handler_name: String,
        call_count: Arc<AtomicU32>,
    }

    impl CountingHandler {
        fn new(name: &str) -> (Self, Arc<AtomicU32>) {
            let count = Arc::new(AtomicU32::new(0));
            (Self { handler_name: name.to_owned(), call_count: Arc::clone(&count) }, count)
        }
    }

    impl JobHandler for CountingHandler {
        fn execute<'a>(
            &'a self,
            _ctx: &'a JobContext,
        ) -> BoxFuture<'a, Result<JobOutput, JobError>> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(JobOutput::new("counted")) })
        }
        fn name(&self) -> &str {
            &self.handler_name
        }
    }

    fn make_scheduler() -> Scheduler {
        Scheduler::new(Box::new(InMemoryJobStore::new()))
    }

    fn register_noop(scheduler: &mut Scheduler, name: &str) {
        let (handler, _) = CountingHandler::new(name);
        let def = JobDefinition::new(name, Schedule::Once, Box::new(handler));
        scheduler.register(def).unwrap();
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    #[test]
    fn register_valid_definition() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        assert_eq!(s.status().registered_definitions, 1);
    }

    #[test]
    fn register_invalid_cron_rejected() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("bad");
        let def = JobDefinition::new("bad", Schedule::Cron("bad".into()), Box::new(handler));
        assert!(s.register(def).is_err());
    }

    #[test]
    fn register_replaces_same_name() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        register_noop(&mut s, "test");
        assert_eq!(s.status().registered_definitions, 1);
    }

    // -----------------------------------------------------------------------
    // Scheduling
    // -----------------------------------------------------------------------

    #[test]
    fn schedule_creates_instance() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let now = Utc::now();
        let id = s.schedule("test", now).unwrap();

        let instance = s.store().get(&id).unwrap().unwrap();
        assert_eq!(instance.definition_name, "test");
        assert_eq!(instance.status, JobStatus::Scheduled);
    }

    #[test]
    fn schedule_unknown_definition_errors() {
        let mut s = make_scheduler();
        assert!(s.schedule("nonexistent", Utc::now()).is_err());
    }

    // -----------------------------------------------------------------------
    // Tick — basic execution
    // -----------------------------------------------------------------------

    #[test]
    fn tick_returns_execute_for_due_jobs() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let now = Utc::now();
        s.schedule("test", now).unwrap();

        let actions = s.tick(now);
        assert_eq!(actions.len(), 1);
        assert!(
            matches!(&actions[0], TickAction::Execute { definition_name, .. } if definition_name == "test")
        );
    }

    #[test]
    fn tick_skips_future_jobs() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let future = Utc::now() + Duration::hours(1);

        // Clear queue and add a future job
        s.queue.clear();
        let inst = JobInstance::new_scheduled("test", future);
        s.store.save(&inst).unwrap();
        s.queue.enqueue(inst).unwrap();

        let now = Utc::now();
        let actions = s.tick(now);
        assert!(actions.is_empty());
    }

    #[test]
    fn tick_respects_concurrent_limit() {
        let mut s = make_scheduler();
        s.max_concurrent = 1;

        register_noop(&mut s, "a");
        register_noop(&mut s, "b");

        let now = Utc::now();
        s.schedule("a", now).unwrap();
        s.schedule("b", now).unwrap();

        let actions = s.tick(now);
        assert_eq!(actions.len(), 1);
        assert_eq!(s.status().running_jobs, 1);
        assert_eq!(s.status().queued_jobs, 1);
    }

    #[test]
    fn tick_requeues_jobs_over_concurrent_limit() {
        let mut s = make_scheduler();
        s.max_concurrent = 1;

        register_noop(&mut s, "a");
        register_noop(&mut s, "b");

        let now = Utc::now();
        s.schedule("a", now).unwrap();
        s.schedule("b", now).unwrap();

        let first_actions = s.tick(now);
        assert_eq!(first_actions.len(), 1);
        assert_eq!(s.status().queued_jobs, 1);

        let TickAction::Execute { job_id: first_job_id, .. } = &first_actions[0] else {
            unreachable!("expected execute action");
        };
        s.complete(*first_job_id, JobOutput::new("ok")).unwrap();

        let second_actions = s.tick(now);
        assert_eq!(second_actions.len(), 1);
    }

    #[test]
    fn tick_processes_multiple_due_jobs() {
        let mut s = make_scheduler();
        s.max_concurrent = 10;

        register_noop(&mut s, "a");
        register_noop(&mut s, "b");
        register_noop(&mut s, "c");

        let now = Utc::now();
        s.schedule("a", now).unwrap();
        s.schedule("b", now).unwrap();
        s.schedule("c", now).unwrap();

        let actions = s.tick(now);
        assert_eq!(actions.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Tick — timeout detection
    // -----------------------------------------------------------------------

    #[test]
    fn tick_detects_timeout() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("slow");
        let def = JobDefinition::new("slow", Schedule::Once, Box::new(handler))
            .with_timeout(std::time::Duration::from_secs(10));
        s.register(def).unwrap();

        let t0 = Utc::now();
        s.schedule("slow", t0).unwrap();

        let actions = s.tick(t0);
        assert_eq!(actions.len(), 1);
        assert_eq!(s.status().running_jobs, 1);

        let t1 = t0 + Duration::seconds(15);
        let actions = s.tick(t1);
        assert!(actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
        assert_eq!(s.status().running_jobs, 0);
    }

    #[test]
    fn tick_no_timeout_before_deadline() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("fast");
        let def = JobDefinition::new("fast", Schedule::Once, Box::new(handler))
            .with_timeout(std::time::Duration::from_secs(60));
        s.register(def).unwrap();

        let t0 = Utc::now();
        s.schedule("fast", t0).unwrap();
        s.tick(t0);

        let t1 = t0 + Duration::seconds(30);
        let actions = s.tick(t1);
        assert!(!actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
        assert_eq!(s.status().running_jobs, 1);
    }

    #[test]
    fn tick_timeout_retries_when_allowed() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("retry_timeout");
        let def = JobDefinition::new("retry_timeout", Schedule::Once, Box::new(handler))
            .with_timeout(std::time::Duration::from_secs(1))
            .with_max_retries(2)
            .with_retry_backoff(BackoffStrategy::fixed(std::time::Duration::from_secs(5)));
        s.register(def).unwrap();

        let t0 = Utc::now();
        let id = s.schedule("retry_timeout", t0).unwrap();
        s.tick(t0);

        let timeout_actions = s.tick(t0 + Duration::seconds(2));
        assert!(timeout_actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
        assert!(timeout_actions.iter().any(|a| matches!(a, TickAction::Retry { .. })));

        let timed_out = s.store().get(&id).unwrap().unwrap();
        assert_eq!(timed_out.status, JobStatus::Retrying);
        assert_eq!(timed_out.attempt, 1);
        assert!(timed_out.error.as_deref().unwrap_or_default().contains("timed out"));
        assert!(timed_out.next_run_at.is_some());
    }

    #[test]
    fn tick_timeout_marks_failed_when_retries_exhausted() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("timeout_fail");
        let def = JobDefinition::new("timeout_fail", Schedule::Once, Box::new(handler))
            .with_timeout(std::time::Duration::from_secs(1))
            .with_max_retries(0);
        s.register(def).unwrap();

        let t0 = Utc::now();
        let id = s.schedule("timeout_fail", t0).unwrap();
        s.tick(t0);

        let timeout_actions = s.tick(t0 + Duration::seconds(2));
        assert!(timeout_actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
        assert!(!timeout_actions.iter().any(|a| matches!(a, TickAction::Retry { .. })));

        let timed_out = s.store().get(&id).unwrap().unwrap();
        assert_eq!(timed_out.status, JobStatus::Failed);
        assert!(timed_out.error.as_deref().unwrap_or_default().contains("retries exhausted"));
    }

    #[test]
    fn tick_with_backwards_time_does_not_timeout_running_job() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("time_travel");
        let def = JobDefinition::new("time_travel", Schedule::Once, Box::new(handler))
            .with_timeout(std::time::Duration::from_secs(30));
        s.register(def).unwrap();

        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap();
        s.schedule("time_travel", t0).unwrap();
        let first_actions = s.tick(t0);
        assert!(first_actions.iter().any(|a| matches!(a, TickAction::Execute { .. })));

        let actions = s.tick(t0 - Duration::seconds(5));
        assert!(actions.is_empty());
        assert_eq!(s.status().running_jobs, 1);
    }

    #[test]
    fn timeout_retry_progresses_under_irregular_ticks() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("chaos_retry");
        let def = JobDefinition::new("chaos_retry", Schedule::Once, Box::new(handler))
            .with_timeout(std::time::Duration::from_secs(5))
            .with_max_retries(1)
            .with_retry_backoff(BackoffStrategy::fixed(std::time::Duration::from_secs(10)));
        s.register(def).unwrap();

        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap();
        let id = s.schedule("chaos_retry", t0).unwrap();
        let first = s.tick(t0);
        assert!(
            first.iter().any(|a| matches!(a, TickAction::Execute { job_id, .. } if *job_id == id))
        );

        let early = s.tick(t0 + Duration::seconds(2));
        assert!(!early.iter().any(|a| matches!(a, TickAction::Timeout { .. })));

        let timeout_actions = s.tick(t0 + Duration::seconds(20));
        assert!(
            timeout_actions
                .iter()
                .any(|a| matches!(a, TickAction::Timeout { job_id } if *job_id == id))
        );
        let retry_at = timeout_actions
            .iter()
            .find_map(|a| match a {
                TickAction::Retry { job_id, retry_at } if *job_id == id => Some(*retry_at),
                _ => None,
            })
            .expect("retry action");
        assert_eq!(retry_at, t0 + Duration::seconds(30));

        let too_early_retry = s.tick(t0 + Duration::seconds(29));
        assert!(
            !too_early_retry
                .iter()
                .any(|a| matches!(a, TickAction::Execute { definition_name, .. } if definition_name == "chaos_retry"))
        );

        let retry_exec = s.tick(t0 + Duration::seconds(31));
        assert!(
            retry_exec.iter().any(
                |a| matches!(a, TickAction::Execute { job_id, context, .. } if *job_id == id && context.attempt == 1)
            )
        );
    }

    // -----------------------------------------------------------------------
    // Complete & reschedule
    // -----------------------------------------------------------------------

    #[test]
    fn complete_marks_job_completed() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let now = Utc::now();
        let id = s.schedule("test", now).unwrap();
        s.tick(now);

        s.complete(id, JobOutput::new("done")).unwrap();
        let inst = s.store().get(&id).unwrap().unwrap();
        assert_eq!(inst.status, JobStatus::Completed);
        assert_eq!(s.status().running_jobs, 0);
    }

    #[test]
    fn complete_interval_reschedules() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("interval");
        let def = JobDefinition::new(
            "interval",
            Schedule::Interval(std::time::Duration::from_secs(60)),
            Box::new(handler),
        );
        s.register(def).unwrap();

        let now = Utc::now();
        let id = s.schedule("interval", now).unwrap();
        s.tick(now);
        s.complete(id, JobOutput::new("done")).unwrap();

        assert!(s.queue.size() > 0);
    }

    #[test]
    fn complete_cron_reschedules() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("cron");
        let def =
            JobDefinition::new("cron", Schedule::Cron("*/5 * * * *".into()), Box::new(handler));
        s.register(def).unwrap();

        let now = Utc::now();
        let id = s.schedule("cron", now).unwrap();
        s.tick(now);
        s.complete(id, JobOutput::new("done")).unwrap();

        assert!(s.queue.size() > 0);
        let next_run = s.status().next_run_at.expect("cron should be rescheduled");
        assert_eq!(next_run.second(), 0);
        assert_eq!(next_run.minute() % 5, 0);
    }

    #[test]
    fn complete_at_cron_progresses_under_irregular_ticks() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("cron_irregular");
        let def = JobDefinition::new(
            "cron_irregular",
            Schedule::Cron("*/5 * * * *".into()),
            Box::new(handler),
        );
        s.register(def).unwrap();

        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap();
        let first_id = s.schedule("cron_irregular", t0).unwrap();
        let first = s.tick(t0);
        assert!(
            first
                .iter()
                .any(|a| matches!(a, TickAction::Execute { job_id, .. } if *job_id == first_id))
        );
        s.complete_at(first_id, JobOutput::new("done"), t0 + Duration::minutes(2)).unwrap();
        assert_eq!(s.status().next_run_at.unwrap(), t0 + Duration::minutes(5));

        let no_run_yet = s.tick(t0 + Duration::minutes(4));
        assert!(no_run_yet.is_empty());

        let second = s.tick(t0 + Duration::minutes(11));
        let second_id = second
            .iter()
            .find_map(|a| match a {
                TickAction::Execute { job_id, definition_name, .. }
                    if definition_name == "cron_irregular" =>
                {
                    Some(*job_id)
                }
                _ => None,
            })
            .expect("second cron execution");
        s.complete_at(second_id, JobOutput::new("done"), t0 + Duration::minutes(11)).unwrap();
        assert_eq!(s.status().next_run_at.unwrap(), t0 + Duration::minutes(15));

        let third = s.tick(t0 + Duration::minutes(37));
        let third_id = third
            .iter()
            .find_map(|a| match a {
                TickAction::Execute { job_id, definition_name, .. }
                    if definition_name == "cron_irregular" =>
                {
                    Some(*job_id)
                }
                _ => None,
            })
            .expect("third cron execution");
        s.complete_at(third_id, JobOutput::new("done"), t0 + Duration::minutes(37)).unwrap();
        assert_eq!(s.status().next_run_at.unwrap(), t0 + Duration::minutes(40));
    }

    // -----------------------------------------------------------------------
    // Fail & retry
    // -----------------------------------------------------------------------

    #[test]
    fn fail_marks_job_failed() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let now = Utc::now();
        let id = s.schedule("test", now).unwrap();
        s.tick(now);

        let retry_action = s.fail(id, "oops").unwrap();
        assert!(retry_action.is_some());
    }

    #[test]
    fn fail_no_retry_when_exhausted() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("no_retry");
        let def =
            JobDefinition::new("no_retry", Schedule::Once, Box::new(handler)).with_max_retries(0);
        s.register(def).unwrap();

        let now = Utc::now();
        let id = s.schedule("no_retry", now).unwrap();
        s.tick(now);

        let retry_action = s.fail(id, "oops").unwrap();
        assert!(retry_action.is_none());
    }

    #[test]
    fn fail_retry_requeues() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("retryable");
        let def = JobDefinition::new("retryable", Schedule::Once, Box::new(handler))
            .with_max_retries(3)
            .with_retry_backoff(BackoffStrategy::fixed(std::time::Duration::from_secs(5)));
        s.register(def).unwrap();

        let now = Utc::now();
        let id = s.schedule("retryable", now).unwrap();
        s.tick(now);

        let action = s.fail(id, "oops").unwrap().unwrap();
        assert!(matches!(action, TickAction::Retry { .. }));
        assert!(s.queue.size() > 0);
    }

    #[test]
    fn fail_at_uses_explicit_time_for_retry_schedule() {
        let mut s = make_scheduler();
        let (handler, _) = CountingHandler::new("retry_time");
        let def = JobDefinition::new("retry_time", Schedule::Once, Box::new(handler))
            .with_max_retries(1)
            .with_retry_backoff(BackoffStrategy::fixed(std::time::Duration::from_secs(7)));
        s.register(def).unwrap();

        let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).single().unwrap();
        let id = s.schedule("retry_time", t0).unwrap();
        s.tick(t0);

        let action = s.fail_at(id, "oops", t0 + Duration::seconds(30)).unwrap().unwrap();
        let TickAction::Retry { retry_at, .. } = action else {
            panic!("expected retry action");
        };
        assert_eq!(retry_at, t0 + Duration::seconds(37));
    }

    // -----------------------------------------------------------------------
    // Cancel
    // -----------------------------------------------------------------------

    #[test]
    fn cancel_removes_from_running() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let now = Utc::now();
        let id = s.schedule("test", now).unwrap();
        s.tick(now);

        assert_eq!(s.status().running_jobs, 1);
        s.cancel(id).unwrap();
        assert_eq!(s.status().running_jobs, 0);
    }

    #[test]
    fn cancel_nonexistent_is_ok() {
        let mut s = make_scheduler();
        assert!(s.cancel(Uuid::new_v4()).is_ok());
    }

    // -----------------------------------------------------------------------
    // Status
    // -----------------------------------------------------------------------

    #[test]
    fn status_reports_correctly() {
        let mut s = make_scheduler();
        s.max_concurrent = 3;

        register_noop(&mut s, "a");
        register_noop(&mut s, "b");

        let status = s.status();
        assert_eq!(status.registered_definitions, 2);
        assert_eq!(status.running_jobs, 0);
        assert_eq!(status.queued_jobs, 0);
        assert_eq!(status.max_concurrent, 3);
        assert!(status.next_run_at.is_none());
    }

    #[test]
    fn status_after_schedule() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let now = Utc::now();
        s.schedule("test", now).unwrap();

        let status = s.status();
        assert_eq!(status.queued_jobs, 1);
        assert!(status.next_run_at.is_some());
    }

    // -----------------------------------------------------------------------
    // Builder
    // -----------------------------------------------------------------------

    #[test]
    fn with_max_concurrent() {
        let s = make_scheduler().with_max_concurrent(10);
        assert_eq!(s.max_concurrent, 10);
    }

    #[test]
    fn with_max_queue_size() {
        let s = make_scheduler().with_max_queue_size(50);
        assert_eq!(s.queue.max_size(), 50);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn tick_empty_queue_returns_no_actions() {
        let mut s = make_scheduler();
        let actions = s.tick(Utc::now());
        assert!(actions.is_empty());
    }

    #[test]
    fn tick_cancelled_job_skipped() {
        let mut s = make_scheduler();
        register_noop(&mut s, "test");
        let now = Utc::now();
        let id = s.schedule("test", now).unwrap();

        s.queue.cancel(id);
        let actions = s.tick(now);
        assert!(actions.is_empty());
    }

    #[test]
    fn complete_nonexistent_errors() {
        let mut s = make_scheduler();
        assert!(s.complete(Uuid::new_v4(), JobOutput::new("nope")).is_err());
    }

    #[test]
    fn fail_nonexistent_errors() {
        let mut s = make_scheduler();
        assert!(s.fail(Uuid::new_v4(), "oops").is_err());
    }

    #[test]
    fn scheduler_debug() {
        let s = make_scheduler();
        let debug = format!("{s:?}");
        assert!(debug.contains("Scheduler"));
    }

    #[test]
    fn scheduler_status_debug() {
        let s = make_scheduler();
        let status = s.status();
        let debug = format!("{status:?}");
        assert!(debug.contains("SchedulerStatus"));
    }

    #[test]
    fn tick_action_debug() {
        let action = TickAction::Timeout { job_id: Uuid::new_v4() };
        let debug = format!("{action:?}");
        assert!(debug.contains("Timeout"));
    }
}
