//! Shared helpers for the Postgres integration tests.
//!
//! Every Postgres test in this crate runs against the same database, so tests
//! that mutate rows they do not own need explicit coordination.

#![cfg(feature = "postgres")]
#![allow(dead_code)]

use sqlx::Connection;
use sqlx::postgres::PgConnection;

/// Advisory lock key guarding [`x402 sweeper`](sweeper_exclusive) access.
/// Arbitrary but must be stable across test binaries.
const X402_SWEEPER_LOCK: i64 = 0x7834_3032;

/// Coordinates the global x402 expiry sweeper across tests.
///
/// `expire_stale_intents_async` is a global sweep: it expires *every* intent in
/// the database whose validity window has closed, not just the caller's. Tests
/// share one database, run concurrently within a binary, and each binary is its
/// own process — so an in-process lock cannot serialise them. This guard takes a
/// Postgres advisory lock, which is visible to every session on the server.
///
/// Held on a dedicated connection that is closed on drop, so the lock is
/// released even if the test panics while holding it.
pub(crate) struct SweeperGuard(Option<PgConnection>);

impl Drop for SweeperGuard {
    fn drop(&mut self) {
        // Dropping the connection ends its Postgres session, which releases any
        // advisory lock it holds. No explicit unlock needed (and none possible
        // here, since Drop cannot await).
        drop(self.0.take());
    }
}

async fn lock(url: &str, sql: &str) -> SweeperGuard {
    let mut conn = PgConnection::connect(url).await.expect("connect for sweeper lock");
    sqlx::query(sql).bind(X402_SWEEPER_LOCK).execute(&mut conn).await.expect("take sweeper lock");
    SweeperGuard(Some(conn))
}

/// Take the sweeper lock exclusively. Required before calling
/// `expire_stale_intents_async`, which expires other tests' intents too.
pub(crate) async fn sweeper_exclusive(url: &str) -> SweeperGuard {
    lock(url, "SELECT pg_advisory_lock($1)").await
}

/// Take the sweeper lock in shared mode. Required by any test that leaves an
/// intent past its validity window while asserting on that intent's status,
/// so a concurrent sweep cannot expire it mid-assertion.
pub(crate) async fn sweeper_shared(url: &str) -> SweeperGuard {
    lock(url, "SELECT pg_advisory_lock_shared($1)").await
}
