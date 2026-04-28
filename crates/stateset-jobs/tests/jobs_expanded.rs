//! Expanded integration tests for stateset-jobs.
//!
//! Covers job scheduling, execution, retry logic, timeout handling,
//! priority ordering, queue behavior, state transitions, stores,
//! and built-in job types.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use stateset_jobs::job::BoxFuture;
use stateset_jobs::store::{FileJobStore, JobStore};
use stateset_jobs::{
    BackoffStrategy, InMemoryJobStore, JobContext, JobDefinition, JobError, JobHandler,
    JobInstance, JobOutput, JobQueue, JobStatus, Schedule, Scheduler, TickAction,
};

// ---------------------------------------------------------------------------
// Test handler helpers
// ---------------------------------------------------------------------------

struct NoopHandler {
    handler_name: String,
}

impl NoopHandler {
    fn new(name: &str) -> Self {
        Self { handler_name: name.to_owned() }
    }
}

impl JobHandler for NoopHandler {
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        Box::pin(async { Ok(JobOutput::new("noop done")) })
    }
    fn name(&self) -> &str {
        &self.handler_name
    }
}

struct CountingHandler {
    handler_name: String,
    count: Arc<AtomicU32>,
}

impl CountingHandler {
    fn new(name: &str) -> (Self, Arc<AtomicU32>) {
        let count = Arc::new(AtomicU32::new(0));
        (Self { handler_name: name.to_owned(), count: Arc::clone(&count) }, count)
    }
}

impl JobHandler for CountingHandler {
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        self.count.fetch_add(1, Ordering::SeqCst);
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
    let def = JobDefinition::new(name, Schedule::Once, Box::new(NoopHandler::new(name)));
    scheduler.register(def).unwrap();
}

// ===========================================================================
// Job scheduling tests
// ===========================================================================

#[test]
fn schedule_creates_instance_in_store() {
    let mut s = make_scheduler();
    register_noop(&mut s, "test_job");

    let now = Utc::now();
    let id = s.schedule("test_job", now).unwrap();

    let instance = s.store().get(&id).unwrap().unwrap();
    assert_eq!(instance.definition_name, "test_job");
    assert_eq!(instance.status, JobStatus::Scheduled);
}

#[test]
fn schedule_unknown_definition_returns_error() {
    let mut s = make_scheduler();
    assert!(s.schedule("unknown", Utc::now()).is_err());
}

#[test]
fn schedule_multiple_jobs_creates_distinct_instances() {
    let mut s = make_scheduler();
    register_noop(&mut s, "job_a");
    register_noop(&mut s, "job_b");

    let now = Utc::now();
    let id_a = s.schedule("job_a", now).unwrap();
    let id_b = s.schedule("job_b", now).unwrap();

    assert_ne!(id_a, id_b);
    assert_eq!(s.status().queued_jobs, 2);
}

#[test]
fn schedule_same_definition_multiple_times() {
    let mut s = make_scheduler();
    register_noop(&mut s, "repeater");

    let now = Utc::now();
    s.schedule("repeater", now).unwrap();
    s.schedule("repeater", now).unwrap();
    s.schedule("repeater", now).unwrap();

    assert_eq!(s.status().queued_jobs, 3);
}

// ===========================================================================
// Job execution via tick
// ===========================================================================

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
    register_noop(&mut s, "future");

    let future = Utc::now() + chrono::Duration::hours(1);
    s.schedule("future", future).unwrap();

    let actions = s.tick(Utc::now());
    assert!(actions.is_empty());
}

#[test]
fn tick_respects_concurrency_limit() {
    let mut s = make_scheduler().with_max_concurrent(2);
    register_noop(&mut s, "a");
    register_noop(&mut s, "b");
    register_noop(&mut s, "c");

    let now = Utc::now();
    s.schedule("a", now).unwrap();
    s.schedule("b", now).unwrap();
    s.schedule("c", now).unwrap();

    let actions = s.tick(now);
    assert_eq!(actions.len(), 2); // only 2 due to concurrency limit
    assert_eq!(s.status().running_jobs, 2);
    assert_eq!(s.status().queued_jobs, 1); // 1 deferred back
}

