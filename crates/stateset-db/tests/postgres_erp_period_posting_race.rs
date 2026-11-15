#![cfg(feature = "postgres")]

#[test]
fn posting_and_voiding_wait_for_period_closure() {
    let Ok(url) = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping PostgreSQL smoke test");
        return;
    };
    let rt = tokio::runtime::Runtime::new().unwrap();
    let schema = format!("period_posting_race_{}", uuid::Uuid::new_v4().simple());
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
        rt.block_on(check_period_locks(&db))
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

async fn check_period_locks(db: &stateset_db::PostgresDatabase) {
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use stateset_core::*;
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts_async().await.unwrap();
    let cash = gl.get_account_by_number_async("1010").await.unwrap().unwrap().id;
    let revenue = gl.get_account_by_number_async("4010").await.unwrap().unwrap().id;
    for (month, action) in [(1, "post"), (2, "void"), (3, "auto")] {
        let start = NaiveDate::from_ymd_opt(2026, month, 1).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, month + 1, 1).unwrap().pred_opt().unwrap();
        let period = gl
            .create_period_async(CreateGlPeriod {
                period_name: action.into(),
                fiscal_year: 2026,
                period_number: month as i32,
                start_date: start,
                end_date: end,
            })
            .await
            .unwrap();
        gl.open_period_async(period.id).await.unwrap();
        let input = CreateJournalEntry {
            entry_date: start,
            entry_type: None,
            description: action.into(),
            lines: vec![
                CreateJournalEntryLine::debit(cash, dec!(100), None),
                CreateJournalEntryLine::credit(revenue, dec!(100), None),
            ],
            source_document_type: None,
            source_document_id: None,
            auto_post: Some(action == "void"),
            fx: None,
        };
        let entry = gl.create_journal_entry_async(input.clone()).await.unwrap();
        let before = gl.get_account_async(cash).await.unwrap().unwrap().current_balance;
        let mut closing = db.pool().begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(closing.as_mut())
            .await
            .unwrap();
        sqlx::query("SELECT id FROM gl_periods WHERE id = $1 FOR UPDATE")
            .bind(period.id)
            .execute(closing.as_mut())
            .await
            .unwrap();
        let worker_gl = gl.clone();
        let worker = tokio::spawn(async move {
            match action {
                "post" => worker_gl.post_journal_entry_async(entry.id, "racer").await,
                "void" => worker_gl.void_journal_entry_async(entry.id).await,
                _ => {
                    worker_gl
                        .create_journal_entry_async(CreateJournalEntry {
                            auto_post: Some(true),
                            ..input
                        })
                        .await
                }
            }
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))").bind(pid).fetch_one(db.pool()).await.unwrap();
            if blocked {
                break;
            }
            assert!(!worker.is_finished(), "{action} bypassed the period lock");
            assert!(std::time::Instant::now() < deadline, "{action} never reached the period lock");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        sqlx::query("UPDATE gl_periods SET status = 'closed' WHERE id = $1")
            .bind(period.id)
            .execute(closing.as_mut())
            .await
            .unwrap();
        closing.commit().await.unwrap();
        assert!(worker.await.unwrap().is_err(), "{action} must refuse after the period closes");
        assert_eq!(gl.get_account_async(cash).await.unwrap().unwrap().current_balance, before);
        assert_eq!(
            gl.list_journal_entries_async(JournalEntryFilter {
                period_id: Some(period.id),
                ..Default::default()
            })
            .await
            .unwrap()
            .len(),
            1,
            "Failed auto-post must not leave a draft"
        );
    }
    let first = CreateGlPeriod {
        period_name: "April A".into(),
        fiscal_year: 2026,
        period_number: 4,
        start_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
        end_date: NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
    };
    let second =
        CreateGlPeriod { period_name: "April B".into(), period_number: 44, ..first.clone() };
    let (a, b) = tokio::join!(gl.create_period_async(first), gl.create_period_async(second));
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "Concurrent overlap checks must serialize"
    );
}
