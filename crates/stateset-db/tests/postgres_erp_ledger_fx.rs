#![cfg(feature = "postgres")]

#[path = "support/erp_ledger_fx.rs"]
mod fx;

#[test]
fn historical_fx_posting_and_revaluation() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL smoke test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("ledger_fx_{}", uuid::Uuid::new_v4().simple());
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
for (currency, rate, date) in [("EUR","1.2","2026-01-01"),("EUR","1.3","2026-02-01"),("EUR","1.8","2026-03-01"),("EUR","2","2026-04-01"),("GBP","1.5","2026-05-01")] {
            sqlx::query("INSERT INTO exchange_rate_history (id,base_currency,quote_currency,rate,source,rate_at) VALUES ($1,$2,'USD',$3,'test',$4)").bind(uuid::Uuid::new_v4()).bind(currency).bind(rate.parse::<rust_decimal::Decimal>().unwrap()).bind(format!("{date}T00:00:00Z").parse::<chrono::DateTime<chrono::Utc>>().unwrap()).execute(db.pool()).await.unwrap();
        }
        (db, admin)
    });
    // Repository traits are synchronous and intentionally refuse calls from
    // inside an entered Tokio runtime (including spawn_blocking workers).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fx::ledger_fx(&db, |rate| {
            rt.block_on(async { sqlx::query("UPDATE exchange_rate_history SET rate=$1 WHERE base_currency='EUR' AND rate_at::date='2026-02-01'").bind(rate.parse::<rust_decimal::Decimal>().unwrap()).execute(db.pool()).await.unwrap(); });
        })
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
