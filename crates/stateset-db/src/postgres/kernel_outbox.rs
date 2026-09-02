use crate::kernel::SealedAuditEntry;
use crate::kernel_outbox::{
    KernelAuditCheckpoint, KernelAuditVerification, KernelOutboxEvent, KernelOutboxHealth,
    KernelReceiptRecord, audit_checkpoint_hash_is_valid, build_audit_checkpoint,
    receipt_audit_hash,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use stateset_core::{CommerceError, Result};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct KernelOutboxRow {
    id: Uuid,
    event_type: String,
    aggregate_type: String,
    aggregate_id: String,
    payload: Value,
    command_id: Option<Uuid>,
    idempotency_key: Option<String>,
    principal_type: Option<String>,
    principal_id: Option<String>,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
    created_at: DateTime<Utc>,
    published_at: Option<DateTime<Utc>>,
    attempts: i32,
    last_error: Option<String>,
    lease_owner: Option<String>,
    lease_expires_at: Option<DateTime<Utc>>,
    next_attempt_at: Option<DateTime<Utc>>,
    dead_lettered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, FromRow)]
struct KernelReceiptRow {
    command_id: Uuid,
    idempotency_key: String,
    command_type: String,
    contract_version: String,
    request_hash: String,
    status: String,
    receipt: Value,
    created_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct KernelAuditRow {
    sequence: i64,
    request_hash: String,
    previous_audit_hash: Option<String>,
    audit_hash: String,
    receipt: Value,
}

#[derive(Debug, FromRow)]
struct MaterializedReceiptAuditRow {
    receipt: Value,
    request_hash: String,
    sequence: Option<i64>,
    audit_receipt: Option<Value>,
    audit_request_hash: Option<String>,
}

impl From<KernelReceiptRow> for KernelReceiptRecord {
    fn from(row: KernelReceiptRow) -> Self {
        Self {
            command_id: row.command_id,
            idempotency_key: row.idempotency_key,
            command_type: row.command_type,
            contract_version: row.contract_version,
            request_hash: row.request_hash,
            status: row.status,
            receipt: row.receipt,
            created_at: row.created_at,
            completed_at: row.completed_at,
        }
    }
}

pub(crate) async fn receipt_by_idempotency_key_tx(
    tx: &mut sqlx::PgConnection,
    key: &str,
) -> Result<Option<KernelReceiptRecord>> {
    sqlx::query_as::<_, KernelReceiptRow>(
        "SELECT command_id, idempotency_key, command_type, contract_version,
                request_hash, status, receipt, created_at, completed_at
         FROM kernel_receipts WHERE idempotency_key = $1 FOR UPDATE",
    )
    .bind(key)
    .fetch_optional(tx)
    .await
    .map(|row| row.map(Into::into))
    .map_err(|error| CommerceError::DatabaseError(error.to_string()))
}

/// Async consumer API for PostgreSQL's durable kernel outbox.
#[derive(Debug, Clone)]
pub struct PgKernelOutboxRepository {
    pool: PgPool,
}

impl PgKernelOutboxRepository {
    pub(crate) const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Return unpublished events in deterministic delivery order.
    pub async fn pending_async(&self, limit: u32) -> Result<Vec<KernelOutboxEvent>> {
        let rows = sqlx::query_as::<_, KernelOutboxRow>(
            "SELECT id, event_type, aggregate_type, aggregate_id, payload, command_id,
                    idempotency_key, principal_type, principal_id, correlation_id,
                    causation_id, created_at, published_at, attempts, last_error,
                    lease_owner, lease_expires_at, next_attempt_at, dead_lettered_at
             FROM kernel_outbox WHERE published_at IS NULL AND dead_lettered_at IS NULL
               AND (next_attempt_at IS NULL OR next_attempt_at <= NOW())
               AND (lease_expires_at IS NULL OR lease_expires_at <= NOW())
             ORDER BY created_at, id LIMIT $1",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        rows.into_iter()
            .map(|row| {
                Ok(KernelOutboxEvent {
                    id: row.id,
                    event_type: row.event_type,
                    aggregate_type: row.aggregate_type,
                    aggregate_id: row.aggregate_id,
                    payload: row.payload,
                    command_id: row.command_id,
                    idempotency_key: row.idempotency_key,
                    principal_type: row.principal_type,
                    principal_id: row.principal_id,
                    correlation_id: row.correlation_id,
                    causation_id: row.causation_id,
                    created_at: row.created_at,
                    published_at: row.published_at,
                    attempts: u32::try_from(row.attempts).map_err(|error| {
                        CommerceError::DatabaseError(format!(
                            "invalid kernel outbox attempt count: {error}"
                        ))
                    })?,
                    last_error: row.last_error,
                    lease_owner: row.lease_owner,
                    lease_expires_at: row.lease_expires_at,
                    next_attempt_at: row.next_attempt_at,
                    dead_lettered_at: row.dead_lettered_at,
                })
            })
            .collect()
    }

    /// Atomically lease deliverable events using `SKIP LOCKED` for competing workers.
    pub async fn claim_pending_async(
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
        let rows = sqlx::query_as::<_, KernelOutboxRow>(
            "WITH claimable AS (
                SELECT id FROM kernel_outbox
                WHERE published_at IS NULL AND dead_lettered_at IS NULL
                  AND (next_attempt_at IS NULL OR next_attempt_at <= NOW())
                  AND (lease_expires_at IS NULL OR lease_expires_at <= NOW())
                ORDER BY created_at, id
                FOR UPDATE SKIP LOCKED LIMIT $1
             )
             UPDATE kernel_outbox o
             SET lease_owner = $2,
                 lease_expires_at = NOW() + make_interval(secs => $3)
             FROM claimable c WHERE o.id = c.id
             RETURNING o.id, o.event_type, o.aggregate_type, o.aggregate_id, o.payload,
                 o.command_id, o.idempotency_key, o.principal_type, o.principal_id,
                 o.correlation_id, o.causation_id, o.created_at, o.published_at,
                 o.attempts, o.last_error, o.lease_owner, o.lease_expires_at,
                 o.next_attempt_at, o.dead_lettered_at",
        )
        .bind(i64::from(limit))
        .bind(worker_id)
        .bind(
            i32::try_from(lease_seconds)
                .map_err(|e| CommerceError::ValidationError(e.to_string()))?,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        rows.into_iter().map(row_to_event).collect()
    }

    /// Acknowledge a leased event only when owned by `worker_id`.
    pub async fn mark_published_by_async(&self, id: Uuid, worker_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE kernel_outbox SET published_at = NOW(), last_error = NULL,
                    lease_owner = NULL, lease_expires_at = NULL
             WHERE id = $1 AND lease_owner = $2 AND published_at IS NULL",
        )
        .bind(id)
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(result.rows_affected() == 1)
    }

