//! Locating the Postgres database that backend-parity tests run against.

/// Environment variable that lets Postgres tests skip when no URL is set.
///
/// This is the same opt-out the `stateset-db` `postgres_guard` test honours.
pub const ALLOW_POSTGRES_SKIP_ENV: &str = "STATESET_ALLOW_POSTGRES_SKIP";

/// The Postgres URL for backend tests: `POSTGRES_URL`, else `DATABASE_URL`.
///
/// Returns `None` (the caller skips) only when [`ALLOW_POSTGRES_SKIP_ENV`] is
/// set to `1`/`true`/`yes`. Otherwise a missing URL panics, so a gate run
/// with the `postgres` feature but no database fails instead of reporting
/// green without running a single Postgres assertion.
///
/// # Panics
///
/// When neither URL variable is set and skipping was not explicitly allowed.
#[must_use]
pub fn postgres_url() -> Option<String> {
    let url = std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok());
    let skip_allowed = matches!(
        std::env::var(ALLOW_POSTGRES_SKIP_ENV).ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    );
    assert!(
        url.is_some() || skip_allowed,
        "Postgres tests require POSTGRES_URL or DATABASE_URL; set \
         {ALLOW_POSTGRES_SKIP_ENV}=1 to skip them explicitly"
    );
    url
}
