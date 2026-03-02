//! RFQ response scoring functions.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

/// Scoring criteria for ranking RFQ responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ScoringCriteria {
    /// Pure price minimization: `score = 1 / total_price`.
    Cheapest,
    /// Blended: 40% reputation + 60% price.
    BestValue,
    /// 50% response speed + 50% price.
    Fastest,
}

impl std::fmt::Display for ScoringCriteria {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cheapest => write!(f, "cheapest"),
            Self::BestValue => write!(f, "best_value"),
            Self::Fastest => write!(f, "fastest"),
        }
    }
}

/// An RFQ response with associated metadata for scoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfqResponse {
    /// Seller address or ID.
    pub seller: String,
    /// Total price quoted.
    pub total_price: Decimal,
    /// Seller's average reputation score (1–5, default 3 if unknown).
    pub reputation_score: Option<Decimal>,
    /// Response time in milliseconds (time to quote).
    pub response_time_ms: Option<u64>,
    /// Computed score (set by scoring functions).
    pub score: Option<Decimal>,
    /// Rank among all responses (1 = best).
    pub rank: Option<u32>,
}

impl RfqResponse {
    /// Create a new RFQ response.
    pub fn new(seller: impl Into<String>, total_price: Decimal) -> Self {
        Self {
            seller: seller.into(),
            total_price,
            reputation_score: None,
            response_time_ms: None,
            score: None,
            rank: None,
        }
    }

    /// Set the reputation score.
    pub const fn with_reputation(mut self, score: Decimal) -> Self {
        self.reputation_score = Some(score);
        self
    }

    /// Set the response time.
    pub const fn with_response_time(mut self, ms: u64) -> Self {
        self.response_time_ms = Some(ms);
        self
    }
}

/// Default reputation score used when no reputation data is available.
const DEFAULT_REPUTATION: Decimal = dec!(3);

/// Score a single response based on the given criteria.
///
/// Returns the computed score (higher is better).
#[must_use]
pub fn score_response(response: &RfqResponse, criteria: ScoringCriteria) -> Decimal {
    match criteria {
        ScoringCriteria::Cheapest => score_cheapest(response),
        ScoringCriteria::BestValue => score_best_value(response),
        ScoringCriteria::Fastest => score_fastest(response),
    }
}

/// Cheapest: `1 / total_price` (higher score = lower price).
fn score_cheapest(response: &RfqResponse) -> Decimal {
    if response.total_price.is_zero() {
        return Decimal::MAX;
    }
    (Decimal::ONE / response.total_price).round_dp(8)
}

/// Best value: 40% reputation + 60% price.
/// `score = (reputation * 0.4) + ((1/price) * 100 * 0.6)`
fn score_best_value(response: &RfqResponse) -> Decimal {
    let reputation = response.reputation_score.unwrap_or(DEFAULT_REPUTATION);
    let price_factor = if response.total_price.is_zero() {
        dec!(100)
    } else {
        (Decimal::ONE / response.total_price * dec!(100)).round_dp(8)
    };

    let score = reputation * dec!(0.4) + price_factor * dec!(0.6);
    score.round_dp(6)
}

/// Fastest: 50% response time + 50% price.
/// `score = ((1/response_time) * 1000 * 0.5) + ((1/price) * 100 * 0.5)`
fn score_fastest(response: &RfqResponse) -> Decimal {
    let time_ms = response.response_time_ms.unwrap_or(1000);
    let time_factor = if time_ms == 0 {
        dec!(1000)
    } else {
        (Decimal::ONE / Decimal::from(time_ms) * dec!(1000)).round_dp(8)
    };

    let price_factor = if response.total_price.is_zero() {
        dec!(100)
    } else {
        (Decimal::ONE / response.total_price * dec!(100)).round_dp(8)
    };

    let score = time_factor * dec!(0.5) + price_factor * dec!(0.5);
    score.round_dp(6)
}

