//! A store that trades in euros must not have its money recorded in USD.
//!
//! `store_currency_settings.base_currency` is the store's own trading currency.
//! When a caller omits `currency` on a create, the record takes that setting —
//! not a hardcoded USD. The seeded value is `'USD'`, so a store that never
//! touched the setting sees no change; only a store that explicitly configured
//! another currency does, which is precisely the bug these tests pin.
//!
//! These tests mutate one row of a database other Postgres suites share, so
//! they all live in a single `#[tokio::test]` that runs them in sequence and
//! puts `'USD'` back before asserting anything.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use sqlx::postgres::{PgPool, PgPoolOptions};
use stateset_core::{
    AccountType, CreateGlAccount, CreatePayment, CreatePriceLevel, CurrencyCode, PaymentMethodType,
    PriceAdjustmentType,
};
use stateset_db::PostgresDatabase;
use std::env;
use uuid::Uuid;

fn postgres_url() -> Option<String> {
    env::var("POSTGRES_URL").ok().or_else(|| env::var("DATABASE_URL").ok())
}

/// Point the shared store at a trading currency.
async fn set_base_currency(pool: &PgPool, code: &str) {
    sqlx::query(
        "INSERT INTO store_currency_settings (id, base_currency)
         VALUES ('default', $1)
         ON CONFLICT (id) DO UPDATE SET base_currency = EXCLUDED.base_currency",
    )
    .bind(code)
    .execute(pool)
    .await
    .expect("set base currency");
}

/// Remove the settings row entirely, so the fallback path is exercised.
async fn clear_settings(pool: &PgPool) {
    sqlx::query("DELETE FROM store_currency_settings WHERE id = 'default'")
        .execute(pool)
        .await
        .expect("clear settings");
}

/// A gl account number that cannot collide with another suite's.
fn unique_account_number() -> String {
    format!("9{}", &Uuid::new_v4().simple().to_string()[..11])
}

#[tokio::test]
async fn postgres_creates_denominate_in_the_store_base_currency() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping store base currency test");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect to postgres and run migrations");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect to postgres for settings fixture");

    // ------------------------------------------------------------------
    // A store configured for euros.
    // ------------------------------------------------------------------
    set_base_currency(&pool, "EUR").await;

    // A payment path: the caller omits the currency.
    let payment = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(42.00),
            currency: None,
            ..Default::default()
        })
        .await;

    // ...and the caller's own choice must still win.
    let explicit_payment = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(42.00),
            currency: Some(CurrencyCode::JPY),
            ..Default::default()
        })
        .await;

    // A general-ledger path.
    let account = db
        .general_ledger()
        .create_account_async(CreateGlAccount {
            account_number: unique_account_number(),
            name: "Cash (EUR store)".into(),
            description: None,
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        })
        .await;

    // A pricing path.
    let level = db
        .price_levels()
        .create_async(CreatePriceLevel {
            name: "Wholesale".into(),
            code: format!("WHOLESALE-{}", Uuid::new_v4().simple()),
            description: None,
            adjustment_type: PriceAdjustmentType::PercentageDiscount,
            adjustment_value: dec!(10),
            currency: None,
        })
        .await;

    // ------------------------------------------------------------------
    // No settings row at all: the USD default must stay intact.
    // ------------------------------------------------------------------
    clear_settings(&pool).await;

    let payment_no_settings = db
        .payments()
        .create_async(CreatePayment {
            payment_method: PaymentMethodType::CreditCard,
            amount: dec!(7.00),
            currency: None,
            ..Default::default()
        })
        .await;

    let account_no_settings = db
        .general_ledger()
        .create_account_async(CreateGlAccount {
            account_number: unique_account_number(),
            name: "Cash (no settings)".into(),
            description: None,
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        })
        .await;

    // ------------------------------------------------------------------
    // Put the shared fixture back BEFORE asserting, so a failure here cannot
    // leave the database configured for euros for every other suite.
    // ------------------------------------------------------------------
    set_base_currency(&pool, "USD").await;
    pool.close().await;

    let payment = payment.expect("create payment");
    let explicit_payment = explicit_payment.expect("create payment with explicit currency");
    let account = account.expect("create gl account");
    let level = level.expect("create price level");
    let payment_no_settings = payment_no_settings.expect("create payment without settings row");
    let account_no_settings = account_no_settings.expect("create gl account without settings row");

    // Every path is reported in one go: a single `assert_eq!` would stop at the
    // first mismatch and hide which of the other paths are also wrong.
    let mut wrong = Vec::new();
    let mut check = |what: &str, got: CurrencyCode, want: CurrencyCode| {
        if got != want {
            wrong.push(format!("{what}: got {got}, want {want}"));
        }
    };
    check("payment takes the store base currency", payment.currency, CurrencyCode::EUR);
    check("an explicit currency still wins", explicit_payment.currency, CurrencyCode::JPY);
    check("gl account takes the store base currency", account.currency, CurrencyCode::EUR);
    check("price level takes the store base currency", level.currency, CurrencyCode::EUR);
    check(
        "payment with no settings row keeps the USD default",
        payment_no_settings.currency,
        CurrencyCode::USD,
    );
    check(
        "gl account with no settings row keeps the USD default",
        account_no_settings.currency,
        CurrencyCode::USD,
    );
    assert!(wrong.is_empty(), "store base currency not honoured:\n  {}", wrong.join("\n  "));

    // The fixture really is back.
    let restored = db.currency().get_settings_async().await.expect("read settings");
    assert_eq!(restored.base_currency.code(), "USD");
}
