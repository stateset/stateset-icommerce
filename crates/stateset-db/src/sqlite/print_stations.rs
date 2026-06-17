//! SQLite implementation of the print station / print job repository

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_enum_row, parse_json_row,
    parse_uuid_row, with_immediate_transaction,
};
use chrono::Utc;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use stateset_core::{
    CommerceError, CreatePrintStation, EnqueuePrintJob, PairStationResult, PrintJob,
    PrintJobFilter, PrintJobId, PrintJobStatus, PrintPayloadKind, PrintStation, PrintStationId,
    PrintStationRepository, Result,
};
use uuid::Uuid;

#[derive(Debug)]
pub struct SqlitePrintStationRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqlitePrintStationRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }

    /// SHA-256 hex digest of a token.
    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn row_to_station(row: &rusqlite::Row<'_>) -> rusqlite::Result<PrintStation> {
        let printers_json: String = row.get("printers")?;
        Ok(PrintStation {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "print_station", "id")?.into(),
            name: row.get("name")?,
            printers: parse_json_row(&printers_json, "print_station", "printers")?,
            revoked: row.get::<_, i32>("revoked")? != 0,
            last_seen_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("last_seen_at")?,
                "print_station",
                "last_seen_at",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "print_station",
                "created_at",
            )?,
            updated_at: parse_datetime_row(
                &row.get::<_, String>("updated_at")?,
                "print_station",
                "updated_at",
            )?,
        })
    }

    fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<PrintJob> {
        Ok(PrintJob {
            id: parse_uuid_row(&row.get::<_, String>("id")?, "print_job", "id")?.into(),
            station_id: parse_uuid_row(
                &row.get::<_, String>("station_id")?,
                "print_job",
                "station_id",
            )?
            .into(),
            printer_name: row.get("printer_name")?,
            payload_kind: parse_enum_row::<PrintPayloadKind>(
                &row.get::<_, String>("payload_kind")?,
                "print_job",
                "payload_kind",
            )?,
            payload: row.get("payload")?,
            status: parse_enum_row::<PrintJobStatus>(
                &row.get::<_, String>("status")?,
                "print_job",
                "status",
            )?,
            created_at: parse_datetime_row(
                &row.get::<_, String>("created_at")?,
                "print_job",
                "created_at",
            )?,
            picked_up_at: parse_datetime_opt_row(
                row.get::<_, Option<String>>("picked_up_at")?,
                "print_job",
                "picked_up_at",
            )?,
        })
    }

    fn conflict(msg: &str) -> rusqlite::Error {
        rusqlite::Error::ToSqlConversionFailure(Box::new(CommerceError::Conflict(msg.to_string())))
    }
}

