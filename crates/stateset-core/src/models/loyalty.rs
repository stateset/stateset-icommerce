//! Loyalty program and rewards domain models
//!
//! Supports multi-tier loyalty programs with point earning, redemption,
//! and configurable reward catalogs.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use stateset_primitives::{
    CustomerId, LoyaltyAccountId, LoyaltyProgramId, LoyaltyTransactionId, RewardId,
};
use strum::{Display, EnumString};

/// Loyalty program status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum LoyaltyProgramStatus {
    /// Program is active and accepting enrollments
    #[default]
    Active,
    /// Program is paused (no new enrollments, existing members keep benefits)
    Paused,
    /// Program has been retired
    Archived,
}

/// Loyalty transaction type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum LoyaltyTransactionType {
    /// Points earned from a purchase
    #[default]
    Earn,
    /// Points redeemed for a reward
    Redeem,
    /// Manual adjustment by admin
    Adjust,
    /// Points expired
    Expire,
    /// Bonus points (promotions, sign-up, etc.)
    Bonus,
    /// Points refunded from a cancelled redemption
    Refund,
}

/// Reward type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum RewardType {
    /// Discount on next order (percentage or fixed)
    #[default]
    Discount,
    /// Free shipping on next order
    FreeShipping,
    /// Free product
    FreeProduct,
    /// Store credit issuance
    StoreCredit,
    /// Exclusive access to products or sales
    ExclusiveAccess,
}

/// A loyalty program definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoyaltyProgram {
    /// Unique program ID
    pub id: LoyaltyProgramId,
    /// Program name
    pub name: String,
    /// Program description
    pub description: Option<String>,
    /// Points earned per dollar spent
    pub points_per_dollar: u32,
    /// Program tiers (ordered by min_points ascending)
    pub tiers: Vec<LoyaltyTier>,
    /// Program status
    pub status: LoyaltyProgramStatus,
    /// When the program was created
    pub created_at: DateTime<Utc>,
    /// When the program was last updated
    pub updated_at: DateTime<Utc>,
}

/// A tier within a loyalty program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoyaltyTier {
    /// Tier name (e.g., "Bronze", "Silver", "Gold", "Platinum")
    pub name: String,
    /// Minimum lifetime points to reach this tier
    pub min_points: u64,
    /// Points earning multiplier (e.g., 1.5 = 50% bonus)
    pub multiplier: f64,
    /// Perks/benefits at this tier
    pub perks: Vec<String>,
}

/// A customer's loyalty account
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoyaltyAccount {
    /// Unique account ID
    pub id: LoyaltyAccountId,
    /// Customer
    pub customer_id: CustomerId,
    /// Program this account belongs to
    pub program_id: LoyaltyProgramId,
    /// Current redeemable points balance
    pub points_balance: i64,
    /// Total lifetime points earned (determines tier)
    pub lifetime_points: u64,
    /// Current tier name
    pub tier: String,
    /// When the account was created (enrolled)
    pub created_at: DateTime<Utc>,
    /// When the account was last updated
    pub updated_at: DateTime<Utc>,
}

/// A loyalty points transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoyaltyTransaction {
    /// Unique transaction ID
    pub id: LoyaltyTransactionId,
    /// Account this transaction belongs to
    pub account_id: LoyaltyAccountId,
    /// Points amount (positive for earn/bonus, negative for redeem)
    pub points: i64,
    /// Transaction type
    pub transaction_type: LoyaltyTransactionType,
    /// Optional reference (order ID, reward ID, etc.)
    pub reference_id: Option<String>,
    /// Optional description
    pub description: Option<String>,
    /// When the transaction occurred
    pub created_at: DateTime<Utc>,
}

/// A reward in the reward catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reward {
    /// Unique reward ID
    pub id: RewardId,
    /// Program this reward belongs to
    pub program_id: LoyaltyProgramId,
    /// Reward name
    pub name: String,
    /// Reward description
    pub description: Option<String>,
    /// Points cost to redeem
    pub points_cost: u64,
    /// Type of reward
    pub reward_type: RewardType,
    /// Monetary value of the reward (for discount/store credit types)
    pub value: Option<Decimal>,
    /// Whether this reward is currently available
    pub is_active: bool,
    /// When the reward was created
    pub created_at: DateTime<Utc>,
    /// When the reward was last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a loyalty program
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLoyaltyProgram {
    /// Program name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Points per dollar
    pub points_per_dollar: u32,
    /// Initial tiers
    pub tiers: Vec<LoyaltyTier>,
}