#[test]
fn tick_empty_queue_returns_no_actions() {
    let mut s = make_scheduler();
    let actions = s.tick(Utc::now());
    assert!(actions.is_empty());
}

#[test]
fn tick_marks_job_running_in_store() {
    let mut s = make_scheduler();
    register_noop(&mut s, "test");
    let now = Utc::now();
    let id = s.schedule("test", now).unwrap();

    s.tick(now);

    let instance = s.store().get(&id).unwrap().unwrap();
    assert_eq!(instance.status, JobStatus::Running);
    assert!(instance.started_at.is_some());
}

// ===========================================================================
// Retry logic tests
// ===========================================================================

#[test]
fn fail_triggers_retry_when_under_limit() {
    let mut s = make_scheduler();
    let def =
        JobDefinition::new("retryable", Schedule::Once, Box::new(NoopHandler::new("retryable")))
            .with_max_retries(3)
            .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(10)));
    s.register(def).unwrap();

    let now = Utc::now();
    let id = s.schedule("retryable", now).unwrap();
    s.tick(now);

    let action = s.fail(id, "oops").unwrap();
    assert!(action.is_some());
    assert!(matches!(action.unwrap(), TickAction::Retry { .. }));
}

#[test]
fn fail_no_retry_when_retries_exhausted() {
    let mut s = make_scheduler();
    let def =
        JobDefinition::new("no_retry", Schedule::Once, Box::new(NoopHandler::new("no_retry")))
            .with_max_retries(0);
    s.register(def).unwrap();

    let now = Utc::now();
    let id = s.schedule("no_retry", now).unwrap();
    s.tick(now);

    let action = s.fail(id, "oops").unwrap();
    assert!(action.is_none());
}

#[test]
fn fail_at_uses_explicit_time_for_retry_calculation() {
    let mut s = make_scheduler();
    let def = JobDefinition::new(
        "timed_retry",
        Schedule::Once,
        Box::new(NoopHandler::new("timed_retry")),
    )
    .with_max_retries(1)
    .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(20)));
    s.register(def).unwrap();

    let t0 = Utc.with_ymd_and_hms(2026, 6, 1, 12, 0, 0).unwrap();
    let id = s.schedule("timed_retry", t0).unwrap();
    s.tick(t0);

    let action = s.fail_at(id, "error", t0 + chrono::Duration::seconds(5)).unwrap().unwrap();
    let TickAction::Retry { retry_at, .. } = action else { panic!("expected retry") };
    assert_eq!(retry_at, t0 + chrono::Duration::seconds(25));
}

#[test]
fn retry_requeues_job_in_queue() {
    let mut s = make_scheduler();
    let def = JobDefinition::new("retry_q", Schedule::Once, Box::new(NoopHandler::new("retry_q")))
        .with_max_retries(3)
        .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(5)));
    s.register(def).unwrap();

    let now = Utc::now();
    let id = s.schedule("retry_q", now).unwrap();
    s.tick(now);

    // Queue should be empty after tick started the job
    assert_eq!(s.status().queued_jobs, 0);

    // Fail the job -- this should trigger a retry and requeue
    let action = s.fail(id, "retry me").unwrap();
    assert!(action.is_some());
    assert!(s.status().queued_jobs > 0); // requeued for retry
}

// ===========================================================================
// Timeout handling tests
// ===========================================================================

