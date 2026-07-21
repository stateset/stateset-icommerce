//! Durable HTTP idempotency response store.
//!
//! Backs the HTTP layer's `Idempotency-Key` middleware with the database so
//! that replays survive process restarts and work across replicas sharing a
//! database. Rows live in the `http_idempotency_keys` table, keyed by
//! `(tenant, idempotency_key)`, and carry the request fingerprint
//! (method + path + body hash) plus the stored response.
//!
//! TTL expiry is enforced lazily: [`HttpIdempotencyRepository::get`] deletes
//! and ignores rows created before the caller-supplied cutoff, and
//! [`HttpIdempotencyRepository::purge_expired`] performs bulk cleanup sweeps.

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
    /// HTTP status code of the stored response.
    pub response_status: u16,
    /// `Content-Type` of the stored response, if any.
    pub content_type: Option<String>,
    /// Raw response body bytes.
    pub response_body: Vec<u8>,
    /// When the entry was first stored (drives TTL expiry).
    pub created_at: DateTime<Utc>,
}

/// Repository over the `http_idempotency_keys` table.
pub trait HttpIdempotencyRepository: Send + Sync {
    /// Fetch the entry for `(tenant, key)`.
    ///
    /// Rows created strictly before `expired_before` are treated as expired:
    /// they are deleted (lazy cleanup) and `None` is returned.
    fn get(
        &self,
        tenant: &str,
        key: &str,
        expired_before: DateTime<Utc>,
    ) -> Result<Option<HttpIdempotencyRecord>>;

    /// Insert a new entry. Returns `false` (without overwriting) when an entry
    /// for `(tenant, key)` already exists — first write wins across replicas.
    fn put(&self, record: &HttpIdempotencyRecord) -> Result<bool>;

    /// Delete all entries created strictly before `expired_before`, returning
    /// the number of rows removed.
    fn purge_expired(&self, expired_before: DateTime<Utc>) -> Result<u64>;
}
