#![cfg(feature = "sqlite")]

//! Exchange-rate precision parity between backends.
//!
//! Postgres stores `exchange_rates.rate` as `DECIMAL(20, 10)`, so a rate set
//! with more than 10 fractional digits is rounded to 10 dp (half away from
//! zero) by the column. SQLite stored the rate as full-precision TEXT, so a
//! high-precision rate produced a different `convert()` result on each backend.
//!
//! SQLite now rounds the stored rate to 10 dp with the same strategy, so both
//! backends agree.

use rust_decimal_macros::dec;
use stateset_embedded::{Commerce, ConvertCurrency, Currency, SetExchangeRate};

#[test]
fn sqlite_rounds_exchange_rate_to_ten_dp_matching_postgres_decimal_scale() {
    let commerce = Commerce::new(":memory:").unwrap();

    // 14 fractional digits. `1.23456789019999::DECIMAL(20,10)` = 1.2345678902.
    let stored = commerce
        .currency()
        .set_rate(SetExchangeRate {
            base_currency: Currency::USD,
            quote_currency: Currency::EUR,
            rate: dec!(1.23456789019999),
            source: Some("test".into()),
        })
        .unwrap();
    assert_eq!(stored.rate, dec!(1.2345678902), "stored rate must be rounded to 10 dp");

    // The conversion uses the rounded rate, so the result matches Postgres.
    let result = commerce
        .currency()
        .convert(ConvertCurrency { amount: dec!(1), from: Currency::USD, to: Currency::EUR })
        .unwrap();
    assert_eq!(result.rate, dec!(1.2345678902));
    assert_eq!(result.converted_amount, dec!(1.2345678902));
}

#[test]
fn sqlite_rounds_exchange_rate_half_away_from_zero() {
    let commerce = Commerce::new(":memory:").unwrap();

    // 11th fractional digit is exactly 5. Postgres rounds half away from zero
    // (`1.23456789005::DECIMAL(20,10)` = 1.2345678901), not banker's rounding
    // (which would keep 1.2345678900).
    let stored = commerce
        .currency()
        .set_rate(SetExchangeRate {
            base_currency: Currency::USD,
            quote_currency: Currency::GBP,
            rate: dec!(1.23456789005),
            source: Some("test".into()),
        })
        .unwrap();
    assert_eq!(
        stored.rate,
        dec!(1.2345678901),
        "midpoint must round away from zero to match Postgres numeric scale"
    );
}
