//! Postgres side of the gift-card / store-credit issuance-amount guard.
//!
//! Neither a negative gift-card balance nor a non-positive store-credit amount
//! may be issued. Postgres was protected only by DB CHECK constraints (which
//! surface as a raw `DatabaseError`); both backends now reject up front with a
//! clean `ValidationError`, matching the SQLite guards (see
//! `sqlite/gift_cards.rs::create_rejects_negative_initial_balance` and
//! `sqlite/store_credits.rs::create_rejects_non_positive_amount`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateGiftCard, CreateStoreCredit, CurrencyCode, CustomerId, StoreCreditReason,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_rejects_non_positive_issuance_with_validation_error() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping issuance guard test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let gc_err = commerce
        .gift_cards()
        .create(CreateGiftCard {
            code: None,
            initial_balance: dec!(-50.00),
            currency: CurrencyCode::USD,
            recipient_email: None,
            sender_name: None,
            message: None,
            expires_at: None,
        })
        .await
        .expect_err("negative gift card initial balance must be rejected");
    assert!(matches!(gc_err, CommerceError::ValidationError(_)), "gift card: {gc_err:?}");

    let store_credit = |amount| CreateStoreCredit {
        customer_id: CustomerId::new(),
        amount,
        currency: CurrencyCode::USD,
        reason: StoreCreditReason::Return,
        reference_id: None,
        note: None,
        expires_at: None,
    };

    for bad in [dec!(-50.00), dec!(0)] {
        let err = commerce
            .store_credits()
            .create(store_credit(bad))
            .await
            .expect_err("non-positive store credit amount must be rejected");
        assert!(matches!(err, CommerceError::ValidationError(_)), "store credit {bad}: {err:?}");
    }
}