    /// Release a failed lease, schedule retry, or dead-letter after `max_attempts`.
    pub async fn record_failure_by_async(
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
        let retry = i32::try_from(retry_after_seconds)
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let max = i32::try_from(max_attempts)
            .map_err(|e| CommerceError::ValidationError(e.to_string()))?;
        let result = sqlx::query(
            "UPDATE kernel_outbox SET attempts = attempts + 1, last_error = $1,
                    lease_owner = NULL, lease_expires_at = NULL,
                    next_attempt_at = CASE WHEN attempts + 1 >= $2 THEN NULL
                                           ELSE NOW() + make_interval(secs => $3) END,
                    dead_lettered_at = CASE WHEN attempts + 1 >= $2 THEN NOW() ELSE NULL END
             WHERE id = $4 AND lease_owner = $5 AND published_at IS NULL",
        )
        .bind(error)
        .bind(max)
        .bind(retry)
        .bind(id)
        .bind(worker_id)
        .execute(&self.pool)
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(result.rows_affected() == 1)
    }

    /// Inspect dead-lettered events without making them deliverable.
    pub async fn dead_letters_async(&self, limit: u32) -> Result<Vec<KernelOutboxEvent>> {
        let rows = sqlx::query_as::<_, KernelOutboxRow>(
            "SELECT id, event_type, aggregate_type, aggregate_id, payload, command_id,
                    idempotency_key, principal_type, principal_id, correlation_id,
                    causation_id, created_at, published_at, attempts, last_error,
                    lease_owner, lease_expires_at, next_attempt_at, dead_lettered_at
             FROM kernel_outbox WHERE dead_lettered_at IS NOT NULL
             ORDER BY dead_lettered_at, id LIMIT $1",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        rows.into_iter().map(row_to_event).collect()
    }

    /// Explicitly return a dead letter to the ready queue.
    pub async fn redrive_dead_letter_async(&self, id: Uuid, reset_attempts: bool) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE kernel_outbox SET dead_lettered_at = NULL, next_attempt_at = NOW(),
                    attempts = CASE WHEN $1 THEN 0 ELSE attempts END, last_error = NULL
             WHERE id = $2 AND dead_lettered_at IS NOT NULL",
        )
        .bind(reset_attempts)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(result.rows_affected() == 1)
    }

