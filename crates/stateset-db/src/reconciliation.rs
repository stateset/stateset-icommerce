//! Operator-only SQLite HTTP receipt reconciliation.
//!
//! Stop the original worker and verify the business/provider outcome first.
//! This API cannot prove external settlement or that a worker cannot resume.
//! It never re-executes a mutation and never deletes a pending reservation.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, TransactionBehavior, params};
use stateset_core::{CommerceError, Result};

use crate::HttpIdempotencyRecord;

/// Evidence supplied by a trusted operator after inspecting the original mutation.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpReceiptResolution {
    /// Explicit tenant namespace (empty for single-tenant deployments).
    pub tenant: String,
    /// The original key; a new key risks executing the mutation again.
    pub idempotency_key: String,
    /// Exact request fingerprint observed before reconciliation.
    pub request_fingerprint: String,
    /// Original reservation time, compared atomically to reject stale evidence.
    pub reserved_at: DateTime<Utc>,
    /// Verified final HTTP response status (200..=599).
    pub response_status: u16,
    /// Verified response media type, if present.
    pub content_type: Option<String>,
    /// Verified response bytes; operator resolutions are bounded to one mebibyte.
    pub response_body: Vec<u8>,
    /// Operator identity recorded in the required audit entry.
    pub actor: String,
    /// Incident or reconciliation evidence reference; do not include secrets.
    pub evidence: String,
}

impl HttpReceiptResolution {
    /// Validate response bounds and required operator evidence without writing.
    pub fn validate(&self) -> Result<()> {
        if self.idempotency_key.is_empty()
            || self.idempotency_key.len() > 255
            || self.request_fingerprint.len() != 64
            || !self.request_fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !(200..=599).contains(&self.response_status)
            || self.response_body.len() > 1024 * 1024
            || self.actor.trim().is_empty()
            || self.actor.len() > 255
            || self.evidence.trim().is_empty()
            || self.evidence.len() > 4096
            || self.content_type.as_ref().is_some_and(|value| {
                value.is_empty()
                    || value.len() > 255
                    || !value.bytes().all(|byte| (32..=126).contains(&byte))
            })
        {
            return Err(CommerceError::ValidationError(
                "receipt resolution requires an original key/fingerprint, final response, operator and evidence reference".into(),
            ));
        }
        Ok(())
    }
}

/// Complete exactly the observed pending reservation and insert its audit entry
/// in one IMMEDIATE transaction. Audit failure rolls back the completion.
/// Repeated/stale resolutions return Conflict, leaving the original response intact.
pub fn reconcile_http_receipt(
    conn: &mut Connection,
    resolution: &HttpReceiptResolution,
) -> Result<HttpIdempotencyRecord> {
    resolution.validate()?;
    let now = Utc::now();
    if resolution.reserved_at > now {
        return Err(CommerceError::ValidationError("reservation time is in the future".into()));
    }
    let map_error = |error: rusqlite::Error| CommerceError::DatabaseError(error.to_string());
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(map_error)?;
    let nanos = i64::from(resolution.reserved_at.timestamp_subsec_nanos() % 1_000_000);
    let updated = tx
        .execute(
            "UPDATE http_idempotency_keys SET response_status = ?1, content_type = ?2,
         response_body = ?3, created_at = ?4, created_at_sub_ms_ns = ?5
         WHERE tenant = ?6 AND idempotency_key = ?7 AND request_fingerprint = ?8
           AND response_status = 0 AND created_at = ?9
           AND (created_at_sub_ms_ns = ?10 OR (created_at_sub_ms_ns IS NULL AND ?10 = 0))",
            params![
                resolution.response_status,
                resolution.content_type,
                resolution.response_body,
                now.timestamp_millis(),
                i64::from(now.timestamp_subsec_nanos() % 1_000_000),
                resolution.tenant,
                resolution.idempotency_key,
                resolution.request_fingerprint,
                resolution.reserved_at.timestamp_millis(),
                nanos
            ],
        )
        .map_err(map_error)?;
    if updated != 1 {
        return Err(CommerceError::Conflict(
            "pending receipt no longer matches the observed tenant, key, fingerprint and reservation time".into(),
        ));
    }
    use sha2::{Digest, Sha256};
    let details = serde_json::json!({
        "tenant": resolution.tenant,
        "request_fingerprint": resolution.request_fingerprint,
        "reserved_at": resolution.reserved_at,
        "response_status": resolution.response_status,
        "response_sha256": format!("{:x}", Sha256::digest(&resolution.response_body)),
        "evidence": resolution.evidence,
    });
    crate::audit::record_audit_required(
        &tx,
        "http_receipt.reconciled",
        "http_idempotency",
        &resolution.idempotency_key,
        Some(&resolution.actor),
        Some(&details.to_string()),
    )
    .map_err(map_error)?;
    tx.commit().map_err(map_error)?;
    Ok(HttpIdempotencyRecord {
        tenant: resolution.tenant.clone(),
        idempotency_key: resolution.idempotency_key.clone(),
        request_fingerprint: resolution.request_fingerprint.clone(),
        response_status: resolution.response_status,
        content_type: resolution.content_type.clone(),
        response_body: resolution.response_body.clone(),
        created_at: now,
    })
}

/// Read-only, bounded inspection of pending receipts for one tenant. Safe on a
/// connection opened with `SQLITE_OPEN_READ_ONLY`; performs no TTL cleanup.
pub fn pending_http_receipts(
    conn: &Connection,
    tenant: &str,
    older_than: DateTime<Utc>,
    limit: u32,
) -> Result<Vec<crate::PendingHttpReceipt>> {
    crate::http_idempotency::validate_pending_limit(limit)?;
    let mut statement = conn
        .prepare(
            "SELECT idempotency_key, request_fingerprint, created_at, created_at_sub_ms_ns
             FROM http_idempotency_keys WHERE tenant = ?1 AND response_status = 0
               AND (created_at < ?2 OR (created_at = ?2
                    AND COALESCE(created_at_sub_ms_ns, 0) <= ?3))
             ORDER BY created_at, COALESCE(created_at_sub_ms_ns, 0), idempotency_key LIMIT ?4",
        )
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    let rows = statement
        .query_map(
            params![
                tenant,
                older_than.timestamp_millis(),
                i64::from(older_than.timestamp_subsec_nanos() % 1_000_000),
                limit
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
    rows.map(|row| {
        let (idempotency_key, request_fingerprint, millis, nanos) =
            row.map_err(|error| CommerceError::DatabaseError(error.to_string()))?;
        let nanos = nanos.unwrap_or(0);
        let reserved_at = DateTime::<Utc>::from_timestamp_millis(millis)
            .filter(|_| (0..1_000_000).contains(&nanos))
            .and_then(|time| time.checked_add_signed(chrono::Duration::nanoseconds(nanos)))
            .ok_or_else(|| {
                CommerceError::DatabaseError("invalid pending receipt timestamp".into())
            })?;
        Ok(crate::PendingHttpReceipt {
            tenant: tenant.into(),
            idempotency_key,
            request_fingerprint,
            reserved_at,
        })
    })
    .collect()
}
