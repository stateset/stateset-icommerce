//! Durable HTTP idempotency response store.
//!
//! Backs the HTTP layer's `Idempotency-Key` middleware with the database so
//! that replays survive process restarts and work across replicas sharing a
//! database. Rows live in the `http_idempotency_keys` table, keyed by
//! `(tenant, idempotency_key)`, and carry the request fingerprint
//! (method + path + body hash) plus the stored response.
//!
//! TTL expiry is enforced lazily: [`HttpIdempotencyRepository::get`] deletes
//! and ignores completed rows created at or before the caller-supplied cutoff, and
//! [`HttpIdempotencyRepository::purge_expired`] performs bulk cleanup sweeps.
//! Unresolved reservations (status zero) are retained until reconciled.

use chrono::{DateTime, Utc};
use stateset_core::Result;

/// A durable idempotency entry: request fingerprint plus the stored response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpIdempotencyRecord {
    /// Tenant namespace (empty string for single-tenant deployments).
    pub tenant: String,
    /// Client-chosen idempotency key.
    pub idempotency_key: String,
    /// Fingerprint of the originating request (hex SHA-256 of
    /// method + path + body).
    pub request_fingerprint: String,
    /// HTTP status code of the stored response. Zero reserves the key before
    /// execution; such unresolved reservations never expire automatically.
    pub response_status: u16,
    /// `Content-Type` of the stored response, if any.
    pub content_type: Option<String>,
    /// Raw response body bytes.
    pub response_body: Vec<u8>,
    /// Reservation time, or response completion time (starts the response TTL).
    pub created_at: DateTime<Utc>,
}

/// Metadata for operator reconciliation. No response payloads or credentials.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PendingHttpReceipt {
    /// Tenant namespace; always supplied by the operator, never inferred.
    pub tenant: String,
    /// Original retry key. Do not create a replacement key to resolve uncertainty.
    pub idempotency_key: String,
    /// Request identity to compare before resolving a pending receipt.
    pub request_fingerprint: String,
    /// Original reservation timestamp.
    pub reserved_at: DateTime<Utc>,
}

/// Bound operator scans so diagnostics cannot accidentally load the entire store.
#[cfg(any(feature = "sqlite", feature = "postgres"))]
pub(crate) fn validate_pending_limit(limit: u32) -> Result<()> {
    if !(1..=1000).contains(&limit) {
        return Err(stateset_core::CommerceError::ValidationError(
            "pending receipt limit must be between 1 and 1000".into(),
        ));
    }
    Ok(())
}

/// Repository over the `http_idempotency_keys` table.
pub trait HttpIdempotencyRepository: Send + Sync {
    /// Read unresolved reservations for one explicit tenant, oldest first,
    /// with `reserved_at <= older_than` and a limit of 1..=1000. This does not
    /// expire, resolve or delete anything. Use the final timestamp/key for
    /// follow-up inspection; an old reservation alone does not prove failure.
    fn list_pending(
        &self,
        _tenant: &str,
        _older_than: DateTime<Utc>,
        _limit: u32,
    ) -> Result<Vec<PendingHttpReceipt>> {
        Err(stateset_core::CommerceError::Internal(
            "pending receipt inspection is not supported by this backend".into(),
        ))
    }

    /// Fetch the entry for `(tenant, key)`.
    ///
    /// Completed rows created at or before `expired_before` are treated as
    /// expired: they are deleted and `None` is returned. Unresolved reservations
    /// are returned regardless of age.
    fn get(
        &self,
        tenant: &str,
        key: &str,
        expired_before: DateTime<Utc>,
    ) -> Result<Option<HttpIdempotencyRecord>>;

    /// Insert a new entry. Returns `false` (without overwriting) when an entry
    /// for `(tenant, key)` already exists — first write wins across replicas.
    fn put(&self, record: &HttpIdempotencyRecord) -> Result<bool>;

    /// Replace an unresolved reservation with its response, only when its
    /// fingerprint matches. Never overwrites an already completed response.
    fn complete(&self, _record: &HttpIdempotencyRecord) -> Result<bool> {
        Err(stateset_core::CommerceError::Internal(
            "durable idempotency completion is not supported by this backend".into(),
        ))
    }

    /// Delete completed entries created at or before `expired_before`, returning
    /// the number of rows removed. Never deletes unresolved reservations.
    fn purge_expired(&self, expired_before: DateTime<Utc>) -> Result<u64>;
}
