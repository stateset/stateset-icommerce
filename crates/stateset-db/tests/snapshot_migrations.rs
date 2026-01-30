//!
//! Snapshot tests for database migrations
//!
//! This module uses `insta` to capture the state of the database schema
//! after each migration runs, ensuring backwards compatibility and detecting
//! unintended schema changes.

use rusqlite::Connection;
use stateset_db::migrations::run_migrations;

fn get_all_tables(conn: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT name, sql FROM sqlite_master 
         WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_%'
         ORDER BY name",
    )?;

    let table_iter = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

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

#[test]
fn snapshot_database_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    let tables = get_all_tables(&conn).expect("Failed to get tables");

    mut_debug::assert_debug_snapshot!(tables);
}

#[test]
fn snapshot_customer_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    let mut stmt = conn
        .prepare("PRAGMA table_info(customers)")
        .expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // name
                row.get::<_, String>(1)?, // type
                row.get::<_, i32>(2)?,    // notnull
                row.get::<_, String>(3)?, // default value
                row.get::<_, i32>(4)?,    // pk
            ))
        })
        .expect("Failed to query customers");

    mut_debug::assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_orders_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    let mut stmt = conn
        .prepare("PRAGMA table_info(orders)")
        .expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })
        .expect("Failed to query orders");

    mut_debug::assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_inventory_items_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    let mut stmt = conn
        .prepare("PRAGMA table_info(inventory_items)")
        .expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })
        .expect("Failed to query inventory_items");

    mut_debug::assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_subscriptions_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    let mut stmt = conn
        .prepare("PRAGMA table_info(subscriptions)")
        .expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })
        .expect("Failed to query subscriptions");

    mut_debug::assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_payments_table_schema() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    let mut stmt = conn
        .prepare("PRAGMA table_info(payments)")
        .expect("Failed to prepare statement");

    let columns = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i32>(4)?,
            ))
        })
        .expect("Failed to query payments");

    mut_debug::assert_debug_snapshot!(columns.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_indexes_for_performance() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    // Check indexes on orders table
    let order_indexes = get_table_indexes(&conn, "orders").expect("Failed to get order indexes");
    mut_debug::assert_debug_snapshot!("orders_indexes", order_indexes);

    // Check indexes on inventory_items table
    let inv_indexes =
        get_table_indexes(&conn, "inventory_items").expect("Failed to get inventory indexes");
    mut_debug::assert_debug_snapshot!("inventory_items_indexes", inv_indexes);
}

#[test]
fn snapshot_migration_versions() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    run_migrations(&conn).expect("Failed to run migrations");

    let mut stmt = conn
        .prepare("SELECT name, applied_at FROM _migrations ORDER BY id")
        .expect("Failed to prepare statement");

    let migrations = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("Failed to query migrations");

    mut_debug::assert_debug_snapshot!(migrations.collect::<Result<Vec<_>, _>>());
}

#[test]
fn snapshot_foreign_key_constraints() {
    let mut conn = Connection::open_in_memory().expect("Failed to create in-memory database");

    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])
        .expect("Failed to enable FKs");

    run_migrations(&conn).expect("Failed to run migrations");

    // Query foreign key information
    let mut stmt = conn
        .prepare(
            "SELECT 
                table_name,
                from_column,
                to_table,
                to_column,
                on_update,
                on_delete
             FROM pragma_foreign_key_list('orders')
             UNION ALL
             SELECT 
                table_name,
                from_column,
                to_table,
                to_column,
                on_update,
                on_delete
             FROM pragma_foreign_key_list('order_items')
             UNION ALL
             SELECT 
                table_name,
                from_column,
                to_table,
                to_column,
                on_update,
                on_delete
             FROM pragma_foreign_key_list('inventory_balances')
             ORDER BY table_name, from_column",
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

    mut_debug::assert_debug_snapshot!(fks.collect::<Result<Vec<_>, _>>());
}
