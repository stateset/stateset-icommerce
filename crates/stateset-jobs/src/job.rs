//! Job definitions, schedules, backoff strategies, and the handler trait.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::context::JobContext;
use crate::error::JobError;
use crate::state::JobOutput;

// ---------------------------------------------------------------------------
// Schedule
// ---------------------------------------------------------------------------

/// Determines when and how often a job runs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Schedule {
    /// Run immediately, once.
    Once,
    /// Repeat every `Duration`.
    Interval(Duration),
    /// Standard 5-field cron expression (`min hour dom month dow`).
    Cron(String),
    /// Triggered by an event type string.
    OnEvent(String),
}

impl Schedule {
    /// Validate the schedule, returning an error for invalid cron expressions.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidSchedule`] if a cron expression is malformed.
    pub fn validate(&self) -> Result<(), JobError> {
        match self {
            Self::Cron(expr) => validate_cron(expr),
            Self::Interval(d) if d.is_zero() => {
                Err(JobError::InvalidSchedule("interval must be > 0".to_owned()))
            }
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Cron validation
// ---------------------------------------------------------------------------

/// Validates a 5-field cron expression.
fn validate_cron(expr: &str) -> Result<(), JobError> {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(JobError::InvalidSchedule(format!(
            "expected 5 fields, got {}: {expr}",
            parts.len()
        )));
    }

    let fields = [
        ("minute", 0u32, 59u32),
        ("hour", 0, 23),
        ("day of month", 1, 31),
        ("month", 1, 12),
        ("day of week", 0, 6),
    ];

    for (i, (label, min, max)) in fields.iter().enumerate() {
        validate_cron_field(parts[i], label, *min, *max)?;
    }

    Ok(())
}

fn validate_cron_field(field: &str, label: &str, min: u32, max: u32) -> Result<(), JobError> {
    if field == "*" {
        return Ok(());
    }

    for token in field.split(',') {
        validate_cron_token(token.trim(), label, min, max)?;
    }

    Ok(())
}

fn validate_cron_token(token: &str, label: &str, min: u32, max: u32) -> Result<(), JobError> {
    let err = |msg: String| JobError::InvalidSchedule(format!("{label}: {msg}"));

    if let Some(step_str) = token.strip_prefix("*/") {
        let step: u32 = step_str.parse().map_err(|_| err(format!("invalid step '{step_str}'")))?;
        if step == 0 {
            return Err(err("step must be > 0".to_owned()));
        }
        return Ok(());
    }

    if token.contains('-') {
        let parts: Vec<&str> = token.splitn(2, '-').collect();
        if parts.len() != 2 {
            return Err(err(format!("malformed range '{token}'")));
        }
        let start: u32 =
            parts[0].parse().map_err(|_| err(format!("invalid range start '{}'", parts[0])))?;
        let end: u32 =
            parts[1].parse().map_err(|_| err(format!("invalid range end '{}'", parts[1])))?;
        if start < min || start > max || end < min || end > max {
            return Err(err(format!("range {start}-{end} out of bounds {min}-{max}")));
        }
        if start > end {
            return Err(err(format!("range start {start} > end {end}")));
        }
        return Ok(());
    }

    let val: u32 = token.parse().map_err(|_| err(format!("invalid value '{token}'")))?;
    if val < min || val > max {
        return Err(err(format!("value {val} out of bounds {min}-{max}")));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// BackoffStrategy
// ---------------------------------------------------------------------------

/// Strategy for computing delay between retries.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackoffStrategy {
    /// Wait a fixed duration between retries.
    Fixed(Duration),
    /// Double the delay each attempt, capped at `max`.
    Exponential {
        /// Base delay (first retry).
        base: Duration,
        /// Maximum delay cap.
        max: Duration,
    },
    /// Increase linearly by `step` each attempt, capped at `max`.
    Linear {
        /// Additive step per attempt.
        step: Duration,
        /// Maximum delay cap.
        max: Duration,
    },
}

impl BackoffStrategy {
    /// Create a fixed backoff.
    #[must_use]
    pub const fn fixed(duration: Duration) -> Self {
        Self::Fixed(duration)
    }

    /// Create an exponential backoff.
    #[must_use]
    pub const fn exponential(base: Duration, max: Duration) -> Self {
        Self::Exponential { base, max }
    }

    /// Create a linear backoff.
    #[must_use]
    pub const fn linear(step: Duration, max: Duration) -> Self {
        Self::Linear { step, max }
    }

    /// Compute the delay for the given attempt number (0-indexed).
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        match self {
            Self::Fixed(d) => *d,
            Self::Exponential { base, max } => {
                let multiplier = 1u64.checked_shl(attempt).unwrap_or(u64::MAX);
                let delay_ms = (base.as_millis() as u64).saturating_mul(multiplier);
                let delay = Duration::from_millis(delay_ms);
                if delay > *max { *max } else { delay }
            }
            Self::Linear { step, max } => {
                let delay_ms =
                    (step.as_millis() as u64).saturating_mul(u64::from(attempt).saturating_add(1));
                let delay = Duration::from_millis(delay_ms);
                if delay > *max { *max } else { delay }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JobHandler trait
// ---------------------------------------------------------------------------

/// A boxed future returned by [`JobHandler::execute`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Trait implemented by types that can execute job work.
///
/// Handlers are `Send + Sync` so they can be shared across threads.
///
/// The `execute` method returns a boxed future to allow dynamic dispatch
/// via `Box<dyn JobHandler>`.
pub trait JobHandler: Send + Sync {
    /// Execute the job, returning output on success.
    fn execute<'a>(&'a self, ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>>;

    /// The unique name of this handler.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// JobDefinition
// ---------------------------------------------------------------------------

/// A complete definition for a schedulable job.
#[derive(Debug)]
pub struct JobDefinition {
    /// Unique name identifying this job type.
    pub name: String,
    /// When / how often to run.
    pub schedule: Schedule,
    /// The handler that performs the work.
    pub handler: Box<dyn JobHandler>,
    /// Maximum wall-clock time before the job is considered timed out.
    pub timeout: Duration,
    /// Maximum number of retry attempts after failure.
    pub max_retries: u32,
    /// Strategy for computing delay between retries.
    pub retry_backoff: BackoffStrategy,
}

impl JobDefinition {
    /// Create a new job definition with sensible defaults.
    ///
    /// Defaults: timeout = 5 min, `max_retries` = 3, backoff = exponential 1s/60s.
    #[must_use]
    pub fn new(name: impl Into<String>, schedule: Schedule, handler: Box<dyn JobHandler>) -> Self {
        Self {
            name: name.into(),
            schedule,
            handler,
            timeout: Duration::from_secs(300),
            max_retries: 3,
            retry_backoff: BackoffStrategy::exponential(
                Duration::from_secs(1),
                Duration::from_secs(60),
            ),
        }
    }

    /// Set the timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum retry count.
    #[must_use]
    pub const fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Set the retry backoff strategy.
    #[must_use]
    pub const fn with_retry_backoff(mut self, backoff: BackoffStrategy) -> Self {
        self.retry_backoff = backoff;
        self
    }

    /// Validate the schedule configuration.
    ///
    /// # Errors
    ///
    /// Returns [`JobError::InvalidSchedule`] if the schedule is invalid.
    pub fn validate(&self) -> Result<(), JobError> {
        self.schedule.validate()
    }
}

// We implement Debug for dyn JobHandler so JobDefinition's derive(Debug) works.
impl std::fmt::Debug for dyn JobHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandler").field("name", &self.name()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Schedule validation
    // -----------------------------------------------------------------------

    #[test]
    fn schedule_once_is_valid() {
        assert!(Schedule::Once.validate().is_ok());
    }

    #[test]
    fn schedule_interval_positive_is_valid() {
        assert!(Schedule::Interval(Duration::from_secs(60)).validate().is_ok());
    }

    #[test]
    fn schedule_interval_zero_is_invalid() {
        assert!(Schedule::Interval(Duration::ZERO).validate().is_err());
    }

    #[test]
    fn schedule_on_event_is_valid() {
        assert!(Schedule::OnEvent("order.created".into()).validate().is_ok());
    }

    #[test]
    fn schedule_cron_valid_every_minute() {
        assert!(Schedule::Cron("* * * * *".into()).validate().is_ok());
    }

    #[test]
    fn schedule_cron_valid_hourly() {
        assert!(Schedule::Cron("0 * * * *".into()).validate().is_ok());
    }

    #[test]
    fn schedule_cron_valid_specific() {
        assert!(Schedule::Cron("30 2 15 6 3".into()).validate().is_ok());
    }

    #[test]
    fn schedule_cron_valid_step() {
        assert!(Schedule::Cron("*/5 * * * *".into()).validate().is_ok());
    }

    #[test]
    fn schedule_cron_valid_range() {
        assert!(Schedule::Cron("1-5 * * * *".into()).validate().is_ok());
    }

    #[test]
    fn schedule_cron_valid_list() {
        assert!(Schedule::Cron("1,15,30 * * * *".into()).validate().is_ok());
    }

    #[test]
    fn schedule_cron_too_few_fields() {
        assert!(Schedule::Cron("* * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_too_many_fields() {
        assert!(Schedule::Cron("* * * * * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_minute_out_of_range() {
        assert!(Schedule::Cron("60 * * * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_hour_out_of_range() {
        assert!(Schedule::Cron("0 24 * * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_dom_out_of_range() {
        assert!(Schedule::Cron("0 0 32 * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_dom_zero_invalid() {
        assert!(Schedule::Cron("0 0 0 * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_month_out_of_range() {
        assert!(Schedule::Cron("0 0 1 13 *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_dow_out_of_range() {
        assert!(Schedule::Cron("0 0 * * 7".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_invalid_step_zero() {
        assert!(Schedule::Cron("*/0 * * * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_invalid_step_text() {
        assert!(Schedule::Cron("*/abc * * * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_invalid_range_reversed() {
        assert!(Schedule::Cron("5-1 * * * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_invalid_value_text() {
        assert!(Schedule::Cron("abc * * * *".into()).validate().is_err());
    }

    #[test]
    fn schedule_cron_empty_string() {
        assert!(Schedule::Cron(String::new()).validate().is_err());
    }

    // -----------------------------------------------------------------------
    // BackoffStrategy
    // -----------------------------------------------------------------------

    #[test]
    fn backoff_fixed_constant() {
        let b = BackoffStrategy::fixed(Duration::from_secs(5));
        assert_eq!(b.delay_for_attempt(0), Duration::from_secs(5));
        assert_eq!(b.delay_for_attempt(1), Duration::from_secs(5));
        assert_eq!(b.delay_for_attempt(10), Duration::from_secs(5));
    }

    #[test]
    fn backoff_exponential_doubles() {
        let b = BackoffStrategy::exponential(Duration::from_secs(1), Duration::from_secs(60));
        assert_eq!(b.delay_for_attempt(0), Duration::from_secs(1));
        assert_eq!(b.delay_for_attempt(1), Duration::from_secs(2));
        assert_eq!(b.delay_for_attempt(2), Duration::from_secs(4));
        assert_eq!(b.delay_for_attempt(3), Duration::from_secs(8));
    }

    #[test]
    fn backoff_exponential_caps_at_max() {
        let b = BackoffStrategy::exponential(Duration::from_secs(1), Duration::from_secs(10));
        assert_eq!(b.delay_for_attempt(5), Duration::from_secs(10));
        assert_eq!(b.delay_for_attempt(100), Duration::from_secs(10));
    }

    #[test]
    fn backoff_linear_increments() {
        let b = BackoffStrategy::linear(Duration::from_secs(5), Duration::from_secs(30));
        assert_eq!(b.delay_for_attempt(0), Duration::from_secs(5));
        assert_eq!(b.delay_for_attempt(1), Duration::from_secs(10));
        assert_eq!(b.delay_for_attempt(2), Duration::from_secs(15));
    }

    #[test]
    fn backoff_linear_caps_at_max() {
        let b = BackoffStrategy::linear(Duration::from_secs(5), Duration::from_secs(15));
        assert_eq!(b.delay_for_attempt(3), Duration::from_secs(15));
        assert_eq!(b.delay_for_attempt(100), Duration::from_secs(15));
    }

    // -----------------------------------------------------------------------
    // JobDefinition
    // -----------------------------------------------------------------------

    struct NoopHandler;

    impl JobHandler for NoopHandler {
        fn execute<'a>(
            &'a self,
            _ctx: &'a JobContext,
        ) -> BoxFuture<'a, Result<JobOutput, JobError>> {
            Box::pin(async { Ok(JobOutput::new("noop")) })
        }
        fn name(&self) -> &str {
            "noop"
        }
    }

    #[test]
    fn job_definition_defaults() {
        let def = JobDefinition::new("test", Schedule::Once, Box::new(NoopHandler));
        assert_eq!(def.name, "test");
        assert_eq!(def.timeout, Duration::from_secs(300));
        assert_eq!(def.max_retries, 3);
    }

    #[test]
    fn job_definition_builder() {
        let def = JobDefinition::new("test", Schedule::Once, Box::new(NoopHandler))
            .with_timeout(Duration::from_secs(10))
            .with_max_retries(5)
            .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(2)));
        assert_eq!(def.timeout, Duration::from_secs(10));
        assert_eq!(def.max_retries, 5);
        assert_eq!(def.retry_backoff, BackoffStrategy::Fixed(Duration::from_secs(2)));
    }

    #[test]
    fn job_definition_validate_valid() {
        let def =
            JobDefinition::new("test", Schedule::Cron("0 * * * *".into()), Box::new(NoopHandler));
        assert!(def.validate().is_ok());
    }

    #[test]
    fn job_definition_validate_invalid_cron() {
        let def = JobDefinition::new("test", Schedule::Cron("bad".into()), Box::new(NoopHandler));
        assert!(def.validate().is_err());
    }

    #[test]
    fn job_handler_debug() {
        let handler: Box<dyn JobHandler> = Box::new(NoopHandler);
        let debug = format!("{handler:?}");
        assert!(debug.contains("noop"));
    }
}
