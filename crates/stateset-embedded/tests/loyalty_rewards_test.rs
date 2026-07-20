#![cfg(feature = "sqlite")]

//! Regression coverage for the loyalty reward catalog against the *real*
//! migrated SQLite schema.
//!
//! The `rewards` table was missing from the SQLite migration set — the
//! sqlite/rewards.rs unit tests created the table by hand, so they never
//! exercised the migration path and the gap went unnoticed until
//! `create_reward` was called on a `Commerce::new(":memory:")` engine. These
//! tests run migrations the way production does, so a missing table fails here.

use rust_decimal_macros::dec;
use stateset_core::{CreateLoyaltyProgram, CreateReward, LoyaltyTier, RewardFilter, RewardType};
use stateset_embedded::Commerce;

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("in-memory engine")
}

fn program(commerce: &Commerce) -> stateset_core::LoyaltyProgramId {
    commerce
        .loyalty()
        .create_program(CreateLoyaltyProgram {
            name: "Rewards Club".into(),
            description: None,
            points_per_dollar: 2,
            tiers: vec![],
        })
        .expect("create program")
        .id
}

#[test]
fn create_reward_works_against_migrated_schema() {
    let commerce = commerce();
    let program_id = program(&commerce);

    let reward = commerce
        .loyalty()
        .create_reward(CreateReward {
            program_id,
            name: "$5 off".into(),
            description: Some("Five dollars off".into()),
            points_cost: 100,
            reward_type: RewardType::Discount,
            value: Some(dec!(5.00)),
        })
        .expect("create_reward must succeed on a migrated database");

    assert_eq!(reward.name, "$5 off");
    assert_eq!(reward.points_cost, 100);
    assert_eq!(reward.value, Some(dec!(5.00)));

    let fetched = commerce.loyalty().get_reward(reward.id).expect("get").expect("exists");
    assert_eq!(fetched.id, reward.id);

    let listed = commerce
        .loyalty()
        .list_rewards(RewardFilter { program_id: Some(program_id), ..Default::default() })
        .expect("list");
    assert_eq!(listed.len(), 1);

    commerce.loyalty().delete_reward(reward.id).expect("delete");
    assert!(commerce.loyalty().get_reward(reward.id).expect("get").is_none());
}

#[test]
fn program_tiers_round_trip() {
    let commerce = commerce();
    let created = commerce
        .loyalty()
        .create_program(CreateLoyaltyProgram {
            name: "Tiered".into(),
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
        .expect("create program with tiers");

    // The returned program carries its tiers (previously silently dropped).
    assert_eq!(created.tiers.len(), 2);

    // And they survive a re-read from the database.
    let fetched = commerce.loyalty().get_program(created.id).expect("get").expect("exists");
    assert_eq!(fetched.tiers.len(), 2);
    assert_eq!(fetched.tiers[0].name, "Silver");
    assert_eq!(fetched.tiers[1].name, "Gold");
    assert_eq!(fetched.tiers[1].min_points, 1000);
    assert_eq!(fetched.tiers[1].perks, vec!["priority support", "early access"]);
}