#[test]
fn tick_detects_timed_out_job() {
    let mut s = make_scheduler();
    let def = JobDefinition::new("slow", Schedule::Once, Box::new(NoopHandler::new("slow")))
        .with_timeout(Duration::from_secs(10));
    s.register(def).unwrap();

    let t0 = Utc::now();
    s.schedule("slow", t0).unwrap();
    s.tick(t0); // starts running

    let t1 = t0 + chrono::Duration::seconds(15);
    let actions = s.tick(t1);
    assert!(actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
    assert_eq!(s.status().running_jobs, 0);
}

#[test]
fn tick_does_not_timeout_before_deadline() {
    let mut s = make_scheduler();
    let def = JobDefinition::new("fast", Schedule::Once, Box::new(NoopHandler::new("fast")))
        .with_timeout(Duration::from_secs(60));
    s.register(def).unwrap();

    let t0 = Utc::now();
    s.schedule("fast", t0).unwrap();
    s.tick(t0);

    let t1 = t0 + chrono::Duration::seconds(30);
    let actions = s.tick(t1);
    assert!(!actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
    assert_eq!(s.status().running_jobs, 1);
}

#[test]
fn timeout_triggers_retry_when_allowed() {
    let mut s = make_scheduler();
    let def = JobDefinition::new(
        "timeout_retry",
        Schedule::Once,
        Box::new(NoopHandler::new("timeout_retry")),
    )
    .with_timeout(Duration::from_secs(5))
    .with_max_retries(2)
    .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(10)));
    s.register(def).unwrap();

    let t0 = Utc::now();
    let id = s.schedule("timeout_retry", t0).unwrap();
    s.tick(t0);

    let t1 = t0 + chrono::Duration::seconds(10);
    let actions = s.tick(t1);

    assert!(actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
    assert!(actions.iter().any(|a| matches!(a, TickAction::Retry { .. })));

    let inst = s.store().get(&id).unwrap().unwrap();
    assert_eq!(inst.status, JobStatus::Retrying);
    assert_eq!(inst.attempt, 1);
}

#[test]
fn timeout_marks_failed_when_retries_exhausted() {
    let mut s = make_scheduler();
    let def = JobDefinition::new(
        "timeout_fail",
        Schedule::Once,
        Box::new(NoopHandler::new("timeout_fail")),
    )
    .with_timeout(Duration::from_secs(5))
    .with_max_retries(0);
    s.register(def).unwrap();

    let t0 = Utc::now();
    let id = s.schedule("timeout_fail", t0).unwrap();
    s.tick(t0);

    let t1 = t0 + chrono::Duration::seconds(10);
    let actions = s.tick(t1);

    assert!(actions.iter().any(|a| matches!(a, TickAction::Timeout { .. })));
    let inst = s.store().get(&id).unwrap().unwrap();
    assert_eq!(inst.status, JobStatus::Failed);
}

// ===========================================================================
// Priority / ordering tests
// ===========================================================================

#[test]
fn queue_dequeues_earliest_first() {
    let mut q = JobQueue::new(100);
    let t1 = Utc::now();
    let t2 = t1 + chrono::Duration::seconds(10);
    let t3 = t1 + chrono::Duration::seconds(20);

    let inst3 = JobInstance::new_scheduled("c", t3);
    let inst1 = JobInstance::new_scheduled("a", t1);
    let inst2 = JobInstance::new_scheduled("b", t2);

    q.enqueue(inst3).unwrap();
    q.enqueue(inst1).unwrap();
    q.enqueue(inst2).unwrap();

    // Only dequeue those due at or before t2
    let ready = q.dequeue_ready(t2);
    assert_eq!(ready.len(), 2);
    assert_eq!(q.size(), 1); // t3 remains
}

#[test]
fn queue_fifo_within_same_timestamp() {
    let mut q = JobQueue::new(100);
    let now = Utc::now();

    let a = JobInstance::new_scheduled("first", now);
    let b = JobInstance::new_scheduled("second", now);
    let a_id = a.id;

    q.enqueue(a).unwrap();
    q.enqueue(b).unwrap();

    let ready = q.dequeue_ready(now);
    assert_eq!(ready.len(), 2);
    assert_eq!(ready[0].id, a_id); // first-in first-out
}

#[test]
fn queue_peek_next_returns_earliest() {
    let mut q = JobQueue::new(100);
    let t1 = Utc::now();
    let t2 = t1 + chrono::Duration::seconds(60);

    q.enqueue(JobInstance::new_scheduled("later", t2)).unwrap();
    q.enqueue(JobInstance::new_scheduled("earlier", t1)).unwrap();

    assert_eq!(q.peek_next(), Some(t1));
}

#[test]
fn queue_full_rejects_enqueue() {
    let mut q = JobQueue::new(2);
    let now = Utc::now();

    q.enqueue(JobInstance::new_scheduled("a", now)).unwrap();
    q.enqueue(JobInstance::new_scheduled("b", now)).unwrap();

    let result = q.enqueue(JobInstance::new_scheduled("c", now));
    assert!(matches!(result, Err(JobError::QueueFull { capacity: 2, current: 2 })));
}

#[test]
fn queue_cancel_frees_capacity() {
    let mut q = JobQueue::new(1);
    let now = Utc::now();

    let inst = JobInstance::new_scheduled("test", now);
    let id = inst.id;
    q.enqueue(inst).unwrap();

    assert!(q.cancel(id));
    assert_eq!(q.size(), 0);
    assert!(q.enqueue(JobInstance::new_scheduled("replacement", now)).is_ok());
}

#[test]
fn queue_clear_empties_all() {
    let mut q = JobQueue::new(100);
    let now = Utc::now();
    for i in 0..10 {
        q.enqueue(JobInstance::new_scheduled(format!("job_{i}"), now)).unwrap();
    }
    assert_eq!(q.size(), 10);

    q.clear();
    assert!(q.is_empty());
    assert_eq!(q.size(), 0);
}

// ===========================================================================
// Complete and reschedule tests
// ===========================================================================

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
    assert!(inst.output.is_some());
    assert_eq!(s.status().running_jobs, 0);
}

