#![cfg(feature = "postgres")]

#[path = "support/erp_asset_accounting.rs"]
mod assets;

#[test]
fn asset_accounting() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL smoke test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("asset_accounting_{}", uuid::Uuid::new_v4().simple());
    let (db, admin) = rt.block_on(async {
        let admin = sqlx::PgPool::connect(&url).await.expect("connect to test database");
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .expect("create isolated schema");
        let separator = if url.contains('?') { '&' } else { '?' };
        let scoped_url = format!("{url}{separator}options=-csearch_path%3D{schema}%2Cpublic");
        let db = stateset_db::PostgresDatabase::connect(scoped_url)
            .await
            .expect("migrate fresh PostgreSQL store");
        for (date,rate) in [("2025-01-01","1.2"),("2025-04-01","1.5")] {
            sqlx::query("INSERT INTO exchange_rate_history (id,base_currency,quote_currency,rate,source,rate_at) VALUES ($1,'EUR','USD',$2,'test',$3)").bind(uuid::Uuid::new_v4()).bind(rate.parse::<rust_decimal::Decimal>().unwrap()).bind(format!("{date}T00:00:00Z").parse::<chrono::DateTime<chrono::Utc>>().unwrap()).execute(db.pool()).await.unwrap();
        }
        (db, admin)
    });
    // Repository traits are synchronous and intentionally refuse calls from
    // inside an entered Tokio runtime (including spawn_blocking workers).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assets::asset_accounting(&db);
        rt.block_on(async {
            let revisions: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM fixed_asset_schedule_revisions")
                    .fetch_one(db.pool())
                    .await
                    .unwrap();
            assert_eq!(revisions, 1);
        });
    }));
    rt.block_on(async {
        db.pool().close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .expect("remove test schema");
    });
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
