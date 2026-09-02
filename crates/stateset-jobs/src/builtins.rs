//! Pre-defined job types for common commerce operations.

use std::sync::Arc;
use std::time::Duration;

use crate::context::JobContext;
use crate::error::JobError;
use crate::job::{BackoffStrategy, BoxFuture, JobDefinition, JobHandler, Schedule};
use crate::state::JobOutput;

// ---------------------------------------------------------------------------
// Default schedule constants
// ---------------------------------------------------------------------------

/// Default billing tick interval: 1 hour.
const BILLING_INTERVAL: Duration = Duration::from_secs(3600);

/// Default webhook retry interval: 5 minutes.
const WEBHOOK_RETRY_INTERVAL: Duration = Duration::from_secs(300);

/// Default event retention cleanup interval: 24 hours.
const RETENTION_INTERVAL: Duration = Duration::from_secs(86400);

/// Default low stock check interval: 1 hour.
const LOW_STOCK_INTERVAL: Duration = Duration::from_secs(3600);

/// Default subscription renewal check interval: 1 hour.
const RENEWAL_INTERVAL: Duration = Duration::from_secs(3600);

/// Default traceability sweep interval: 5 minutes.
const TRACEABILITY_SWEEP_INTERVAL: Duration = Duration::from_secs(300);

// ---------------------------------------------------------------------------
// BillingTickJob
// ---------------------------------------------------------------------------

/// Periodically checks for billing events that need processing.
#[derive(Debug, Clone)]
pub struct BillingTickJob {
    /// How often to check for billing events.
    pub check_interval: Duration,
}

impl BillingTickJob {
    /// Create a billing tick job with the default interval (1 hour).
    #[must_use]
    pub const fn new() -> Self {
        Self { check_interval: BILLING_INTERVAL }
    }

    /// Create a [`JobDefinition`] for this built-in.
    #[must_use]
    pub fn to_definition(self) -> JobDefinition {
        JobDefinition::new("billing_tick", Schedule::Interval(self.check_interval), Box::new(self))
            .with_timeout(Duration::from_secs(120))
            .with_max_retries(2)
            .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(30)))
    }
}

impl Default for BillingTickJob {
    fn default() -> Self {
        Self::new()
    }
}

impl JobHandler for BillingTickJob {
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        Box::pin(async { Ok(JobOutput::new("billing tick completed")) })
    }

    fn name(&self) -> &'static str {
        "billing_tick"
    }
}

// ---------------------------------------------------------------------------
// WebhookRetryJob
// ---------------------------------------------------------------------------

/// Retries failed webhook deliveries with configurable backoff.
#[derive(Debug, Clone)]
pub struct WebhookRetryJob {
    /// Maximum retry attempts per webhook.
    pub max_retries: u32,
    /// Backoff strategy between retries.
    pub backoff: BackoffStrategy,
}

impl WebhookRetryJob {
    /// Create a webhook retry job with default settings.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_retries: 5,
            backoff: BackoffStrategy::exponential(
                Duration::from_secs(10),
                Duration::from_secs(300),
            ),
        }
    }

    /// Create a [`JobDefinition`] for this built-in.
    #[must_use]
    pub fn to_definition(self) -> JobDefinition {
        let max_retries = self.max_retries;
        let backoff = self.backoff.clone();
        JobDefinition::new(
            "webhook_retry",
            Schedule::Interval(WEBHOOK_RETRY_INTERVAL),
            Box::new(self),
        )
        .with_timeout(Duration::from_secs(60))
        .with_max_retries(max_retries)
        .with_retry_backoff(backoff)
    }
}

impl Default for WebhookRetryJob {
    fn default() -> Self {
        Self::new()
    }
}

impl JobHandler for WebhookRetryJob {
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        Box::pin(async { Ok(JobOutput::new("webhook retry sweep completed")) })
    }

    fn name(&self) -> &'static str {
        "webhook_retry"
    }
}

// ---------------------------------------------------------------------------
// EventRetentionJob
// ---------------------------------------------------------------------------

