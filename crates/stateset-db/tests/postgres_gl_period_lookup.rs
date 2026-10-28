#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use stateset_core::{CommerceError, CreateGlPeriod};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    stateset_test_utils::postgres_url()
}

async fn connect() -> Option<PostgresDatabase> {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping postgres GL period lookup test");
        return None;
    };
    Some(PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations"))
}

#[tokio::test]
async fn postgres_overlapping_periods_are_refused_and_lookup_finds_the_covering_period() {
    let Some(db) = connect().await else { return };
    let gl = db.general_ledger();

    // Unique suffix to avoid cross-test collisions on shared DBs.
    let id = Uuid::new_v4();
    let suffix = id.to_string();
    // Periods may not overlap anywhere, so a fresh fiscal year per run keeps a
    // reused database from colliding with an earlier run.
    let fiscal_year =
        2300 + i32::from(u16::from_be_bytes([id.as_bytes()[0], id.as_bytes()[1]]) % 7000);

    let p1 = gl
        .create_period_async(CreateGlPeriod {
            period_name: format!("{fiscal_year}-07-A-{suffix}"),
            fiscal_year,
            period_number: 7,
            start_date: NaiveDate::from_ymd_opt(fiscal_year, 7, 1).expect("date"),
            end_date: NaiveDate::from_ymd_opt(fiscal_year, 7, 31).expect("date"),
        })
        .await
        .expect("create p1");
    let overlap = gl
        .create_period_async(CreateGlPeriod {
            period_name: format!("{fiscal_year}-07-B-{suffix}"),
            fiscal_year,
            period_number: 70,
            start_date: NaiveDate::from_ymd_opt(fiscal_year, 7, 10).expect("date"),
            end_date: NaiveDate::from_ymd_opt(fiscal_year, 7, 20).expect("date"),
        })
        .await;
    assert!(
        matches!(overlap, Err(CommerceError::ValidationError(ref m)) if m.contains("overlap")),
        "an overlapping period must be refused, got {overlap:?}"
    );

    let p1 = gl.open_period_async(p1.id).await.expect("open p1");
    let date = NaiveDate::from_ymd_opt(fiscal_year, 7, 15).expect("date");
    let selected = gl.get_period_for_date_async(date).await.expect("lookup").expect("some");
    assert_eq!(selected.id, p1.id, "must select the period covering the date");
    assert!(selected.can_post(), "selected period is open");
}
