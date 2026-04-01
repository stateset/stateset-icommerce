#![cfg(feature = "sqlite")]
//!
//!
//! Snapshot tests for database migrations
//!
//! This module uses `insta` to capture the state of the database schema
//! after each migration runs, ensuring backwards compatibility and detecting
//! unintended schema changes.

use insta::assert_debug_snapshot;
use rusqlite::{Connection, OptionalExtension};
use stateset_db::migrations::run_migrations;

fn get_all_tables(conn: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT name, sql FROM sqlite_master 
         WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_%'
         ORDER BY name",
    )?;

    let table_iter =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;

    table_iter.collect()
}

fn get_table_indexes(conn: &Connection, table_name: &str) -> Result<Vec<String>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT sql FROM sqlite_master 
         WHERE type='index' AND tbl_name = ? AND sql IS NOT NULL
         ORDER BY name",
    )?;

    let index_iter = stmt.query_map([table_name], |row| row.get::<_, String>(0))?;
    index_iter.collect()
}

fn fts5_available(conn: &Connection) -> bool {
    conn.query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |row| {
        row.get::<_, i32>(0)
    })
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0)
        == 1
}

fn expected_applied_migration_names(conn: &Connection) -> Vec<String> {
    let mut names = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations"))
        .expect("Failed to list migrations directory")
        .map(|entry| entry.expect("Failed to read migrations directory entry"))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".sql"))
        .map(|name| {
            name.strip_suffix(".sql").expect("Migration file should end with .sql").to_string()
        })
        .collect::<Vec<_>>();

    names.sort();
    names.retain(|name| name != "027_vector_search" || cfg!(feature = "vector"));
    names.retain(|name| name != "028_bm25_search" || fts5_available(conn));
    names
}

#[test]
fn snapshot_database_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    let tables = get_all_tables(&conn).expect("Failed to get tables");

    assert_debug_snapshot!(tables);
}

#[test]
fn snapshot_customer_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    let mut stmt =
        conn.prepare("PRAGMA table_info(customers)").expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,            // cid
                row.get::<_, String>(1)?,         // name
                row.get::<_, String>(2)?,         // type
                row.get::<_, i32>(3)?,            // notnull
                row.get::<_, Option<String>>(4)?, // default value (nullable)
                row.get::<_, i32>(5)?,            // pk
            ))
        })
        .expect("Failed to query customers");

    assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_orders_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    let mut stmt = conn.prepare("PRAGMA table_info(orders)").expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,            // cid
                row.get::<_, String>(1)?,         // name
                row.get::<_, String>(2)?,         // type
                row.get::<_, i32>(3)?,            // notnull
                row.get::<_, Option<String>>(4)?, // default value (nullable)
                row.get::<_, i32>(5)?,            // pk
            ))
        })
        .expect("Failed to query orders");

    assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_inventory_items_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    let mut stmt =
        conn.prepare("PRAGMA table_info(inventory_items)").expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,            // cid
                row.get::<_, String>(1)?,         // name
                row.get::<_, String>(2)?,         // type
                row.get::<_, i32>(3)?,            // notnull
                row.get::<_, Option<String>>(4)?, // default value (nullable)
                row.get::<_, i32>(5)?,            // pk
            ))
        })
        .expect("Failed to query inventory_items");

    assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_subscriptions_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    let mut stmt =
        conn.prepare("PRAGMA table_info(subscriptions)").expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,            // cid
                row.get::<_, String>(1)?,         // name
                row.get::<_, String>(2)?,         // type
                row.get::<_, i32>(3)?,            // notnull
                row.get::<_, Option<String>>(4)?, // default value (nullable)
                row.get::<_, i32>(5)?,            // pk
            ))
        })
        .expect("Failed to query subscriptions");

    assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_payments_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    let mut stmt =
        conn.prepare("PRAGMA table_info(payments)").expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i32>(0)?,            // cid
                row.get::<_, String>(1)?,         // name
                row.get::<_, String>(2)?,         // type
                row.get::<_, i32>(3)?,            // notnull
                row.get::<_, Option<String>>(4)?, // default value (nullable)
                row.get::<_, i32>(5)?,            // pk
            ))
        })
        .expect("Failed to query payments");

    assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_indexes_for_performance() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    // Check indexes on orders table
    let order_indexes = get_table_indexes(&conn, "orders").expect("Failed to get order indexes");
    assert_debug_snapshot!("orders_indexes", order_indexes);

    // Check indexes on inventory_items table
    let inv_indexes =
        get_table_indexes(&conn, "inventory_items").expect("Failed to get inventory indexes");
    assert_debug_snapshot!("inventory_items_indexes", inv_indexes);
}

#[test]
fn snapshot_migration_versions() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&mut conn).expect("Failed to run migrations");

    // Only capture migration names (not timestamps which change between runs)
    let mut stmt = conn
        .prepare("SELECT name FROM _migrations ORDER BY id")
        .expect("Failed to prepare statement");

    let migrations =
        stmt.query_map([], |row| row.get::<_, String>(0)).expect("Failed to query migrations");
    let applied_migrations = migrations.collect::<Result<Vec<_>, _>>().expect("Collect migrations");

    assert_eq!(applied_migrations, expected_applied_migration_names(&conn));
}

#[test]
fn snapshot_foreign_key_constraints() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", []).expect("Failed to enable FKs");

    run_migrations(&mut conn).expect("Failed to run migrations");

    // Query foreign key information
    // pragma_foreign_key_list returns: id, seq, table, from, to, on_update, on_delete, match
    let mut stmt = conn
        .prepare(
            "SELECT
                'orders' as src_table,
                \"table\" as ref_table,
                \"from\" as from_col,
                \"to\" as to_col,
                on_update,
                on_delete
             FROM pragma_foreign_key_list('orders')
             UNION ALL
             SELECT
                'order_items' as src_table,
                \"table\" as ref_table,
                \"from\" as from_col,
                \"to\" as to_col,
                on_update,
                on_delete
             FROM pragma_foreign_key_list('order_items')
             UNION ALL
             SELECT
                'inventory_balances' as src_table,
                \"table\" as ref_table,
                \"from\" as from_col,
                \"to\" as to_col,
                on_update,
                on_delete
             FROM pragma_foreign_key_list('inventory_balances')
             ORDER BY src_table, from_col",
        )
        .expect("Failed to prepare statement");

    let fks = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .expect("Failed to query FKS");

    assert_debug_snapshot!(fks.collect::<Result<Vec<_>, _>>());
}
