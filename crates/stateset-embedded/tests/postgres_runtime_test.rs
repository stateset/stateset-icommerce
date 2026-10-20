//! PostgreSQL runtime-lifetime regression tests.
//!
//! A `sqlx` pool is bound to the runtime that was current when it was
//! created: once that runtime shuts down, every later acquire hangs until
//! the acquire timeout — even from other live runtimes. `Commerce` sync
//! constructors must therefore create the pool on a process-shared immortal
//! runtime; connecting on a throwaway runtime bricked the pool on return
//! (every later sync query hung 30s and failed with a pool timeout).
//!
//! Requires the `postgres` feature and a live database (`POSTGRES_URL` or
//! `DATABASE_URL`); without one the tests print a notice and pass.

#![cfg(feature = "postgres")]

use stateset_embedded::Commerce;

fn pg_url() -> Option<String> {
    let url = std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok());
    if url.is_none() {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres runtime test");
    }
    url
}

/// The regression: a pool built by `Commerce::with_postgres` (which connects
/// off any ambient runtime) must serve sync queries afterwards instead of
/// hanging on pool acquire.
#[test]
fn sync_queries_work_after_with_postgres_connect() {
    let Some(url) = pg_url() else { return };
    let commerce = Commerce::with_postgres(&url).expect("connect");
    let accounts = commerce
        .general_ledger()
        .list_accounts(Default::default())
        .expect("list accounts after connect");
    eprintln!("SYNC-AFTER-CONNECT: {} accounts", accounts.len());
}

/// Companion: the async path (pool and queries on one living runtime) keeps
/// working.
#[tokio::test]
async fn async_context_queries_work() {
    let Some(url) = pg_url() else { return };
    let commerce = stateset_embedded::AsyncCommerce::connect(&url).await.expect("connect");
    let accounts =
        commerce.general_ledger().list_accounts(Default::default()).await.expect("list accounts");
    eprintln!("ASYNC-CTX: {} accounts", accounts.len());
}
