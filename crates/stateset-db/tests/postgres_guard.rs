#![cfg(feature = "postgres")]

use std::env;

fn has_postgres_url() -> bool {
    env::var("POSTGRES_URL").ok().is_some() || env::var("DATABASE_URL").ok().is_some()
}

fn allow_postgres_skip() -> bool {
    matches!(
        env::var("STATESET_ALLOW_POSTGRES_SKIP").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

#[test]
fn postgres_suite_requires_database_url_or_explicit_opt_out() {
    assert!(
        has_postgres_url() || allow_postgres_skip(),
        "Postgres tests require POSTGRES_URL or DATABASE_URL when the `postgres` feature is enabled. \
Set STATESET_ALLOW_POSTGRES_SKIP=1 to explicitly opt out."
    );
}
