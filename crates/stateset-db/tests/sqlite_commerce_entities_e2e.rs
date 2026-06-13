#![cfg(feature = "sqlite")]
//! End-to-end tests for the commerce entity repositories against the live
//! SQLite engine (no `#[cfg(test)]` `CREATE TABLE` scaffolding).
//!
//! Before migration `037_commerce_entities` these tables existed only inside
//! `#[cfg(test)]` blocks and the PostgreSQL backend, so the mounted REST
//! endpoints (`crates/stateset-http/src/routes/<entity>.rs`) returned
//! HTTP 500 `no such table`. These tests drive the same repository methods the
//! HTTP handlers call (`create` / `get` / `list`) through a fresh `:memory:`
//! database to prove the full path now works.

use rust_decimal_macros::dec;
use stateset_core::{
    CreateGiftCard, CreateReview, CreateShippingZone, CreateStoreCredit, CreateWishlist,
    CurrencyCode, CustomerId, GiftCardFilter, GiftCardRepository, ProductId, ReviewFilter,
    ReviewRepository, ShippingZoneFilter, ShippingZoneRepository, StoreCreditFilter,
    StoreCreditRepository, WishlistFilter, WishlistRepository,
};
use stateset_db::SqliteDatabase;

fn db() -> SqliteDatabase {
    SqliteDatabase::in_memory().expect("create in-memory sqlite db")
}

#[test]
fn shipping_zones_crud_end_to_end() {
    let db = db();

    let created = db
        .shipping_zones()
        .create(CreateShippingZone {
            name: "Domestic US".into(),
            countries: vec!["US".into()],
            regions: vec![],
            postal_codes: vec![],
            priority: Some(1),
        })
        .expect("create shipping zone against live SQLite schema");

    assert_eq!(created.name, "Domestic US");
    assert_eq!(created.priority, 1);
    assert!(created.is_active);

    let fetched = db.shipping_zones().get(created.id).expect("get").expect("zone present");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.countries, vec!["US".to_string()]);

    let listed =
        db.shipping_zones().list(ShippingZoneFilter::default()).expect("list shipping zones");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[test]
fn gift_cards_crud_end_to_end() {
    let db = db();

    let created = db
        .gift_cards()
        .create(CreateGiftCard {
            code: Some("GIFT-E2E-0001".into()),
            initial_balance: dec!(75.00),
            currency: CurrencyCode::USD,
            recipient_email: Some("alice@example.com".into()),
            sender_name: Some("Bob".into()),
            message: Some("Enjoy!".into()),
            expires_at: None,
        })
        .expect("create gift card against live SQLite schema");

    assert_eq!(created.code, "GIFT-E2E-0001");
    assert_eq!(created.current_balance, dec!(75.00));

    let fetched = db.gift_cards().get(created.id).expect("get").expect("gift card present");
    assert_eq!(fetched.id, created.id);

    let by_code =
        db.gift_cards().get_by_code("GIFT-E2E-0001").expect("get_by_code").expect("present");
    assert_eq!(by_code.id, created.id);

    let listed = db.gift_cards().list(GiftCardFilter::default()).expect("list gift cards");
    assert_eq!(listed.len(), 1);
}

#[test]
fn reviews_crud_end_to_end() {
    let db = db();
    let product_id = ProductId::new();

    let created = db
        .reviews()
        .create(CreateReview {
            product_id,
            customer_id: CustomerId::new(),
            rating: 5,
            title: Some("Great product".into()),
            body: Some("Really loved it".into()),
            verified_purchase: true,
        })
        .expect("create review against live SQLite schema");

    assert_eq!(created.rating, 5);
    assert!(created.verified_purchase);

    let fetched = db.reviews().get(created.id).expect("get").expect("review present");
    assert_eq!(fetched.id, created.id);

    let listed = db
        .reviews()
        .list(ReviewFilter { product_id: Some(product_id), ..Default::default() })
        .expect("list reviews by product");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
}

#[test]
fn store_credits_crud_end_to_end() {
    let db = db();
    let customer_id = CustomerId::new();

    let created = db
        .store_credits()
        .create(CreateStoreCredit {
            customer_id,
            amount: dec!(40.00),
            currency: CurrencyCode::USD,
            reason: Default::default(),
            reference_id: Some("RET-001".into()),
            note: Some("return credit".into()),
            expires_at: None,
        })
        .expect("create store credit against live SQLite schema");

    assert_eq!(created.original_balance, dec!(40.00));
    assert_eq!(created.current_balance, dec!(40.00));

    let fetched = db.store_credits().get(created.id).expect("get").expect("store credit present");
    assert_eq!(fetched.id, created.id);

    let listed = db.store_credits().list(StoreCreditFilter::default()).expect("list store credits");
    assert_eq!(listed.len(), 1);
}

#[test]
fn wishlists_crud_end_to_end() {
    let db = db();
    let customer_id = CustomerId::new();

    let created = db
        .wishlists()
        .create(CreateWishlist { customer_id, name: "Birthday Ideas".into(), is_public: true })
        .expect("create wishlist against live SQLite schema");

    assert_eq!(created.name, "Birthday Ideas");
    assert!(created.is_public);

    let fetched = db.wishlists().get(created.id).expect("get").expect("wishlist present");
    assert_eq!(fetched.id, created.id);

    let listed = db.wishlists().list(WishlistFilter::default()).expect("list wishlists");
    assert_eq!(listed.len(), 1);
}
