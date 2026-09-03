//! Background stock and traceability sweeps.
//!
//! The engine expires inventory reservations, backorder allocations, lots and
//! lot/serial reservations *lazily*: a hold only comes back when something
//! else touches the same `(item, location)`. `stateset-jobs` has shipped
//! [`ReservationSweepJob`] and [`TraceabilitySweepJob`] for exactly this, but
//! nothing depended on that crate, so in a real deployment the sweeps never
//! ran and an idle SKU kept counting holds that timed out long ago.
//!
//! This module is the missing wiring. It
//!
//! 1. turns a [`Commerce`] into the two sweeper trait objects the jobs crate
//!    expects ([`reservation_sweeper`], [`traceability_sweeper`]),
//! 2. registers both built-ins on a [`Scheduler`] ([`sweep_scheduler`]),
//! 3. starts that scheduler next to the HTTP listener
//!    ([`spawn_background_sweeps`], called from
//!    [`ServerBuilder::serve`](crate::ServerBuilder::serve)), and
//! 4. runs both sweeps on demand for an operator
//!    ([`run_sweeps_now`], behind `POST /api/v1/inventory/sweeps/run`).
//!
//! Both sweeps call straight into the blocking SQLite/Postgres repositories,
//! so the background runner lives on its own OS thread with its own
//! single-threaded runtime, and the on-demand run is dispatched to the
//! blocking pool. Neither can stall an axum worker.

use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use stateset_embedded::Commerce;
use stateset_jobs::{
    FnReservationSweeper, FnTraceabilitySweeper, InMemoryJobStore, JobError, JobRunner,
    JobRunnerHandle, ReservationSweepJob, Scheduler, TraceabilitySweepJob,
};

use crate::error::HttpError;

/// Registered name of the stock-hold sweep (inventory reservations + backorder
/// allocations).
pub const RESERVATION_SWEEP_JOB: &str = "reservation_sweep";
/// Registered name of the traceability sweep (lot expiry, lot and serial
/// reservations).
pub const TRACEABILITY_SWEEP_JOB: &str = "traceability_sweep";

/// Name of the OS thread the background runner uses.
const SWEEP_THREAD_NAME: &str = "stateset-sweeps";

/// How the two sweeps are scheduled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SweepConfig {
    /// Gap between stock-hold sweeps.
    pub reservation_interval: Duration,
    /// Gap between traceability sweeps.
    pub traceability_interval: Duration,
    /// Inventory reservations expired per transaction.
    pub batch_size: u32,
    /// Maximum batches per reservation-sweep run.
    pub max_batches: u32,
    /// Gap between scheduler ticks.
    pub tick_interval: Duration,
}

impl Default for SweepConfig {
    fn default() -> Self {
        Self {
            reservation_interval: stateset_jobs::builtins::RESERVATION_SWEEP_INTERVAL,
            traceability_interval: Duration::from_secs(300),
            batch_size: stateset_jobs::builtins::RESERVATION_SWEEP_BATCH_SIZE,
            max_batches: stateset_jobs::builtins::RESERVATION_SWEEP_MAX_BATCHES,
            tick_interval: stateset_jobs::DEFAULT_TICK_INTERVAL,
        }
    }
}

impl SweepConfig {
    /// Override both sweep intervals (used by tests to sweep aggressively).
    #[must_use]
    pub const fn with_intervals(mut self, reservation: Duration, traceability: Duration) -> Self {
        self.reservation_interval = reservation;
        self.traceability_interval = traceability;
        self
    }

    /// Override the scheduler tick.
    #[must_use]
    pub const fn with_tick_interval(mut self, tick_interval: Duration) -> Self {
        self.tick_interval = tick_interval;
        self
    }
}

/// Wrap `commerce`'s stock-hold sweeps for [`ReservationSweepJob`].
#[must_use]
pub fn reservation_sweeper(commerce: &Arc<Commerce>) -> FnReservationSweeper {
    let for_inventory = Arc::clone(commerce);
    let for_backorders = Arc::clone(commerce);
    FnReservationSweeper::new(
        move |now, limit| {
            for_inventory.inventory().expire_reservations(now, limit).map_err(|e| e.to_string())
        },
        move |_now| {
            for_backorders
                .backorder()
                .expire_allocations()
                .map(u64::from)
                .map_err(|e| e.to_string())
        },
    )
}

