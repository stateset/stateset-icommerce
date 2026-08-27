use crate::kernel_outbox::{
    KernelAuditCheckpoint, KernelAuditVerification, KernelOutboxEvent, KernelOutboxHealth,
    KernelReceiptRecord, audit_checkpoint_hash_is_valid, build_audit_checkpoint,
    receipt_audit_hash,
};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{OptionalExtension, params};
use stateset_core::{CommerceError, Result};
use uuid::Uuid;

use super::{
    map_db_error, parse_datetime_opt_row, parse_datetime_row, parse_uuid_opt_row, parse_uuid_row,
    with_immediate_transaction,
};

/// Consumer API for SQLite's durable kernel outbox.
#[derive(Debug)]
pub struct SqliteKernelOutboxRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteKernelOutboxRepository {
    pub(crate) const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    /// Return unpublished events in deterministic delivery order.
    pub fn pending(&self, limit: u32) -> Result<Vec<KernelOutboxEvent>> {
        let conn =
            self.pool.get().map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let mut statement = conn
            .prepare(
                "SELECT id, event_type, aggregate_type, aggregate_id, payload, command_id,
                        idempotency_key, principal_type, principal_id, correlation_id,
                        causation_id, created_at, published_at, attempts, last_error,
                        lease_owner, lease_expires_at, next_attempt_at, dead_lettered_at
                 FROM kernel_outbox WHERE published_at IS NULL AND dead_lettered_at IS NULL
                   AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
                   AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
                 ORDER BY created_at, id LIMIT ?",
            )
            .map_err(map_db_error)?;
        let now = chrono::Utc::now().to_rfc3339();
        let rows = statement
            .query_map(params![now, now, i64::from(limit)], |row| {
                let payload: String = row.get("payload")?;
                Ok(KernelOutboxEvent {
                    id: parse_uuid_row(&row.get::<_, String>("id")?, "kernel_outbox", "id")?,
                    event_type: row.get("event_type")?,
                    aggregate_type: row.get("aggregate_type")?,
                    aggregate_id: row.get("aggregate_id")?,
                    payload: serde_json::from_str(&payload).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
                    command_id: parse_uuid_opt_row(
                        row.get("command_id")?,
                        "kernel_outbox",
                        "command_id",
                    )?,
                    idempotency_key: row.get("idempotency_key")?,
                    principal_type: row.get("principal_type")?,
                    principal_id: row.get("principal_id")?,
                    correlation_id: parse_uuid_opt_row(
                        row.get("correlation_id")?,
                        "kernel_outbox",
                        "correlation_id",
                    )?,
                    causation_id: parse_uuid_opt_row(
                        row.get("causation_id")?,
                        "kernel_outbox",
                        "causation_id",
                    )?,
                    created_at: parse_datetime_row(
                        &row.get::<_, String>("created_at")?,
                        "kernel_outbox",
                        "created_at",
                    )?,
                    published_at: parse_datetime_opt_row(
                        row.get("published_at")?,
                        "kernel_outbox",
                        "published_at",
                    )?,
                    attempts: row.get("attempts")?,
                    last_error: row.get("last_error")?,
                    lease_owner: row.get("lease_owner")?,
                    lease_expires_at: parse_datetime_opt_row(
                        row.get("lease_expires_at")?,
                        "kernel_outbox",
                        "lease_expires_at",
                    )?,
                    next_attempt_at: parse_datetime_opt_row(
                        row.get("next_attempt_at")?,
                        "kernel_outbox",
                        "next_attempt_at",
                    )?,
                    dead_lettered_at: parse_datetime_opt_row(
                        row.get("dead_lettered_at")?,
                        "kernel_outbox",
                        "dead_lettered_at",
                    )?,
                })
            })
            .map_err(map_db_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Atomically lease deliverable events to one worker.
    pub fn claim_pending(
        &self,
        worker_id: &str,
        limit: u32,
        lease_seconds: u32,
    ) -> Result<Vec<KernelOutboxEvent>> {
        if worker_id.trim().is_empty() || lease_seconds == 0 {
            return Err(CommerceError::ValidationError(
                "worker_id and a positive lease duration are required".into(),
            ));
        }
        let now = chrono::Utc::now();
        let lease_expires = now + chrono::Duration::seconds(i64::from(lease_seconds));
        with_immediate_transaction(&self.pool, |tx| {
            tx.execute(
                "UPDATE kernel_outbox SET lease_owner = ?, lease_expires_at = ?
                 WHERE id IN (
                    SELECT id FROM kernel_outbox
                    WHERE published_at IS NULL AND dead_lettered_at IS NULL
                      AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
                      AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
                    ORDER BY created_at, id LIMIT ?
                 )",
                params![
                    worker_id,
                    lease_expires.to_rfc3339(),
                    now.to_rfc3339(),
                    now.to_rfc3339(),
                    i64::from(limit),
                ],
            )?;
            let mut statement = tx.prepare(
                "SELECT id, event_type, aggregate_type, aggregate_id, payload, command_id,
                        idempotency_key, principal_type, principal_id, correlation_id,
                        causation_id, created_at, published_at, attempts, last_error,
                        lease_owner, lease_expires_at, next_attempt_at, dead_lettered_at
                 FROM kernel_outbox WHERE lease_owner = ? AND lease_expires_at = ?
                 ORDER BY created_at, id",
            )?;
            let rows = statement
                .query_map(params![worker_id, lease_expires.to_rfc3339()], event_from_row)?;
            rows.collect()
        })
    }

    /// Acknowledge a leased event only when owned by `worker_id`.
    pub fn mark_published_by(&self, id: Uuid, worker_id: &str) -> Result<bool> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let changed = conn
            .execute(
                "UPDATE kernel_outbox SET published_at = ?, last_error = NULL,
                    lease_owner = NULL, lease_expires_at = NULL
             WHERE id = ? AND lease_owner = ? AND published_at IS NULL",
                params![chrono::Utc::now().to_rfc3339(), id.to_string(), worker_id],
            )
            .map_err(map_db_error)?;
        Ok(changed == 1)
    }

    /// Release a failed lease, schedule retry, or dead-letter after `max_attempts`.
    pub fn record_failure_by(
        &self,
        id: Uuid,
        worker_id: &str,
        error: &str,
        retry_after_seconds: u32,
        max_attempts: u32,
    ) -> Result<bool> {
        if max_attempts == 0 {
            return Err(CommerceError::ValidationError("max_attempts must be positive".into()));
        }
        let now = chrono::Utc::now();
        let next = now + chrono::Duration::seconds(i64::from(retry_after_seconds));
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let changed = conn
            .execute(
                "UPDATE kernel_outbox SET attempts = attempts + 1, last_error = ?,
                    lease_owner = NULL, lease_expires_at = NULL,
                    next_attempt_at = CASE WHEN attempts + 1 >= ? THEN NULL ELSE ? END,
                    dead_lettered_at = CASE WHEN attempts + 1 >= ? THEN ? ELSE NULL END
             WHERE id = ? AND lease_owner = ? AND published_at IS NULL",
                params![
                    error,
                    max_attempts,
                    next.to_rfc3339(),
                    max_attempts,
                    now.to_rfc3339(),
                    id.to_string(),
                    worker_id
                ],
            )
            .map_err(map_db_error)?;
        Ok(changed == 1)
    }

    /// Inspect dead-lettered events without making them deliverable.
    pub fn dead_letters(&self, limit: u32) -> Result<Vec<KernelOutboxEvent>> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let mut statement = conn
            .prepare(
                "SELECT id, event_type, aggregate_type, aggregate_id, payload, command_id,
                    idempotency_key, principal_type, principal_id, correlation_id,
                    causation_id, created_at, published_at, attempts, last_error,
                    lease_owner, lease_expires_at, next_attempt_at, dead_lettered_at
             FROM kernel_outbox WHERE dead_lettered_at IS NOT NULL
             ORDER BY dead_lettered_at, id LIMIT ?",
            )
            .map_err(map_db_error)?;
        let rows = statement.query_map([i64::from(limit)], event_from_row).map_err(map_db_error)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_db_error)
    }

    /// Explicitly return a dead letter to the ready queue.
    pub fn redrive_dead_letter(&self, id: Uuid, reset_attempts: bool) -> Result<bool> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let changed = conn
            .execute(
                "UPDATE kernel_outbox SET dead_lettered_at = NULL, next_attempt_at = ?,
                    attempts = CASE WHEN ? THEN 0 ELSE attempts END, last_error = NULL
             WHERE id = ? AND dead_lettered_at IS NOT NULL",
                params![chrono::Utc::now().to_rfc3339(), reset_attempts, id.to_string()],
            )
            .map_err(map_db_error)?;
        Ok(changed == 1)
    }

    /// Count delivery states for health checks and metrics exporters.
    pub fn delivery_health(&self) -> Result<KernelOutboxHealth> {
        let conn = self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        let now = chrono::Utc::now().to_rfc3339();
        let counts: (i64, i64, i64, i64, i64) = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN published_at IS NULL AND dead_lettered_at IS NULL
                     AND (next_attempt_at IS NULL OR next_attempt_at <= ?)
                     AND (lease_expires_at IS NULL OR lease_expires_at <= ?) THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN published_at IS NULL AND lease_expires_at > ? THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN published_at IS NULL AND dead_lettered_at IS NULL AND next_attempt_at > ? THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN dead_lettered_at IS NOT NULL THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN published_at IS NOT NULL THEN 1 ELSE 0 END), 0)
             FROM kernel_outbox",
            params![now, now, now, now],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        ).map_err(map_db_error)?;
        Ok(KernelOutboxHealth {
            ready: u64::try_from(counts.0).unwrap_or(0),
            leased: u64::try_from(counts.1).unwrap_or(0),
            delayed: u64::try_from(counts.2).unwrap_or(0),
            dead_lettered: u64::try_from(counts.3).unwrap_or(0),
            published: u64::try_from(counts.4).unwrap_or(0),
        })
    }

    /// Acknowledge successful delivery.
    pub fn mark_published(&self, id: Uuid) -> Result<()> {
        let conn =
            self.pool.get().map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        conn.execute(
            "UPDATE kernel_outbox SET published_at = ?, last_error = NULL WHERE id = ?",
            params![chrono::Utc::now().to_rfc3339(), id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Record a failed delivery attempt without losing the event.
    pub fn record_failure(&self, id: Uuid, error: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|pool_error| CommerceError::DatabaseError(pool_error.to_string()))?;
        conn.execute(
            "UPDATE kernel_outbox SET attempts = attempts + 1, last_error = ? WHERE id = ?",
            params![error, id.to_string()],
        )
        .map_err(map_db_error)?;
        Ok(())
    }

    /// Load the final receipt associated with a retry key.
    pub fn receipt_by_idempotency_key(&self, key: &str) -> Result<Option<KernelReceiptRecord>> {
        let conn =
            self.pool.get().map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let result = conn.query_row(
            "SELECT command_id, idempotency_key, command_type, contract_version,
                    request_hash, status, receipt, created_at, completed_at
             FROM kernel_receipts WHERE idempotency_key = ?",
            [key],
            receipt_from_row,
        );
        match result {
            Ok(receipt) => Ok(Some(receipt)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(map_db_error(error)),
        }
    }

    /// Recompute every receipt link and report the first broken chain position.
    pub fn verify_audit_chain(&self) -> Result<KernelAuditVerification> {
        let conn =
            self.pool.get().map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let mut statement = conn
            .prepare(
                "SELECT sequence, request_hash, previous_audit_hash, audit_hash, receipt
                 FROM kernel_receipt_audit_log ORDER BY sequence",
            )
            .map_err(map_db_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(map_db_error)?;
        let mut previous: Option<String> = None;
        let mut entries = 0_u64;
        for row in rows {
            let (sequence, request_hash, declared_previous, declared_hash, receipt_json) =
                row.map_err(map_db_error)?;
            entries += 1;
            let receipt = match serde_json::from_str(&receipt_json) {
                Ok(value) => value,
                Err(_) => {
                    return Ok(KernelAuditVerification {
                        valid: false,
                        entries,
                        head_hash: previous,
                        first_invalid_sequence: Some(sequence),
                    });
                }
            };
            let computed = receipt_audit_hash(previous.as_deref(), &request_hash, &receipt)
                .map_err(CommerceError::DatabaseError)?;
            if declared_previous != previous || declared_hash != computed {
                return Ok(KernelAuditVerification {
                    valid: false,
                    entries,
                    head_hash: previous,
                    first_invalid_sequence: Some(sequence),
                });
            }
            previous = Some(declared_hash);
        }
        let mut current = conn
            .prepare(
                "SELECT r.receipt, r.request_hash, a.sequence, a.receipt, a.request_hash
                 FROM kernel_receipts r
                 LEFT JOIN kernel_receipt_audit_log a
                   ON a.audit_hash = json_extract(r.receipt, '$.audit_hash')",
            )
            .map_err(map_db_error)?;
        let current_rows = current
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(map_db_error)?;
        for row in current_rows {
            let (sealed_json, request_hash, sequence, audit_json, audit_request_hash) =
                row.map_err(map_db_error)?;
            let mut sealed: serde_json::Value = serde_json::from_str(&sealed_json)
                .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
            if let Some(object) = sealed.as_object_mut() {
                object.insert("audit_hash".into(), serde_json::Value::Null);
            }
            let audit_receipt = audit_json
                .as_deref()
                .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
            if sequence.is_none()
                || audit_request_hash.as_deref() != Some(request_hash.as_str())
                || audit_receipt.as_ref() != Some(&sealed)
            {
                return Ok(KernelAuditVerification {
                    valid: false,
                    entries,
                    head_hash: previous,
                    first_invalid_sequence: Some(sequence.unwrap_or(0)),
                });
            }
        }
        Ok(KernelAuditVerification {
            valid: true,
            entries,
            head_hash: previous,
            first_invalid_sequence: None,
        })
    }

    /// Create a portable checkpoint for publication outside this database.
    pub fn audit_checkpoint(&self) -> Result<KernelAuditCheckpoint> {
        build_audit_checkpoint(&self.verify_audit_chain()?).map_err(CommerceError::ValidationError)
    }

    /// Verify an externally retained checkpoint against the complete local
    /// chain. Checkpoints remain valid after newer receipts are appended.
    pub fn verify_audit_checkpoint(&self, checkpoint: &KernelAuditCheckpoint) -> Result<bool> {
        if checkpoint.contract_version != "1.0"
            || checkpoint.algorithm != "sha256-jcs-v1"
            || !audit_checkpoint_hash_is_valid(checkpoint)
                .map_err(CommerceError::ValidationError)?
        {
            return Ok(false);
        }
        let verification = self.verify_audit_chain()?;
        if !verification.valid || checkpoint.entries > verification.entries {
            return Ok(false);
        }
        if checkpoint.entries == 0 {
            return Ok(checkpoint.head_hash.is_none());
        }
        let conn =
            self.pool.get().map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let expected_hash = checkpoint.head_hash.as_deref().ok_or_else(|| {
            CommerceError::ValidationError("non-empty checkpoint is missing head_hash".into())
        })?;
        let Ok(expected_sequence) = i64::try_from(checkpoint.entries) else {
            return Ok(false);
        };
        let local_hash = conn
            .query_row(
                "SELECT audit_hash FROM kernel_receipt_audit_log
                 WHERE sequence = ? AND audit_hash = ?",
                params![expected_sequence, expected_hash],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(map_db_error)?;
        Ok(local_hash == checkpoint.head_hash)
    }
}

fn event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KernelOutboxEvent> {
    let payload: String = row.get("payload")?;
    Ok(KernelOutboxEvent {
        id: parse_uuid_row(&row.get::<_, String>("id")?, "kernel_outbox", "id")?,
        event_type: row.get("event_type")?,
        aggregate_type: row.get("aggregate_type")?,
        aggregate_id: row.get("aggregate_id")?,
        payload: serde_json::from_str(&payload).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        command_id: parse_uuid_opt_row(row.get("command_id")?, "kernel_outbox", "command_id")?,
        idempotency_key: row.get("idempotency_key")?,
        principal_type: row.get("principal_type")?,
        principal_id: row.get("principal_id")?,
        correlation_id: parse_uuid_opt_row(
            row.get("correlation_id")?,
            "kernel_outbox",
            "correlation_id",
        )?,
        causation_id: parse_uuid_opt_row(
            row.get("causation_id")?,
            "kernel_outbox",
            "causation_id",
        )?,
        created_at: parse_datetime_row(
            &row.get::<_, String>("created_at")?,
            "kernel_outbox",
            "created_at",
        )?,
        published_at: parse_datetime_opt_row(
            row.get("published_at")?,
            "kernel_outbox",
            "published_at",
        )?,
        attempts: row.get("attempts")?,
        last_error: row.get("last_error")?,
        lease_owner: row.get("lease_owner")?,
        lease_expires_at: parse_datetime_opt_row(
            row.get("lease_expires_at")?,
            "kernel_outbox",
            "lease_expires_at",
        )?,
        next_attempt_at: parse_datetime_opt_row(
            row.get("next_attempt_at")?,
            "kernel_outbox",
            "next_attempt_at",
        )?,
        dead_lettered_at: parse_datetime_opt_row(
            row.get("dead_lettered_at")?,
            "kernel_outbox",
            "dead_lettered_at",
        )?,
    })
}

fn receipt_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KernelReceiptRecord> {
    let receipt: String = row.get("receipt")?;
    Ok(KernelReceiptRecord {
        command_id: parse_uuid_row(
            &row.get::<_, String>("command_id")?,
            "kernel_receipts",
            "command_id",
        )?,
        idempotency_key: row.get("idempotency_key")?,
        command_type: row.get("command_type")?,
        contract_version: row.get("contract_version")?,
        request_hash: row.get("request_hash")?,
        status: row.get("status")?,
        receipt: serde_json::from_str(&receipt).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        created_at: parse_datetime_row(
            &row.get::<_, String>("created_at")?,
            "kernel_receipts",
            "created_at",
        )?,
        completed_at: parse_datetime_row(
            &row.get::<_, String>("completed_at")?,
            "kernel_receipts",
            "completed_at",
        )?,
    })
}

pub(crate) fn receipt_by_idempotency_key_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
) -> rusqlite::Result<Option<KernelReceiptRecord>> {
    let result = tx.query_row(
        "SELECT command_id, idempotency_key, command_type, contract_version,
                request_hash, status, receipt, created_at, completed_at
         FROM kernel_receipts WHERE idempotency_key = ?",
        [key],
        receipt_from_row,
    );
    match result {
        Ok(receipt) => Ok(Some(receipt)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn append_kernel_event_tx(
    tx: &rusqlite::Transaction<'_>,
    event: &KernelOutboxEvent,
) -> rusqlite::Result<()> {
    let payload = serde_json::to_string(&event.payload)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    tx.execute(
        "INSERT INTO kernel_outbox (
            id, event_type, aggregate_type, aggregate_id, payload, command_id,
            idempotency_key, principal_type, principal_id, correlation_id,
            causation_id, created_at, published_at, attempts, last_error
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            event.id.to_string(),
            event.event_type,
            event.aggregate_type,
            event.aggregate_id,
            payload,
            event.command_id.map(|id| id.to_string()),
            event.idempotency_key,
            event.principal_type,
            event.principal_id,
            event.correlation_id.map(|id| id.to_string()),
            event.causation_id.map(|id| id.to_string()),
            event.created_at.to_rfc3339(),
            event.published_at.map(|time| time.to_rfc3339()),
            event.attempts,
            event.last_error,
        ],
    )?;
    Ok(())
}

pub(crate) fn append_kernel_receipt_tx(
    tx: &rusqlite::Transaction<'_>,
    record: &KernelReceiptRecord,
) -> rusqlite::Result<String> {
    let previous_audit_hash = tx
        .query_row(
            "SELECT audit_hash FROM kernel_receipt_audit_log ORDER BY sequence DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let audit_hash =
        receipt_audit_hash(previous_audit_hash.as_deref(), &record.request_hash, &record.receipt)
            .map_err(|error| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(error)))
        })?;
    let receipt_id =
        record.receipt.get("receipt_id").and_then(serde_json::Value::as_str).ok_or_else(|| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::other(
                "receipt_id missing from receipt",
            )))
        })?;
    let audit_receipt = serde_json::to_string(&record.receipt)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    tx.execute(
        "INSERT INTO kernel_receipt_audit_log (
            receipt_id, command_id, idempotency_key, request_hash,
            previous_audit_hash, audit_hash, receipt, created_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            receipt_id,
            record.command_id.to_string(),
            record.idempotency_key,
            record.request_hash,
            previous_audit_hash,
            audit_hash,
            audit_receipt,
            record.completed_at.to_rfc3339(),
        ],
    )?;
    let mut sealed_receipt = record.receipt.clone();
    if let Some(object) = sealed_receipt.as_object_mut() {
        object.insert("audit_hash".into(), serde_json::Value::String(audit_hash.clone()));
    }
    let receipt = serde_json::to_string(&sealed_receipt)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    tx.execute(
        "INSERT INTO kernel_receipts (
            command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(idempotency_key) DO UPDATE SET
            command_id = excluded.command_id,
            command_type = excluded.command_type,
            contract_version = excluded.contract_version,
            request_hash = excluded.request_hash,
            status = excluded.status,
            receipt = excluded.receipt,
            completed_at = excluded.completed_at",
        params![
            record.command_id.to_string(),
            record.idempotency_key,
            record.command_type,
            record.contract_version,
            record.request_hash,
            record.status,
            receipt,
            record.created_at.to_rfc3339(),
            record.completed_at.to_rfc3339(),
        ],
    )?;
    Ok(audit_hash)
}