#[test]
fn complete_interval_job_reschedules() {
    let mut s = make_scheduler();
    let (handler, _) = CountingHandler::new("interval");
    let def = JobDefinition::new(
        "interval",
        Schedule::Interval(Duration::from_secs(60)),
        Box::new(handler),
    );
    s.register(def).unwrap();

    let now = Utc::now();
    let id = s.schedule("interval", now).unwrap();
    s.tick(now);
    s.complete(id, JobOutput::new("done")).unwrap();

    assert!(s.status().queued_jobs > 0); // rescheduled
}

#[test]
fn complete_cron_job_reschedules() {
    let mut s = make_scheduler();
    let (handler, _) = CountingHandler::new("cron");
    let def = JobDefinition::new("cron", Schedule::Cron("*/10 * * * *".into()), Box::new(handler));
    s.register(def).unwrap();

    let now = Utc::now();
    let id = s.schedule("cron", now).unwrap();
    s.tick(now);
    s.complete(id, JobOutput::new("done")).unwrap();

    assert!(s.status().queued_jobs > 0);
    let next = s.status().next_run_at.expect("should be rescheduled");
    assert!(next > now);
}

#[test]
fn complete_once_job_does_not_reschedule() {
    let mut s = make_scheduler();
    register_noop(&mut s, "once");

    let now = Utc::now();
    let id = s.schedule("once", now).unwrap();
    s.tick(now);
    s.complete(id, JobOutput::new("done")).unwrap();

    assert_eq!(s.status().queued_jobs, 0); // no reschedule
}

// ===========================================================================
// Cancel tests
// ===========================================================================

#[test]
fn cancel_running_job() {
    let mut s = make_scheduler();
    register_noop(&mut s, "test");
    let now = Utc::now();
    let id = s.schedule("test", now).unwrap();
    s.tick(now);

    assert_eq!(s.status().running_jobs, 1);
    s.cancel(id).unwrap();
    assert_eq!(s.status().running_jobs, 0);

    let inst = s.store().get(&id).unwrap().unwrap();
    assert_eq!(inst.status, JobStatus::Cancelled);
}

#[test]
fn cancel_queued_job() {
    let mut s = make_scheduler();
    register_noop(&mut s, "test");
    let future = Utc::now() + chrono::Duration::hours(1);
    let id = s.schedule("test", future).unwrap();

    s.cancel(id).unwrap();
    let inst = s.store().get(&id).unwrap().unwrap();
    assert_eq!(inst.status, JobStatus::Cancelled);
}

#[test]
fn cancel_nonexistent_is_ok() {
    let mut s = make_scheduler();
    assert!(s.cancel(Uuid::new_v4()).is_ok());
}

// ===========================================================================
// State transition tests
// ===========================================================================

