#[cfg(feature = "postgres")]
use sqlx::postgres::PgPoolOptions;
#[cfg(feature = "postgres")]
use stateset_db::PostgresDatabase;
#[cfg(feature = "postgres")]
use std::env;

#[cfg(feature = "postgres")]
fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

#[cfg(feature = "postgres")]
#[tokio::test]
async fn postgres_migrations_apply_and_currency_schema_is_present() {
    let url = match postgres_url() {
        Some(url) => url,
        None => {
            eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres migration test");
            return;
        }
    };

    PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect to postgres for verification");

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _migrations")
        .fetch_one(&pool)
        .await
        .expect("count _migrations");
    let expected = if cfg!(feature = "saga") { 58 } else { 57 };
    assert_eq!(applied, expected, "expected all embedded migrations to apply");

    let mut tables = vec![
        "exchange_rates",
        "store_currency_settings",
        "exchange_rate_history",
        "x402_credit_accounts",
        "x402_credit_transactions",
        "x402_payment_intents",
        "agent_cards",
        "a2a_quotes",
        "a2a_purchases",
        "agent_identities",
        "agent_identity_metadata",
        "agent_feedback",
        "agent_feedback_responses",
        "agent_validation_requests",
        "agent_validation_responses",
        "custom_object_types",
        "custom_object_records",
    ];
    if cfg!(feature = "saga") {
        tables.extend(["sagas", "saga_steps"]);
    }

    for table in tables {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public' AND table_name = $1",
        )
        .bind(table)
        .fetch_one(&pool)
        .await
        .expect("query information_schema.tables");
        assert!(count > 0, "missing table `{table}`");
    }

    let currency_cols: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'orders' AND column_name = 'currency'",
    )
    .fetch_one(&pool)
    .await
    .expect("query orders.currency");
    assert_eq!(currency_cols, 1, "`orders.currency` should exist exactly once");

    let cart_cols: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema = 'public' AND table_name = 'orders' AND column_name = 'cart_id'",
    )
    .fetch_one(&pool)
    .await
    .expect("query orders.cart_id");
    assert_eq!(cart_cols, 1, "`orders.cart_id` should exist exactly once");

    let defaults: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM store_currency_settings WHERE id = 'default'")
            .fetch_one(&pool)
            .await
            .expect("query store_currency_settings default row");
    assert_eq!(defaults, 1, "expected a default store_currency_settings row");
}
