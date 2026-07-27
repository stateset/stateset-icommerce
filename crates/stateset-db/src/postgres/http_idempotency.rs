//! PostgreSQL implementation of the durable HTTP idempotency store.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::postgres::PgPool;
use stateset_core::Result;

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
            // Lazy cleanup: drop an expired row for this key before reading.
            sqlx::query(
                "DELETE FROM http_idempotency_keys
                 WHERE tenant = $1 AND idempotency_key = $2 AND created_at <= $3",
            )
            .bind(tenant)
            .bind(key)
            .bind(expired_before)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;

            let row: Option<HttpIdempotencyRow> = sqlx::query_as(
                "SELECT request_fingerprint, response_status, content_type,
                        response_body, created_at
                 FROM http_idempotency_keys
                 WHERE tenant = $1 AND idempotency_key = $2",
            )
            .bind(tenant)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)?;

            Ok(row.map(|row| HttpIdempotencyRecord {
                tenant: tenant.to_string(),
                idempotency_key: key.to_string(),
                request_fingerprint: row.request_fingerprint,
                response_status: u16::try_from(row.response_status).unwrap_or(500),
                content_type: row.content_type,
                response_body: row.response_body,
                created_at: row.created_at,
            }))
        })
    }

    fn put(&self, record: &HttpIdempotencyRecord) -> Result<bool> {
        block_on(async {
            let result = sqlx::query(
                "INSERT INTO http_idempotency_keys
                 (tenant, idempotency_key, request_fingerprint, response_status,
                  content_type, response_body, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (tenant, idempotency_key) DO NOTHING",
            )
            .bind(&record.tenant)
            .bind(&record.idempotency_key)
            .bind(&record.request_fingerprint)
            .bind(i32::from(record.response_status))
            .bind(&record.content_type)
            .bind(&record.response_body)
            .bind(record.created_at)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)?;
            Ok(result.rows_affected() > 0)
        })
    }

    fn purge_expired(&self, expired_before: DateTime<Utc>) -> Result<u64> {
        block_on(async {
            let result = sqlx::query("DELETE FROM http_idempotency_keys WHERE created_at < $1")
                .bind(expired_before)
                .execute(&self.pool)
                .await
                .map_err(map_db_error)?;
            Ok(result.rows_affected())
        })
    }
}
