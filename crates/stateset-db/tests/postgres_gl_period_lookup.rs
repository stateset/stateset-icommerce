#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use stateset_core::{CreateGlPeriod, GeneralLedgerRepository};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

async fn connect() -> Option<PostgresDatabase> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres GL period lookup test");
        return None;
    };
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

#[tokio::test]
async fn postgres_get_period_for_date_prefers_open_when_overlapping() {
    let Some(db) = connect().await else { return };
    let gl = db.general_ledger();

    // Unique suffix to avoid cross-test collisions on shared DBs.
    let suffix = Uuid::new_v4().to_string();

    let p1 = gl
        .create_period_async(CreateGlPeriod {
            period_name: format!("2026-07-A-{suffix}"),
            fiscal_year: 2026,
            period_number: 7,
            start_date: NaiveDate::from_ymd_opt(2026, 7, 1).expect("date"),
            end_date: NaiveDate::from_ymd_opt(2026, 7, 31).expect("date"),
        })
        .await
        .expect("create p1");
    let p2 = gl
        .create_period_async(CreateGlPeriod {
            period_name: format!("2026-07-B-{suffix}"),
            fiscal_year: 2026,
            period_number: 70,
            start_date: NaiveDate::from_ymd_opt(2026, 7, 10).expect("date"),
            end_date: NaiveDate::from_ymd_opt(2026, 7, 20).expect("date"),
        })
        .await
        .expect("create p2");

    let p1 = gl.open_period_async(p1.id).await.expect("open p1");
    let p2 = gl.open_period_async(p2.id).await.expect("open p2");
    let _ = gl.close_period_async(p2.id, "tester").await.expect("close p2");

    let date = NaiveDate::from_ymd_opt(2026, 7, 15).expect("date");
    let selected = gl.get_period_for_date_async(date).await.expect("lookup").expect("some");
    assert_eq!(selected.id, p1.id, "must select the open period covering the date");
    assert!(selected.can_post(), "selected period is open");
}

