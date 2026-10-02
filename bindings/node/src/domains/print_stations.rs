//! Print stations  (paired agents + print job queue).
//!
//! Split out of the former single-file binding; shares helpers and sibling
//! types through `use super::*`.

#![allow(clippy::wildcard_imports)]

use super::*;

// ============================================================================
// Print stations  (paired agents + print job queue)
// ============================================================================

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CreatePrintStationInput {
    pub name: String,
    pub printers: Option<Vec<String>>,
}

/// Filter for `listStations()`; omit for every station.
#[napi(object)]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PrintStationFilterInput {
    /// Only revoked (`true`) or only live (`false`) stations
    pub revoked: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct EnqueuePrintJobInput {
    pub printer_name: Option<String>,
    /// zpl or pdf
    #[napi(ts_type = "PrintPayloadKind")]
    pub payload_kind: Option<String>,
    pub payload: String,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrintJobFilterInput {
    /// queued, picked_up, printed, failed
    #[napi(ts_type = "PrintJobStatus")]
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrintStationOutput {
    pub id: String,
    pub name: String,
    pub printers: Vec<String>,
    pub revoked: bool,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<stateset_core::PrintStation> for PrintStationOutput {
    fn from(s: stateset_core::PrintStation) -> Self {
        Self {
            id: s.id.to_string(),
            name: s.name,
            printers: s.printers,
            revoked: s.revoked,
            last_seen_at: s.last_seen_at.map(|d| d.to_rfc3339()),
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PairStationResultOutput {
    pub station: PrintStationOutput,
    /// One-time pairing token; shown only at pairing time
    pub token: String,
}

impl From<stateset_core::PairStationResult> for PairStationResultOutput {
    fn from(r: stateset_core::PairStationResult) -> Self {
        Self { station: r.station.into(), token: r.token }
    }
}

#[napi(object)]
#[derive(Serialize, Deserialize, Clone)]
pub struct PrintJobOutput {
    pub id: String,
    pub station_id: String,
    pub printer_name: Option<String>,
    /// zpl or pdf
    #[napi(ts_type = "PrintPayloadKind")]
    pub payload_kind: String,
    pub payload: String,
    /// queued, picked_up, printed, failed
    #[napi(ts_type = "PrintJobStatus")]
    pub status: String,
    pub created_at: String,
    pub picked_up_at: Option<String>,
}

impl From<stateset_core::PrintJob> for PrintJobOutput {
    fn from(j: stateset_core::PrintJob) -> Self {
        Self {
            id: j.id.to_string(),
            station_id: j.station_id.to_string(),
            printer_name: j.printer_name,
            payload_kind: j.payload_kind.to_string(),
            payload: j.payload,
            status: j.status.to_string(),
            created_at: j.created_at.to_rfc3339(),
            picked_up_at: j.picked_up_at.map(|d| d.to_rfc3339()),
        }
    }
}

pub(crate) fn parse_print_payload_kind(s: &str) -> Result<stateset_core::PrintPayloadKind> {
    s.parse::<stateset_core::PrintPayloadKind>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid print payload kind: {s}")))
}

pub(crate) fn parse_print_job_status(s: &str) -> Result<stateset_core::PrintJobStatus> {
    s.parse::<stateset_core::PrintJobStatus>()
        .map_err(|_| coded(ErrCode::Validation, format!("Invalid print job status: {s}")))
}

#[napi]
pub struct PrintStations {
    pub(crate) commerce: Handle,
}

#[napi]
impl PrintStations {
    /// Whether the print-stations backend is available on this engine build.
    #[napi]
    pub async fn is_supported(&self) -> Result<bool> {
        let commerce = self.commerce.get()?;
        Ok(commerce.print_stations().is_supported())
    }

    /// Pair a new station, returning the station and its one-time token.
    #[napi]
    pub async fn pair(&self, input: CreatePrintStationInput) -> Result<PairStationResultOutput> {
        let commerce = self.commerce.get()?;
        let result = commerce
            .print_stations()
            .pair(stateset_core::CreatePrintStation {
                name: input.name,
                printers: input.printers.unwrap_or_default(),
            })
            .map_err(|e| wrap(ErrCode::Internal, "Failed to pair print station", e))?;
        Ok(result.into())
    }

    /// List paired stations, optionally filtered by `revoked` and windowed
    /// by `limit`/`offset`.
    #[napi]
    pub async fn list_stations(
        &self,
        filter: Option<PrintStationFilterInput>,
    ) -> Result<Vec<PrintStationOutput>> {
        let commerce = self.commerce.get()?;
        let filter = filter.unwrap_or_default();
        let stations = commerce
            .print_stations()
            .list_stations()
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list print stations", e))?;
        let stations: Vec<_> = stations
            .into_iter()
            .filter(|s| filter.revoked.is_none_or(|r| s.revoked == r))
            .collect();
        Ok(page(stations, filter.limit, filter.offset).into_iter().map(Into::into).collect())
    }

    #[napi]
    pub async fn get_station(&self, id: String) -> Result<Option<PrintStationOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "print_station")?;
        let station = commerce
            .print_stations()
            .get_station(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get print station", e))?;
        Ok(station.map(Into::into))
    }

    #[napi]
    pub async fn revoke_station(&self, id: String) -> Result<PrintStationOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&id, "print_station")?;
        let station = commerce
            .print_stations()
            .revoke_station(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to revoke print station", e))?;
        Ok(station.into())
    }

    #[napi]
    pub async fn enqueue_job(
        &self,
        station_id: String,
        input: EnqueuePrintJobInput,
    ) -> Result<PrintJobOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&station_id, "print_station")?;
        let payload_kind = match input.payload_kind.as_deref() {
            Some(s) => parse_print_payload_kind(s)?,
            None => stateset_core::PrintPayloadKind::default(),
        };
        let job = commerce
            .print_stations()
            .enqueue_job(
                uuid.into(),
                stateset_core::EnqueuePrintJob {
                    printer_name: input.printer_name,
                    payload_kind,
                    payload: input.payload,
                },
            )
            .map_err(|e| wrap(ErrCode::Internal, "Failed to enqueue print job", e))?;
        Ok(job.into())
    }

    /// Pick up the next queued job for a station.
    #[napi]
    pub async fn next_job(&self, station_id: String) -> Result<Option<PrintJobOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&station_id, "print_station")?;
        let job = commerce
            .print_stations()
            .next_job(uuid.into())
            .map_err(|e| wrap(ErrCode::Internal, "Failed to get next print job", e))?;
        Ok(job.map(Into::into))
    }

    /// Mark a job printed (success) or failed.
    #[napi]
    pub async fn complete_job(&self, job_id: String, success: bool) -> Result<PrintJobOutput> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&job_id, "print_job")?;
        let job = commerce
            .print_stations()
            .complete_job(uuid.into(), success)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to complete print job", e))?;
        Ok(job.into())
    }

    #[napi]
    pub async fn list_jobs(
        &self,
        station_id: String,
        filter: Option<PrintJobFilterInput>,
    ) -> Result<Vec<PrintJobOutput>> {
        let commerce = self.commerce.get()?;
        let uuid = parse_uuid_str(&station_id, "print_station")?;
        let filter = filter.map_or_else(
            || Ok(stateset_core::PrintJobFilter::default()),
            |f| -> Result<stateset_core::PrintJobFilter> {
                Ok(stateset_core::PrintJobFilter {
                    status: f.status.as_deref().map(parse_print_job_status).transpose()?,
                    limit: f.limit,
                    offset: f.offset,
                })
            },
        )?;
        let jobs = commerce
            .print_stations()
            .list_jobs(uuid.into(), filter)
            .map_err(|e| wrap(ErrCode::Internal, "Failed to list print jobs", e))?;
        Ok(jobs.into_iter().map(Into::into).collect())
    }
}
