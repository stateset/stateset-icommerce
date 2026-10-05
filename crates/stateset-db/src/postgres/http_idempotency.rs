//! PostgreSQL implementation of the durable HTTP idempotency store.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::{CommerceError, Result};

use super::{block_on, map_db_error};
use crate::{HttpIdempotencyRecord, HttpIdempotencyRepository};

/// PostgreSQL repository over the `http_idempotency_keys` table.
#[derive(Debug, Clone)]
pub struct PgHttpIdempotencyRepository {
    pool: PgPool,
}

#[derive(FromRow)]
struct HttpIdempotencyRow {
    request_fingerprint: String,
    response_status: i32,
    content_type: Option<String>,
    response_body: Vec<u8>,
    created_at: DateTime<Utc>,
    created_at_epoch_seconds: Option<i64>,
    created_at_subsec_ns: Option<i64>,
}

impl PgHttpIdempotencyRepository {
    #[must_use]
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl HttpIdempotencyRepository for PgHttpIdempotencyRepository {
    fn get(
        &self,
        tenant: &str,
        key: &str,
        expired_before: DateTime<Utc>,
    ) -> Result<Option<HttpIdempotencyRecord>> {
        block_on(async {
            // Exact components prevent a live response from expiring within
            // PostgreSQL's microsecond timestamp resolution. Legacy rows use
            // a conservative strict comparison at that boundary.
            sqlx::query(
                "DELETE FROM http_idempotency_keys
                 WHERE tenant = $1 AND idempotency_key = $2 AND response_status != 0 AND (
                   (created_at_epoch_seconds IS NOT NULL AND
                    (created_at_epoch_seconds < $3 OR
                     (created_at_epoch_seconds = $3 AND created_at_subsec_ns <= $4)))
                   OR (created_at_epoch_seconds IS NULL AND created_at < $5))",
            )
            .bind(tenant)
            .bind(key)
            .bind(expired_before.timestamp())
            .bind(i64::from(expired_before.timestamp_subsec_nanos()))
            .bind(expired_before)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

            let row: Option<HttpIdempotencyRow> = sqlx::query_as(
                "SELECT request_fingerprint, response_status, content_type,
                        response_body, created_at, created_at_epoch_seconds,
                        created_at_subsec_ns
                 FROM http_idempotency_keys
                 WHERE tenant = $1 AND idempotency_key = $2",
            )
            .bind(tenant)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

            row.map(|row| {
                let created_at = match (row.created_at_epoch_seconds, row.created_at_subsec_ns) {
                    (Some(seconds), Some(nanos)) => u32::try_from(nanos)
                        .ok()
                        .and_then(|nanos| DateTime::<Utc>::from_timestamp(seconds, nanos))
                        .ok_or_else(|| {
                            CommerceError::DatabaseError(
                                "http_idempotency_keys exact creation time out of range".into(),
                            )
                        })?,
                    (None, None) => row.created_at,
                    _ => {
                        return Err(CommerceError::DatabaseError(
                            "http_idempotency_keys exact creation time is incomplete".into(),
                        ));
                    }
                };
                Ok(HttpIdempotencyRecord {
                    tenant: tenant.to_string(),
                    idempotency_key: key.to_string(),
                    request_fingerprint: row.request_fingerprint,
                    response_status: u16::try_from(row.response_status).unwrap_or(500),
                    content_type: row.content_type,
                    response_body: row.response_body,
                    created_at,
                })
            })
            .transpose()
        })
    }

    fn put(&self, record: &HttpIdempotencyRecord) -> Result<bool> {
        block_on(async {
            let result = sqlx::query(
                "INSERT INTO http_idempotency_keys
                 (tenant, idempotency_key, request_fingerprint, response_status,
                  content_type, response_body, created_at,
                  created_at_epoch_seconds, created_at_subsec_ns)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (tenant, idempotency_key) DO NOTHING",
            )
            .bind(&record.tenant)
            .bind(&record.idempotency_key)
            .bind(&record.request_fingerprint)
            .bind(i32::from(record.response_status))
            .bind(&record.content_type)
            .bind(&record.response_body)
            .bind(record.created_at)
            .bind(record.created_at.timestamp())
            .bind(i64::from(record.created_at.timestamp_subsec_nanos()))
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
            Ok(result.rows_affected() > 0)
        })
    }

    fn complete(&self, record: &HttpIdempotencyRecord) -> Result<bool> {
        if record.response_status == 0 {
            return Err(CommerceError::ValidationError(
                "completion requires an HTTP response".into(),
            ));
        }
        block_on(async {
            let updated = sqlx::query(
                "UPDATE http_idempotency_keys SET response_status = $1, content_type = $2,
                 response_body = $3, created_at = $4, created_at_epoch_seconds = $5,
                 created_at_subsec_ns = $6
                 WHERE tenant = $7 AND idempotency_key = $8 AND request_fingerprint = $9
                   AND response_status = 0",
            )
            .bind(i32::from(record.response_status))
            .bind(&record.content_type)
            .bind(&record.response_body)
            .bind(record.created_at)
            .bind(record.created_at.timestamp())
            .bind(i64::from(record.created_at.timestamp_subsec_nanos()))
            .bind(&record.tenant)
            .bind(&record.idempotency_key)
            .bind(&record.request_fingerprint)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
            Ok(updated.rows_affected() == 1)
        })
    }

    fn purge_expired(&self, expired_before: DateTime<Utc>) -> Result<u64> {
        block_on(async {
            let result = sqlx::query(
                "DELETE FROM http_idempotency_keys WHERE response_status != 0 AND (
                   (created_at_epoch_seconds IS NOT NULL AND
                    (created_at_epoch_seconds < $1 OR
                     (created_at_epoch_seconds = $1 AND created_at_subsec_ns <= $2)))
                   OR (created_at_epoch_seconds IS NULL AND created_at < $3))",
            )
            .bind(expired_before.timestamp())
            .bind(i64::from(expired_before.timestamp_subsec_nanos()))
            .bind(expired_before)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
            Ok(result.rows_affected())
        })
    }
}
