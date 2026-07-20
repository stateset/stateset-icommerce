//! Postgres parity for loyalty program tier persistence (SQLite covered by
//! `loyalty_rewards_test.rs`). Both backends previously dropped
//! `LoyaltyProgram.tiers`; migration 051 + the Pg row mapping fix this.

#![cfg(feature = "postgres")]

use stateset_core::{CreateLoyaltyProgram, LoyaltyTier};
use stateset_embedded::AsyncCommerce;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_loyalty_tiers_round_trip() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL or DATABASE_URL not set; skipping loyalty tiers test");
        return;
    };

    let commerce =
        AsyncCommerce::connect(&url).await.expect("connect to postgres and run migrations");

    let created = commerce
        .loyalty()
        .create_program(CreateLoyaltyProgram {
            name: "PG Tiered".into(),
            description: None,
            points_per_dollar: 1,
            tiers: vec![
                LoyaltyTier {
                    name: "Silver".into(),
                    min_points: 0,
                    multiplier: 1.0,
                    perks: vec!["free shipping".into()],
                },
                LoyaltyTier {
                    name: "Gold".into(),
                    min_points: 1000,
                    multiplier: 1.5,
                    perks: vec!["priority support".into(), "early access".into()],
                },
            ],
        })
        .await
        .expect("create program with tiers");

    assert_eq!(created.tiers.len(), 2, "tiers must be returned on create");

    let fetched =
        commerce.loyalty().get_program(created.id).await.expect("get").expect("program exists");
    assert_eq!(fetched.tiers.len(), 2, "tiers must survive a re-read");
    assert_eq!(fetched.tiers[1].name, "Gold");
    assert_eq!(fetched.tiers[1].min_points, 1000);
    assert_eq!(fetched.tiers[1].perks, vec!["priority support", "early access"]);
}
