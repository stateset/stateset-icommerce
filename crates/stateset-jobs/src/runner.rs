//! Async driver for a [`Scheduler`].
//!
//! [`Scheduler`] is deliberately synchronous: [`Scheduler::tick`] returns
//! [`TickAction`]s and never touches an async runtime. That makes it testable
//! but leaves a gap — *something* has to look up the handler named by
//! [`TickAction::Execute`], await it, and hand the result back to the
//! scheduler. Before this module nothing did, so the built-in sweeps
//! ([`crate::ReservationSweepJob`], [`crate::TraceabilitySweepJob`]) existed
//! but could not run in a deployment.
//!
//! [`JobRunner`] closes that gap:
//!
//! - [`JobRunner::tick_once`] executes everything one tick produces.
//! - [`JobRunner::run_now`] schedules one definition and drives it to
//!   completion — the primitive behind an operator's "sweep now" endpoint.
//! - [`JobRunner::spawn`] / [`JobRunner::spawn_on_dedicated_thread`] run the
//!   loop in the background and return a [`JobRunnerHandle`] that can trigger
//!   a job on demand, read scheduler status, and shut the loop down.
//!
//! ```rust
//! use stateset_jobs::{InMemoryJobStore, JobRunner, Scheduler};
//! use std::time::Duration;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let scheduler = Scheduler::new(Box::new(InMemoryJobStore::new()));
//! let handle = JobRunner::new(scheduler)
//!     .with_tick_interval(Duration::from_secs(1))
//!     .spawn();
//! // ... later
//! handle.shutdown().await;
//! # Ok(())
//! # }
//! ```

use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::{FutureExt, StreamExt, stream::FuturesUnordered};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::error::JobError;
use crate::job::BoxFuture;
use crate::scheduler::{Scheduler, SchedulerStatus, TickAction};
use crate::state::JobOutput;

/// Default gap between scheduler ticks.
pub const DEFAULT_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// Depth of the command channel between a [`JobRunnerHandle`] and its loop.
const COMMAND_CHANNEL_DEPTH: usize = 32;

/// What happened to one job the runner executed.
#[derive(Debug, Clone)]
pub struct JobRunOutcome {
    /// The job instance that ran.
    pub job_id: Uuid,
    /// The definition it ran for.
    pub definition_name: String,
    /// The handler's output, or the error message it failed with.
    pub result: Result<JobOutput, String>,
}

impl JobRunOutcome {
    /// Whether the handler succeeded.
    #[must_use]
    pub const fn is_ok(&self) -> bool {
        self.result.is_ok()
    }
}

/// A command sent from a [`JobRunnerHandle`] to a running loop.
enum RunnerCommand {
    /// Run one definition immediately and report its output.
    Trigger { name: String, reply: oneshot::Sender<Result<JobOutput, JobError>> },
    /// Snapshot the scheduler.
    Status { reply: oneshot::Sender<SchedulerStatus> },
    /// Stop the loop.
    Shutdown { reply: oneshot::Sender<()> },
}

impl std::fmt::Debug for RunnerCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trigger { name, .. } => f.debug_struct("Trigger").field("name", name).finish(),
            Self::Status { .. } => f.write_str("Status"),
            Self::Shutdown { .. } => f.write_str("Shutdown"),
        }
    }
}

/// Handle to a background [`JobRunner`] loop.
///
/// Dropping the handle does not stop the loop; call [`Self::shutdown`].
#[derive(Debug, Clone)]
pub struct JobRunnerHandle {
    commands: mpsc::Sender<RunnerCommand>,
}

/// The loop is gone (shut down, or its thread/task died).
const RUNNER_GONE: &str = "job runner is not running";

