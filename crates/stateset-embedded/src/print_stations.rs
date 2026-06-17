//! Print station operations (paired agents + print job queue).

use stateset_core::{
    CreatePrintStation, EnqueuePrintJob, PairStationResult, PrintJob, PrintJobFilter, PrintJobId,
    PrintStation, PrintStationId, Result,
};
use stateset_db::{Database, DatabaseCapability};
use std::sync::Arc;

/// Print station operations.
pub struct PrintStations {
    db: Arc<dyn Database>,
}

impl std::fmt::Debug for PrintStations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrintStations").finish_non_exhaustive()
    }
}

impl PrintStations {
    pub(crate) fn new(db: Arc<dyn Database>) -> Self {
        Self { db }
    }

    /// Whether print stations are supported by the active backend.
    #[must_use]
    pub fn is_supported(&self) -> bool {
        self.db.supports_capability(DatabaseCapability::PrintStations)
    }

    fn ensure(&self) -> Result<()> {
        self.db.ensure_capability(DatabaseCapability::PrintStations)
    }

    /// Pair a new print station, returning the one-time token.
    pub fn pair(&self, input: CreatePrintStation) -> Result<PairStationResult> {
        self.ensure()?;
        self.db.print_stations().pair(input)
    }

    /// List paired stations.
    pub fn list_stations(&self) -> Result<Vec<PrintStation>> {
        self.ensure()?;
        self.db.print_stations().list_stations()
    }

    /// Get a station by ID.
    pub fn get_station(&self, id: PrintStationId) -> Result<Option<PrintStation>> {
        self.ensure()?;
        self.db.print_stations().get_station(id)
    }

    /// Revoke a station.
    pub fn revoke_station(&self, id: PrintStationId) -> Result<PrintStation> {
        self.ensure()?;
        self.db.print_stations().revoke_station(id)
    }

    /// Enqueue a print job to a station.
    pub fn enqueue_job(
        &self,
        station_id: PrintStationId,
        input: EnqueuePrintJob,
    ) -> Result<PrintJob> {
        self.ensure()?;
        self.db.print_stations().enqueue_job(station_id, input)
    }

    /// Pick up the next queued job for a station.
    pub fn next_job(&self, station_id: PrintStationId) -> Result<Option<PrintJob>> {
        self.ensure()?;
        self.db.print_stations().next_job(station_id)
    }

    /// Mark a job printed or failed.
    pub fn complete_job(&self, job_id: PrintJobId, success: bool) -> Result<PrintJob> {
        self.ensure()?;
        self.db.print_stations().complete_job(job_id, success)
    }

    /// List jobs for a station.
    pub fn list_jobs(
        &self,
        station_id: PrintStationId,
        filter: PrintJobFilter,
    ) -> Result<Vec<PrintJob>> {
        self.ensure()?;
        self.db.print_stations().list_jobs(station_id, filter)
    }
}
