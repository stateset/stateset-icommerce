//! SQLite implementation of the durable HTTP idempotency store.

use chrono::{DateTime, Utc};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OptionalExtension as _;
use stateset_core::{CommerceError, Result};

use crate::{HttpIdempotencyRecord, HttpIdempotencyRepository};

/// SQLite repository over the `http_idempotency_keys` table.
#[derive(Debug)]
pub struct SqliteHttpIdempotencyRepository {
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteHttpIdempotencyRepository {
    #[must_use]
    pub const fn new(pool: Pool<SqliteConnectionManager>) -> Self {
        Self { pool }
    }

    fn conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool.get().map_err(|e| CommerceError::DatabaseError(e.to_string()))
    }
}

fn map_err(e: rusqlite::Error) -> CommerceError {
    CommerceError::DatabaseError(e.to_string())
}

impl HttpIdempotencyRepository for SqliteHttpIdempotencyRepository {
    fn get(
        &self,
        tenant: &str,
        key: &str,
        expired_before: DateTime<Utc>,
    ) -> Result<Option<HttpIdempotencyRecord>> {
        let conn = self.conn()?;
        // Lazy cleanup: compare the stored millisecond and nanosecond remainder
        // together. Legacy rows have no remainder and survive an ambiguous
        // same-millisecond cutoff until the next millisecond.
        conn.execute(
            "DELETE FROM http_idempotency_keys
             WHERE tenant = ?1 AND idempotency_key = ?2
               AND (created_at < ?3 OR
                    (created_at = ?3 AND created_at_sub_ms_ns IS NOT NULL
                     AND created_at_sub_ms_ns <= ?4))",
            rusqlite::params![
                tenant,
                key,
                expired_before.timestamp_millis(),
                i64::from(expired_before.timestamp_subsec_nanos() % 1_000_000),
            ],
        )
        .map_err(map_err)?;

        conn.query_row(
            "SELECT request_fingerprint, response_status, content_type, response_body,
                    created_at, created_at_sub_ms_ns
             FROM http_idempotency_keys
             WHERE tenant = ?1 AND idempotency_key = ?2",
            rusqlite::params![tenant, key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u16>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()
        .map_err(map_err)?
        .map(
            |(
                request_fingerprint,
                response_status,
                content_type,
                response_body,
                created_ms,
                sub_ms_ns,
            )| {
                let sub_ms_ns = sub_ms_ns.unwrap_or(0);
                if !(0..1_000_000).contains(&sub_ms_ns) {
                    return Err(CommerceError::DatabaseError(format!(
                        "http_idempotency_keys.created_at_sub_ms_ns out of range: {sub_ms_ns}"
                    )));
                }
                let created_at = DateTime::<Utc>::from_timestamp_millis(created_ms)
                    .and_then(|millisecond| {
                        millisecond.checked_add_signed(chrono::Duration::nanoseconds(sub_ms_ns))
                    })
                    .ok_or_else(|| {
                        CommerceError::DatabaseError(format!(
                            "http_idempotency_keys.created_at out of range: {created_ms}"
                        ))
                    })?;
                Ok(HttpIdempotencyRecord {
                    tenant: tenant.to_string(),
                    idempotency_key: key.to_string(),
                    request_fingerprint,
                    response_status,
                    content_type,
                    response_body,
                    created_at,
                })
            },
        )
        .transpose()
    }

    fn put(&self, record: &HttpIdempotencyRecord) -> Result<bool> {
        let conn = self.conn()?;
        let inserted = conn
            .execute(
                "INSERT INTO http_idempotency_keys
                 (tenant, idempotency_key, request_fingerprint, response_status,
                  content_type, response_body, created_at, created_at_sub_ms_ns)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(tenant, idempotency_key) DO NOTHING",
                rusqlite::params![
                    record.tenant,
                    record.idempotency_key,
                    record.request_fingerprint,
                    record.response_status,
                    record.content_type,
                    record.response_body,
                    record.created_at.timestamp_millis(),
                    i64::from(record.created_at.timestamp_subsec_nanos() % 1_000_000),
                ],
            )
            .map_err(map_err)?;
        Ok(inserted > 0)
    }

    fn purge_expired(&self, expired_before: DateTime<Utc>) -> Result<u64> {
        let conn = self.conn()?;
        let deleted = conn
            .execute(
                "DELETE FROM http_idempotency_keys
                 WHERE created_at < ?1 OR
                       (created_at = ?1 AND created_at_sub_ms_ns IS NOT NULL
                        AND created_at_sub_ms_ns <= ?2)",
                rusqlite::params![
                    expired_before.timestamp_millis(),
                    i64::from(expired_before.timestamp_subsec_nanos() % 1_000_000),
                ],
            )
            .map_err(map_err)?;
        Ok(deleted as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatabaseConfig, SqliteDatabase};
    use chrono::Duration;
    use std::sync::{Arc, Barrier};

    fn repo() -> SqliteHttpIdempotencyRepository {
        let db = SqliteDatabase::new(&DatabaseConfig::in_memory()).expect("in-memory db");
        db.http_idempotency()
    }

    fn record(key: &str, created_at: DateTime<Utc>) -> HttpIdempotencyRecord {
        HttpIdempotencyRecord {
            tenant: "tenant-a".into(),
            idempotency_key: key.into(),
            request_fingerprint: "f".repeat(64),
            response_status: 201,
            content_type: Some("application/json".into()),
            response_body: b"{\"id\":\"order-1\"}".to_vec(),
            created_at,
        }
    }

    #[test]
    fn put_then_get_round_trips() {
        let repo = repo();
        let now = Utc::now();
        let rec = record("k1", now);
        assert!(repo.put(&rec).unwrap());

        let cutoff = now - Duration::hours(1);
        let loaded = repo.get("tenant-a", "k1", cutoff).unwrap().expect("record");
        assert_eq!(loaded.request_fingerprint, rec.request_fingerprint);
        assert_eq!(loaded.response_status, 201);
        assert_eq!(loaded.content_type.as_deref(), Some("application/json"));
        assert_eq!(loaded.response_body, rec.response_body);
        assert_eq!(loaded.created_at, now);
        // Other tenants and keys see nothing.
        assert!(repo.get("tenant-b", "k1", cutoff).unwrap().is_none());
        assert!(repo.get("tenant-a", "other", cutoff).unwrap().is_none());
    }

    #[test]
    fn put_is_first_write_wins() {
        let repo = repo();
        let now = Utc::now();
        assert!(repo.put(&record("k1", now)).unwrap());

        let mut second = record("k1", now);
        second.response_body = b"different".to_vec();
        // Duplicate insert is ignored, not overwritten.
        assert!(!repo.put(&second).unwrap());
        let loaded = repo.get("tenant-a", "k1", now - Duration::hours(1)).unwrap().expect("record");
        assert_eq!(loaded.response_body, b"{\"id\":\"order-1\"}");
    }

    #[test]
    fn concurrent_puts_preserve_one_first_response() {
        let repo = Arc::new(repo());
        let barrier = Arc::new(Barrier::new(2));
        let now = Utc::now();
        let mut handles = Vec::new();
        for body in [b"first".to_vec(), b"second".to_vec()] {
            let repo = Arc::clone(&repo);
            let barrier = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let mut rec = record("race", now);
                rec.response_body = body.clone();
                barrier.wait();
                (repo.put(&rec).expect("put"), body)
            }));
        }
        let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().expect("join")).collect();
        let winners: Vec<_> = outcomes.iter().filter(|(inserted, _)| *inserted).collect();
        assert_eq!(winners.len(), 1);
        let stored =
            repo.get("tenant-a", "race", now - Duration::hours(1)).expect("get").expect("record");
        assert_eq!(stored.response_body, winners[0].1);
    }

    #[test]
    fn get_lazily_deletes_expired_rows() {
        let repo = repo();
        let created = Utc::now() - Duration::hours(25);
        assert!(repo.put(&record("k1", created)).unwrap());

        // With a 24h TTL the row is expired: get() returns None and deletes it.
        let cutoff = Utc::now() - Duration::hours(24);
        assert!(repo.get("tenant-a", "k1", cutoff).unwrap().is_none());
        // Even a permissive cutoff now finds nothing — the row is gone.
        assert!(repo.get("tenant-a", "k1", DateTime::<Utc>::MIN_UTC).unwrap().is_none());
        // The key is reusable after expiry.
        assert!(repo.put(&record("k1", Utc::now())).unwrap());
    }

    #[test]
    fn purge_expired_sweeps_only_old_rows() {
        let repo = repo();
        let now = Utc::now();
        assert!(repo.put(&record("old-1", now - Duration::hours(30))).unwrap());
        assert!(repo.put(&record("old-2", now - Duration::hours(25))).unwrap());
        assert!(repo.put(&record("fresh", now)).unwrap());

        let purged = repo.purge_expired(now - Duration::hours(24)).unwrap();
        assert_eq!(purged, 2);
        assert!(repo.get("tenant-a", "fresh", now - Duration::hours(24)).unwrap().is_some());
    }

    #[test]
    fn purge_expired_includes_exact_cutoff_and_frees_key() {
        let repo = repo();
        let cutoff = DateTime::<Utc>::from_timestamp(1_700_000_000, 0).expect("timestamp");
        assert!(repo.put(&record("boundary-sweep", cutoff)).expect("put"));
        assert_eq!(repo.purge_expired(cutoff).expect("purge"), 1);
        assert!(repo.put(&record("boundary-sweep", Utc::now())).expect("reuse key"));
    }

    #[test]
    fn same_millisecond_cutoff_does_not_expire_later_response() {
        let repo = repo();
        let cutoff = DateTime::<Utc>::from_timestamp(1_700_000_000, 100_000).expect("cutoff");
        let created = DateTime::<Utc>::from_timestamp(1_700_000_000, 900_000).expect("created");
        assert!(repo.put(&record("same-ms-get", created)).expect("put"));
        assert!(repo.put(&record("same-ms-purge", created)).expect("put"));

        let loaded = repo.get("tenant-a", "same-ms-get", cutoff).expect("get").expect("live");
        assert_eq!(loaded.created_at, created);
        assert_eq!(repo.purge_expired(cutoff).expect("purge"), 0);
        assert!(repo.get("tenant-a", "same-ms-purge", cutoff).expect("get").is_some());

        assert!(repo.get("tenant-a", "same-ms-get", created).expect("get").is_none());
        assert_eq!(repo.purge_expired(created).expect("purge"), 1);
    }

    #[test]
    fn legacy_millisecond_row_waits_for_unambiguous_expiry() {
        let repo = repo();
        let created_ms = DateTime::<Utc>::from_timestamp_millis(1_700_000_000_000).expect("ms");
        assert!(repo.put(&record("legacy", created_ms)).expect("put"));
        repo.conn()
            .expect("connection")
            .execute(
                "UPDATE http_idempotency_keys SET created_at_sub_ms_ns = NULL
                 WHERE idempotency_key = 'legacy'",
                [],
            )
            .expect("simulate pre-migration row");

        assert!(repo.get("tenant-a", "legacy", created_ms).expect("get").is_some());
        assert_eq!(repo.purge_expired(created_ms).expect("purge"), 0);
        assert_eq!(repo.purge_expired(created_ms + Duration::milliseconds(1)).expect("purge"), 1);
    }
    #[test]
    fn entry_created_exactly_at_cutoff_is_expired() {
        // Boundary contract with the in-memory cache (`age >= ttl` = expired):
        // an entry whose created_at equals the cutoff must NOT be returned.
        // With a zero TTL (cutoff == now), a same-millisecond write used to
        // survive the strict `<` cleanup and replay a response that the memory
        // path had already declared expired.
        let repo = repo();
        let created = Utc::now();
        assert!(repo.put(&record("boundary", created)).unwrap());

        let hit = repo.get("tenant-a", "boundary", created).unwrap();
        assert!(hit.is_none(), "created_at == cutoff must be expired: {hit:?}");
    }
}