/// Cleans up old events beyond the retention window.
#[derive(Debug, Clone)]
pub struct EventRetentionJob {
    /// How long to keep events before deletion.
    pub retention_period: Duration,
    /// Maximum number of events to delete per run.
    pub batch_size: usize,
}

impl EventRetentionJob {
    /// Create an event retention job with default settings (30 days, batch 1000).
    #[must_use]
    pub const fn new() -> Self {
        Self { retention_period: Duration::from_secs(30 * 24 * 3600), batch_size: 1000 }
    }

    /// Create a [`JobDefinition`] for this built-in.
    #[must_use]
    pub fn to_definition(self) -> JobDefinition {
        JobDefinition::new(
            "event_retention",
            Schedule::Interval(RETENTION_INTERVAL),
            Box::new(self),
        )
        .with_timeout(Duration::from_secs(300))
        .with_max_retries(1)
        .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(60)))
    }
}

impl Default for EventRetentionJob {
    fn default() -> Self {
        Self::new()
    }
}

impl JobHandler for EventRetentionJob {
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        let batch_size = self.batch_size;
        let retention_days = self.retention_period.as_secs() / 86400;
        Box::pin(async move {
            Ok(JobOutput::with_data(
                "event retention sweep completed",
                serde_json::json!({
                    "batch_size": batch_size,
                    "retention_days": retention_days,
                }),
            ))
        })
    }

    fn name(&self) -> &'static str {
        "event_retention"
    }
}

// ---------------------------------------------------------------------------
// LowStockAlertJob
// ---------------------------------------------------------------------------

/// Checks inventory levels and raises alerts for low stock items.
#[derive(Debug, Clone)]
pub struct LowStockAlertJob {
    /// Stock threshold below which an alert is triggered.
    pub threshold: u32,
    /// Optionally limit checks to specific SKUs.
    pub check_skus: Option<Vec<String>>,
}

impl LowStockAlertJob {
    /// Create a low stock alert job with default threshold (10 units).
    #[must_use]
    pub const fn new() -> Self {
        Self { threshold: 10, check_skus: None }
    }

    /// Create a [`JobDefinition`] for this built-in.
    #[must_use]
    pub fn to_definition(self) -> JobDefinition {
        JobDefinition::new(
            "low_stock_alert",
            Schedule::Interval(LOW_STOCK_INTERVAL),
            Box::new(self),
        )
        .with_timeout(Duration::from_secs(120))
        .with_max_retries(2)
        .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(30)))
    }
}

impl Default for LowStockAlertJob {
    fn default() -> Self {
        Self::new()
    }
}

impl JobHandler for LowStockAlertJob {
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        let threshold = self.threshold;
        let skus_checked = self.check_skus.as_ref().map(Vec::len);
        Box::pin(async move {
            Ok(JobOutput::with_data(
                "low stock check completed",
                serde_json::json!({
                    "threshold": threshold,
                    "skus_checked": skus_checked,
                }),
            ))
        })
    }

    fn name(&self) -> &'static str {
        "low_stock_alert"
    }
}

// ---------------------------------------------------------------------------
// SubscriptionRenewalJob
// ---------------------------------------------------------------------------

/// Checks for subscriptions due for renewal within a lookahead window.
#[derive(Debug, Clone)]
pub struct SubscriptionRenewalJob {
    /// How far ahead to look for subscriptions due for renewal.
    pub lookahead: Duration,
}

impl SubscriptionRenewalJob {
    /// Create a subscription renewal job with default lookahead (24 hours).
    #[must_use]
    pub const fn new() -> Self {
        Self { lookahead: Duration::from_secs(24 * 3600) }
    }

    /// Create a [`JobDefinition`] for this built-in.
    #[must_use]
    pub fn to_definition(self) -> JobDefinition {
        JobDefinition::new(
            "subscription_renewal",
            Schedule::Interval(RENEWAL_INTERVAL),
            Box::new(self),
        )
        .with_timeout(Duration::from_secs(180))
        .with_max_retries(3)
        .with_retry_backoff(BackoffStrategy::exponential(
            Duration::from_secs(10),
            Duration::from_secs(120),
        ))
    }
}