/// Wrap `commerce`'s traceability sweeps for [`TraceabilitySweepJob`].
#[must_use]
pub fn traceability_sweeper(commerce: &Arc<Commerce>) -> FnTraceabilitySweeper {
    let for_lots = Arc::clone(commerce);
    let for_lot_reservations = Arc::clone(commerce);
    let for_serials = Arc::clone(commerce);
    FnTraceabilitySweeper::new(
        move |_now| for_lots.lots().expire_lots().map_err(|e| e.to_string()),
        move |now| {
            for_lot_reservations.lots().release_expired_reservations(now).map_err(|e| e.to_string())
        },
        move |now| {
            for_serials.serials().release_expired_reservations(now).map_err(|e| e.to_string())
        },
    )
}

/// Build a scheduler carrying both engine sweeps against `commerce`.
///
/// # Errors
///
/// Returns [`JobError::InvalidSchedule`] if a configured interval is zero.
pub fn sweep_scheduler(
    commerce: &Arc<Commerce>,
    config: SweepConfig,
) -> Result<Scheduler, JobError> {
    let mut scheduler = Scheduler::new(Box::new(InMemoryJobStore::new()));
    scheduler.register(
        ReservationSweepJob::new(Arc::new(reservation_sweeper(commerce)))
            .with_interval(config.reservation_interval)
            .with_batch_size(config.batch_size)
            .with_max_batches(config.max_batches)
            .to_definition(),
    )?;
    scheduler.register(
        TraceabilitySweepJob::new(Arc::new(traceability_sweeper(commerce)))
            .with_interval(config.traceability_interval)
            .to_definition(),
    )?;
    Ok(scheduler)
}

/// Start both sweeps in the background on a dedicated thread.
///
/// The returned handle keeps the loop alive; call
/// [`JobRunnerHandle::shutdown`] to stop it, or
/// [`JobRunnerHandle::trigger`] to run one sweep immediately.
///
/// # Errors
///
/// Returns [`JobError`] if the schedule is invalid or the thread could not be
/// started.
pub fn spawn_background_sweeps(
    commerce: &Arc<Commerce>,
    config: SweepConfig,
) -> Result<JobRunnerHandle, JobError> {
    let scheduler = sweep_scheduler(commerce, config)?;
    JobRunner::new(scheduler)
        .with_tick_interval(config.tick_interval)
        .spawn_on_dedicated_thread(SWEEP_THREAD_NAME)
}

/// What one sweep reclaimed.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct SweepJobReport {
    /// The registered job name.
    pub job: String,
    /// Whether the sweep succeeded.
    pub ok: bool,
    /// Human-readable summary from the job handler.
    pub message: String,
    /// Per-sweep counters (`inventory_reservations_expired`,
    /// `backorder_allocations_expired`, `lots_expired`, …).
    #[schema(value_type = Object)]
    pub reclaimed: serde_json::Value,
}

/// Result of an operator-triggered sweep run.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, utoipa::ToSchema)]
pub struct SweepRunReport {
    /// One entry per sweep, in the order they ran.
    pub sweeps: Vec<SweepJobReport>,
}

