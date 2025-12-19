//! Database migrations
//!
//! Embedded SQL migrations that run automatically on database initialization.

use rusqlite::Connection;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("Migration failed: {0}")]
    Failed(String),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}

/// Run all migrations on the database
pub fn run_migrations(conn: &Connection) -> Result<(), MigrationError> {
    // Create migrations table if not exists
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _migrations (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    let migrations = get_migrations();

    for (name, sql) in migrations {
        // Check if migration already applied
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM _migrations WHERE name = ?",
            [name],
            |row| row.get(0),
        )?;

        if count == 0 {
            // Run migration
            conn.execute_batch(sql)?;

            // Record migration
            conn.execute(
                "INSERT INTO _migrations (name) VALUES (?)",
                [name],
            )?;
        }
    }

    Ok(())
}

/// Get list of migrations in order
fn get_migrations() -> Vec<(&'static str, &'static str)> {
    vec![
        ("001_initial_schema", include_str!("../migrations/001_initial_schema.sql")),
        ("002_inventory", include_str!("../migrations/002_inventory.sql")),
        ("003_returns", include_str!("../migrations/003_returns.sql")),
        ("004_manufacturing", include_str!("../migrations/004_manufacturing.sql")),
        ("005_shipments", include_str!("../migrations/005_shipments.sql")),
        ("006_payments_warranties_po_invoices", include_str!("../migrations/006_payments_warranties_po_invoices.sql")),
        ("007_carts", include_str!("../migrations/007_carts.sql")),
        ("008_multi_currency", include_str!("../migrations/008_multi_currency.sql")),
        ("009_tax", include_str!("../migrations/009_tax.sql")),
        ("010_promotions", include_str!("../migrations/010_promotions.sql")),
        ("011_subscriptions", include_str!("../migrations/011_subscriptions.sql")),
    ]
}
