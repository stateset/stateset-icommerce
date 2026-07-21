//! Postgres integration coverage for revenue recognition: contract +
//! obligation persistence, ratable schedule generation, and the
//! recognize-through-a-date flow (including idempotent re-recognition).
//!
//! Uses the async Postgres repository via `AsyncCommerce::database()` because
//! `AsyncCommerce` does not (yet) expose a `revenue_recognition()` accessor.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`);
//! skipped otherwise.

#![cfg(feature = "postgres")]

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CreatePerformanceObligation, CreateRevenueContract, RecognitionMethod, RevenueContractFilter,
    RevenueEntryStatus,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

#[tokio::test]
async fn postgres_revenue_contract_and_ratable_recognition_flow() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping revenue recognition test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let revrec = commerce.database().revenue_recognition();

    let customer_id = uuid::Uuid::new_v4();
    let contract = revrec
        .create_contract_async(CreateRevenueContract {
            contract_number: None,
            customer_id,
            order_id: None,
            invoice_id: None,
            transaction_price: dec!(1200),
            currency: None,
            effective_date: date(2026, 1, 1),
            obligations: vec![CreatePerformanceObligation {
                description: "12-month support".into(),
                standalone_selling_price: Some(dec!(1200)),
                allocated_amount: dec!(1200),
                recognition_method: RecognitionMethod::RatableOverTime {
                    start: date(2026, 1, 1),
                    end: date(2026, 12, 1),
                },
            }],
        })
        .await
        .expect("create contract");
    assert!(!contract.contract_number.is_empty(), "contract number must be generated");
    assert_eq!(contract.obligations.len(), 1);
    let obligation = &contract.obligations[0];
    assert_eq!(obligation.allocated_amount, dec!(1200));
    assert_eq!(obligation.recognized_amount, Decimal::ZERO);

    // Contract round-trips (with obligations) through Postgres.
    let fetched = revrec
        .get_contract_async(contract.id)
        .await
        .expect("get contract")
        .expect("contract exists");
    assert_eq!(fetched.customer_id, customer_id);
    assert_eq!(fetched.obligations.len(), 1);
    let listed = revrec
        .list_contracts_async(RevenueContractFilter {
            customer_id: Some(customer_id),
            ..Default::default()
        })
        .await
        .expect("list contracts");
    assert_eq!(listed.len(), 1, "only this customer's contract is listed");

    // Recognition requires a generated schedule.
    assert!(revrec.recognize_period_async(obligation.id, date(2026, 3, 31)).await.is_err());

    let schedule = revrec.generate_schedule_async(obligation.id).await.expect("generate schedule");
    assert_eq!(schedule.entries.len(), 12);
    assert_eq!(schedule.total_amount, dec!(1200));
    assert_eq!(schedule.entries.iter().map(|e| e.amount).sum::<Decimal>(), dec!(1200));
    assert!(schedule.entries.iter().all(|e| e.status == RevenueEntryStatus::Deferred));

    // Schedule persists across a re-read.
    let persisted = revrec
        .get_schedule_async(obligation.id)
        .await
        .expect("get schedule")
        .expect("schedule exists");
    assert_eq!(persisted.entries.len(), 12);

    // Recognize through March: three monthly entries flip to recognized.
    let after =
        revrec.recognize_period_async(obligation.id, date(2026, 3, 31)).await.expect("recognize");
    let recognized: Vec<_> =
        after.entries.iter().filter(|e| e.status == RevenueEntryStatus::Recognized).collect();
    assert_eq!(recognized.len(), 3);
    assert_eq!(recognized.iter().map(|e| e.amount).sum::<Decimal>(), dec!(300));

    let obligations = revrec.list_obligations_async(contract.id).await.expect("list obligations");
    assert_eq!(obligations[0].recognized_amount, dec!(300));

    // Re-recognizing through the same date is idempotent.
    let again =
        revrec.recognize_period_async(obligation.id, date(2026, 3, 31)).await.expect("recognize");
    assert_eq!(
        again.entries.iter().filter(|e| e.status == RevenueEntryStatus::Recognized).count(),
        3
    );
    let obligations = revrec.list_obligations_async(contract.id).await.expect("list obligations");
    assert_eq!(obligations[0].recognized_amount, dec!(300), "no double recognition");

    // A schedule with recognized entries cannot be regenerated.
    assert!(revrec.generate_schedule_async(obligation.id).await.is_err());

    // Recognize everything through year end.
    revrec.recognize_period_async(obligation.id, date(2026, 12, 31)).await.expect("recognize all");
    let obligations = revrec.list_obligations_async(contract.id).await.expect("list obligations");
    assert_eq!(obligations[0].recognized_amount, dec!(1200));
}

#[tokio::test]
async fn postgres_revenue_point_in_time_recognizes_in_full() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping point-in-time revrec test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let revrec = commerce.database().revenue_recognition();

    let contract = revrec
        .create_contract_async(CreateRevenueContract {
            contract_number: None,
            customer_id: uuid::Uuid::new_v4(),
            order_id: None,
            invoice_id: None,
            transaction_price: dec!(250),
            currency: None,
            effective_date: date(2026, 6, 15),
            obligations: vec![CreatePerformanceObligation {
                description: "hardware delivery".into(),
                standalone_selling_price: None,
                allocated_amount: dec!(250),
                recognition_method: RecognitionMethod::PointInTime,
            }],
        })
        .await
        .expect("create contract");
    let obligation_id = contract.obligations[0].id;

    let schedule = revrec.generate_schedule_async(obligation_id).await.expect("generate schedule");
    assert_eq!(schedule.entries.len(), 1);
    assert_eq!(schedule.entries[0].period_start, date(2026, 6, 15));

    // Recognizing before the recognition date is a no-op...
    let early =
        revrec.recognize_period_async(obligation_id, date(2026, 6, 1)).await.expect("recognize");
    assert!(early.entries.iter().all(|e| e.status == RevenueEntryStatus::Deferred));

    // ...and on/after it recognizes the full amount.
    let after =
        revrec.recognize_period_async(obligation_id, date(2026, 6, 30)).await.expect("recognize");
    assert!(after.entries.iter().all(|e| e.status == RevenueEntryStatus::Recognized));
    let obligations = revrec.list_obligations_async(contract.id).await.expect("list obligations");
    assert_eq!(obligations[0].recognized_amount, dec!(250));
}
