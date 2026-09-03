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

use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use crate::error::JobError;
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
    /// Run `name` right now and wait for its output.
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

    /// Stop the loop and wait for it to finish. Idempotent.
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
    /// itself when each run completes.
    ///
    /// # Errors
    ///
    /// Returns the first scheduling error (a full queue).
    pub fn bootstrap(&mut self, now: DateTime<Utc>) -> Result<(), JobError> {
        for name in self.scheduler.definition_names() {
            self.scheduler.schedule(&name, now)?;
        }
        Ok(())
    }

    /// Execute every action produced by a single tick at `now`.
    ///
    /// Handler failures are recorded on the scheduler (which may schedule a
    /// retry) and reported in the returned outcomes; they never abort the tick.
    pub async fn tick_once(&mut self, now: DateTime<Utc>) -> Vec<JobRunOutcome> {
        let actions = self.scheduler.tick(now);
        let mut outcomes = Vec::new();
        for action in actions {
            if let TickAction::Execute { job_id, definition_name, context } = action {
                let result = match self.scheduler.definition(&definition_name) {
                    Some(definition) => definition.handler.execute(&context).await,
                    // tick() only emits Execute for a queued instance and
                    // definitions are never removed, so this is unreachable in
                    // practice; report it rather than panicking.
                    None => Err(JobError::ExecutionFailed(format!(
                        "no handler registered for '{definition_name}'"
                    ))),
                };
                match result {
                    Ok(output) => {
                        if let Err(err) = self.scheduler.complete_at(job_id, output.clone(), now) {
                            tracing::warn!(%job_id, %definition_name, error = %err,
                                "failed to record job completion");
                        }
                        outcomes.push(JobRunOutcome {
                            job_id,
                            definition_name,
                            result: Ok(output),
                        });
                    }
                    Err(err) => {
                        let message = err.to_string();
                        if let Err(fail_err) = self.scheduler.fail_at(job_id, &message, now) {
                            tracing::warn!(%job_id, %definition_name, error = %fail_err,
                                "failed to record job failure");
                        }
                        tracing::warn!(%job_id, %definition_name, error = %message, "job failed");
                        outcomes.push(JobRunOutcome {
                            job_id,
                            definition_name,
                            result: Err(message),
                        });
                    }
                }
            }
        }
        outcomes
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
        Err(JobError::ExecutionFailed(format!(
            "job '{name}' did not run: the scheduler is at its concurrency limit"
        )))
    }

    /// Run the loop until a shutdown command arrives.
    ///
    /// Bootstraps every registered definition, then alternates between ticking
    /// and serving commands.
    async fn run(mut self, mut commands: mpsc::Receiver<RunnerCommand>) {
        if let Err(err) = self.bootstrap(Utc::now()) {
            tracing::error!(error = %err, "failed to bootstrap job runner");
        }
        let mut ticker = tokio::time::interval(self.tick_interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                command = commands.recv() => {
                    match command {
                        Some(RunnerCommand::Trigger { name, reply }) => {
                            let result = self.run_now(&name, Utc::now()).await;
                            let _ = reply.send(result);
                        }
                        Some(RunnerCommand::Status { reply }) => {
                            let _ = reply.send(self.scheduler.status());
                        }
                        Some(RunnerCommand::Shutdown { reply }) => {
                            let _ = reply.send(());
                            return;
                        }
                        // Every handle was dropped: nothing can trigger or
                        // stop us, but the recurring schedule is still the
                        // point of the loop, so keep ticking.
                        None => {
                            ticker.tick().await;
                            self.tick_once(Utc::now()).await;
                        }
                    }
                }
                _ = ticker.tick() => {
                    self.tick_once(Utc::now()).await;
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