/// Score and rank all responses using the given criteria.
///
/// Returns responses sorted by descending score with rank assigned.
#[must_use]
pub fn rank_responses(responses: &[RfqResponse], criteria: ScoringCriteria) -> Vec<RfqResponse> {
    let mut scored: Vec<RfqResponse> = responses
        .iter()
        .map(|r| {
            let mut scored_r = r.clone();
            scored_r.score = Some(score_response(r, criteria));
            scored_r
        })
        .collect();

    // Sort by descending score
    scored.sort_by(|a, b| {
        b.score
            .unwrap_or(Decimal::ZERO)
            .cmp(&a.score.unwrap_or(Decimal::ZERO))
    });

    // Assign ranks
    for (i, r) in scored.iter_mut().enumerate() {
        r.rank = Some((i + 1) as u32);
    }

    scored
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn scoring_criteria_display() {
        assert_eq!(ScoringCriteria::Cheapest.to_string(), "cheapest");
        assert_eq!(ScoringCriteria::BestValue.to_string(), "best_value");
        assert_eq!(ScoringCriteria::Fastest.to_string(), "fastest");
    }

    // ===== Cheapest scoring =====

    #[test]
    fn cheapest_lower_price_wins() {
        let cheap = RfqResponse::new("A", dec!(100));
        let expensive = RfqResponse::new("B", dec!(200));

        let s1 = score_response(&cheap, ScoringCriteria::Cheapest);
        let s2 = score_response(&expensive, ScoringCriteria::Cheapest);
        assert!(s1 > s2);
    }

    #[test]
    fn cheapest_equal_prices() {
        let r1 = RfqResponse::new("A", dec!(100));
        let r2 = RfqResponse::new("B", dec!(100));

        let s1 = score_response(&r1, ScoringCriteria::Cheapest);
        let s2 = score_response(&r2, ScoringCriteria::Cheapest);
        assert_eq!(s1, s2);
    }

    #[test]
    fn cheapest_zero_price() {
        let r = RfqResponse::new("A", Decimal::ZERO);
        let s = score_response(&r, ScoringCriteria::Cheapest);
        assert_eq!(s, Decimal::MAX);
    }

    // ===== Best value scoring =====

    #[test]
    fn best_value_reputation_matters() {
        let high_rep = RfqResponse::new("A", dec!(100)).with_reputation(dec!(5));
        let low_rep = RfqResponse::new("B", dec!(100)).with_reputation(dec!(1));

        let s1 = score_response(&high_rep, ScoringCriteria::BestValue);
        let s2 = score_response(&low_rep, ScoringCriteria::BestValue);
        assert!(s1 > s2);
    }

    #[test]
    fn best_value_price_matters() {
        let cheap = RfqResponse::new("A", dec!(100)).with_reputation(dec!(3));
        let expensive = RfqResponse::new("B", dec!(200)).with_reputation(dec!(3));

        let s1 = score_response(&cheap, ScoringCriteria::BestValue);
        let s2 = score_response(&expensive, ScoringCriteria::BestValue);
        assert!(s1 > s2);
    }

    #[test]
    fn best_value_default_reputation() {
        let r = RfqResponse::new("A", dec!(100));
        let s = score_response(&r, ScoringCriteria::BestValue);
        // Default reputation = 3, so 3 * 0.4 + (1/100 * 100) * 0.6 = 1.2 + 0.6 = 1.8
        assert!(s > Decimal::ZERO);
    }

    // ===== Fastest scoring =====

    #[test]
    fn fastest_speed_matters() {
        let fast = RfqResponse::new("A", dec!(100)).with_response_time(100);
        let slow = RfqResponse::new("B", dec!(100)).with_response_time(1000);

        let s1 = score_response(&fast, ScoringCriteria::Fastest);
        let s2 = score_response(&slow, ScoringCriteria::Fastest);
        assert!(s1 > s2);
    }

    #[test]
    fn fastest_price_also_matters() {
        let r1 = RfqResponse::new("A", dec!(100)).with_response_time(500);
        let r2 = RfqResponse::new("B", dec!(200)).with_response_time(500);

        let s1 = score_response(&r1, ScoringCriteria::Fastest);
        let s2 = score_response(&r2, ScoringCriteria::Fastest);
        assert!(s1 > s2);
    }

    #[test]
    fn fastest_default_time() {
        let r = RfqResponse::new("A", dec!(100));
        let s = score_response(&r, ScoringCriteria::Fastest);
        assert!(s > Decimal::ZERO);
    }

    // ===== Ranking =====

    #[test]
    fn rank_responses_cheapest() {
        let responses = vec![
            RfqResponse::new("Expensive", dec!(300)),
            RfqResponse::new("Cheap", dec!(100)),
            RfqResponse::new("Medium", dec!(200)),
        ];

        let ranked = rank_responses(&responses, ScoringCriteria::Cheapest);

        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].seller, "Cheap");
        assert_eq!(ranked[0].rank, Some(1));
        assert_eq!(ranked[1].seller, "Medium");
        assert_eq!(ranked[1].rank, Some(2));
        assert_eq!(ranked[2].seller, "Expensive");
        assert_eq!(ranked[2].rank, Some(3));
    }

    #[test]
    fn rank_responses_empty() {
        let ranked = rank_responses(&[], ScoringCriteria::Cheapest);
        assert!(ranked.is_empty());
    }

    #[test]
    fn rank_responses_single() {
        let responses = vec![RfqResponse::new("Only", dec!(100))];
        let ranked = rank_responses(&responses, ScoringCriteria::Cheapest);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].rank, Some(1));
    }

    #[test]
    fn rank_responses_all_scored() {
        let responses = vec![
            RfqResponse::new("A", dec!(100)),
            RfqResponse::new("B", dec!(200)),
        ];
        let ranked = rank_responses(&responses, ScoringCriteria::BestValue);
        assert!(ranked.iter().all(|r| r.score.is_some()));
    }

    #[test]
    fn rfq_response_builder() {
        let r = RfqResponse::new("seller1", dec!(500))
            .with_reputation(dec!(4.5))
            .with_response_time(200);
        assert_eq!(r.seller, "seller1");
        assert_eq!(r.total_price, dec!(500));
        assert_eq!(r.reputation_score, Some(dec!(4.5)));
        assert_eq!(r.response_time_ms, Some(200));
    }

    #[test]
    fn scoring_criteria_serde_roundtrip() {
        let json = serde_json::to_string(&ScoringCriteria::BestValue).unwrap();
        assert_eq!(json, "\"best_value\"");
        let parsed: ScoringCriteria = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ScoringCriteria::BestValue);
    }
}