/// Input for enrolling a customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollCustomer {
    /// Customer to enroll
    pub customer_id: CustomerId,
    /// Program to enroll in
    pub program_id: LoyaltyProgramId,
}

/// Input for earning/redeeming points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustPoints {
    /// Account to adjust
    pub account_id: LoyaltyAccountId,
    /// Points amount (positive or negative)
    pub points: i64,
    /// Transaction type
    pub transaction_type: LoyaltyTransactionType,
    /// Reference
    pub reference_id: Option<String>,
    /// Description
    pub description: Option<String>,
}

/// Input for creating a reward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateReward {
    /// Program this reward belongs to
    pub program_id: LoyaltyProgramId,
    /// Reward name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Points cost
    pub points_cost: u64,
    /// Reward type
    pub reward_type: RewardType,
    /// Monetary value
    pub value: Option<Decimal>,
}

/// Filter for listing loyalty accounts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoyaltyAccountFilter {
    /// Filter by customer
    pub customer_id: Option<CustomerId>,
    /// Filter by program
    pub program_id: Option<LoyaltyProgramId>,
    /// Filter by tier
    pub tier: Option<String>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Filter for listing rewards
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RewardFilter {
    /// Filter by program
    pub program_id: Option<LoyaltyProgramId>,
    /// Filter by reward type
    pub reward_type: Option<RewardType>,
    /// Only active rewards
    pub is_active: Option<bool>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl LoyaltyProgram {
    /// Get the tier for a given lifetime points value
    pub fn tier_for_points(&self, lifetime_points: u64) -> Option<&LoyaltyTier> {
        self.tiers
            .iter()
            .rev()
            .find(|tier| lifetime_points >= tier.min_points)
    }

    /// Whether the program is accepting new enrollments
    pub fn is_active(&self) -> bool {
        self.status == LoyaltyProgramStatus::Active
    }
}

impl LoyaltyAccount {
    /// Whether the account has enough points to redeem a reward
    pub fn can_redeem(&self, points_cost: u64) -> bool {
        self.points_balance >= 0 && (self.points_balance as u64) >= points_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stateset_primitives::{CustomerId, LoyaltyAccountId, LoyaltyProgramId};

    fn make_program_with_tiers() -> LoyaltyProgram {
        LoyaltyProgram {
            id: LoyaltyProgramId::new(),
            name: "Test Program".to_string(),
            description: None,
            points_per_dollar: 1,
            tiers: vec![
                LoyaltyTier {
                    name: "Bronze".to_string(),
                    min_points: 0,
                    multiplier: 1.0,
                    perks: vec![],
                },
                LoyaltyTier {
                    name: "Silver".to_string(),
                    min_points: 500,
                    multiplier: 1.5,
                    perks: vec![],
                },
                LoyaltyTier {
                    name: "Gold".to_string(),
                    min_points: 2000,
                    multiplier: 2.0,
                    perks: vec![],
                },
            ],
            status: LoyaltyProgramStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_account(points_balance: i64) -> LoyaltyAccount {
        LoyaltyAccount {
            id: LoyaltyAccountId::new(),
            customer_id: CustomerId::new(),
            program_id: LoyaltyProgramId::new(),
            points_balance,
            lifetime_points: points_balance.max(0) as u64,
            tier: "Bronze".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- tier_for_points ----

    #[test]
    fn tier_for_points_returns_bronze_at_zero() {
        let program = make_program_with_tiers();
        let tier = program.tier_for_points(0).unwrap();
        assert_eq!(tier.name, "Bronze");
    }

    #[test]
    fn tier_for_points_returns_silver_at_500() {
        let program = make_program_with_tiers();
        let tier = program.tier_for_points(500).unwrap();
        assert_eq!(tier.name, "Silver");
    }

    #[test]
    fn tier_for_points_returns_highest_tier_at_large_value() {
        let program = make_program_with_tiers();
        let tier = program.tier_for_points(10_000).unwrap();
        assert_eq!(tier.name, "Gold");
    }

    #[test]
    fn tier_for_points_returns_none_for_empty_tiers() {
        let program = LoyaltyProgram {
            tiers: vec![],
            ..make_program_with_tiers()
        };
        assert!(program.tier_for_points(0).is_none());
    }

    #[test]
    fn tier_for_points_returns_none_when_below_minimum() {
        // All tiers have min_points > 0
        let program = LoyaltyProgram {
            tiers: vec![
                LoyaltyTier { name: "Silver".to_string(), min_points: 500, multiplier: 1.5, perks: vec![] },
                LoyaltyTier { name: "Gold".to_string(), min_points: 2000, multiplier: 2.0, perks: vec![] },
            ],
            ..make_program_with_tiers()
        };
        assert!(program.tier_for_points(0).is_none());
    }

    // ---- is_active ----

    #[test]
    fn program_is_active_when_active() {
        let program = make_program_with_tiers();
        assert!(program.is_active());
    }

    #[test]
    fn program_is_not_active_when_paused() {
        let program = LoyaltyProgram { status: LoyaltyProgramStatus::Paused, ..make_program_with_tiers() };
        assert!(!program.is_active());
    }

    #[test]
    fn program_is_not_active_when_archived() {
        let program = LoyaltyProgram { status: LoyaltyProgramStatus::Archived, ..make_program_with_tiers() };
        assert!(!program.is_active());
    }

    // ---- can_redeem ----

    #[test]
    fn can_redeem_with_sufficient_points() {
        let account = make_account(1000);
        assert!(account.can_redeem(500));
    }

    #[test]
    fn can_redeem_with_exact_points() {
        let account = make_account(500);
        assert!(account.can_redeem(500));
    }

    #[test]
    fn cannot_redeem_with_insufficient_points() {
        let account = make_account(100);
        assert!(!account.can_redeem(500));
    }

    #[test]
    fn cannot_redeem_with_negative_balance() {
        let account = make_account(-100);
        assert!(!account.can_redeem(0));
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn loyalty_program_status_display_fromstr_roundtrip() {
        for status in [
            LoyaltyProgramStatus::Active,
            LoyaltyProgramStatus::Paused,
            LoyaltyProgramStatus::Archived,
        ] {
            let s = status.to_string();
            let parsed: LoyaltyProgramStatus = s.parse().unwrap();
            assert_eq!(parsed, status, "round-trip failed for {s}");
        }
    }

    #[test]
    fn loyalty_transaction_type_display_fromstr_roundtrip() {
        for tx_type in [
            LoyaltyTransactionType::Earn,
            LoyaltyTransactionType::Redeem,
            LoyaltyTransactionType::Adjust,
            LoyaltyTransactionType::Expire,
            LoyaltyTransactionType::Bonus,
            LoyaltyTransactionType::Refund,
        ] {
            let s = tx_type.to_string();
            let parsed: LoyaltyTransactionType = s.parse().unwrap();
            assert_eq!(parsed, tx_type, "round-trip failed for {s}");
        }
    }

    #[test]
    fn reward_type_display_fromstr_roundtrip() {
        for reward_type in [
            RewardType::Discount,
            RewardType::FreeShipping,
            RewardType::FreeProduct,
            RewardType::StoreCredit,
            RewardType::ExclusiveAccess,
        ] {
            let s = reward_type.to_string();
            let parsed: RewardType = s.parse().unwrap();
            assert_eq!(parsed, reward_type, "round-trip failed for {s}");
        }
    }

    // ---- Defaults ----

    #[test]
    fn loyalty_program_status_default_is_active() {
        assert_eq!(LoyaltyProgramStatus::default(), LoyaltyProgramStatus::Active);
    }

    #[test]
    fn loyalty_transaction_type_default_is_earn() {
        assert_eq!(LoyaltyTransactionType::default(), LoyaltyTransactionType::Earn);
    }

    #[test]
    fn reward_type_default_is_discount() {
        assert_eq!(RewardType::default(), RewardType::Discount);
    }
}
