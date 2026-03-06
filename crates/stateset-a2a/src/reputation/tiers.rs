//! Trust tier definitions and promotion logic.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Transaction types for reputation tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TransactionType {
    /// A price quote.
    Quote,
    /// A direct payment.
    Payment,
    /// An escrow transaction.
    Escrow,
    /// A service call.
    Service,
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quote => write!(f, "quote"),
            Self::Payment => write!(f, "payment"),
            Self::Escrow => write!(f, "escrow"),
            Self::Service => write!(f, "service"),
        }
    }
}

/// Trust tiers for A2A agents, in ascending order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TrustTier {
    /// Default tier for new agents with no history.
    Sandbox,
    /// Basic trust: 5+ transactions, 3.5+ avg score.
    Standard,
    /// High trust: 25+ transactions, 4.0+ avg, 0 unresolved disputes.
    Verified,
    /// Maximum trust: 100+ transactions, 4.5+ avg, <2% dispute rate.
    Enterprise,
}

impl Default for TrustTier {
    fn default() -> Self {
        Self::Sandbox
    }
}

impl std::fmt::Display for TrustTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sandbox => write!(f, "sandbox"),
            Self::Standard => write!(f, "standard"),
            Self::Verified => write!(f, "verified"),
            Self::Enterprise => write!(f, "enterprise"),
        }
    }
}

/// Promotion thresholds for a specific trust tier.
#[derive(Debug, Clone)]
pub struct TierRequirements {
    /// Minimum number of completed transactions.
    pub min_transactions: u64,
    /// Minimum average score (1–5).
    pub min_avg_score: Decimal,
    /// Maximum number of unresolved disputes (None = no limit).
    pub max_unresolved_disputes: Option<u64>,
    /// Maximum dispute rate (None = no limit).
    pub max_dispute_rate: Option<Decimal>,
}

impl TrustTier {
    /// Get the promotion requirements for this tier.
    #[must_use]
    pub const fn requirements(self) -> TierRequirements {
        match self {
            Self::Sandbox => TierRequirements {
                min_transactions: 0,
                min_avg_score: Decimal::ZERO,
                max_unresolved_disputes: None,
                max_dispute_rate: None,
            },
            Self::Standard => TierRequirements {
                min_transactions: 5,
                min_avg_score: dec!(3.5),
                max_unresolved_disputes: None,
                max_dispute_rate: None,
            },
            Self::Verified => TierRequirements {
                min_transactions: 25,
                min_avg_score: dec!(4.0),
                max_unresolved_disputes: Some(0),
                max_dispute_rate: None,
            },
            Self::Enterprise => TierRequirements {
                min_transactions: 100,
                min_avg_score: dec!(4.5),
                max_unresolved_disputes: None,
                max_dispute_rate: Some(dec!(0.02)),
            },
        }
    }

    /// Whether this tier is at least as high as `other`.
    #[must_use]
    pub fn is_at_least(self, other: Self) -> bool {
        self >= other
    }

    /// Compute the appropriate trust tier for given statistics.
    ///
    /// Checks tiers in descending order (enterprise → verified → standard → sandbox)
    /// and returns the highest tier whose requirements are all met.
    #[must_use]
    pub fn compute_tier(
        total_transactions: u64,
        average_score: Decimal,
        unresolved_disputes: u64,
        dispute_rate: Decimal,
    ) -> Self {
        // Check tiers in descending order
        for &tier in &[Self::Enterprise, Self::Verified, Self::Standard] {
            let reqs = tier.requirements();

            if total_transactions < reqs.min_transactions {
                continue;
            }
            if average_score < reqs.min_avg_score {
                continue;
            }
            if let Some(max_disputes) = reqs.max_unresolved_disputes {
                if unresolved_disputes > max_disputes {
                    continue;
                }
            }
            if let Some(max_rate) = reqs.max_dispute_rate {
                if dispute_rate >= max_rate {
                    continue;
                }
            }
            return tier;
        }

        Self::Sandbox
    }

