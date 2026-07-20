//! Postgres side of the `get_average_days_to_pay` fractional-days parity guard.
//!
//! SQLite averages the fractional day difference (`JULIANDAY(applied) -
//! JULIANDAY(invoice)`); Postgres used `EXTRACT(DAY FROM interval)`, which drops
//! the sub-day part and floors each invoice's latency before averaging. Both now
//! use fractional days. This asserts the fractional result on a live database
//! (see `sqlite/accounts_receivable.rs::average_days_to_pay_uses_fractional_days`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use sqlx::postgres::PgPoolOptions;
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_average_days_to_pay_uses_fractional_days() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping avg-days-to-pay test");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await.expect("pool");

    let customer = uuid::Uuid::new_v4();
    let payment = uuid::Uuid::new_v4();
    let inv1 = uuid::Uuid::new_v4();
    let inv2 = uuid::Uuid::new_v4();
    let base = chrono::Utc::now();
    let uniq = customer.simple().to_string();

    sqlx::query(
        "INSERT INTO customers (id, email, first_name, last_name) VALUES ($1, $2, 'Test', 'Customer')",
    )
    .bind(customer)
    .bind(format!("adp-{}@example.com", &uniq[..8]))
    .execute(&pool)
    .await
    .expect("seed customer");

    sqlx::query("INSERT INTO payments (id, payment_number, amount) VALUES ($1, $2, 100)")
        .bind(payment)
        .bind(format!("PAY-{}", &uniq[..8]))
        .execute(&pool)
        .await
        .expect("seed payment");

    for (n, id) in [(1, inv1), (2, inv2)] {
        sqlx::query(
            "INSERT INTO invoices (id, invoice_number, customer_id, status, invoice_date, due_date)
             VALUES ($1, $2, $3, 'paid', $4, $4)",
        )
        .bind(id)
        .bind(format!("INV-{}-{n}", &uniq[..8]))
        .bind(customer)
        .bind(base)
        .execute(&pool)
        .await
        .expect("seed invoice");
    }

    // Applied at +10.5 and +11.5 days.
    for (id, hours) in [(inv1, 10 * 24 + 12i64), (inv2, 11 * 24 + 12i64)] {
        let applied = base + chrono::Duration::hours(hours);
        sqlx::query(
            "INSERT INTO ar_payment_applications (payment_id, invoice_id, applied_amount, applied_date)
             VALUES ($1, $2, 100, $3)",
        )
        .bind(payment)
        .bind(id)
        .bind(applied)
        .execute(&pool)
        .await
        .expect("seed application");
    }

    let avg = db
        .accounts_receivable()
        .get_average_days_to_pay_async(customer)
        .await
        .expect("average days to pay");
    // Fractional: AVG(10.5, 11.5) = 11.0 -> 11. Whole-day EXTRACT(DAY) would floor
    // each to 10/11, averaging to 10.5 -> 10.
    assert_eq!(avg, Some(11));
}
