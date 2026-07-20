//! PostgreSQL implementation of the print station / print job repository

use super::{block_on, map_db_error};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPool;
use sqlx::{FromRow, Postgres, QueryBuilder};
use stateset_core::{
    CommerceError, CreatePrintStation, EnqueuePrintJob, PairStationResult, PrintJob,
    PrintJobFilter, PrintJobId, PrintJobStatus, PrintPayloadKind, PrintStation, PrintStationId,
    PrintStationRepository, Result,
};
use uuid::Uuid;

/// PostgreSQL-backed [`PrintStationRepository`].
#[derive(Debug, Clone)]
pub struct PgPrintStationRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct StationRow {
    id: PrintStationId,
    name: String,
    printers: String,
    revoked: bool,
    last_seen_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(FromRow)]
struct JobRow {
    id: PrintJobId,
    station_id: PrintStationId,
    printer_name: Option<String>,
    payload_kind: String,
    payload: String,
    status: String,
    created_at: DateTime<Utc>,
    picked_up_at: Option<DateTime<Utc>>,
}

impl PgPrintStationRepository {
    /// Create a new repository over the given pool.
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// SHA-256 hex digest of a token.
    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn row_to_station(row: StationRow) -> Result<PrintStation> {
        let printers: Vec<String> = serde_json::from_str(&row.printers).map_err(|e| {
            CommerceError::DatabaseError(format!("Invalid print_station.printers: {e}"))
        })?;
        Ok(PrintStation {
            id: row.id,
            name: row.name,
            printers,
            revoked: row.revoked,
            last_seen_at: row.last_seen_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    fn row_to_job(row: JobRow) -> Result<PrintJob> {
        let payload_kind: PrintPayloadKind = row.payload_kind.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid print_job.payload_kind '{}': {}",
                row.payload_kind, e
            ))
        })?;
        let status: PrintJobStatus = row.status.parse().map_err(|e| {
            CommerceError::DatabaseError(format!(
                "Invalid print_job.status '{}': {}",
                row.status, e
            ))
        })?;
        Ok(PrintJob {
            id: row.id,
            station_id: row.station_id,
            printer_name: row.printer_name,
            payload_kind,
            payload: row.payload,
            status,
            created_at: row.created_at,
            picked_up_at: row.picked_up_at,
        })
    }

    async fn fetch_station(&self, id: PrintStationId) -> Result<Option<PrintStation>> {
        let row = sqlx::query_as::<_, StationRow>("SELECT * FROM print_stations WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_station).transpose()
    }

    async fn fetch_job(&self, id: PrintJobId) -> Result<PrintJob> {
        let row = sqlx::query_as::<_, JobRow>("SELECT * FROM print_jobs WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;
        row.map(Self::row_to_job).transpose()?.ok_or(CommerceError::NotFound)
    }

    /// Pair a new print station, returning the station and its one-time token.
    pub async fn pair_async(&self, input: CreatePrintStation) -> Result<PairStationResult> {
        let id = PrintStationId::new();
        let now = Utc::now();
        // Token = two random UUIDs (~244 bits). Only its hash is persisted.
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let token_hash = Self::hash_token(&token);
        let printers_json = serde_json::to_string(&input.printers)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;

        sqlx::query(
            "INSERT INTO print_stations (id, name, token_hash, printers, revoked, created_at, updated_at)
             VALUES ($1, $2, $3, $4, FALSE, $5, $5)",
        )
        .bind(id)
        .bind(&input.name)
        .bind(&token_hash)
        .bind(&printers_json)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        let station = self
            .fetch_station(id)
            .await?
            .ok_or_else(|| CommerceError::DatabaseError("Failed to create print station".into()))?;
        Ok(PairStationResult { station, token })
    }

    /// List paired stations (most recently paired first).
    pub async fn list_stations_async(&self) -> Result<Vec<PrintStation>> {
        let rows = sqlx::query_as::<_, StationRow>(
            "SELECT * FROM print_stations ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_station).collect::<Result<Vec<_>>>()
    }

    /// Get a station by ID.
    pub async fn get_station_async(&self, id: PrintStationId) -> Result<Option<PrintStation>> {
        self.fetch_station(id).await
    }

    /// Revoke a station's token.
    pub async fn revoke_station_async(&self, id: PrintStationId) -> Result<PrintStation> {
        sqlx::query("UPDATE print_stations SET revoked = TRUE, updated_at = $1 WHERE id = $2")
            .bind(Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        self.fetch_station(id).await?.ok_or(CommerceError::NotFound)
    }

    /// Enqueue a print job to a station. Errors if the station is revoked.
    pub async fn enqueue_job_async(
        &self,
        station_id: PrintStationId,
        input: EnqueuePrintJob,
    ) -> Result<PrintJob> {
        let revoked: Option<bool> =
            sqlx::query_scalar("SELECT revoked FROM print_stations WHERE id = $1")
                .bind(station_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_db_error)?;
        match revoked {
            None => return Err(CommerceError::NotFound),
            Some(true) => return Err(CommerceError::Conflict("station is revoked".into())),
            Some(false) => {}
        }

        let job_id = PrintJobId::new();
        sqlx::query(
            "INSERT INTO print_jobs (id, station_id, printer_name, payload_kind, payload, status, created_at)
             VALUES ($1, $2, $3, $4, $5, 'queued', $6)",
        )
        .bind(job_id)
        .bind(station_id)
        .bind(&input.printer_name)
        .bind(input.payload_kind.to_string())
        .bind(&input.payload)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(map_db_error)?;

        self.fetch_job(job_id).await
    }

    /// Pick up the next queued job for a station, marking it picked up and
    /// updating the station's last-seen time.
    pub async fn next_job_async(&self, station_id: PrintStationId) -> Result<Option<PrintJob>> {
        let now = Utc::now();
        let mut tx = self.pool.begin().await.map_err(map_db_error)?;

        // Touch last-seen on every poll.
        sqlx::query("UPDATE print_stations SET last_seen_at = $1, updated_at = $1 WHERE id = $2")
            .bind(now)
            .bind(station_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        let job_id: Option<PrintJobId> = sqlx::query_scalar(
            "SELECT id FROM print_jobs WHERE station_id = $1 AND status = 'queued' ORDER BY created_at ASC LIMIT 1 FOR UPDATE SKIP LOCKED",
        )
        .bind(station_id)
        .fetch_optional(tx.as_mut())
        .await
        .map_err(map_db_error)?;

        let Some(job_id) = job_id else {
            tx.commit().await.map_err(map_db_error)?;
            return Ok(None);
        };

        sqlx::query("UPDATE print_jobs SET status = 'picked_up', picked_up_at = $1 WHERE id = $2")
            .bind(now)
            .bind(job_id)
            .execute(tx.as_mut())
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        self.fetch_job(job_id).await.map(Some)
    }

    /// Mark a job printed (`success = true`) or failed.
    pub async fn complete_job_async(&self, job_id: PrintJobId, success: bool) -> Result<PrintJob> {
        let status = if success { PrintJobStatus::Printed } else { PrintJobStatus::Failed };
        sqlx::query("UPDATE print_jobs SET status = $1 WHERE id = $2")
            .bind(status.to_string())
            .bind(job_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
        self.fetch_job(job_id).await
    }

    /// List jobs for a station.
    pub async fn list_jobs_async(
        &self,
        station_id: PrintStationId,
        filter: PrintJobFilter,
    ) -> Result<Vec<PrintJob>> {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT * FROM print_jobs WHERE station_id = ");
        builder.push_bind(station_id);
        if let Some(status) = filter.status {
            builder.push(" AND status = ").push_bind(status.to_string());
        }
        builder.push(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            builder.push(" LIMIT ").push_bind(i64::from(limit));
        }
        if let Some(offset) = filter.offset {
            builder.push(" OFFSET ").push_bind(i64::from(offset));
        }
        let rows =
            builder.build_query_as::<JobRow>().fetch_all(&self.pool).await.map_err(map_db_error)?;
        rows.into_iter().map(Self::row_to_job).collect::<Result<Vec<_>>>()
    }
}

impl PrintStationRepository for PgPrintStationRepository {
    fn pair(&self, input: CreatePrintStation) -> Result<PairStationResult> {
        block_on(self.pair_async(input))
    }

    fn list_stations(&self) -> Result<Vec<PrintStation>> {
        block_on(self.list_stations_async())
    }

    fn get_station(&self, id: PrintStationId) -> Result<Option<PrintStation>> {
        block_on(self.get_station_async(id))
    }

    fn revoke_station(&self, id: PrintStationId) -> Result<PrintStation> {
        block_on(self.revoke_station_async(id))
    }

    fn enqueue_job(&self, station_id: PrintStationId, input: EnqueuePrintJob) -> Result<PrintJob> {
        block_on(self.enqueue_job_async(station_id, input))
    }

    fn next_job(&self, station_id: PrintStationId) -> Result<Option<PrintJob>> {
        block_on(self.next_job_async(station_id))
    }

    fn complete_job(&self, job_id: PrintJobId, success: bool) -> Result<PrintJob> {
        block_on(self.complete_job_async(job_id, success))
    }

    fn list_jobs(
        &self,
        station_id: PrintStationId,
        filter: PrintJobFilter,
    ) -> Result<Vec<PrintJob>> {
        block_on(self.list_jobs_async(station_id, filter))
    }
}