    /// Count delivery states for health checks and metrics exporters.
    pub async fn delivery_health_async(&self) -> Result<KernelOutboxHealth> {
        let counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT
                COUNT(*) FILTER (WHERE published_at IS NULL AND dead_lettered_at IS NULL
                    AND (next_attempt_at IS NULL OR next_attempt_at <= NOW())
                    AND (lease_expires_at IS NULL OR lease_expires_at <= NOW())),
                COUNT(*) FILTER (WHERE published_at IS NULL AND lease_expires_at > NOW()),
                COUNT(*) FILTER (WHERE published_at IS NULL AND dead_lettered_at IS NULL AND next_attempt_at > NOW()),
                COUNT(*) FILTER (WHERE dead_lettered_at IS NOT NULL),
                COUNT(*) FILTER (WHERE published_at IS NOT NULL)
             FROM kernel_outbox",
        ).fetch_one(&self.pool).await.map_err(|e| CommerceError::DatabaseError(e.to_string()))?;
        Ok(KernelOutboxHealth {
            ready: u64::try_from(counts.0)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?,
            leased: u64::try_from(counts.1)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?,
            delayed: u64::try_from(counts.2)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?,
            dead_lettered: u64::try_from(counts.3)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?,
            published: u64::try_from(counts.4)
                .map_err(|e| CommerceError::DatabaseError(e.to_string()))?,
        })
    }

    /// Acknowledge successful delivery.
    pub async fn mark_published_async(&self, id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE kernel_outbox SET published_at = NOW(), last_error = NULL WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        Ok(())
    }

    /// Record a failed delivery attempt without losing the event.
    pub async fn record_failure_async(&self, id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE kernel_outbox SET attempts = attempts + 1, last_error = $1 WHERE id = $2",
        )
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|db_error| CommerceError::DatabaseError(db_error.to_string()))?;
        Ok(())
    }

    /// Load the final receipt associated with a retry key.
    pub async fn receipt_by_idempotency_key_async(
        &self,
        key: &str,
    ) -> Result<Option<KernelReceiptRecord>> {
        sqlx::query_as::<_, KernelReceiptRow>(
            "SELECT command_id, idempotency_key, command_type, contract_version,
                    request_hash, status, receipt, created_at, completed_at
             FROM kernel_receipts WHERE idempotency_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await
        .map(|row| row.map(Into::into))
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))
    }

    /// Recompute every receipt link and report the first broken chain position.
    pub async fn verify_audit_chain_async(&self) -> Result<KernelAuditVerification> {
        let rows = sqlx::query_as::<_, KernelAuditRow>(
            "SELECT sequence, request_hash, previous_audit_hash, audit_hash, receipt
             FROM kernel_receipt_audit_log ORDER BY sequence",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let mut previous: Option<String> = None;
        let mut entries = 0_u64;
        for row in rows {
            entries += 1;
            let computed = receipt_audit_hash(previous.as_deref(), &row.request_hash, &row.receipt)
                .map_err(CommerceError::DatabaseError)?;
            if row.previous_audit_hash != previous || row.audit_hash != computed {
                return Ok(KernelAuditVerification {
                    valid: false,
                    entries,
                    head_hash: previous,
                    first_invalid_sequence: Some(row.sequence),
                });
            }
            previous = Some(row.audit_hash);
        }
        let materialized = sqlx::query_as::<_, MaterializedReceiptAuditRow>(
            "SELECT r.receipt, r.request_hash, a.sequence,
                    a.receipt AS audit_receipt, a.request_hash AS audit_request_hash
             FROM kernel_receipts r
             LEFT JOIN kernel_receipt_audit_log a
               ON a.audit_hash = r.receipt->>'audit_hash'",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        for row in materialized {
            let mut unsealed = row.receipt;
            if let Some(object) = unsealed.as_object_mut() {
                object.insert("audit_hash".into(), Value::Null);
            }
            if row.sequence.is_none()
                || row.audit_request_hash.as_deref() != Some(row.request_hash.as_str())
                || row.audit_receipt.as_ref() != Some(&unsealed)
            {
                return Ok(KernelAuditVerification {
                    valid: false,
                    entries,
                    head_hash: previous,
                    first_invalid_sequence: Some(row.sequence.unwrap_or(0)),
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
    pub async fn audit_checkpoint_async(&self) -> Result<KernelAuditCheckpoint> {
        build_audit_checkpoint(&self.verify_audit_chain_async().await?)
            .map_err(CommerceError::ValidationError)
    }

    /// Verify an externally retained checkpoint against the complete local
    /// chain. Checkpoints remain valid after newer receipts are appended.
    pub async fn verify_audit_checkpoint_async(
        &self,
        checkpoint: &KernelAuditCheckpoint,
    ) -> Result<bool> {
        if checkpoint.contract_version != "1.0"
            || checkpoint.algorithm != "sha256-jcs-v1"
            || !audit_checkpoint_hash_is_valid(checkpoint)
                .map_err(CommerceError::ValidationError)?
        {
            return Ok(false);
        }
        let verification = self.verify_audit_chain_async().await?;
        if !verification.valid || checkpoint.entries > verification.entries {
            return Ok(false);
        }
        if checkpoint.entries == 0 {
            return Ok(checkpoint.head_hash.is_none());
        }
        let expected_hash = checkpoint.head_hash.as_deref().ok_or_else(|| {
            CommerceError::ValidationError("non-empty checkpoint is missing head_hash".into())
        })?;
        // Address the entry by ordinal position, not by `sequence` value: a
        // rolled-back append still consumes a BIGSERIAL value, so the chain
        // may legitimately contain gaps.
        let Ok(offset) = i64::try_from(checkpoint.entries - 1) else {
            return Ok(false);
        };
        let local_hash = sqlx::query_scalar::<_, String>(
            "SELECT audit_hash FROM kernel_receipt_audit_log
             ORDER BY sequence OFFSET $1 LIMIT 1",
        )
        .bind(offset)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        Ok(local_hash.as_deref() == Some(expected_hash))
    }
}

fn row_to_event(row: KernelOutboxRow) -> Result<KernelOutboxEvent> {
    Ok(KernelOutboxEvent {
        id: row.id,
        event_type: row.event_type,
        aggregate_type: row.aggregate_type,
        aggregate_id: row.aggregate_id,
        payload: row.payload,
        command_id: row.command_id,
        idempotency_key: row.idempotency_key,
        principal_type: row.principal_type,
        principal_id: row.principal_id,
        correlation_id: row.correlation_id,
        causation_id: row.causation_id,
        created_at: row.created_at,
        published_at: row.published_at,
        attempts: u32::try_from(row.attempts).map_err(|e| {
            CommerceError::DatabaseError(format!("invalid kernel outbox attempt count: {e}"))
        })?,
        last_error: row.last_error,
        lease_owner: row.lease_owner,
        lease_expires_at: row.lease_expires_at,
        next_attempt_at: row.next_attempt_at,
        dead_lettered_at: row.dead_lettered_at,
    })
}

pub(crate) async fn append_kernel_event_tx(
    tx: &mut sqlx::PgConnection,
    event: &KernelOutboxEvent,
) -> Result<()> {
    let attempts = i32::try_from(event.attempts).map_err(|error| {
        CommerceError::DatabaseError(format!("kernel outbox attempt count overflow: {error}"))
    })?;
    sqlx::query(
        "INSERT INTO kernel_outbox (
            id, event_type, aggregate_type, aggregate_id, payload, command_id,
            idempotency_key, principal_type, principal_id, correlation_id,
            causation_id, created_at, published_at, attempts, last_error
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)",
    )
    .bind(event.id)
    .bind(&event.event_type)
    .bind(&event.aggregate_type)
    .bind(&event.aggregate_id)
    .bind(&event.payload)
    .bind(event.command_id)
    .bind(&event.idempotency_key)
    .bind(&event.principal_type)
    .bind(&event.principal_id)
    .bind(event.correlation_id)
    .bind(event.causation_id)
    .bind(event.created_at)
    .bind(event.published_at)
    .bind(attempts)
    .bind(&event.last_error)
    .execute(tx)
    .await
    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    Ok(())
}

pub(crate) async fn append_kernel_receipt_tx(
    tx: &mut sqlx::PgConnection,
    record: &KernelReceiptRecord,
) -> Result<String> {
    // A single transaction-scoped lock prevents concurrent commands from
    // forking the global audit chain while retaining per-command concurrency.
    sqlx::query("SELECT pg_advisory_xact_lock(600451391847133116)")
        .execute(&mut *tx)
        .await
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    let previous_audit_hash = sqlx::query_scalar::<_, String>(
        "SELECT audit_hash FROM kernel_receipt_audit_log ORDER BY sequence DESC LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    let audit_hash =
        receipt_audit_hash(previous_audit_hash.as_deref(), &record.request_hash, &record.receipt)
            .map_err(CommerceError::DatabaseError)?;
    let receipt_id = record
        .receipt
        .get("receipt_id")
        .and_then(Value::as_str)
        .ok_or_else(|| CommerceError::DatabaseError("receipt_id missing from receipt".into()))?
        .parse::<Uuid>()
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    sqlx::query(
        "INSERT INTO kernel_receipt_audit_log (
            receipt_id, command_id, idempotency_key, request_hash,
            previous_audit_hash, audit_hash, receipt, created_at
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(receipt_id)
    .bind(record.command_id)
    .bind(&record.idempotency_key)
    .bind(&record.request_hash)
    .bind(&previous_audit_hash)
    .bind(&audit_hash)
    .bind(&record.receipt)
    .bind(record.completed_at)
    .execute(&mut *tx)
    .await
    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    let mut sealed_receipt = record.receipt.clone();
    if let Some(object) = sealed_receipt.as_object_mut() {
        object.insert("audit_hash".into(), Value::String(audit_hash.clone()));
    }
    sqlx::query(
        "INSERT INTO kernel_receipts (
            command_id, idempotency_key, command_type, contract_version,
            request_hash, status, receipt, created_at, completed_at
         ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT(idempotency_key) DO UPDATE SET
            command_id = EXCLUDED.command_id,
            command_type = EXCLUDED.command_type,
            contract_version = EXCLUDED.contract_version,
            request_hash = EXCLUDED.request_hash,
            status = EXCLUDED.status,
            receipt = EXCLUDED.receipt,
            completed_at = EXCLUDED.completed_at",
    )
    .bind(record.command_id)
    .bind(&record.idempotency_key)
    .bind(&record.command_type)
    .bind(&record.contract_version)
    .bind(&record.request_hash)
    .bind(&record.status)
    .bind(&sealed_receipt)
    .bind(record.created_at)
    .bind(record.completed_at)
    .execute(tx)
    .await
    .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    Ok(audit_hash)
}

/// Load the sealed audit-log entry a materialized receipt claims through its
/// `audit_hash`, for replay verification.
pub(crate) async fn sealed_audit_entry_tx(
    tx: &mut sqlx::PgConnection,
    existing: &KernelReceiptRecord,
) -> Result<Option<SealedAuditEntry>> {
    let Some(audit_hash) = existing.receipt.get("audit_hash").and_then(Value::as_str) else {
        return Ok(None);
    };
    sqlx::query_as::<_, (Option<String>, String)>(
        "SELECT previous_audit_hash, request_hash FROM kernel_receipt_audit_log
         WHERE audit_hash = $1",
    )
    .bind(audit_hash)
    .fetch_optional(tx)
    .await
    .map(|row| {
        row.map(|(previous_audit_hash, request_hash)| SealedAuditEntry {
            previous_audit_hash,
            request_hash,
        })
    })
    .map_err(|error| CommerceError::DatabaseError(error.to_string()))
}
