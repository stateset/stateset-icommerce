//! Job status, instances, and output types.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::JobError;
use crate::job::BackoffStrategy;

// ---------------------------------------------------------------------------
// JobStatus
// ---------------------------------------------------------------------------

/// The lifecycle status of a job instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum JobStatus {
    /// Waiting to be scheduled.
    Pending,
    /// Scheduled for a future run time.
    Scheduled,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Completed,
    /// Finished with an error (retries exhausted or not retryable).
    Failed,
    /// Waiting for a retry attempt.
    Retrying,
    /// Cancelled by the user or system.
    Cancelled,
    /// Exceeded the configured timeout.
    TimedOut,
}

impl JobStatus {
    /// Returns `true` if this status represents a terminal state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    /// Returns the set of statuses that are valid successors of `self`.
    #[must_use]
    pub const fn valid_transitions(self) -> &'static [Self] {
        match self {
            Self::Pending => &[Self::Scheduled, Self::Running, Self::Cancelled],
            Self::Scheduled => &[Self::Running, Self::Cancelled],
            Self::Running => {
                &[Self::Completed, Self::Failed, Self::TimedOut, Self::Retrying, Self::Cancelled]
            }
            Self::Retrying => &[Self::Running, Self::Cancelled, Self::Failed],
            Self::Failed => &[Self::Retrying],
            Self::TimedOut => &[Self::Retrying, Self::Failed],
            Self::Completed | Self::Cancelled => &[],
        }
    }

    /// Check whether transitioning from `self` to `target` is allowed.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the transition is not allowed.
    pub fn validate_transition(self, target: Self) -> Result<(), JobError> {
        if self.valid_transitions().contains(&target) {
            Ok(())
        } else {
            Err(JobError::InvalidTransition {
                from: format!("{self:?}"),
                to: format!("{target:?}"),
            })
        }
    }
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Scheduled => write!(f, "scheduled"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Retrying => write!(f, "retrying"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::TimedOut => write!(f, "timed_out"),
        }
    }
}

// ---------------------------------------------------------------------------
// JobOutput
// ---------------------------------------------------------------------------

/// The output produced by a successful job execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutput {
    /// Human-readable result message.
    pub message: String,
    /// Optional structured data.
    pub data: Option<serde_json::Value>,
}

impl JobOutput {
    /// Create a new output with only a message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), data: None }
    }

    /// Create a new output with a message and structured data.
    #[must_use]
    pub fn with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self { message: message.into(), data: Some(data) }
    }
}

// ---------------------------------------------------------------------------
// JobInstance
// ---------------------------------------------------------------------------

/// A concrete instance of a job that has been enqueued or executed.
#[derive(Debug, Clone)]
pub struct JobInstance {
    /// Unique identifier for this instance.
    pub id: Uuid,
    /// Name of the [`JobDefinition`](crate::job::JobDefinition) this instance belongs to.
    pub definition_name: String,
    /// Current lifecycle status.
    pub status: JobStatus,
    /// Current attempt number (starts at 0).
    pub attempt: u32,
    /// When this instance was created.
    pub created_at: DateTime<Utc>,
    /// When execution started (if it has).
    pub started_at: Option<DateTime<Utc>>,
    /// When execution completed (if it has).
    pub completed_at: Option<DateTime<Utc>>,
    /// When the next run is scheduled (for recurring/retrying jobs).
    pub next_run_at: Option<DateTime<Utc>>,
    /// Output from the last successful execution.
    pub output: Option<JobOutput>,
    /// Error message from the last failed execution.
    pub error: Option<String>,
}