    /// All trust tiers in ascending order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Sandbox, Self::Standard, Self::Verified, Self::Enterprise]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn default_tier_is_sandbox() {
        assert_eq!(TrustTier::default(), TrustTier::Sandbox);
    }

    #[test]
    fn tier_ordering() {
        assert!(TrustTier::Sandbox < TrustTier::Standard);
        assert!(TrustTier::Standard < TrustTier::Verified);
        assert!(TrustTier::Verified < TrustTier::Enterprise);
    }

    #[test]
    fn is_at_least() {
        assert!(TrustTier::Enterprise.is_at_least(TrustTier::Sandbox));
        assert!(TrustTier::Verified.is_at_least(TrustTier::Verified));
        assert!(!TrustTier::Sandbox.is_at_least(TrustTier::Standard));
    }

    #[test]
    fn display_tiers() {
        assert_eq!(TrustTier::Sandbox.to_string(), "sandbox");
        assert_eq!(TrustTier::Standard.to_string(), "standard");
        assert_eq!(TrustTier::Verified.to_string(), "verified");
        assert_eq!(TrustTier::Enterprise.to_string(), "enterprise");
    }

    #[test]
    fn compute_tier_sandbox_default() {
        assert_eq!(TrustTier::compute_tier(0, Decimal::ZERO, 0, Decimal::ZERO), TrustTier::Sandbox);
    }

    #[test]
    fn compute_tier_sandbox_insufficient_transactions() {
        assert_eq!(TrustTier::compute_tier(4, dec!(4.5), 0, Decimal::ZERO), TrustTier::Sandbox);
    }

    #[test]
    fn compute_tier_sandbox_low_score() {
        assert_eq!(TrustTier::compute_tier(10, dec!(3.0), 0, Decimal::ZERO), TrustTier::Sandbox);
    }

    #[test]
    fn compute_tier_standard() {
        assert_eq!(TrustTier::compute_tier(5, dec!(3.5), 0, Decimal::ZERO), TrustTier::Standard);
    }

    #[test]
    fn compute_tier_standard_boundary() {
        // Exactly at threshold
        assert_eq!(TrustTier::compute_tier(5, dec!(3.5), 0, Decimal::ZERO), TrustTier::Standard);
        // Just below score threshold
        assert_eq!(TrustTier::compute_tier(5, dec!(3.49), 0, Decimal::ZERO), TrustTier::Sandbox);
    }

    #[test]
    fn compute_tier_verified() {
        assert_eq!(TrustTier::compute_tier(25, dec!(4.0), 0, Decimal::ZERO), TrustTier::Verified);
    }

    #[test]
    fn compute_tier_verified_blocked_by_disputes() {
        // 25 txns, 4.0 avg, but has unresolved disputes → standard
        assert_eq!(TrustTier::compute_tier(25, dec!(4.0), 1, Decimal::ZERO), TrustTier::Standard);
    }

    #[test]
    fn compute_tier_enterprise() {
        assert_eq!(TrustTier::compute_tier(100, dec!(4.5), 0, dec!(0.01)), TrustTier::Enterprise);
    }

    #[test]
    fn compute_tier_enterprise_boundary() {
        // Exactly 2% dispute rate → blocked
        assert_ne!(TrustTier::compute_tier(100, dec!(4.5), 0, dec!(0.02)), TrustTier::Enterprise);
        // Just under 2%
        assert_eq!(TrustTier::compute_tier(100, dec!(4.5), 0, dec!(0.019)), TrustTier::Enterprise);
    }

    #[test]
    fn compute_tier_enterprise_high_dispute_rate_falls_to_verified() {
        // High dispute rate blocks enterprise, but 0 unresolved allows verified
        assert_eq!(TrustTier::compute_tier(100, dec!(4.5), 0, dec!(0.05)), TrustTier::Verified);
    }

    #[test]
    fn transaction_type_display() {
        assert_eq!(TransactionType::Quote.to_string(), "quote");
        assert_eq!(TransactionType::Payment.to_string(), "payment");
        assert_eq!(TransactionType::Escrow.to_string(), "escrow");
        assert_eq!(TransactionType::Service.to_string(), "service");
    }

    #[test]
    fn all_tiers_returns_ascending() {
        let tiers = TrustTier::all();
        assert_eq!(tiers.len(), 4);
        assert_eq!(tiers[0], TrustTier::Sandbox);
        assert_eq!(tiers[3], TrustTier::Enterprise);
    }

    #[test]
    fn requirements_sandbox() {
        let r = TrustTier::Sandbox.requirements();
        assert_eq!(r.min_transactions, 0);
    }

    #[test]
    fn requirements_standard() {
        let r = TrustTier::Standard.requirements();
        assert_eq!(r.min_transactions, 5);
        assert_eq!(r.min_avg_score, dec!(3.5));
    }

    #[test]
    fn requirements_verified() {
        let r = TrustTier::Verified.requirements();
        assert_eq!(r.min_transactions, 25);
        assert_eq!(r.min_avg_score, dec!(4.0));
        assert_eq!(r.max_unresolved_disputes, Some(0));
    }

    #[test]
    fn requirements_enterprise() {
        let r = TrustTier::Enterprise.requirements();
        assert_eq!(r.min_transactions, 100);
        assert_eq!(r.min_avg_score, dec!(4.5));
        assert_eq!(r.max_dispute_rate, Some(dec!(0.02)));
    }
}
