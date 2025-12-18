#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

#[cfg(feature = "sqlite")]
fn column_names(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .expect("prepare PRAGMA table_info");
    stmt.query_map([], |row| row.get::<_, String>(1))
        .expect("query PRAGMA table_info")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("collect PRAGMA table_info rows")
}

#[cfg(feature = "sqlite")]
fn has_table(conn: &rusqlite::Connection, table: &str) -> bool {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
            [table],
            |row| row.get(0),
        )
        .expect("query sqlite_master");
    count > 0
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_migrations_apply_and_multi_currency_schema_is_present() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let conn = db.conn().expect("get sqlite connection");

    let applied: i64 = conn
        .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
        .expect("count _migrations");
    assert_eq!(applied, 8, "expected all embedded migrations to apply");

    for table in [
        "exchange_rates",
        "store_currency_settings",
        "product_currency_prices",
        "exchange_rate_history",
    ] {
        assert!(has_table(&conn, table), "missing table `{table}`");
    }

    let orders = column_names(&conn, "orders");
    assert!(
        orders.iter().filter(|c| c.as_str() == "currency").count() == 1,
        "`orders.currency` should exist exactly once"
    );
    assert!(orders.contains(&"exchange_rate".to_string()));
    assert!(orders.contains(&"base_currency_total".to_string()));

    let order_items = column_names(&conn, "order_items");
    assert!(order_items.contains(&"currency".to_string()));
    assert!(order_items.contains(&"unit_price_base".to_string()));

    let cart_items = column_names(&conn, "cart_items");
    assert!(cart_items.contains(&"currency".to_string()));

    let defaults: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM store_currency_settings WHERE id = 'default'",
            [],
            |row| row.get(0),
        )
        .expect("query store_currency_settings default row");
    assert_eq!(defaults, 1, "expected a default store_currency_settings row");
}

