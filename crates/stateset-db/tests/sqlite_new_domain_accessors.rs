#![cfg(feature = "sqlite")]

use stateset_core::{
    FraudAssessmentFilter, GiftCardFilter, ReviewFilter, RewardFilter, SearchConfigFilter,
    SegmentFilter, ShippingZoneFilter, StoreCreditFilter, WishlistFilter, ZoneShippingMethodFilter,
};
use stateset_db::{Database, SqliteDatabase};

#[test]
fn sqlite_new_domain_accessors_do_not_panic() {
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let dyn_db: &dyn Database = &db;

    let outcomes = [
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.gift_cards().list(GiftCardFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.store_credits().list(StoreCreditFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.segments().list(SegmentFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.shipping_zones().list(ShippingZoneFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.zone_shipping_methods().list(ZoneShippingMethodFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.reviews().list(ReviewFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.wishlists().list(WishlistFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| dyn_db.loyalty_programs().list()))
            .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.rewards().list(RewardFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.fraud().list_assessments(FraudAssessmentFilter::default())
        }))
        .is_ok(),
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dyn_db.search_configs().list(SearchConfigFilter::default())
        }))
        .is_ok(),
    ];

    assert!(outcomes.into_iter().all(std::convert::identity));
}

/// The SQLite `gift_cards` table (migration 037) stores `recipient_email` in
/// its `customer_id` column (and `sender_name`/`message` in `issued_by`/`notes`);
/// `create`, `update` and the row mapper all use that mapping, so an updated
/// recipient round-trips and nothing else moves.
#[test]
fn sqlite_gift_card_update_round_trips_recipient_email() {
    use stateset_core::{CreateGiftCard, CurrencyCode, GiftCardRepository, UpdateGiftCard};
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let cards = db.gift_cards();
    let card = cards
        .create(CreateGiftCard {
            code: Some("GC-ROUNDTRIP".into()),
            initial_balance: rust_decimal::Decimal::new(2500, 2),
            currency: CurrencyCode::USD,
            recipient_email: Some("first@example.com".into()),
            sender_name: Some("Sender".into()),
            message: Some("Enjoy".into()),
            expires_at: None,
        })
        .expect("create gift card");
    assert_eq!(card.recipient_email.as_deref(), Some("first@example.com"));
    let updated = cards
        .update(
            card.id,
            UpdateGiftCard {
                recipient_email: Some("second@example.com".into()),
                ..Default::default()
            },
        )
        .expect("update recipient");
    let stored = cards.get(card.id).expect("get").expect("gift card");
    for card in [&updated, &stored] {
        assert_eq!(card.recipient_email.as_deref(), Some("second@example.com"));
        assert_eq!(card.sender_name.as_deref(), Some("Sender"));
        assert_eq!(card.message.as_deref(), Some("Enjoy"));
    }
}

/// Activating a search config that does not exist changes nothing: the
/// currently active config stays active. Twin of the Postgres
/// `postgres_search_config_set_active_missing_id_changes_nothing`.
#[test]
fn sqlite_search_config_set_active_missing_id_changes_nothing() {
    use stateset_core::{
        CommerceError, CreateSearchConfig, SearchConfigId, SearchConfigRepository,
    };
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let configs = db.search_configs();
    let config = configs
        .create(CreateSearchConfig {
            name: "Primary".into(),
            description: None,
            searchable_fields: vec![],
            facets: vec![],
            synonyms: vec![],
            boost_rules: vec![],
        })
        .expect("create config");
    configs.set_active(config.id).expect("activate config");
    let err = configs.set_active(SearchConfigId::new()).expect_err("missing id");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");
    let active = configs.get_active().expect("get active").expect("an active config");
    assert_eq!(active.id, config.id, "the refused activation deactivated nothing");
}

/// Activating a config through `update` (`is_active: Some(true)`) deactivates
/// the others, as `set_active` does, and an activation of a missing config
/// changes nothing. Twin of the Postgres
/// `postgres_search_config_update_activation_deactivates_others`.
#[test]
fn sqlite_search_config_update_activation_deactivates_others() {
    use stateset_core::{
        CommerceError, CreateSearchConfig, SearchConfigId, SearchConfigRepository,
        UpdateSearchConfig,
    };
    let db = SqliteDatabase::in_memory().expect("create in-memory sqlite db");
    let configs = db.search_configs();
    let create = |name: &str| {
        configs
            .create(CreateSearchConfig {
                name: name.into(),
                description: None,
                searchable_fields: vec![],
                facets: vec![],
                synonyms: vec![],
                boost_rules: vec![],
            })
            .expect("create config")
    };
    let active = || {
        configs
            .list(SearchConfigFilter { is_active: Some(true), ..Default::default() })
            .expect("list active")
            .into_iter()
            .map(|config| config.id)
            .collect::<Vec<_>>()
    };
    let first = create("First");
    let second = create("Second");
    configs.set_active(first.id).expect("activate first");

    let updated = configs
        .update(
            second.id,
            UpdateSearchConfig {
                name: Some("Renamed".into()),
                is_active: Some(true),
                ..Default::default()
            },
        )
        .expect("activate second through update");
    assert!(updated.is_active);
    assert_eq!(updated.name, "Renamed", "the update's other fields still apply");
    assert_eq!(active(), vec![second.id], "exactly one config is active");

    let err = configs
        .update(
            SearchConfigId::new(),
            UpdateSearchConfig { is_active: Some(true), ..Default::default() },
        )
        .expect_err("missing id");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");
    assert_eq!(active(), vec![second.id], "the refused activation deactivated nothing");

    let deactivated = configs
        .update(second.id, UpdateSearchConfig { is_active: Some(false), ..Default::default() })
        .expect("deactivate second");
    assert!(!deactivated.is_active);
    assert!(active().is_empty());
}
