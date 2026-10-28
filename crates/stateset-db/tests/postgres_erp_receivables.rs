#![cfg(feature = "postgres")]

#[path = "support/erp_receivables.rs"]
mod receivables;

fn run(test: impl FnOnce(&dyn stateset_db::Database)) {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL smoke test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("receivables_{}", uuid::Uuid::new_v4().simple());
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
fn funding_guards() {
    run(receivables::funding_guards);
}
#[test]
fn partial_writeoffs() {
    run(receivables::partial_writeoffs);
}
#[test]
fn terminal_unapply() {
    run(receivables::terminal_unapply);
}
#[test]
fn allocation_race() {
    run(receivables::allocation_race);
}
#[test]
fn refund_allocation_race() {
    run(receivables::refund_allocation_race);
}

#[test]
fn draft_edits() {
    run(receivables::draft_edits);
}

#[test]
fn issue_edit_race() {
    run(receivables::issue_edit_race);
}

#[test]
fn statements() {
    run(receivables::statements);
}

#[test]
fn invoice_writeoff() {
    run(receivables::invoice_writeoff);
}

#[test]
fn credit_filters() {
    run(receivables::credit_filters);
}
