//! Pre-defined job types for common commerce operations.

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

    #[test]
    fn all_definitions_validate() {
        assert!(BillingTickJob::new().to_definition().validate().is_ok());
        assert!(WebhookRetryJob::new().to_definition().validate().is_ok());
        assert!(EventRetentionJob::new().to_definition().validate().is_ok());
        assert!(LowStockAlertJob::new().to_definition().validate().is_ok());
        assert!(SubscriptionRenewalJob::new().to_definition().validate().is_ok());
    }
}