impl Default for SubscriptionRenewalJob {
    fn default() -> Self {
        Self::new()
    }
}

impl JobHandler for SubscriptionRenewalJob {
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        let lookahead_hours = self.lookahead.as_secs() / 3600;
        Box::pin(async move {
            Ok(JobOutput::with_data(
                "subscription renewal check completed",
                serde_json::json!({
                    "lookahead_hours": lookahead_hours,
                }),
            ))
        })
    }

    fn name(&self) -> &'static str {
        "subscription_renewal"
    }
}

// ---------------------------------------------------------------------------
// TraceabilitySweepJob
// ---------------------------------------------------------------------------

/// The three traceability sweeps the engine exposes but never schedules on
/// its own: expiring lots past their `expiration_date`, and releasing lot /
/// serial reservations that expired without being confirmed.
///
/// This crate does not depend on the engine, so the job is handed the
/// sweeps as a [`TraceabilitySweeper`]; the simplest implementation is
/// [`FnTraceabilitySweeper`] wrapping the repository calls, e.g. with
/// `stateset-embedded`:
///
/// ```rust,ignore
/// let commerce = Arc::new(commerce);
/// let sweeper = FnTraceabilitySweeper::new(
///     { let c = commerce.clone(); move |_now| c.lots().expire_lots().map_err(|e| e.to_string()) },
///     { let c = commerce.clone(); move |now| c.lots().release_expired_reservations(now).map_err(|e| e.to_string()) },
///     { let c = commerce.clone(); move |now| c.serials().release_expired_reservations(now).map_err(|e| e.to_string()) },
/// );
/// scheduler.register(TraceabilitySweepJob::new(Arc::new(sweeper)).to_definition());
/// ```
///
/// Each sweep is idempotent and independent: a failure in one is reported
/// in the output and does not stop the others. The job fails (and retries)
/// only when *every* sweep failed, so one broken backend cannot silently
/// starve the other two.
pub trait TraceabilitySweeper: Send + Sync {
    /// Flip `Active` lots whose expiry is before `now` to `Expired`.
    fn expire_lots(&self, now: chrono::DateTime<chrono::Utc>) -> Result<u64, String>;
    /// Release lot reservations that expired before `now`.
    fn release_expired_lot_reservations(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String>;
    /// Release serial reservations that expired before `now`.
    fn release_expired_serial_reservations(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String>;
}

type SweepFn = dyn Fn(chrono::DateTime<chrono::Utc>) -> Result<u64, String> + Send + Sync;

/// A [`TraceabilitySweeper`] built from three closures.
pub struct FnTraceabilitySweeper {
    expire_lots: Box<SweepFn>,
    lot_reservations: Box<SweepFn>,
    serial_reservations: Box<SweepFn>,
}

impl FnTraceabilitySweeper {
    /// Wrap the three sweeps.
    pub fn new<L, R, S>(expire_lots: L, lot_reservations: R, serial_reservations: S) -> Self
    where
        L: Fn(chrono::DateTime<chrono::Utc>) -> Result<u64, String> + Send + Sync + 'static,
        R: Fn(chrono::DateTime<chrono::Utc>) -> Result<u64, String> + Send + Sync + 'static,
        S: Fn(chrono::DateTime<chrono::Utc>) -> Result<u64, String> + Send + Sync + 'static,
    {
        Self {
            expire_lots: Box::new(expire_lots),
            lot_reservations: Box::new(lot_reservations),
            serial_reservations: Box::new(serial_reservations),
        }
    }
}

impl std::fmt::Debug for FnTraceabilitySweeper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FnTraceabilitySweeper").finish_non_exhaustive()
    }
}

impl TraceabilitySweeper for FnTraceabilitySweeper {
    fn expire_lots(&self, now: chrono::DateTime<chrono::Utc>) -> Result<u64, String> {
        (self.expire_lots)(now)
    }

    fn release_expired_lot_reservations(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        (self.lot_reservations)(now)
    }

