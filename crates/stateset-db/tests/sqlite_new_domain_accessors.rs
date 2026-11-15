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