#[test]
fn job_instance_full_lifecycle() {
    let mut inst = JobInstance::new("lifecycle");
    assert_eq!(inst.status, JobStatus::Pending);

    inst.mark_running().unwrap();
    assert_eq!(inst.status, JobStatus::Running);
    assert!(inst.started_at.is_some());

    inst.mark_completed(JobOutput::new("success")).unwrap();
    assert_eq!(inst.status, JobStatus::Completed);
    assert!(inst.completed_at.is_some());
    assert!(inst.output.is_some());
}

#[test]
fn job_instance_retry_lifecycle() {
    let mut inst = JobInstance::new("retry_lc");
    inst.mark_running().unwrap();
    inst.mark_failed("error").unwrap();
    assert_eq!(inst.status, JobStatus::Failed);
    assert_eq!(inst.error.as_deref(), Some("error"));

    let retry_at = Utc::now() + chrono::Duration::seconds(10);
    inst.mark_retrying(retry_at).unwrap();
    assert_eq!(inst.status, JobStatus::Retrying);
    assert_eq!(inst.attempt, 1);
    assert_eq!(inst.next_run_at, Some(retry_at));

    inst.mark_running().unwrap();
    inst.mark_completed(JobOutput::new("ok")).unwrap();
    assert_eq!(inst.status, JobStatus::Completed);
}

#[test]
fn job_instance_timeout_lifecycle() {
    let mut inst = JobInstance::new("timeout_lc");
    inst.mark_running().unwrap();
    inst.mark_timed_out().unwrap();
    assert_eq!(inst.status, JobStatus::TimedOut);
    assert!(inst.error.is_some());

    let retry_at = Utc::now() + chrono::Duration::seconds(30);
    inst.mark_retrying(retry_at).unwrap();
    assert_eq!(inst.status, JobStatus::Retrying);
    assert_eq!(inst.attempt, 1);
}

#[test]
fn invalid_transitions_are_rejected() {
    let mut inst = JobInstance::new("bad_transition");

    // Cannot complete a pending job
    assert!(inst.mark_completed(JobOutput::new("nope")).is_err());

    // Cannot fail a pending job
    assert!(inst.mark_failed("nope").is_err());

    // Cannot time out a pending job
    assert!(inst.mark_timed_out().is_err());

    inst.mark_running().unwrap();
    inst.mark_completed(JobOutput::new("ok")).unwrap();

    // Cannot run a completed job
    assert!(inst.mark_running().is_err());

    // Cannot cancel a completed job
    assert!(inst.mark_cancelled().is_err());
}

#[test]
fn should_retry_logic() {
    let mut inst = JobInstance::new("retry_check");
    inst.status = JobStatus::Failed;
    inst.attempt = 0;
    assert!(inst.should_retry(3));

    inst.attempt = 3;
    assert!(!inst.should_retry(3));

    inst.status = JobStatus::TimedOut;
    inst.attempt = 0;
    assert!(inst.should_retry(1));

    inst.status = JobStatus::Completed;
    assert!(!inst.should_retry(10));
}

// ===========================================================================
// Backoff strategy tests
// ===========================================================================

#[test]
fn backoff_fixed_is_constant() {
    let b = BackoffStrategy::fixed(Duration::from_secs(10));
    for attempt in 0..20 {
        assert_eq!(b.delay_for_attempt(attempt), Duration::from_secs(10));
    }
}

#[test]
fn backoff_exponential_doubles_then_caps() {
    let b = BackoffStrategy::exponential(Duration::from_secs(1), Duration::from_secs(30));
    assert_eq!(b.delay_for_attempt(0), Duration::from_secs(1));
    assert_eq!(b.delay_for_attempt(1), Duration::from_secs(2));
    assert_eq!(b.delay_for_attempt(2), Duration::from_secs(4));
    assert_eq!(b.delay_for_attempt(3), Duration::from_secs(8));
    assert_eq!(b.delay_for_attempt(4), Duration::from_secs(16));
    assert_eq!(b.delay_for_attempt(5), Duration::from_secs(30)); // capped
    assert_eq!(b.delay_for_attempt(10), Duration::from_secs(30)); // still capped
}

