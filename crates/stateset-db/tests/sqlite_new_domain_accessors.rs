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