/// Run both sweeps once, right now, against `commerce`.
///
/// The sweeps go through a real [`Scheduler`] + [`JobRunner`], so an on-demand
/// run is bookkept exactly like a scheduled one. Execution is dispatched to
/// the blocking pool because the sweeps call into blocking repositories.
///
/// # Errors
///
/// Returns [`HttpError::InternalError`] if the sweep runtime could not be
/// created; an individual sweep that failed is reported with `ok: false`
/// rather than failing the whole request.
pub async fn run_sweeps_now(
    commerce: Arc<Commerce>,
    config: SweepConfig,
) -> Result<SweepRunReport, HttpError> {
    tokio::task::spawn_blocking(move || {
        let scheduler = sweep_scheduler(&commerce, config)
            .map_err(|e| HttpError::InternalError(format!("could not build sweep runner: {e}")))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .map_err(|e| HttpError::InternalError(format!("could not start sweep runtime: {e}")))?;
        let mut runner = JobRunner::new(scheduler);
        runtime.block_on(async move {
            let mut sweeps = Vec::with_capacity(2);
            for job in [RESERVATION_SWEEP_JOB, TRACEABILITY_SWEEP_JOB] {
                let now = chrono::Utc::now();
                sweeps.push(match runner.run_now(job, now).await {
                    Ok(output) => SweepJobReport {
                        job: job.to_owned(),
                        ok: true,
                        message: output.message,
                        reclaimed: output.data.unwrap_or(serde_json::Value::Null),
                    },
                    Err(err) => SweepJobReport {
                        job: job.to_owned(),
                        ok: false,
                        message: err.to_string(),
                        reclaimed: serde_json::Value::Null,
                    },
                });
            }
            Ok(SweepRunReport { sweeps })
        })
    })
    .await
    .map_err(|e| HttpError::InternalError(format!("sweep task failed: {e}")))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal_macros::dec;
    use stateset_core::CreateInventoryItem;

    fn commerce_with_expired_hold(sku: &str) -> Arc<Commerce> {
        let commerce = Arc::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        commerce
            .inventory()
            .create_item(CreateInventoryItem {
                sku: sku.into(),
                name: "Sweepable".into(),
                initial_quantity: Some(dec!(10)),
                ..Default::default()
            })
            .expect("create item");
        let reservation =
            commerce.inventory().reserve(sku, dec!(4), "cart", "cart-1", Some(1)).expect("reserve");
        // Nothing else touches this SKU, so only a sweep can reclaim it.
        assert_eq!(
            commerce.inventory().get_stock(sku).expect("stock").expect("exists").total_allocated,
            dec!(4)
        );
        let _ = reservation;
        commerce
    }

    #[test]
    fn scheduler_registers_both_engine_sweeps() {
        let commerce = Arc::new(Commerce::new(":memory:").expect("in-memory Commerce"));
        let scheduler = sweep_scheduler(&commerce, SweepConfig::default()).expect("scheduler");
        assert_eq!(
            scheduler.definition_names(),
            vec![RESERVATION_SWEEP_JOB.to_owned(), TRACEABILITY_SWEEP_JOB.to_owned()]
        );
        assert_eq!(scheduler.status().registered_definitions, 2);
    }

    #[tokio::test]
    async fn scheduler_tick_runs_both_sweeps_and_reclaims_the_expired_hold() {
        let commerce = commerce_with_expired_hold("SWEEP-TICK");
        // Backdate the hold so it is expired.
        std::thread::sleep(Duration::from_millis(1100));

        let scheduler =
            sweep_scheduler(&commerce, SweepConfig::default()).expect("build sweep scheduler");
        let mut runner = JobRunner::new(scheduler);
        let now = Utc::now();
        runner.bootstrap(now).expect("bootstrap");
        let outcomes = runner.tick_once(now).await;

        assert_eq!(outcomes.len(), 2, "one tick must run both sweeps: {outcomes:?}");
        assert!(outcomes.iter().all(stateset_jobs::JobRunOutcome::is_ok), "{outcomes:?}");
        let reservation = outcomes
            .iter()
            .find(|o| o.definition_name == RESERVATION_SWEEP_JOB)
            .expect("reservation sweep ran");
        let data = reservation.result.as_ref().expect("ok").data.clone().expect("data");
        assert_eq!(data["inventory_reservations_expired"], 1);

        let stock =
            commerce.inventory().get_stock("SWEEP-TICK").expect("stock").expect("item exists");
        assert_eq!(stock.total_allocated, dec!(0), "the sweep must hand the units back");
        assert_eq!(stock.total_available, dec!(10));
    }

    #[tokio::test]
    async fn run_sweeps_now_reports_what_it_reclaimed() {
        let commerce = commerce_with_expired_hold("SWEEP-NOW");
        std::thread::sleep(Duration::from_millis(1100));

        let report =
            run_sweeps_now(Arc::clone(&commerce), SweepConfig::default()).await.expect("sweep");
        assert_eq!(report.sweeps.len(), 2);
        assert!(report.sweeps.iter().all(|s| s.ok), "{report:?}");
        let reservation = report
            .sweeps
            .iter()
            .find(|s| s.job == RESERVATION_SWEEP_JOB)
            .expect("reservation sweep");
        assert_eq!(reservation.reclaimed["inventory_reservations_expired"], 1);
        assert_eq!(
            commerce
                .inventory()
                .get_stock("SWEEP-NOW")
                .expect("stock")
                .expect("exists")
                .total_allocated,
            dec!(0)
        );
    }

    #[tokio::test]
    async fn background_sweeps_reclaim_without_any_traffic_on_the_sku() {
        let commerce = commerce_with_expired_hold("SWEEP-BG");
        let handle = spawn_background_sweeps(
            &commerce,
            SweepConfig::default()
                .with_intervals(Duration::from_millis(50), Duration::from_millis(50))
                .with_tick_interval(Duration::from_millis(20)),
        )
        .expect("spawn sweeps");

        // Wait for the hold to expire and the background loop to catch it.
        let mut reclaimed = false;
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let allocated = commerce
                .inventory()
                .get_stock("SWEEP-BG")
                .expect("stock")
                .expect("exists")
                .total_allocated;
            if allocated == dec!(0) {
                reclaimed = true;
                break;
            }
        }
        handle.shutdown().await;
        assert!(reclaimed, "the background scheduler must reclaim the expired hold on its own");
    }
}