#[test]
fn backoff_linear_increments_then_caps() {
    let b = BackoffStrategy::linear(Duration::from_secs(5), Duration::from_secs(25));
    assert_eq!(b.delay_for_attempt(0), Duration::from_secs(5)); // (0+1)*5 = 5
    assert_eq!(b.delay_for_attempt(1), Duration::from_secs(10)); // (1+1)*5 = 10
    assert_eq!(b.delay_for_attempt(2), Duration::from_secs(15)); // (2+1)*5 = 15
    assert_eq!(b.delay_for_attempt(3), Duration::from_secs(20)); // (3+1)*5 = 20
    assert_eq!(b.delay_for_attempt(4), Duration::from_secs(25)); // (4+1)*5 = 25
    assert_eq!(b.delay_for_attempt(100), Duration::from_secs(25)); // capped
}

#[test]
fn backoff_exponential_overflow_does_not_panic() {
    let b = BackoffStrategy::exponential(Duration::from_secs(1), Duration::from_secs(60));
    // Very high attempt number should not overflow or panic
    let delay = b.delay_for_attempt(u32::MAX);
    assert_eq!(delay, Duration::from_secs(60)); // capped at max
}

// ===========================================================================
// Schedule validation tests
// ===========================================================================

#[test]
fn schedule_once_validates() {
    assert!(Schedule::Once.validate().is_ok());
}

#[test]
fn schedule_interval_positive_validates() {
    assert!(Schedule::Interval(Duration::from_secs(60)).validate().is_ok());
}

#[test]
fn schedule_interval_zero_rejects() {
    assert!(Schedule::Interval(Duration::ZERO).validate().is_err());
}

#[test]
fn schedule_on_event_validates() {
    assert!(Schedule::OnEvent("order.created".into()).validate().is_ok());
}

#[test]
fn schedule_cron_valid_expressions() {
    let valid = vec![
        "* * * * *",
        "0 * * * *",
        "*/5 * * * *",
        "0 0 1 1 *",
        "30 2 15 6 3",
        "1-5 * * * *",
        "1,15,30 * * * *",
    ];
    for expr in valid {
        assert!(Schedule::Cron(expr.into()).validate().is_ok(), "expected valid: {expr}");
    }
}

#[test]
fn schedule_cron_invalid_expressions() {
    let invalid = vec![
        "",
        "* * *",
        "* * * * * *",
        "60 * * * *",
        "0 24 * * *",
        "0 0 32 * *",
        "0 0 0 * *",
        "0 0 1 13 *",
        "0 0 * * 7",
        "*/0 * * * *",
        "abc * * * *",
        "5-1 * * * *",
    ];
    for expr in invalid {
        assert!(Schedule::Cron(expr.into()).validate().is_err(), "expected invalid: {expr}");
    }
}

#[test]
fn schedule_next_run_after_interval() {
    let from = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
    let next = Schedule::Interval(Duration::from_secs(90)).next_run_after(from);
    assert_eq!(next, Some(from + chrono::Duration::seconds(90)));
}

#[test]
fn schedule_next_run_after_once_returns_none() {
    assert!(Schedule::Once.next_run_after(Utc::now()).is_none());
}

#[test]
fn schedule_next_run_after_on_event_returns_none() {
    assert!(Schedule::OnEvent("x".into()).next_run_after(Utc::now()).is_none());
}

// ===========================================================================
// Store tests
// ===========================================================================

#[test]
fn in_memory_store_save_and_get() {
    let store = InMemoryJobStore::new();
    let inst = JobInstance::new("test");
    let id = inst.id;
    store.save(&inst).unwrap();

    let retrieved = store.get(&id).unwrap().unwrap();
    assert_eq!(retrieved.definition_name, "test");
}

#[test]
fn in_memory_store_list_by_status() {
    let store = InMemoryJobStore::new();

    let pending = JobInstance::new("pending");
    store.save(&pending).unwrap();

    let mut running = JobInstance::new("running");
    running.status = JobStatus::Running;
    store.save(&running).unwrap();

    assert_eq!(store.list_by_status(JobStatus::Pending).unwrap().len(), 1);
    assert_eq!(store.list_by_status(JobStatus::Running).unwrap().len(), 1);
    assert_eq!(store.list_by_status(JobStatus::Completed).unwrap().len(), 0);
}

