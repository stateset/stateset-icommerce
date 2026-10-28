#![cfg(feature = "postgres")]

#[path = "support/erp_ledger_sources.rs"]
mod sources;

#[test]
fn source_lifecycle() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL smoke test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("ledger_sources_{}", uuid::Uuid::new_v4().simple());
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
        sources::source_lifecycle(&db);
        rt.block_on(source_posting_race(&db));
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

async fn source_posting_race(db: &stateset_db::PostgresDatabase) {
    use rust_decimal_macros::dec;
    use stateset_core::*;
    let customer = db
        .customers()
        .create_async(CreateCustomer {
            email: "source-race@example.com".into(),
            first_name: "Source".into(),
            last_name: "Race".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let invoice = db
        .invoices()
        .create_async(CreateInvoice {
            customer_id: customer.id,
            items: vec![CreateInvoiceItem {
                description: "Race".into(),
                quantity: dec!(1),
                unit_price: dec!(100),
                ..Default::default()
            }],
            ..Default::default()
        })
        .await
        .unwrap();
    db.invoices().send_async(invoice.id.into()).await.unwrap();
    let mut voiding = db.pool().begin().await.unwrap();
    let pid: i32 =
        sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(voiding.as_mut()).await.unwrap();
    sqlx::query("SELECT id FROM invoices WHERE id = $1 FOR UPDATE")
        .bind(invoice.id.into_uuid())
        .execute(voiding.as_mut())
        .await
        .unwrap();
    let gl = db.general_ledger();
    let worker = tokio::spawn(async move { gl.auto_post_invoice_async(invoice.id.into()).await });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))",
        )
        .bind(pid)
        .fetch_one(db.pool())
        .await
        .unwrap();
        if blocked {
            break;
        }
        assert!(!worker.is_finished(), "Posting bypassed the source document lock");
        assert!(std::time::Instant::now() < deadline, "Posting never reached the source lock");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    sqlx::query("UPDATE invoices SET status = 'voided' WHERE id = $1")
        .bind(invoice.id.into_uuid())
        .execute(voiding.as_mut())
        .await
        .unwrap();
    voiding.commit().await.unwrap();
    assert!(worker.await.unwrap().is_err());
    assert!(
        db.general_ledger()
            .list_journal_entries_async(JournalEntryFilter {
                source_document_type: Some("invoice".into()),
                source_document_id: Some(invoice.id.into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .is_empty()
    );
}
