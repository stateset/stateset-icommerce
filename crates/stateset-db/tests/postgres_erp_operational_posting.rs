#![cfg(feature = "postgres")]

#[path = "support/erp_operational_posting.rs"]
mod posting;

fn run(test: impl FnOnce(&dyn stateset_db::Database)) {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL smoke test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("operational_posting_{}", uuid::Uuid::new_v4().simple());
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
        test(&db);
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

#[test]
fn cash_refunds() {
    run(posting::refunds);
}
#[test]
fn stock() {
    run(posting::stock);
}

#[test]
fn manufacturing() {
    run(posting::manufacturing);
}

#[test]
fn restock() {
    run(posting::restock);
}

#[test]
fn methods() {
    run(posting::methods);
}

#[test]
fn transit() {
    run(posting::transit);
}

#[test]
fn foreign_stock() {
    run(posting::foreign_stock);
}

#[test]
fn specific_lots() {
    run(posting::specific_lots);
}

#[test]
fn unconfigured_cost_currency() {
    run(posting::unconfigured_cost_currency);
}

#[test]
fn sales_reporting() {
    run(posting::sales_reporting);
}
