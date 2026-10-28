#![cfg(feature = "postgres")]

#[path = "support/erp_ledger_close.rs"]
mod close;

#[test]
fn closing_rolls_back_if_period_transition_fails() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL smoke test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("ledger_close_{}", uuid::Uuid::new_v4().simple());
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
        (db, admin)
    });
    // Repository traits are synchronous and intentionally refuse calls from
    // inside an entered Tokio runtime (including spawn_blocking workers).
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        close::close_is_atomic(&db, |fail| {
            rt.block_on(async {
                if fail {
                    sqlx::query("CREATE FUNCTION fail_close() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'close failed'; END; $$").execute(db.pool()).await.unwrap();
                    sqlx::query("CREATE TRIGGER fail_period_close BEFORE UPDATE ON gl_periods FOR EACH ROW WHEN (NEW.status = 'closed') EXECUTE FUNCTION fail_close()").execute(db.pool()).await.unwrap();
                } else {
                    sqlx::query("DROP TRIGGER fail_period_close ON gl_periods").execute(db.pool()).await.unwrap();
                    sqlx::query("DROP FUNCTION fail_close()").execute(db.pool()).await.unwrap();
                }
            });
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
