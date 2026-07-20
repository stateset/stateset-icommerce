//! Postgres side of the `get_locations_for_warehouse` parity guard.
//!
//! `get_locations_for_warehouse` returns ALL locations for a warehouse — active
//! and inactive (filtered subsets have their own accessors). Postgres already
//! does this; SQLite used to silently filter to active only. This asserts the
//! Postgres behavior the SQLite backend now matches (see
//! `sqlite/warehouse.rs::get_locations_for_warehouse_includes_inactive`).
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`); skipped
//! otherwise.

#![cfg(feature = "postgres")]

use stateset_core::{CreateLocation, CreateWarehouse, LocationType, UpdateLocation};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_get_locations_for_warehouse_includes_inactive() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping locations-for-warehouse test");
        return;
    };
    let commerce = AsyncCommerce::connect(&url).await.expect("connect + migrate");

    let unique = uuid::Uuid::new_v4().to_string();
    let wh = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: format!("WH-{}", &unique[..8]),
            name: "Inactive".into(),
            ..Default::default()
        })
        .await
        .expect("create warehouse");

    let mk = |code: String| {
        let commerce = &commerce;
        let wh_id = wh.id;
        async move {
            commerce
                .warehouse()
                .create_location(CreateLocation {
                    warehouse_id: wh_id,
                    code: Some(code),
                    location_type: LocationType::Bulk,
                    ..Default::default()
                })
                .await
                .expect("create location")
        }
    };

    let active = mk(format!("ACT-{}", &unique[..8])).await;
    let inactive = mk(format!("INACT-{}", &unique[..8])).await;
    commerce
        .warehouse()
        .update_location(
            inactive.id,
            UpdateLocation { is_active: Some(false), ..Default::default() },
        )
        .await
        .expect("deactivate");

    let locs = commerce.warehouse().get_locations_for_warehouse(wh.id).await.expect("locations");
    let ids: Vec<i32> = locs.iter().map(|l| l.id).collect();
    assert_eq!(locs.len(), 2, "must return active AND inactive locations: {ids:?}");
    assert!(ids.contains(&active.id));
    assert!(ids.contains(&inactive.id));
}
