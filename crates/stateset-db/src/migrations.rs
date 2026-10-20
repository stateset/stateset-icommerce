//! Canonical SQLite migrations, shared with the standalone migration API.

pub use stateset_migrations::engine::{
    MigrationError, known_migration_names, latest_known_migration,
};

/// Run the canonical engine migrations. The mutable parameter is retained for
/// source compatibility with the original database API.
#[allow(clippy::needless_pass_by_ref_mut)]
pub fn run_migrations(conn: &mut rusqlite::Connection) -> Result<(), MigrationError> {
    stateset_migrations::engine::run_migrations(conn)
}