impl JobRunnerHandle {
    /// Enqueue `name` immediately and wait for its first attempt's output.
    /// If all execution slots are occupied, it waits in the scheduler queue.
    ///
    /// This is what an operator-facing "run the sweep now" endpoint calls: it
    /// goes through the same scheduler bookkeeping as a scheduled run, so the
    /// job instance, retry state and store record are identical.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if no such definition is registered,
    /// [`JobError::ExecutionFailed`] if the handler failed or the runner is no
    /// longer running.
    pub async fn trigger(&self, name: impl Into<String>) -> Result<JobOutput, JobError> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(RunnerCommand::Trigger { name: name.into(), reply })
            .await
            .map_err(|_| JobError::ExecutionFailed(RUNNER_GONE.to_owned()))?;
        response.await.map_err(|_| JobError::ExecutionFailed(RUNNER_GONE.to_owned()))?
    }

    /// Snapshot the running scheduler.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::ExecutionFailed`] if the runner is no longer running.
    pub async fn status(&self) -> Result<SchedulerStatus, JobError> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(RunnerCommand::Status { reply })
            .await
            .map_err(|_| JobError::ExecutionFailed(RUNNER_GONE.to_owned()))?;
        response.await.map_err(|_| JobError::ExecutionFailed(RUNNER_GONE.to_owned()))
    }

    /// Cancel active handler futures, persist interrupted attempts as failures,
    /// and stop the loop. Queued operator triggers are cancelled. Idempotent.
    ///
    /// Handlers must yield and support cancellation; blocking work or external
    /// effects cannot be interrupted or undone. Retry policies still apply to
    /// interrupted attempts when a durable scheduler is restarted.
    pub async fn shutdown(&self) {
        let (reply, response) = oneshot::channel();
        if self.commands.send(RunnerCommand::Shutdown { reply }).await.is_ok() {
            let _ = response.await;
        }
    }

    /// Whether the loop is still accepting commands.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.commands.is_closed()
    }
}

struct ExecutionResult {
    job_id: Uuid,
    definition_name: String,
    result: Result<JobOutput, JobError>,
    finished_at: DateTime<Utc>,
}

type Executions = FuturesUnordered<BoxFuture<'static, ExecutionResult>>;
type TriggerReplies = HashMap<Uuid, oneshot::Sender<Result<JobOutput, JobError>>>;

/// Drives a [`Scheduler`] on an async runtime.
#[derive(Debug)]
pub struct JobRunner {
    scheduler: Scheduler,
    tick_interval: Duration,
}

impl JobRunner {
    /// Wrap a scheduler with the default one-second tick.
    #[must_use]
    pub const fn new(scheduler: Scheduler) -> Self {
        Self { scheduler, tick_interval: DEFAULT_TICK_INTERVAL }
    }

    /// Override the gap between ticks (clamped to at least 1ms).
    #[must_use]
    pub const fn with_tick_interval(mut self, interval: Duration) -> Self {
        self.tick_interval = if interval.is_zero() { Duration::from_millis(1) } else { interval };
        self
    }