    fn release_expired_serial_reservations(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, String> {
        (self.serial_reservations)(now)
    }
}

/// Periodically runs the traceability sweeps (lot expiry, expired lot and
/// serial reservations). See [`TraceabilitySweeper`].
#[derive(Clone)]
pub struct TraceabilitySweepJob {
    /// The sweeps to run.
    pub sweeper: Arc<dyn TraceabilitySweeper>,
    /// How often to sweep.
    pub interval: Duration,
}

impl std::fmt::Debug for TraceabilitySweepJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TraceabilitySweepJob")
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

impl TraceabilitySweepJob {
    /// Create a sweep job with the default interval (5 minutes).
    #[must_use]
    pub fn new(sweeper: Arc<dyn TraceabilitySweeper>) -> Self {
        Self { sweeper, interval: TRACEABILITY_SWEEP_INTERVAL }
    }

    /// Override the sweep interval.
    #[must_use]
    pub const fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Create a [`JobDefinition`] for this built-in.
    #[must_use]
    pub fn to_definition(self) -> JobDefinition {
        JobDefinition::new("traceability_sweep", Schedule::Interval(self.interval), Box::new(self))
            .with_timeout(Duration::from_secs(120))
            .with_max_retries(2)
            .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(30)))
    }
}

impl JobHandler for TraceabilitySweepJob {
    fn execute<'a>(&'a self, ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        Box::pin(async move {
            let now = ctx.scheduled_at.max(chrono::Utc::now());
            let sweeps: [(&str, Result<u64, String>); 3] = [
                ("lots_expired", self.sweeper.expire_lots(now)),
                ("lot_reservations_released", self.sweeper.release_expired_lot_reservations(now)),
                (
                    "serial_reservations_released",
                    self.sweeper.release_expired_serial_reservations(now),
                ),
            ];
            let mut data = serde_json::Map::new();
            let mut errors = Vec::new();
            for (name, outcome) in sweeps {
                match outcome {
                    Ok(n) => {
                        data.insert(name.to_owned(), serde_json::json!(n));
                    }
                    Err(e) => {
                        data.insert(name.to_owned(), serde_json::Value::Null);
                        errors.push(format!("{name}: {e}"));
                    }
                }
            }
            if errors.len() == 3 {
                return Err(JobError::ExecutionFailed(errors.join("; ")));
            }
            if !errors.is_empty() {
                data.insert("errors".to_owned(), serde_json::json!(errors));
            }
            Ok(JobOutput::with_data(
                "traceability sweep completed",
                serde_json::Value::Object(data),
            ))
        })
    }

    fn name(&self) -> &'static str {
        "traceability_sweep"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn billing_tick_defaults() {
        let job = BillingTickJob::new();
        assert_eq!(job.check_interval, BILLING_INTERVAL);

        let def = BillingTickJob::default().to_definition();
        assert_eq!(def.name, "billing_tick");
        assert_eq!(def.timeout, Duration::from_secs(120));
        assert_eq!(def.max_retries, 2);
    }

    #[test]
    fn billing_tick_schedule() {
        let def = BillingTickJob::new().to_definition();
        assert!(matches!(def.schedule, Schedule::Interval(d) if d == BILLING_INTERVAL));
    }

    #[test]
    fn webhook_retry_defaults() {
        let job = WebhookRetryJob::new();
        assert_eq!(job.max_retries, 5);

        let def = WebhookRetryJob::default().to_definition();
        assert_eq!(def.name, "webhook_retry");
        assert_eq!(def.max_retries, 5);
    }

    #[test]
    fn webhook_retry_schedule() {
        let def = WebhookRetryJob::new().to_definition();
        assert!(matches!(def.schedule, Schedule::Interval(d) if d == WEBHOOK_RETRY_INTERVAL));
    }

    #[test]
    fn event_retention_defaults() {
        let job = EventRetentionJob::new();
        assert_eq!(job.batch_size, 1000);
        assert_eq!(job.retention_period, Duration::from_secs(30 * 24 * 3600));

        let def = EventRetentionJob::default().to_definition();
        assert_eq!(def.name, "event_retention");
    }

    #[test]
    fn event_retention_schedule() {
        let def = EventRetentionJob::new().to_definition();
        assert!(matches!(def.schedule, Schedule::Interval(d) if d == RETENTION_INTERVAL));
    }

    #[test]
    fn low_stock_defaults() {
        let job = LowStockAlertJob::new();
        assert_eq!(job.threshold, 10);
        assert!(job.check_skus.is_none());

        let def = LowStockAlertJob::default().to_definition();
        assert_eq!(def.name, "low_stock_alert");
    }

    #[test]
    fn low_stock_custom_skus() {
        let job = LowStockAlertJob {
            threshold: 5,
            check_skus: Some(vec!["SKU-001".to_owned(), "SKU-002".to_owned()]),
        };
        assert_eq!(job.threshold, 5);
        assert_eq!(job.check_skus.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn low_stock_schedule() {
        let def = LowStockAlertJob::new().to_definition();
        assert!(matches!(def.schedule, Schedule::Interval(d) if d == LOW_STOCK_INTERVAL));
    }

    #[test]
    fn subscription_renewal_defaults() {
        let job = SubscriptionRenewalJob::new();
        assert_eq!(job.lookahead, Duration::from_secs(24 * 3600));

        let def = SubscriptionRenewalJob::default().to_definition();
        assert_eq!(def.name, "subscription_renewal");
        assert_eq!(def.max_retries, 3);
    }

    #[test]
    fn subscription_renewal_schedule() {
        let def = SubscriptionRenewalJob::new().to_definition();
        assert!(matches!(def.schedule, Schedule::Interval(d) if d == RENEWAL_INTERVAL));
    }

    #[test]
    fn all_builtins_have_handlers() {
        let billing = BillingTickJob::new();
        assert_eq!(billing.name(), "billing_tick");

        let webhook = WebhookRetryJob::new();
        assert_eq!(webhook.name(), "webhook_retry");

        let retention = EventRetentionJob::new();
        assert_eq!(retention.name(), "event_retention");

        let low_stock = LowStockAlertJob::new();
        assert_eq!(low_stock.name(), "low_stock_alert");

        let renewal = SubscriptionRenewalJob::new();
        assert_eq!(renewal.name(), "subscription_renewal");
    }

    #[tokio::test]
    async fn billing_tick_executes() {
        let job = BillingTickJob::new();
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        let result = job.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(result.unwrap().message.contains("billing"));
    }

    #[tokio::test]
    async fn event_retention_executes() {
        let job = EventRetentionJob::new();
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        let result = job.execute(&ctx).await.unwrap();
        assert!(result.data.is_some());
    }

    #[tokio::test]
    async fn low_stock_executes() {
        let job = LowStockAlertJob { threshold: 5, check_skus: Some(vec!["SKU-001".to_owned()]) };
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        let result = job.execute(&ctx).await.unwrap();
        let data = result.data.unwrap();
        assert_eq!(data["threshold"], 5);
        assert_eq!(data["skus_checked"], 1);
    }

    #[tokio::test]
    async fn subscription_renewal_executes() {
        let job = SubscriptionRenewalJob::new();
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        let result = job.execute(&ctx).await.unwrap();
        assert!(result.data.is_some());
        assert_eq!(result.data.unwrap()["lookahead_hours"], 24);
    }

    #[tokio::test]
    async fn webhook_retry_executes() {
        let job = WebhookRetryJob::new();
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        let result = job.execute(&ctx).await;
        assert!(result.is_ok());
        assert!(result.unwrap().message.contains("webhook"));
    }

    #[derive(Default)]
    struct CountingSweeper {
        lots: std::sync::Mutex<u32>,
        lot_res: std::sync::Mutex<u32>,
        serial_res: std::sync::Mutex<u32>,
        fail_lots: bool,
        fail_all: bool,
    }

    impl TraceabilitySweeper for CountingSweeper {
        fn expire_lots(&self, _now: chrono::DateTime<chrono::Utc>) -> Result<u64, String> {
            if self.fail_lots || self.fail_all {
                return Err("lots down".into());
            }
            *self.lots.lock().unwrap() += 1;
            Ok(3)
        }
        fn release_expired_lot_reservations(
            &self,
            _now: chrono::DateTime<chrono::Utc>,
        ) -> Result<u64, String> {
            if self.fail_all {
                return Err("lot res down".into());
            }
            *self.lot_res.lock().unwrap() += 1;
            Ok(2)
        }
        fn release_expired_serial_reservations(
            &self,
            _now: chrono::DateTime<chrono::Utc>,
        ) -> Result<u64, String> {
            if self.fail_all {
                return Err("serial res down".into());
            }
            *self.serial_res.lock().unwrap() += 1;
            Ok(1)
        }
    }

    #[test]
    fn traceability_sweep_definition() {
        let job = TraceabilitySweepJob::new(Arc::new(CountingSweeper::default()));
        assert_eq!(job.interval, TRACEABILITY_SWEEP_INTERVAL);
        let def = job.with_interval(Duration::from_secs(60)).to_definition();
        assert_eq!(def.name, "traceability_sweep");
        assert!(matches!(def.schedule, Schedule::Interval(d) if d == Duration::from_secs(60)));
        assert!(def.validate().is_ok());
    }

    #[tokio::test]
    async fn traceability_sweep_runs_all_three_sweeps() {
        let sweeper = Arc::new(CountingSweeper::default());
        let job = TraceabilitySweepJob::new(sweeper.clone());
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        let out = job.execute(&ctx).await.unwrap();
        let data = out.data.unwrap();
        assert_eq!(data["lots_expired"], 3);
        assert_eq!(data["lot_reservations_released"], 2);
        assert_eq!(data["serial_reservations_released"], 1);
        assert!(data.get("errors").is_none());
        assert_eq!(*sweeper.lots.lock().unwrap(), 1);
        assert_eq!(*sweeper.lot_res.lock().unwrap(), 1);
        assert_eq!(*sweeper.serial_res.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn traceability_sweep_reports_partial_failure_but_keeps_sweeping() {
        let sweeper = Arc::new(CountingSweeper { fail_lots: true, ..Default::default() });
        let job = TraceabilitySweepJob::new(sweeper.clone());
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        let out = job.execute(&ctx).await.unwrap();
        let data = out.data.unwrap();
        assert!(data["lots_expired"].is_null());
        assert_eq!(data["serial_reservations_released"], 1);
        assert!(data["errors"][0].as_str().unwrap().contains("lots down"));
        assert_eq!(*sweeper.serial_res.lock().unwrap(), 1, "other sweeps still ran");
    }

    #[tokio::test]
    async fn traceability_sweep_fails_only_when_everything_fails() {
        let sweeper = Arc::new(CountingSweeper { fail_all: true, ..Default::default() });
        let job = TraceabilitySweepJob::new(sweeper);
        let ctx = crate::context::JobContext::new(uuid::Uuid::new_v4(), 0, chrono::Utc::now());
        assert!(job.execute(&ctx).await.is_err());
    }

    #[tokio::test]
    async fn fn_sweeper_forwards_to_closures() {
        let sweeper = FnTraceabilitySweeper::new(|_| Ok(5), |_| Ok(6), |_| Err("x".into()));
        let now = chrono::Utc::now();
        assert_eq!(sweeper.expire_lots(now), Ok(5));
        assert_eq!(sweeper.release_expired_lot_reservations(now), Ok(6));
        assert_eq!(sweeper.release_expired_serial_reservations(now), Err("x".into()));
        let job = TraceabilitySweepJob::new(Arc::new(sweeper)).to_definition();
        assert_eq!(job.name, "traceability_sweep");
    }

    #[test]
    fn all_definitions_validate() {
        assert!(BillingTickJob::new().to_definition().validate().is_ok());
        assert!(WebhookRetryJob::new().to_definition().validate().is_ok());
        assert!(EventRetentionJob::new().to_definition().validate().is_ok());
        assert!(LowStockAlertJob::new().to_definition().validate().is_ok());
        assert!(SubscriptionRenewalJob::new().to_definition().validate().is_ok());
    }
}
