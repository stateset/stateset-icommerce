#[cfg(feature = "sqlite")]
use rusqlite::OptionalExtension;
#[cfg(feature = "sqlite")]
use stateset_db::SqliteDatabase;

#[cfg(feature = "sqlite")]
fn column_names(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    let mut stmt =
        conn.prepare(&format!("PRAGMA table_info({table})")).expect("prepare PRAGMA table_info");
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
fn fts5_available(conn: &rusqlite::Connection) -> bool {
    conn.query_row("SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |row| {
        row.get::<_, i32>(0)
    })
    .optional()
    .ok()
    .flatten()
    .unwrap_or(0)
        == 1
}

#[cfg(feature = "sqlite")]
#[test]
fn sqlite_migrations_apply_and_multi_currency_schema_is_present() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let conn = db.conn().expect("get sqlite connection");

    let applied: i64 = conn
        .query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))
        .expect("count _migrations");
    // Base: 35 migrations (001-035)
    // 027_vector_search is skipped without vector feature
    // 028_bm25_search is skipped without FTS5
    let expected = 35
        - if cfg!(feature = "vector") { 0 } else { 1 }
        - if fts5_available(&conn) { 0 } else { 1 };
    assert_eq!(applied, expected, "expected all embedded migrations to apply");

    for table in [
        "exchange_rates",
        "store_currency_settings",
        "product_currency_prices",
        "exchange_rate_history",
    ] {
        assert!(has_table(&conn, table), "missing table `{table}`");
    }

    if cfg!(feature = "vector") {
        for table in [
            "product_embeddings",
            "customer_embeddings",
            "order_embeddings",
            "inventory_embeddings",
            "embedding_metadata",
        ] {
            assert!(has_table(&conn, table), "missing table `{table}`");
        }
        if fts5_available(&conn) {
            for table in ["product_fts", "customer_fts", "order_fts", "inventory_fts"] {
                assert!(has_table(&conn, table), "missing table `{table}`");
            }
        }
    }

    let orders = column_names(&conn, "orders");
    assert!(
        orders.iter().filter(|c| c.as_str() == "currency").count() == 1,
        "`orders.currency` should exist exactly once"
    );
    assert!(orders.contains(&"exchange_rate".to_string()));
    assert!(orders.contains(&"base_currency_total".to_string()));
    assert!(orders.contains(&"cart_id".to_string()));

    let order_items = column_names(&conn, "order_items");
    assert!(order_items.contains(&"currency".to_string()));
    assert!(order_items.contains(&"unit_price_base".to_string()));

    let cart_items = column_names(&conn, "cart_items");
    assert!(cart_items.contains(&"currency".to_string()));

    let defaults: i64 = conn
        .query_row("SELECT COUNT(*) FROM store_currency_settings WHERE id = 'default'", [], |row| {
            row.get(0)
        })
        .expect("query store_currency_settings default row");
    assert_eq!(defaults, 1, "expected a default store_currency_settings row");

    for table in [
        "x402_payment_intents",
        "agent_cards",
        "a2a_quotes",
        "a2a_purchases",
        "x402_credit_accounts",
        "x402_credit_transactions",
    ] {
        assert!(has_table(&conn, table), "missing table `{table}`");
    }

    for table in [
        "agent_identities",
        "agent_identity_metadata",
        "agent_feedback",
        "agent_feedback_responses",
        "agent_validation_requests",
        "agent_validation_responses",
    ] {
        assert!(has_table(&conn, table), "missing table `{table}`");
    }
}