    /// Borrow the scheduler (registration, status).
    #[must_use]
    pub const fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    /// Mutably borrow the scheduler (registration).
    pub const fn scheduler_mut(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    /// Enqueue a first run of every registered definition at `now`, so
    /// recurring jobs start ticking. Later runs are re-queued by the scheduler
    /// itself when each run completes. Definitions with recovered active work
    /// retain their existing run and retry deadline instead of being duplicated.
    ///
    /// # Errors
    ///
    /// Returns the first store or scheduling error (such as a full queue).
    pub fn bootstrap(&mut self, now: DateTime<Utc>) -> Result<(), JobError> {
        let active_names: HashSet<_> = self
            .scheduler
            .store()
            .list_active()?
            .into_iter()
            .map(|job| job.definition_name)
            .collect();
        for name in self.scheduler.definition_names() {
            if !active_names.contains(&name) {
                self.scheduler.schedule(&name, now)?;
            }
        }
        Ok(())
    }

    /// Execute every action produced by a single tick at `now`.
    ///
    /// Handler failures are recorded on the scheduler (which may schedule a
    /// retry) and reported in the returned outcomes; they never abort the tick.
    /// Ready handlers are polled concurrently and cancelled at their configured
    /// timeout. Handlers must yield to the runtime and make cancellation safe;
    /// blocking work and external side effects cannot be undone by a timeout.
    pub async fn tick_once(&mut self, now: DateTime<Utc>) -> Vec<JobRunOutcome> {
        let mut executions = Executions::new();
        let mut active = HashSet::new();
        self.start_ready(now, &executions, &mut active);
        let mut outcomes = Vec::new();
        while let Some(execution) = executions.next().await {
            outcomes.push(self.finish_execution(execution));
        }
        outcomes
    }

    fn start_ready(
        &mut self,
        now: DateTime<Utc>,
        executions: &Executions,
        active: &mut HashSet<Uuid>,
    ) {
        for action in self.scheduler.tick_with_managed_jobs(now, active) {
            let TickAction::Execute { job_id, definition_name, context } = action else {
                continue;
            };
            let definition = self.scheduler.shared_definition(&definition_name);
            active.insert(job_id);
            executions.push(Box::pin(async move {
                let started = tokio::time::Instant::now();
                // Catch both execute() construction and future-poll panics.
                // A handler panic must not take other jobs or the command loop down.
                let result = AssertUnwindSafe(async {
                    match definition {
                        Some(definition) => tokio::time::timeout(
                            definition.timeout,
                            definition.handler.execute(&context),
                        )
                        .await
                        .unwrap_or(Err(JobError::Timeout(definition.timeout))),
                        None => Err(JobError::ExecutionFailed(format!(
                            "no handler registered for '{definition_name}'"
                        ))),
                    }
                })
                .catch_unwind()
                .await
                .unwrap_or_else(|_| Err(JobError::ExecutionFailed("job handler panicked".into())));
                let finished_at = chrono::Duration::from_std(started.elapsed())
                    .ok()
                    .and_then(|elapsed| now.checked_add_signed(elapsed))
                    .unwrap_or(now);
                ExecutionResult { job_id, definition_name, result, finished_at }
            }));
        }
    }

    fn finish_execution(&mut self, execution: ExecutionResult) -> JobRunOutcome {
        let ExecutionResult { job_id, definition_name, result, finished_at } = execution;
        let result = match result {
            Ok(output) => self
                .scheduler
                .complete_at(job_id, output.clone(), finished_at)
                .map(|()| output)
                .map_err(|err| err.to_string()),
            Err(err) => {
                let message = err.to_string();
                match self.scheduler.fail_at(job_id, &message, finished_at) {
                    Ok(_) => Err(message),
                    Err(store_err) => {
                        Err(format!("{message}; failed to persist failure: {store_err}"))
                    }
                }
            }
        };
        JobRunOutcome { job_id, definition_name, result }
    }

    /// Schedule `name` at `now` and drive it to completion, returning its
    /// output.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::NotFound`] if the definition is not registered and
    /// [`JobError::ExecutionFailed`] if the handler failed (or, defensively,
    /// if the tick did not pick the job up because the concurrency limit was
    /// saturated).
    pub async fn run_now(&mut self, name: &str, now: DateTime<Utc>) -> Result<JobOutput, JobError> {
        let job_id = self.scheduler.schedule(name, now)?;
        for outcome in self.tick_once(now).await {
            if outcome.job_id == job_id {
                return outcome.result.map_err(JobError::ExecutionFailed);
            }
        }
        self.scheduler.cancel(job_id)?;
        Err(JobError::ExecutionFailed(format!(
            "job '{name}' did not run: the scheduler is at its concurrency limit"
        )))
    }

    /// Run handlers alongside command processing. Each completion is persisted
    /// immediately and frees a concurrency slot for queued work.
    async fn run(mut self, mut commands: mpsc::Receiver<RunnerCommand>) {
        if let Err(err) = self.bootstrap(Utc::now()) {
            tracing::error!(error = %err, "failed to bootstrap job runner");
        }
        let mut ticker = tokio::time::interval(self.tick_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut executions = Executions::new();
        let mut active = HashSet::new();
        let mut replies = TriggerReplies::new();
        let mut commands_open = true;
        loop {
            tokio::select! {
                execution = executions.next(), if !executions.is_empty() => {
                    if let Some(execution) = execution {
                        active.remove(&execution.job_id);
                        let outcome = self.finish_execution(execution);
                        if let Some(reply) = replies.remove(&outcome.job_id) {
                            let _ = reply.send(outcome.result.map_err(JobError::ExecutionFailed));
                        } else if let Err(error) = outcome.result {
                            tracing::warn!(job_id = %outcome.job_id, %error, "background job failed");
                        }
                        self.start_ready(Utc::now(), &executions, &mut active);
                    }
                }
                command = commands.recv(), if commands_open => {
                    match command {
                        Some(RunnerCommand::Trigger { name, reply }) => {
                            match self.scheduler.schedule(&name, Utc::now()) {
                                Ok(job_id) => {
                                    replies.insert(job_id, reply);
                                    self.start_ready(Utc::now(), &executions, &mut active);
                                }
                                Err(error) => { let _ = reply.send(Err(error)); }
                            }
                        }
                        Some(RunnerCommand::Status { reply }) => {
                            let _ = reply.send(self.scheduler.status());
                        }
                        Some(RunnerCommand::Shutdown { reply }) => {
                            // Drop handler futures before acknowledging shutdown.
                            // Record interrupted attempts as failures so their normal
                            // retry policy and durable restart recovery still apply.
                            drop(executions);
                            for job_id in &active {
                                if let Err(error) = self.scheduler.fail(*job_id, "job runner shut down") {
                                    tracing::error!(%job_id, %error, "failed to persist interrupted job");
                                }
                            }
                            for (job_id, pending_reply) in replies {
                                if !active.contains(&job_id) {
                                    if let Err(error) = self.scheduler.cancel(job_id) {
                                        tracing::error!(%job_id, %error, "failed to cancel queued trigger");
                                    }
                                }
                                let _ = pending_reply.send(Err(JobError::ExecutionFailed(RUNNER_GONE.into())));
                            }
                            commands.close();
                            let _ = reply.send(());
                            return;
                        }
                        // Dropping every handle does not stop recurring jobs.
                        None => commands_open = false,
                    }
                }
                _ = ticker.tick() => {
                    self.start_ready(Utc::now(), &executions, &mut active);
                }
            }
        }
    }

    /// Spawn the loop as a Tokio task on the current runtime.
    ///
    /// Handlers run *on* that runtime, so a blocking handler blocks a worker
    /// thread. Use [`Self::spawn_on_dedicated_thread`] for handlers that do
    /// blocking database work.
    #[must_use]
    pub fn spawn(self) -> JobRunnerHandle {
        let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_DEPTH);
        tokio::spawn(self.run(rx));
        JobRunnerHandle { commands: tx }
    }

    /// Run the loop on its own OS thread with its own single-threaded Tokio
    /// runtime, and return a handle usable from any runtime.
    ///
    /// This is the right shape for the engine's built-in sweeps: they call
    /// straight into blocking SQLite/Postgres repositories, and isolating them
    /// on a dedicated thread keeps a slow sweep from stalling the HTTP
    /// server's worker pool.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::ExecutionFailed`] if the runtime or thread could
    /// not be created.
    pub fn spawn_on_dedicated_thread(
        self,
        thread_name: impl Into<String>,
    ) -> Result<JobRunnerHandle, JobError> {
        let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_DEPTH);
        std::thread::Builder::new()
            .name(thread_name.into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_time()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(err) => {
                        tracing::error!(error = %err, "job runner thread could not build a runtime");
                        return;
                    }
                };
                runtime.block_on(self.run(rx));
            })
            .map_err(|e| {
                JobError::ExecutionFailed(format!("could not start job runner thread: {e}"))
            })?;
        Ok(JobRunnerHandle { commands: tx })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::JobContext;
    use crate::job::{BoxFuture, JobDefinition, JobHandler, Schedule};
    use crate::store::InMemoryJobStore;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingJob {
        name: &'static str,
        runs: Arc<AtomicU32>,
        fail: bool,
    }

    struct GateJob {
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Semaphore>,
        dropped: Arc<AtomicU32>,
    }

    struct DropCounter(Arc<AtomicU32>);
    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl JobHandler for GateJob {
        fn name(&self) -> &str {
            "gate"
        }
        fn execute<'a>(&'a self, _: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
            Box::pin(async move {
                let _guard = DropCounter(self.dropped.clone());
                self.started.notify_one();
                self.release.acquire().await.unwrap().forget();
                Ok(JobOutput::new("released"))
            })
        }
    }

    fn register_gate(
        scheduler: &mut Scheduler,
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Semaphore>,
        dropped: Arc<AtomicU32>,
    ) {
        scheduler
            .register(
                JobDefinition::new(
                    "gate",
                    Schedule::Once,
                    Box::new(GateJob { started, release, dropped }),
                )
                .with_timeout(Duration::from_secs(300))
                .with_max_retries(0),
            )
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn slow_handler_does_not_block_status_triggers_or_shutdown() {
        use crate::{JobStatus, JobStore};
        let store = InMemoryJobStore::new();
        let mut scheduler = Scheduler::new(Box::new(store.clone()));
        let started = Arc::new(tokio::sync::Notify::new());
        let dropped = Arc::new(AtomicU32::new(0));
        register_gate(
            &mut scheduler,
            started.clone(),
            Arc::new(tokio::sync::Semaphore::new(0)),
            dropped.clone(),
        );
        scheduler
            .register(JobDefinition::new(
                "healthy",
                Schedule::Once,
                Box::new(CountingJob {
                    name: "healthy",
                    runs: Arc::new(AtomicU32::new(0)),
                    fail: false,
                }),
            ))
            .unwrap();
        let handle =
            JobRunner::new(scheduler).with_tick_interval(Duration::from_secs(3600)).spawn();
        started.notified().await;
        let status =
            tokio::time::timeout(Duration::from_secs(1), handle.status()).await.unwrap().unwrap();
        assert!(status.running_jobs >= 1);
        let output = tokio::time::timeout(Duration::from_secs(1), handle.trigger("healthy"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(output.message, "ok");
        assert!(!store.list_by_status(JobStatus::Completed).unwrap().is_empty());
        assert_eq!(store.list_by_status(JobStatus::Running).unwrap().len(), 1);
        tokio::time::timeout(Duration::from_secs(1), handle.shutdown()).await.unwrap();
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(store.list_by_status(JobStatus::Failed).unwrap().len(), 1);
        assert!(!handle.is_running());
    }

    #[tokio::test(start_paused = true)]
    async fn trigger_waits_for_capacity_and_runs_when_a_slot_opens() {
        let mut scheduler =
            Scheduler::new(Box::new(InMemoryJobStore::new())).with_max_concurrent(1);
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        register_gate(
            &mut scheduler,
            started.clone(),
            release.clone(),
            Arc::new(AtomicU32::new(0)),
        );
        let runs = Arc::new(AtomicU32::new(0));
        scheduler
            .register(JobDefinition::new(
                "healthy",
                Schedule::Once,
                Box::new(CountingJob { name: "healthy", runs: runs.clone(), fail: false }),
            ))
            .unwrap();
        let handle =
            JobRunner::new(scheduler).with_tick_interval(Duration::from_secs(3600)).spawn();
        started.notified().await;
        let (reply, mut response) = oneshot::channel();
        handle
            .commands
            .send(RunnerCommand::Trigger { name: "healthy".into(), reply })
            .await
            .unwrap();
        let status = handle.status().await.unwrap();
        assert_eq!(status.running_jobs, 1);
        assert_eq!(status.queued_jobs, 2);
        assert!(matches!(response.try_recv(), Err(oneshot::error::TryRecvError::Empty)));
        assert_eq!(runs.load(Ordering::SeqCst), 0);
        release.add_permits(1);
        tokio::time::timeout(Duration::from_secs(1), response).await.unwrap().unwrap().unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 2);
        handle.shutdown().await;
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_resolves_and_cancels_queued_triggers() {
        use crate::{JobStatus, JobStore};
        let store = InMemoryJobStore::new();
        let mut scheduler = Scheduler::new(Box::new(store.clone())).with_max_concurrent(1);
        let started = Arc::new(tokio::sync::Notify::new());
        register_gate(
            &mut scheduler,
            started.clone(),
            Arc::new(tokio::sync::Semaphore::new(0)),
            Arc::new(AtomicU32::new(0)),
        );
        let handle = JobRunner::new(scheduler).spawn();
        started.notified().await;
        let (reply, response) = oneshot::channel();
        handle.commands.send(RunnerCommand::Trigger { name: "gate".into(), reply }).await.unwrap();
        assert_eq!(handle.status().await.unwrap().queued_jobs, 1);
        tokio::time::timeout(Duration::from_secs(1), handle.shutdown()).await.unwrap();
        assert!(response.await.unwrap().is_err());
        assert_eq!(store.list_by_status(JobStatus::Cancelled).unwrap().len(), 1);
        assert!(store.list_active().unwrap().is_empty());
    }

    #[tokio::test]
    async fn interrupted_attempt_is_recovered_once_without_bypassing_retry_backoff() {
        use crate::{BackoffStrategy, FileJobStore, JobStatus, JobStore};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("jobs.json");
        let mut scheduler = Scheduler::new(Box::new(FileJobStore::open(&path).unwrap()));
        let started = Arc::new(tokio::sync::Notify::new());
        scheduler
            .register(
                JobDefinition::new(
                    "gate",
                    Schedule::Once,
                    Box::new(GateJob {
                        started: started.clone(),
                        release: Arc::new(tokio::sync::Semaphore::new(0)),
                        dropped: Arc::new(AtomicU32::new(0)),
                    }),
                )
                .with_max_retries(1)
                .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(60))),
            )
            .unwrap();
        let handle = JobRunner::new(scheduler).spawn();
        started.notified().await;
        handle.shutdown().await;
        let store = FileJobStore::open(&path).unwrap();
        let active = store.list_active().unwrap();
        assert_eq!(active.len(), 1);
        let interrupted = &active[0];
        assert_eq!(interrupted.status, JobStatus::Retrying);
        assert_eq!(interrupted.attempt, 1);
        let retry_at = interrupted.next_run_at.unwrap();
        let runs = Arc::new(AtomicU32::new(0));
        let mut scheduler = Scheduler::new(Box::new(store));
        scheduler
            .register(JobDefinition::new(
                "gate",
                Schedule::Once,
                Box::new(CountingJob { name: "gate", runs: runs.clone(), fail: false }),
            ))
            .unwrap();
        let mut runner = JobRunner::new(scheduler);
        runner.bootstrap(Utc::now()).unwrap();
        assert!(runner.tick_once(retry_at - chrono::Duration::seconds(1)).await.is_empty());
        let outcomes = runner.tick_once(retry_at).await;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].job_id, interrupted.id);
        assert!(outcomes[0].is_ok());
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert!(runner.scheduler().store().list_active().unwrap().is_empty());
    }

    #[test]
    fn refused_schedule_does_not_appear_during_restart_recovery() {
        use crate::JobStore;
        let store = InMemoryJobStore::new();
        let mut scheduler = Scheduler::new(Box::new(store.clone())).with_max_queue_size(1);
        scheduler
            .register(JobDefinition::new("hang", Schedule::Once, Box::new(HangingJob)))
            .unwrap();
        let accepted = scheduler.schedule("hang", Utc::now()).unwrap();
        assert!(matches!(scheduler.schedule("hang", Utc::now()), Err(JobError::QueueFull { .. })));
        assert_eq!(store.list_active().unwrap().len(), 1);
        let mut recovered = Scheduler::new(Box::new(store));
        let actions = recovered.tick(Utc::now());
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], TickAction::Execute { job_id, .. } if job_id == accepted));
    }

    struct PanickingJob(bool);
    impl JobHandler for PanickingJob {
        fn name(&self) -> &str {
            "panic"
        }
        fn execute<'a>(&'a self, _: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
            assert!(!self.0, "panic constructing a future");
            Box::pin(async { panic!("panic polling a future") })
        }
    }

    #[tokio::test]
    async fn handler_panics_are_persisted_without_losing_other_jobs() {
        use crate::{JobStatus, JobStore};
        let store = InMemoryJobStore::new();
        let mut scheduler = Scheduler::new(Box::new(store.clone()));
        for (name, synchronous) in [("construct", true), ("poll", false)] {
            scheduler
                .register(
                    JobDefinition::new(name, Schedule::Once, Box::new(PanickingJob(synchronous)))
                        .with_max_retries(0),
                )
                .unwrap();
        }
        scheduler
            .register(JobDefinition::new(
                "healthy",
                Schedule::Once,
                Box::new(CountingJob {
                    name: "healthy",
                    runs: Arc::new(AtomicU32::new(0)),
                    fail: false,
                }),
            ))
            .unwrap();
        let mut runner = JobRunner::new(scheduler);
        runner.bootstrap(Utc::now()).unwrap();
        let outcomes = runner.tick_once(Utc::now()).await;
        assert_eq!(outcomes.iter().filter(|o| o.is_ok()).count(), 1);
        assert_eq!(
            outcomes
                .iter()
                .filter(|o| o.result.as_ref().is_err_and(|e| e.contains("panicked")))
                .count(),
            2
        );
        assert_eq!(store.list_by_status(JobStatus::Failed).unwrap().len(), 2);
        assert_eq!(store.list_by_status(JobStatus::Completed).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn run_now_at_capacity_does_not_leave_an_unacknowledged_queued_job() {
        use crate::JobStatus;
        let mut runner = JobRunner::new(
            Scheduler::new(Box::new(InMemoryJobStore::new())).with_max_concurrent(0),
        );
        runner
            .scheduler_mut()
            .register(JobDefinition::new("hang", Schedule::Once, Box::new(HangingJob)))
            .unwrap();
        assert!(runner.run_now("hang", Utc::now()).await.is_err());
        assert_eq!(runner.scheduler().status().queued_jobs, 0);
        assert_eq!(
            runner.scheduler().store().list_by_status(JobStatus::Cancelled).unwrap().len(),
            1
        );
    }

    struct HangingJob;
    impl JobHandler for HangingJob {
        fn name(&self) -> &str {
            "hang"
        }
        fn execute<'a>(&'a self, _: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
            Box::pin(std::future::pending())
        }
    }

    struct RendezvousJob(Arc<tokio::sync::Barrier>);
    impl JobHandler for RendezvousJob {
        fn name(&self) -> &str {
            "rendezvous"
        }
        fn execute<'a>(&'a self, _: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
            Box::pin(async move {
                self.0.wait().await;
                Ok(JobOutput::new("met peer"))
            })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ready_handlers_can_make_progress_together() {
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let mut scheduler = Scheduler::new(Box::new(InMemoryJobStore::new()));
        for name in ["first", "second"] {
            scheduler
                .register(
                    JobDefinition::new(
                        name,
                        Schedule::Once,
                        Box::new(RendezvousJob(barrier.clone())),
                    )
                    .with_timeout(Duration::from_secs(1))
                    .with_max_retries(0),
                )
                .unwrap();
        }
        let mut runner = JobRunner::new(scheduler);
        runner.bootstrap(Utc::now()).unwrap();
        let outcomes = runner.tick_once(Utc::now()).await;
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().all(JobRunOutcome::is_ok));
    }

    #[tokio::test(start_paused = true)]
    async fn hanging_handler_times_out_and_other_jobs_complete() {
        let runs = Arc::new(AtomicU32::new(0));
        let mut runner = runner_with("healthy", false, Arc::clone(&runs));
        runner
            .scheduler_mut()
            .register(
                JobDefinition::new("hang", Schedule::Once, Box::new(HangingJob))
                    .with_timeout(Duration::from_millis(5))
                    .with_max_retries(0),
            )
            .unwrap();
        runner.bootstrap(Utc::now()).unwrap();
        let outcomes = tokio::time::timeout(Duration::from_secs(1), runner.tick_once(Utc::now()))
            .await
            .unwrap();
        assert_eq!(runs.load(Ordering::SeqCst), 1);
        assert!(outcomes.iter().any(|o| o.definition_name == "healthy" && o.is_ok()));
        assert!(
            outcomes.iter().any(|o| o.definition_name == "hang"
                && o.result.as_ref().unwrap_err().contains("timed out"))
        );
        assert_eq!(runner.scheduler().status().running_jobs, 0);
    }

    impl JobHandler for CountingJob {
        fn name(&self) -> &str {
            self.name
        }
        fn execute<'a>(
            &'a self,
            _ctx: &'a JobContext,
        ) -> BoxFuture<'a, Result<JobOutput, JobError>> {
            let runs = Arc::clone(&self.runs);
            let fail = self.fail;
            Box::pin(async move {
                let n = runs.fetch_add(1, Ordering::SeqCst) + 1;
                if fail {
                    Err(JobError::ExecutionFailed("boom".to_owned()))
                } else {
                    Ok(JobOutput::with_data("ok", serde_json::json!({ "runs": n })))
                }
            })
        }
    }

    fn runner_with(name: &'static str, fail: bool, runs: Arc<AtomicU32>) -> JobRunner {
        let mut scheduler = Scheduler::new(Box::new(InMemoryJobStore::new()));
        scheduler
            .register(JobDefinition::new(
                name,
                Schedule::Interval(Duration::from_secs(60)),
                Box::new(CountingJob { name, runs, fail }),
            ))
            .expect("register");
        JobRunner::new(scheduler)
    }

    #[tokio::test]
    async fn run_now_executes_the_handler_and_returns_its_output() {
        let runs = Arc::new(AtomicU32::new(0));
        let mut runner = runner_with("sweep", false, Arc::clone(&runs));
        let output = runner.run_now("sweep", Utc::now()).await.expect("ran");
        assert_eq!(output.message, "ok");
        assert_eq!(output.data.expect("data")["runs"], 1);
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_now_reports_an_unknown_definition() {
        let mut runner = runner_with("sweep", false, Arc::new(AtomicU32::new(0)));
        let err = runner.run_now("nope", Utc::now()).await.expect_err("unknown");
        assert!(matches!(err, JobError::NotFound(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn run_now_surfaces_handler_failure() {
        let mut runner = runner_with("sweep", true, Arc::new(AtomicU32::new(0)));
        let err = runner.run_now("sweep", Utc::now()).await.expect_err("failed");
        assert!(matches!(err, JobError::ExecutionFailed(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn bootstrap_then_tick_runs_every_registered_definition() {
        let runs = Arc::new(AtomicU32::new(0));
        let mut runner = runner_with("sweep", false, Arc::clone(&runs));
        let now = Utc::now();
        runner.bootstrap(now).expect("bootstrap");
        let outcomes = runner.tick_once(now).await;
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].is_ok());
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn interval_jobs_are_rescheduled_after_each_run() {
        let runs = Arc::new(AtomicU32::new(0));
        let mut runner = runner_with("sweep", false, Arc::clone(&runs));
        let now = Utc::now();
        runner.bootstrap(now).expect("bootstrap");
        runner.tick_once(now).await;
        // Nothing is due yet ...
        assert!(runner.tick_once(now).await.is_empty());
        // ... but the next interval is queued.
        let later = now + chrono::Duration::seconds(61);
        assert_eq!(runner.tick_once(later).await.len(), 1);
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn handle_triggers_and_shuts_down_a_spawned_loop() {
        let runs = Arc::new(AtomicU32::new(0));
        let runner = runner_with("sweep", false, Arc::clone(&runs))
            .with_tick_interval(Duration::from_secs(3600));
        let handle = runner.spawn();
        let output = handle.trigger("sweep").await.expect("triggered");
        assert_eq!(output.message, "ok");
        let status = handle.status().await.expect("status");
        assert_eq!(status.registered_definitions, 1);
        handle.shutdown().await;
        // After shutdown the loop is gone and further commands fail cleanly.
        let err = handle.trigger("sweep").await.expect_err("runner stopped");
        assert!(matches!(err, JobError::ExecutionFailed(_)), "got {err:?}");
        assert!(!handle.is_running());
    }

    #[tokio::test]
    async fn dedicated_thread_runner_serves_triggers() {
        let runs = Arc::new(AtomicU32::new(0));
        let runner = runner_with("sweep", false, Arc::clone(&runs))
            .with_tick_interval(Duration::from_secs(3600));
        let handle = runner.spawn_on_dedicated_thread("test-sweeps").expect("thread");
        handle.trigger("sweep").await.expect("triggered");
        assert!(runs.load(Ordering::SeqCst) >= 1);
        handle.shutdown().await;
    }
}
