#![cfg(feature = "sqlite")]

//! Regression: SQLite `gift_cards::list` and `store_credits::list` appended
//! `OFFSET <n>` independently of `LIMIT`. SQLite rejects `OFFSET` without a
//! preceding `LIMIT` with a syntax error, so listing with an offset but no limit
//! crashed at runtime — while Postgres (which allows bare `OFFSET`) worked. Both
//! now always emit a `LIMIT` (the server-side default page size when the filter
//! supplies none) before any `OFFSET`.

use rust_decimal_macros::dec;
use stateset_core::{
    CreateGiftCard, CreateStoreCredit, CurrencyCode, GiftCardFilter, GiftCardRepository,
    StoreCreditFilter, StoreCreditReason, StoreCreditRepository,
};
use stateset_db::SqliteDatabase;

#[test]
fn sqlite_gift_card_list_offset_without_limit_paginates() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let gc = db.gift_cards();

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
        .expect("create gift card");
    }

    // Offset with no limit must not error and must skip the offset rows.
    let page = gc
        .list(GiftCardFilter { offset: Some(1), ..Default::default() })
        .expect("list with offset-and-no-limit must not error");
    assert_eq!(page.len(), 2, "offset 1 of 3 gift cards should return 2");
}

#[test]
fn sqlite_store_credit_list_offset_without_limit_paginates() {
    let db = SqliteDatabase::in_memory().expect("in-memory sqlite");
    let sc = db.store_credits();

    for _ in 0..3 {
        sc.create(CreateStoreCredit {
            customer_id: uuid::Uuid::new_v4().into(),
            amount: dec!(25.00),
            currency: CurrencyCode::USD,
            reason: StoreCreditReason::Return,
            reference_id: None,
            note: None,
            expires_at: None,
        })
        .expect("create store credit");
    }

    let page = sc
        .list(StoreCreditFilter { offset: Some(1), ..Default::default() })
        .expect("list with offset-and-no-limit must not error");
    assert_eq!(page.len(), 2, "offset 1 of 3 store credits should return 2");
}
