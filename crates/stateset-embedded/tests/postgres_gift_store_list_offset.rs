//! Postgres parity: listing gift cards / store credits with an offset but no limit.
//!
//! SQLite crashed on `OFFSET` without `LIMIT`; Postgres allows a bare `OFFSET`, so
//! it already paginated correctly. This test locks in that behavior: an offset of 1
//! skips exactly one row on Postgres too.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use rust_decimal_macros::dec;
use stateset_core::{
    CreateCustomer, CreateGiftCard, CreateStoreCredit, CurrencyCode, GiftCardFilter,
    StoreCreditFilter, StoreCreditReason,
};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_gift_card_list_offset_without_limit_paginates() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping gift-card offset test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");
    let gc = commerce.gift_cards();

    for _ in 0..3 {
        gc.create(CreateGiftCard {
            code: None,
            initial_balance: dec!(50.00),
            currency: CurrencyCode::USD,
            recipient_email: None,
            sender_name: None,
            message: None,
            expires_at: None,
        })
        .await
        .expect("create gift card");
    }

    // Robust on a shared DB: offset of 1 must skip exactly one row of the full list.
    let all = gc.list(GiftCardFilter::default()).await.expect("list all").len();
    let page = gc
        .list(GiftCardFilter { offset: Some(1), ..Default::default() })
        .await
        .expect("list with offset");
    assert_eq!(page.len(), all - 1, "offset 1 should skip exactly one gift card");
}

#[tokio::test]
async fn postgres_store_credit_list_offset_without_limit_paginates() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping store-credit offset test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    // Postgres enforces store_credits → customers, so seed a real customer and scope
    // the assertions to it.
    let unique = uuid::Uuid::new_v4().simple().to_string();
    let customer = commerce
        .customers()
        .create(CreateCustomer {
            email: format!("sc-{}@example.com", &unique[..8]),
            first_name: "Grace".into(),
            last_name: "Hopper".into(),
            phone: None,
            accepts_marketing: None,
            tags: None,
            metadata: None,
        })
        .await
        .expect("create customer");

    let sc = commerce.store_credits();
    for _ in 0..3 {
        sc.create(CreateStoreCredit {
            customer_id: customer.id,
            amount: dec!(25.00),
            currency: CurrencyCode::USD,
            reason: StoreCreditReason::Return,
            reference_id: None,
            note: None,
            expires_at: None,
        })
        .await
        .expect("create store credit");
    }

    let scoped = || StoreCreditFilter { customer_id: Some(customer.id), ..Default::default() };
    let all = sc.list(scoped()).await.expect("list all").len();
    assert_eq!(all, 3);
    let page =
        sc.list(StoreCreditFilter { offset: Some(1), ..scoped() }).await.expect("list with offset");
    assert_eq!(page.len(), 2, "offset 1 of 3 store credits should return 2");
}
