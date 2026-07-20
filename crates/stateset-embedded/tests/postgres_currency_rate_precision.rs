//! Exchange-rate precision parity, Postgres side.
//!
//! Postgres stores `exchange_rates.rate` as `DECIMAL(20, 10)`, so a
//! higher-precision rate is rounded to 10 dp (half away from zero) by the
//! column. This asserts the exact values the SQLite backend now matches (see
//! `currency_rate_precision.rs`), guarding the two backends against drifting
//! apart again.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{ConvertCurrency, Currency, SetExchangeRate};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_rounds_exchange_rate_to_ten_dp() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    // 14 fractional digits → DECIMAL(20,10) rounds to 1.2345678902.
    let stored = commerce
        .currency()
        .set_rate(SetExchangeRate {
            base_currency: Currency::USD,
            quote_currency: Currency::EUR,
            rate: dec!(1.23456789019999),
            source: Some("test".into()),
        })
        .await
        .expect("set rate");
    assert_eq!(stored.rate, dec!(1.2345678902));

    let result = commerce
        .currency()
        .convert(ConvertCurrency { amount: dec!(1), from: Currency::USD, to: Currency::EUR })
        .await
        .expect("convert");
    assert_eq!(result.rate, dec!(1.2345678902));
    assert_eq!(result.converted_amount, dec!(1.2345678902));

    // Midpoint at the 11th digit rounds away from zero.
    let midpoint = commerce
        .currency()
        .set_rate(SetExchangeRate {
            base_currency: Currency::USD,
            quote_currency: Currency::GBP,
            rate: dec!(1.23456789005),
            source: Some("test".into()),
        })
        .await
        .expect("set midpoint rate");
    assert_eq!(midpoint.rate, dec!(1.2345678901));
}