impl JobInstance {
    /// Create a new pending job instance.
    #[must_use]
    pub fn new(definition_name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            definition_name: definition_name.into(),
            status: JobStatus::Pending,
            attempt: 0,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            next_run_at: None,
            output: None,
            error: None,
        }
    }

    /// Create a new pending job instance with a specific run time.
    #[must_use]
    pub fn new_scheduled(definition_name: impl Into<String>, run_at: DateTime<Utc>) -> Self {
        let mut instance = Self::new(definition_name);
        instance.status = JobStatus::Scheduled;
        instance.next_run_at = Some(run_at);
        instance
    }

    /// Transition the status, validating the transition.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the transition is not valid.
    pub fn transition_to(&mut self, target: JobStatus) -> Result<(), JobError> {
        self.status.validate_transition(target)?;
        self.status = target;
        Ok(())
    }

    /// Returns `true` if this job should be retried given `max_retries`.
    #[must_use]
    pub const fn should_retry(&self, max_retries: u32) -> bool {
        matches!(self.status, JobStatus::Failed | JobStatus::TimedOut) && self.attempt < max_retries
    }

    /// Compute the next retry time using the given backoff strategy.
    #[must_use]
    pub fn next_retry_at(&self, backoff: &BackoffStrategy, from: DateTime<Utc>) -> DateTime<Utc> {
        let delay = backoff.delay_for_attempt(self.attempt);
        from + chrono::Duration::from_std(delay).unwrap_or(chrono::Duration::seconds(60))
    }

    /// Mark this instance as running.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the current status does not
    /// allow transitioning to [`JobStatus::Running`].
    pub fn mark_running(&mut self) -> Result<(), JobError> {
        self.transition_to(JobStatus::Running)?;
        self.started_at = Some(Utc::now());
        Ok(())
    }

    /// Mark this instance as completed with output.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the current status does not
    /// allow transitioning to [`JobStatus::Completed`].
    pub fn mark_completed(&mut self, output: JobOutput) -> Result<(), JobError> {
        self.transition_to(JobStatus::Completed)?;
        self.completed_at = Some(Utc::now());
        self.output = Some(output);
        self.error = None;
        Ok(())
    }

    /// Mark this instance as failed with an error message.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the current status does not
    /// allow transitioning to [`JobStatus::Failed`].
    pub fn mark_failed(&mut self, error: impl Into<String>) -> Result<(), JobError> {
        self.transition_to(JobStatus::Failed)?;
        self.completed_at = Some(Utc::now());
        self.error = Some(error.into());
        Ok(())
    }

    /// Mark this instance as timed out.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the current status does not
    /// allow transitioning to [`JobStatus::TimedOut`].
    pub fn mark_timed_out(&mut self) -> Result<(), JobError> {
        self.transition_to(JobStatus::TimedOut)?;
        self.completed_at = Some(Utc::now());
        self.error = Some("timed out".to_owned());
        Ok(())
    }

    /// Mark this instance as retrying, incrementing the attempt counter.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the current status does not
    /// allow transitioning to [`JobStatus::Retrying`].
    pub fn mark_retrying(&mut self, next_run: DateTime<Utc>) -> Result<(), JobError> {
        self.transition_to(JobStatus::Retrying)?;
        self.attempt += 1;
        self.next_run_at = Some(next_run);
        self.completed_at = None;
        self.started_at = None;
        Ok(())
    }

    /// Mark this instance as cancelled.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidTransition`] if the current status does not
    /// allow transitioning to [`JobStatus::Cancelled`].
    pub fn mark_cancelled(&mut self) -> Result<(), JobError> {
        self.transition_to(JobStatus::Cancelled)?;
        self.completed_at = Some(Utc::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // JobStatus transitions
    // -----------------------------------------------------------------------

    #[test]
    fn pending_can_transition_to_running() {
        assert!(JobStatus::Pending.validate_transition(JobStatus::Running).is_ok());
    }

    #[test]
    fn pending_can_transition_to_scheduled() {
        assert!(JobStatus::Pending.validate_transition(JobStatus::Scheduled).is_ok());
    }

    #[test]
    fn pending_can_transition_to_cancelled() {
        assert!(JobStatus::Pending.validate_transition(JobStatus::Cancelled).is_ok());
    }

    #[test]
    fn pending_cannot_transition_to_completed() {
        assert!(JobStatus::Pending.validate_transition(JobStatus::Completed).is_err());
    }

    #[test]
    fn pending_cannot_transition_to_failed() {
        assert!(JobStatus::Pending.validate_transition(JobStatus::Failed).is_err());
    }

    #[test]
    fn scheduled_can_transition_to_running() {
        assert!(JobStatus::Scheduled.validate_transition(JobStatus::Running).is_ok());
    }

    #[test]
    fn scheduled_can_transition_to_cancelled() {
        assert!(JobStatus::Scheduled.validate_transition(JobStatus::Cancelled).is_ok());
    }

    #[test]
    fn scheduled_cannot_transition_to_completed() {
        assert!(JobStatus::Scheduled.validate_transition(JobStatus::Completed).is_err());
    }

    #[test]
    fn running_can_transition_to_completed() {
        assert!(JobStatus::Running.validate_transition(JobStatus::Completed).is_ok());
    }

    #[test]
    fn running_can_transition_to_failed() {
        assert!(JobStatus::Running.validate_transition(JobStatus::Failed).is_ok());
    }

    #[test]
    fn running_can_transition_to_timed_out() {
        assert!(JobStatus::Running.validate_transition(JobStatus::TimedOut).is_ok());
    }

    #[test]
    fn running_can_transition_to_retrying() {
        assert!(JobStatus::Running.validate_transition(JobStatus::Retrying).is_ok());
    }

    #[test]
    fn running_can_transition_to_cancelled() {
        assert!(JobStatus::Running.validate_transition(JobStatus::Cancelled).is_ok());
    }

    #[test]
    fn running_cannot_transition_to_pending() {
        assert!(JobStatus::Running.validate_transition(JobStatus::Pending).is_err());
    }

    #[test]
    fn retrying_can_transition_to_running() {
        assert!(JobStatus::Retrying.validate_transition(JobStatus::Running).is_ok());
    }

    #[test]
    fn retrying_can_transition_to_cancelled() {
        assert!(JobStatus::Retrying.validate_transition(JobStatus::Cancelled).is_ok());
    }

    #[test]
    fn retrying_can_transition_to_failed() {
        assert!(JobStatus::Retrying.validate_transition(JobStatus::Failed).is_ok());
    }

    #[test]
    fn completed_is_terminal() {
        assert!(JobStatus::Completed.is_terminal());
        assert!(JobStatus::Completed.validate_transition(JobStatus::Running).is_err());
    }

    #[test]
    fn failed_allows_retry() {
        assert!(!JobStatus::Failed.is_terminal());
        assert!(JobStatus::Failed.validate_transition(JobStatus::Retrying).is_ok());
        assert!(JobStatus::Failed.validate_transition(JobStatus::Running).is_err());
    }

    #[test]
    fn cancelled_is_terminal() {
        assert!(JobStatus::Cancelled.is_terminal());
    }

    #[test]
    fn timed_out_allows_retry() {
        assert!(!JobStatus::TimedOut.is_terminal());
        assert!(JobStatus::TimedOut.validate_transition(JobStatus::Retrying).is_ok());
        assert!(JobStatus::TimedOut.validate_transition(JobStatus::Failed).is_ok());
    }

    #[test]
    fn non_terminal_statuses() {
        assert!(!JobStatus::Pending.is_terminal());
        assert!(!JobStatus::Scheduled.is_terminal());
        assert!(!JobStatus::Running.is_terminal());
        assert!(!JobStatus::Retrying.is_terminal());
        assert!(!JobStatus::Failed.is_terminal());
        assert!(!JobStatus::TimedOut.is_terminal());
    }

    #[test]
    fn status_display() {
        assert_eq!(JobStatus::Pending.to_string(), "pending");
        assert_eq!(JobStatus::Running.to_string(), "running");
        assert_eq!(JobStatus::Completed.to_string(), "completed");
        assert_eq!(JobStatus::TimedOut.to_string(), "timed_out");
    }

    // -----------------------------------------------------------------------
    // JobOutput
    // -----------------------------------------------------------------------

    #[test]
    fn job_output_message_only() {
        let out = JobOutput::new("done");
        assert_eq!(out.message, "done");
        assert!(out.data.is_none());
    }

    #[test]
    fn job_output_with_data() {
        let data = serde_json::json!({"count": 42});
        let out = JobOutput::with_data("processed", data.clone());
        assert_eq!(out.message, "processed");
        assert_eq!(out.data.unwrap(), data);
    }

    // -----------------------------------------------------------------------
    // JobInstance
    // -----------------------------------------------------------------------

    #[test]
    fn new_instance_is_pending() {
        let inst = JobInstance::new("test_job");
        assert_eq!(inst.status, JobStatus::Pending);
        assert_eq!(inst.attempt, 0);
        assert!(inst.started_at.is_none());
        assert!(inst.completed_at.is_none());
    }

    #[test]
    fn new_scheduled_instance() {
        let run_at = Utc::now() + chrono::Duration::hours(1);
        let inst = JobInstance::new_scheduled("test_job", run_at);
        assert_eq!(inst.status, JobStatus::Scheduled);
        assert_eq!(inst.next_run_at, Some(run_at));
    }

    #[test]
    fn mark_running_sets_started_at() {
        let mut inst = JobInstance::new("test");
        inst.mark_running().unwrap();
        assert_eq!(inst.status, JobStatus::Running);
        assert!(inst.started_at.is_some());
    }

    #[test]
    fn mark_completed_sets_output() {
        let mut inst = JobInstance::new("test");
        inst.mark_running().unwrap();
        inst.mark_completed(JobOutput::new("ok")).unwrap();
        assert_eq!(inst.status, JobStatus::Completed);
        assert!(inst.completed_at.is_some());
        assert!(inst.output.is_some());
        assert!(inst.error.is_none());
    }

    #[test]
    fn mark_failed_sets_error() {
        let mut inst = JobInstance::new("test");
        inst.mark_running().unwrap();
        inst.mark_failed("oops").unwrap();
        assert_eq!(inst.status, JobStatus::Failed);
        assert_eq!(inst.error.as_deref(), Some("oops"));
    }

    #[test]
    fn mark_timed_out() {
        let mut inst = JobInstance::new("test");
        inst.mark_running().unwrap();
        inst.mark_timed_out().unwrap();
        assert_eq!(inst.status, JobStatus::TimedOut);
    }

    #[test]
    fn mark_retrying_increments_attempt() {
        let mut inst = JobInstance::new("test");
        inst.mark_running().unwrap();
        inst.mark_timed_out().unwrap();
        let next = Utc::now() + chrono::Duration::seconds(5);
        inst.mark_retrying(next).unwrap();
        assert_eq!(inst.status, JobStatus::Retrying);
        assert_eq!(inst.attempt, 1);
        assert_eq!(inst.next_run_at, Some(next));
        assert!(inst.completed_at.is_none());
    }

    #[test]
    fn mark_cancelled_from_pending() {
        let mut inst = JobInstance::new("test");
        inst.mark_cancelled().unwrap();
        assert_eq!(inst.status, JobStatus::Cancelled);
    }

    #[test]
    fn cannot_complete_pending() {
        let mut inst = JobInstance::new("test");
        assert!(inst.mark_completed(JobOutput::new("nope")).is_err());
    }

    #[test]
    fn cannot_run_completed() {
        let mut inst = JobInstance::new("test");
        inst.mark_running().unwrap();
        inst.mark_completed(JobOutput::new("ok")).unwrap();
        assert!(inst.mark_running().is_err());
    }

    // -----------------------------------------------------------------------
    // should_retry
    // -----------------------------------------------------------------------

    #[test]
    fn should_retry_when_failed_and_under_limit() {
        let mut inst = JobInstance::new("test");
        inst.status = JobStatus::Failed;
        inst.attempt = 1;
        assert!(inst.should_retry(3));
    }

    #[test]
    fn should_not_retry_when_at_limit() {
        let mut inst = JobInstance::new("test");
        inst.status = JobStatus::Failed;
        inst.attempt = 3;
        assert!(!inst.should_retry(3));
    }

    #[test]
    fn should_not_retry_when_completed() {
        let mut inst = JobInstance::new("test");
        inst.status = JobStatus::Completed;
        assert!(!inst.should_retry(3));
    }

    #[test]
    fn should_retry_when_timed_out_and_under_limit() {
        let mut inst = JobInstance::new("test");
        inst.status = JobStatus::TimedOut;
        inst.attempt = 0;
        assert!(inst.should_retry(3));
    }

    // -----------------------------------------------------------------------
    // next_retry_at
    // -----------------------------------------------------------------------

    #[test]
    fn next_retry_at_fixed() {
        let inst = JobInstance::new("test");
        let backoff = BackoffStrategy::fixed(std::time::Duration::from_secs(10));
        let now = Utc::now();
        let next = inst.next_retry_at(&backoff, now);
        assert_eq!(next, now + chrono::Duration::seconds(10));
    }

    #[test]
    fn next_retry_at_exponential() {
        let mut inst = JobInstance::new("test");
        inst.attempt = 2;
        let backoff = BackoffStrategy::exponential(
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(60),
        );
        let now = Utc::now();
        let next = inst.next_retry_at(&backoff, now);
        // attempt=2 -> 2^2 = 4 seconds
        assert_eq!(next, now + chrono::Duration::seconds(4));
    }

    #[test]
    fn next_retry_at_linear() {
        let mut inst = JobInstance::new("test");
        inst.attempt = 2;
        let backoff = BackoffStrategy::linear(
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(30),
        );
        let now = Utc::now();
        let next = inst.next_retry_at(&backoff, now);
        // attempt=2 -> (2+1)*5 = 15 seconds
        assert_eq!(next, now + chrono::Duration::seconds(15));
    }
}
