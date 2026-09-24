//! Postgres counterparts to the SQLite regressions for formal settlement guards.
//! Requires POSTGRES_URL or DATABASE_URL; skipped otherwise.

#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreatePaymentObligation, Currency, CurrencyCode, IssueCostLayers,
    PaymentObligationStatus, SetExchangeRate,
};
use stateset_db::PostgresDatabase;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_rate_publication_records_the_returned_quote() {
    let Some(url) = postgres_url() else { return };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let source = format!("formal-rate-{}", Uuid::new_v4());
    let published = db
        .currency()
        .set_rate_async(SetExchangeRate {
            base_currency: Currency::USD,
            quote_currency: Currency::EUR,
            rate: dec!(1.2345678901),
            source: Some(source.clone()),
        })
        .await
        .expect("publish");
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM exchange_rate_history
         WHERE base_currency = $1 AND quote_currency = $2 AND rate = $3 AND source = $4 AND rate_at = $5",
    )
    .bind("USD")
    .bind("EUR")
    .bind(published.rate)
    .bind(&source)
    .bind(published.rate_at)
    .fetch_one(db.pool())
    .await
    .expect("history query");
    assert_eq!(count, 1);
}

#[tokio::test]
async fn postgres_obligation_manual_status_cannot_forge_or_reopen_payment() {
    let Some(url) = postgres_url() else { return };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let repo = db.payment_obligations();
    let obligation = repo
        .create_async(CreatePaymentObligation {
            supplier_id: Uuid::new_v4(),
            purchase_order_id: None,
            amount: dec!(100),
            currency: Some(CurrencyCode::USD),
            due_date: NaiveDate::from_ymd_opt(2026, 12, 1).expect("date"),
            notes: None,
        })
        .await
        .expect("create");
    assert!(matches!(
        repo.set_status_async(obligation.id, PaymentObligationStatus::Paid).await,
        Err(CommerceError::Conflict(_))
    ));
    repo.record_payment_async(obligation.id, dec!(40)).await.expect("partial");
    repo.set_status_async(obligation.id, PaymentObligationStatus::Cancelled).await.expect("cancel");
    assert!(matches!(
        repo.set_status_async(obligation.id, PaymentObligationStatus::Scheduled).await,
        Err(CommerceError::Conflict(_))
    ));
    let stored = repo.get_async(obligation.id).await.expect("get").expect("obligation");
    assert_eq!(stored.amount_paid, dec!(40));
    assert_eq!(stored.status, PaymentObligationStatus::Cancelled);
}

#[tokio::test]
async fn postgres_cost_layer_issue_requires_positive_quantity() {
    let Some(url) = postgres_url() else { return };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let request = |quantity| IssueCostLayers {
        sku: format!("ISSUE-{}", Uuid::new_v4()),
        quantity,
        reference_type: None,
        reference_id: None,
        notes: None,
    };
    assert!(matches!(
        db.cost_accounting().issue_fifo_async(request(dec!(0))).await,
        Err(CommerceError::ValidationError(_))
    ));
    assert!(matches!(
        db.cost_accounting().issue_lifo_async(request(dec!(-1))).await,
        Err(CommerceError::ValidationError(_))
    ));
}