#[test]
fn in_memory_store_update_status() {
    let store = InMemoryJobStore::new();
    let inst = JobInstance::new("test");
    let id = inst.id;
    store.save(&inst).unwrap();

    store.update_status(&id, JobStatus::Running).unwrap();
    assert_eq!(store.get(&id).unwrap().unwrap().status, JobStatus::Running);
}

#[test]
fn in_memory_store_update_nonexistent_fails() {
    let store = InMemoryJobStore::new();
    assert!(store.update_status(&Uuid::new_v4(), JobStatus::Running).is_err());
}

#[test]
fn file_store_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.json");

    let inst = JobInstance::new("file_test");
    let id = inst.id;
    {
        let store = FileJobStore::open(&path).unwrap();
        store.save(&inst).unwrap();
    }

    let store = FileJobStore::open(&path).unwrap();
    let retrieved = store.get(&id).unwrap().unwrap();
    assert_eq!(retrieved.definition_name, "file_test");
}

#[test]
fn store_delete_completed_before() {
    let store = InMemoryJobStore::new();
    let now = Utc::now();

    let mut old = JobInstance::new("old");
    old.status = JobStatus::Running;
    old.mark_completed(JobOutput::new("done")).unwrap();
    old.completed_at = Some(now - chrono::Duration::hours(2));
    store.save(&old).unwrap();

    let pending = JobInstance::new("pending");
    store.save(&pending).unwrap();

    let cutoff = now - chrono::Duration::hours(1);
    let deleted = store.delete_completed_before(cutoff).unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(store.len(), 1);
}

// ===========================================================================
// Scheduler status tests
// ===========================================================================

#[test]
fn scheduler_status_reflects_state() {
    let mut s = make_scheduler().with_max_concurrent(5);
    register_noop(&mut s, "a");
    register_noop(&mut s, "b");

    let status = s.status();
    assert_eq!(status.registered_definitions, 2);
    assert_eq!(status.running_jobs, 0);
    assert_eq!(status.queued_jobs, 0);
    assert_eq!(status.max_concurrent, 5);
    assert!(status.next_run_at.is_none());
    assert!(status.last_internal_error.is_none());
}

#[test]
fn scheduler_status_after_schedule_and_tick() {
    let mut s = make_scheduler();
    register_noop(&mut s, "test");
    let now = Utc::now();
    s.schedule("test", now).unwrap();

    assert_eq!(s.status().queued_jobs, 1);
    s.tick(now);
    assert_eq!(s.status().queued_jobs, 0);
    assert_eq!(s.status().running_jobs, 1);
}

// ===========================================================================
// Built-in job type tests
// ===========================================================================

#[tokio::test]
async fn builtin_billing_tick_executes() {
    let job = stateset_jobs::BillingTickJob::new();
    let ctx = JobContext::new(Uuid::new_v4(), 0, Utc::now());
    let result = job.execute(&ctx).await.unwrap();
    assert!(result.message.contains("billing"));
}

#[tokio::test]
async fn builtin_webhook_retry_executes() {
    let job = stateset_jobs::WebhookRetryJob::new();
    let ctx = JobContext::new(Uuid::new_v4(), 0, Utc::now());
    let result = job.execute(&ctx).await.unwrap();
    assert!(result.message.contains("webhook"));
}

#[tokio::test]
async fn builtin_event_retention_executes() {
    let job = stateset_jobs::EventRetentionJob::new();
    let ctx = JobContext::new(Uuid::new_v4(), 0, Utc::now());
    let result = job.execute(&ctx).await.unwrap();
    assert!(result.data.is_some());
}

#[tokio::test]
async fn builtin_low_stock_alert_executes() {
    let job = stateset_jobs::LowStockAlertJob::new();
    let ctx = JobContext::new(Uuid::new_v4(), 0, Utc::now());
    let result = job.execute(&ctx).await.unwrap();
    assert!(result.message.contains("low stock"));
}

#[tokio::test]
async fn builtin_subscription_renewal_executes() {
    let job = stateset_jobs::SubscriptionRenewalJob::new();
    let ctx = JobContext::new(Uuid::new_v4(), 0, Utc::now());
    let result = job.execute(&ctx).await.unwrap();
    assert!(result.data.is_some());
}
