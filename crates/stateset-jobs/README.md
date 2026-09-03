# stateset-jobs

[![crates.io](https://img.shields.io/crates/v/stateset-jobs.svg)](https://crates.io/crates/stateset-jobs)
[![docs.rs](https://docs.rs/stateset-jobs/badge.svg)](https://docs.rs/stateset-jobs)

A Tokio-based background job scheduler for commerce workloads: cron, intervals,
one-shot and event-triggered jobs, with retry backoff, a validated lifecycle state
machine, and pluggable storage.

**Status: wired into the server.** `stateset-http` depends on this crate and starts a
`JobRunner` carrying the two engine sweeps — `ReservationSweepJob` (expired inventory
holds and backorder allocations) and `TraceabilitySweepJob` (lot expiry, lot and serial
reservations) — alongside the HTTP listener; see `stateset_http::sweeps`. Embedding the
crate directly and registering your own jobs against your own `Commerce` instance works
the same way.

A `Scheduler` only *decides* what should run (it returns `TickAction`s). `JobRunner` is
what executes the handlers: `JobRunner::spawn` on the current runtime, or
`JobRunner::spawn_on_dedicated_thread` for handlers that do blocking database work.

## Features

- **Cron scheduling** — standard 5-field cron expressions
- **Interval scheduling** — repeat every *N* seconds or minutes
- **One-shot jobs** — run immediately, once
- **Event-triggered jobs** — fired by an event type string
- **Retry with backoff** — fixed, exponential, or linear
- **State machine** — validated transitions for the job lifecycle
- **Pluggable storage** — `JobStore` trait with `InMemoryJobStore` and `FileJobStore`
- **Built-in jobs** — billing, webhook retry, retention, low stock, subscription renewal,
  reservation sweep, traceability sweep
- **Async runner** — `JobRunner` drives a `Scheduler`, executes handlers, and serves
  on-demand triggers through a `JobRunnerHandle`

## Usage

```rust
use stateset_jobs::{
    JobDefinition, Schedule, BackoffStrategy, JobContext, JobOutput,
    JobHandler, JobError, Scheduler, InMemoryJobStore,
};
use stateset_jobs::job::BoxFuture;
use std::time::Duration;

struct MyJob;

impl JobHandler for MyJob {
    fn name(&self) -> &str { "my_job" }
    fn execute<'a>(&'a self, _ctx: &'a JobContext) -> BoxFuture<'a, Result<JobOutput, JobError>> {
        Box::pin(async { Ok(JobOutput::new("done")) })
    }
}

let def = JobDefinition::new("my_job", Schedule::Once, Box::new(MyJob))
    .with_timeout(Duration::from_secs(30))
    .with_max_retries(3)
    .with_retry_backoff(BackoffStrategy::fixed(Duration::from_secs(5)));
```

Schedules cover the cases a commerce back office actually needs:

```rust
use stateset_jobs::Schedule;
use std::time::Duration;

let nightly = Schedule::Cron("0 2 * * *".into());          // 02:00 every day
let every_five = Schedule::Interval(Duration::from_secs(300));
let on_demand = Schedule::Once;
let on_event = Schedule::OnEvent("order.created".into());
```

## Retry Semantics

`BackoffStrategy` is `fixed`, `exponential`, or `linear`. Retries are bounded by
`with_max_retries`, and a job that exhausts them lands in a terminal failed state
rather than being retried forever — the state machine rejects the transition. Timeouts
are per-attempt, not per-job.

## Part of StateSet iCommerce

Designed to run alongside
[`stateset-embedded`](https://crates.io/crates/stateset-embedded). Part of the
[StateSet iCommerce](https://github.com/stateset/stateset-icommerce) engine.

## License

MIT OR Apache-2.0
