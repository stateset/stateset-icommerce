//! Verifies the Postgres exchange-rate seed removal (migration 053).
//!
//! Postgres used to seed 9 hardcoded FX rates, so `convert(USD->EUR)` returned a
//! stale seeded rate while SQLite errored "No exchange rate found". The seed is
//! now removed, so conversion requires an explicit rate on both backends.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{CommerceError, ConvertCurrency, Currency, SetExchangeRate};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_convert_requires_explicit_rate_after_seed_removed() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    // No seeded rate remains: conversion errors until a rate is set explicitly
    // (matching the SQLite backend).
    let err = commerce
        .currency()
        .convert(ConvertCurrency { amount: dec!(100), from: Currency::USD, to: Currency::EUR })
        .await
        .expect_err("USD->EUR must have no seeded rate");
    assert!(matches!(err, CommerceError::ValidationError(_)), "got {err:?}");

    // After explicitly setting a rate, conversion works.
    commerce
        .currency()
        .set_rate(SetExchangeRate {
            base_currency: Currency::USD,
            quote_currency: Currency::EUR,
            rate: dec!(0.90),
            source: None,
        })
        .await
        .expect("set rate");
    let converted = commerce
        .currency()
        .convert(ConvertCurrency { amount: dec!(100), from: Currency::USD, to: Currency::EUR })
        .await
        .expect("convert after setting a rate");
    assert_eq!(converted.converted_amount, dec!(90));
}