impl PrintStationRepository for SqlitePrintStationRepository {
    fn pair(&self, input: CreatePrintStation) -> Result<PairStationResult> {
        let id = PrintStationId::new();
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        // Token = two random UUIDs (~244 bits). Only its hash is persisted.
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let token_hash = Self::hash_token(&token);
        let printers_json = serde_json::to_string(&input.printers)
            .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let station = with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "INSERT INTO print_stations (id, name, token_hash, printers, revoked, created_at, updated_at)
                 VALUES (?, ?, ?, ?, 0, ?, ?)",
                rusqlite::params![&id_str, &input.name, &token_hash, &printers_json, &now, &now],
            )?;
            tx.query_row(
                "SELECT * FROM print_stations WHERE id = ?",
                [&id_str],
                Self::row_to_station,
            )
        })?;
        Ok(PairStationResult { station, token })
    }

    fn list_stations(&self) -> Result<Vec<PrintStation>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT * FROM print_stations ORDER BY created_at DESC")
            .map_err(map_db_error)?;
        let rows = stmt
            .query_map([], Self::row_to_station)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }

    fn get_station(&self, id: PrintStationId) -> Result<Option<PrintStation>> {
        let conn = self.conn()?;
        match conn.query_row(
            "SELECT * FROM print_stations WHERE id = ?",
            [id.to_string()],
            Self::row_to_station,
        ) {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_db_error(e)),
        }
    }

    fn revoke_station(&self, id: PrintStationId) -> Result<PrintStation> {
        let id_str = id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE print_stations SET revoked = 1, updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &id_str],
            )?;
            tx.query_row(
                "SELECT * FROM print_stations WHERE id = ?",
                [&id_str],
                Self::row_to_station,
            )
        })
    }

    fn enqueue_job(&self, station_id: PrintStationId, input: EnqueuePrintJob) -> Result<PrintJob> {
        let station_str = station_id.to_string();
        let job_id = PrintJobId::new();
        let job_str = job_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            let revoked: Option<i32> = tx
                .query_row("SELECT revoked FROM print_stations WHERE id = ?", [&station_str], |r| {
                    r.get(0)
                })
                .optional()?;
            match revoked {
                None => return Err(rusqlite::Error::QueryReturnedNoRows),
                Some(r) if r != 0 => return Err(Self::conflict("station is revoked")),
                _ => {}
            }
            tx.execute(
                "INSERT INTO print_jobs (id, station_id, printer_name, payload_kind, payload, status, created_at)
                 VALUES (?, ?, ?, ?, ?, 'queued', ?)",
                rusqlite::params![
                    &job_str,
                    &station_str,
                    &input.printer_name,
                    input.payload_kind.to_string(),
                    &input.payload,
                    &now,
                ],
            )?;
            tx.query_row("SELECT * FROM print_jobs WHERE id = ?", [&job_str], Self::row_to_job)
        })
    }

    fn next_job(&self, station_id: PrintStationId) -> Result<Option<PrintJob>> {
        let station_str = station_id.to_string();
        let now = Utc::now().to_rfc3339();
        with_immediate_transaction(&self.pool, |tx| {
            // Touch last-seen on every poll.
            tx.execute(
                "UPDATE print_stations SET last_seen_at = ?, updated_at = ? WHERE id = ?",
                rusqlite::params![&now, &now, &station_str],
            )?;
            let job_id: Option<String> = tx
                .query_row(
                    "SELECT id FROM print_jobs WHERE station_id = ? AND status = 'queued' ORDER BY created_at ASC LIMIT 1",
                    [&station_str],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(job_id) = job_id else {
                return Ok(None);
            };
            tx.execute(
                "UPDATE print_jobs SET status = 'picked_up', picked_up_at = ? WHERE id = ?",
                rusqlite::params![&now, &job_id],
            )?;
            let job =
                tx.query_row("SELECT * FROM print_jobs WHERE id = ?", [&job_id], Self::row_to_job)?;
            Ok(Some(job))
        })
    }

    fn complete_job(&self, job_id: PrintJobId, success: bool) -> Result<PrintJob> {
        let job_str = job_id.to_string();
        let status = if success { PrintJobStatus::Printed } else { PrintJobStatus::Failed };
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE print_jobs SET status = ? WHERE id = ?",
                rusqlite::params![status.to_string(), &job_str],
            )?;
            tx.query_row("SELECT * FROM print_jobs WHERE id = ?", [&job_str], Self::row_to_job)
        })
    }

    fn list_jobs(
        &self,
        station_id: PrintStationId,
        filter: PrintJobFilter,
    ) -> Result<Vec<PrintJob>> {
        let conn = self.conn()?;
        let mut sql = "SELECT * FROM print_jobs WHERE station_id = ?".to_string();
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> =
            vec![Box::new(station_id.to_string())];
        if let Some(status) = filter.status {
            sql.push_str(" AND status = ?");
            params.push(Box::new(status.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = filter.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&sql).map_err(map_db_error)?;
        let rows = stmt
            .query_map(param_refs.as_slice(), Self::row_to_job)
            .map_err(map_db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_db_error)?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DatabaseConfig;
    use crate::sqlite::SqliteDatabase;

    fn test_repo() -> SqlitePrintStationRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        SqlitePrintStationRepository::new(db.pool().clone())
    }

    fn pair(repo: &SqlitePrintStationRepository) -> PairStationResult {
        repo.pair(CreatePrintStation {
            name: "Packing Bench 1".into(),
            printers: vec!["Zebra-1".into()],
        })
        .expect("pair")
    }

    fn job(printer: Option<&str>) -> EnqueuePrintJob {
        EnqueuePrintJob {
            printer_name: printer.map(String::from),
            payload_kind: PrintPayloadKind::Zpl,
            payload: "^XA^FO50,50^FDhi^FS^XZ".into(),
        }
    }

    #[test]
    fn pair_returns_token_and_persists_hash() {
        let repo = test_repo();
        let result = pair(&repo);
        assert!(!result.token.is_empty());
        assert!(!result.station.revoked);
        // token hash is not surfaced on the station struct
        let fetched = repo.get_station(result.station.id).expect("get").expect("found");
        assert_eq!(fetched.printers, vec!["Zebra-1".to_string()]);
    }

    #[test]
    fn enqueue_and_pick_up_in_fifo_order() {
        let repo = test_repo();
        let station = pair(&repo).station;
        repo.enqueue_job(station.id, job(Some("Zebra-1"))).expect("enqueue 1");
        repo.enqueue_job(station.id, job(Some("Zebra-1"))).expect("enqueue 2");

        let first = repo.next_job(station.id).expect("next").expect("job");
        assert_eq!(first.status, PrintJobStatus::PickedUp);
        assert!(first.picked_up_at.is_some());
        // station last-seen updated
        let s = repo.get_station(station.id).expect("get").expect("found");
        assert!(s.last_seen_at.is_some());

        // completing the first leaves one queued
        let done = repo.complete_job(first.id, true).expect("complete");
        assert_eq!(done.status, PrintJobStatus::Printed);
        let second = repo.next_job(station.id).expect("next").expect("job");
        assert_ne!(second.id, first.id);
        // queue now empty
        assert!(repo.next_job(station.id).expect("next").is_none());
    }

    #[test]
    fn revoked_station_rejects_jobs() {
        let repo = test_repo();
        let station = pair(&repo).station;
        repo.revoke_station(station.id).expect("revoke");
        assert!(repo.enqueue_job(station.id, job(None)).is_err());
    }

    #[test]
    fn list_jobs_filters_by_status() {
        let repo = test_repo();
        let station = pair(&repo).station;
        let j = repo.enqueue_job(station.id, job(None)).expect("enqueue");
        repo.enqueue_job(station.id, job(None)).expect("enqueue2");
        repo.complete_job(j.id, false).expect("fail");
        let failed = repo
            .list_jobs(
                station.id,
                PrintJobFilter { status: Some(PrintJobStatus::Failed), ..Default::default() },
            )
            .expect("list");
        assert_eq!(failed.len(), 1);
    }
}
