//! Database migrations
//!
//! Embedded SQL migrations that run automatically on database initialization.

use rusqlite::{Connection, OptionalExtension};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MigrationError {
    #[error("Migration failed: {0}")]
    Failed(String),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
}

/// Run all migrations on the database
pub fn run_migrations(conn: &mut Connection) -> Result<(), MigrationError> {
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
        if name == "027_vector_search" && !cfg!(feature = "vector") {
            continue;
        }
        if name == "028_bm25_search" && !fts5_available(conn) {
            continue;
        }
        // Check if migration already applied
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM _migrations WHERE name = ?",
            [name],
            |row| row.get(0),
        )?;

        if count == 0 {
            // Run migration atomically.
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.execute("INSERT INTO _migrations (name) VALUES (?)", [name])?;
            tx.commit()?;
        }
    }

    Ok(())
}

fn fts5_available(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT sqlite_compileoption_used('ENABLE_FTS5')",
        [],
        |row| row.get::<_, i32>(0),
    )
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0)
        == 1
}

/// Get list of migrations in order
fn get_migrations() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "001_initial_schema",
            include_str!("../migrations/001_initial_schema.sql"),
        ),
        (
            "002_inventory",
            include_str!("../migrations/002_inventory.sql"),
        ),
        ("003_returns", include_str!("../migrations/003_returns.sql")),
        (
            "004_manufacturing",
            include_str!("../migrations/004_manufacturing.sql"),
        ),
        (
            "005_shipments",
            include_str!("../migrations/005_shipments.sql"),
        ),
        (
            "006_payments_warranties_po_invoices",
            include_str!("../migrations/006_payments_warranties_po_invoices.sql"),
        ),
        ("007_carts", include_str!("../migrations/007_carts.sql")),
        (
            "008_multi_currency",
            include_str!("../migrations/008_multi_currency.sql"),
        ),
        ("009_tax", include_str!("../migrations/009_tax.sql")),
        (
            "010_promotions",
            include_str!("../migrations/010_promotions.sql"),
        ),
        (
            "011_subscriptions",
            include_str!("../migrations/011_subscriptions.sql"),
        ),
        (
            "012_versioning",
            include_str!("../migrations/012_versioning.sql"),
        ),
        ("013_quality", include_str!("../migrations/013_quality.sql")),
        ("014_lots", include_str!("../migrations/014_lots.sql")),
        ("015_serials", include_str!("../migrations/015_serials.sql")),
        (
            "016_warehouse",
            include_str!("../migrations/016_warehouse.sql"),
        ),
        (
            "017_receiving",
            include_str!("../migrations/017_receiving.sql"),
        ),
        (
            "018_fulfillment",
            include_str!("../migrations/018_fulfillment.sql"),
        ),
        (
            "019_accounts_payable",
            include_str!("../migrations/019_accounts_payable.sql"),
        ),
        (
            "020_cost_accounting",
            include_str!("../migrations/020_cost_accounting.sql"),
        ),
        ("021_credit", include_str!("../migrations/021_credit.sql")),
        (
            "022_backorder",
            include_str!("../migrations/022_backorder.sql"),
        ),
        (
            "023_accounts_receivable",
            include_str!("../migrations/023_accounts_receivable.sql"),
        ),
        (
            "024_general_ledger",
            include_str!("../migrations/024_general_ledger.sql"),
        ),
        (
            "025_performance_indexes",
            include_str!("../migrations/025_performance_indexes.sql"),
        ),
        (
            "026_idempotency_keys",
            include_str!("../migrations/026_idempotency_keys.sql"),
        ),
        // Vector search migration (requires sqlite-vec extension to be loaded first)
        (
            "027_vector_search",
            include_str!("../migrations/027_vector_search.sql"),
        ),
        // Full-text search migration (requires FTS5)
        (
            "028_bm25_search",
            include_str!("../migrations/028_bm25_search.sql"),
        ),
        // x402 Payment Intents and Agent Cards for A2A Commerce
        (
            "029_x402_a2a",
            include_str!("../migrations/029_x402_a2a.sql"),
        ),
        // x402 Credit Ledger for metered billing
        (
            "030_x402_credits",
            include_str!("../migrations/030_x402_credits.sql"),
        ),
        // ERC-8004 Trustless Agents registries
        ("031_erc8004", include_str!("../migrations/031_erc8004.sql")),
        // Custom objects (custom states / metaobjects)
        (
            "032_custom_objects",
            include_str!("../migrations/032_custom_objects.sql"),
        ),
        // Orders <-> carts linkage for checkout idempotency (retry safety)
        (
            "033_orders_cart_id",
            include_str!("../migrations/033_orders_cart_id.sql"),
        ),
    ]
}
