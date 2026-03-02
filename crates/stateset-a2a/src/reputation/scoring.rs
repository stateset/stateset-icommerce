//! Score calculation and feedback aggregation.

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::error::{A2AError, A2AResult};

use super::tiers::TrustTier;

/// The four scoring dimensions for agent reputation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ScoreDimension {
    /// Agent follows through on commitments.
    Reliability,
    /// Output/service quality.
    Quality,
    /// Response time and delivery speed.
    Speed,
    /// Clarity and responsiveness in communication.
    Communication,
}

impl std::fmt::Display for ScoreDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reliability => write!(f, "reliability"),
            Self::Quality => write!(f, "quality"),
            Self::Speed => write!(f, "speed"),
            Self::Communication => write!(f, "communication"),
        }
    }
}

/// Per-dimension scores for an agent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimensionScores {
    /// Reliability score (1–5, or 0 if no data).
    pub reliability: Decimal,
    /// Quality score (1–5, or 0 if no data).
    pub quality: Decimal,
    /// Speed score (1–5, or 0 if no data).
    pub speed: Decimal,
    /// Communication score (1–5, or 0 if no data).
    pub communication: Decimal,
}

impl DimensionScores {
    /// Get the score for a specific dimension.
    #[must_use]
    pub const fn get(&self, dimension: ScoreDimension) -> &Decimal {
        match dimension {
            ScoreDimension::Reliability => &self.reliability,
            ScoreDimension::Quality => &self.quality,
            ScoreDimension::Speed => &self.speed,
            ScoreDimension::Communication => &self.communication,
        }
    }

    /// Set the score for a specific dimension.
    pub const fn set(&mut self, dimension: ScoreDimension, value: Decimal) {
        match dimension {
            ScoreDimension::Reliability => self.reliability = value,
            ScoreDimension::Quality => self.quality = value,
            ScoreDimension::Speed => self.speed = value,
            ScoreDimension::Communication => self.communication = value,
        }
    }
}

/// A single feedback entry from a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    /// Overall score (1–5).
    pub score: Decimal,
    /// Per-dimension scores (optional).
    pub dimensions: Option<DimensionScores>,
    /// Whether this feedback has been revoked.
    pub revoked: bool,
}

/// Aggregated reputation summary for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationSummary {
    /// Total number of transactions.
    pub total_transactions: u64,
    /// Number of successful transactions (score >= 3).
    pub successful_transactions: u64,
    /// Number of disputed transactions (score <= 2).
    pub disputed_transactions: u64,
    /// Overall average score (0 if no transactions).
    pub average_score: Decimal,
    /// Per-dimension averages.
    pub dimension_scores: DimensionScores,
    /// Current trust tier.
    pub trust_tier: TrustTier,
}

impl Default for ReputationSummary {
    fn default() -> Self {
        Self {
            total_transactions: 0,
            successful_transactions: 0,
            disputed_transactions: 0,
            average_score: Decimal::ZERO,
            dimension_scores: DimensionScores::default(),
            trust_tier: TrustTier::Sandbox,
        }
    }
}

/// Validate that a score is within the valid 1–5 range.
///
/// # Errors
///
/// Returns [`A2AError::ScoreOutOfRange`] if the score is outside 1–5.
pub fn validate_score(score: Decimal) -> A2AResult<()> {
    if score < dec!(1) || score > dec!(5) {
        return Err(A2AError::ScoreOutOfRange { value: score });
    }
    Ok(())
}

/// Compute the average of a slice of scores, rounded to 2 decimal places.
///
/// Returns `Decimal::ZERO` if the slice is empty.
#[must_use]
pub fn average_scores(scores: &[Decimal]) -> Decimal {
    if scores.is_empty() {
        return Decimal::ZERO;
    }
    let sum: Decimal = scores.iter().sum();
    let count = Decimal::from(scores.len() as u64);
    (sum / count).round_dp(2)
}

/// Aggregate feedback entries into a [`ReputationSummary`].
///
/// Only non-revoked feedback entries are considered. The trust tier is
/// calculated based on the aggregated statistics.
#[must_use]
pub fn aggregate_feedback(entries: &[FeedbackEntry]) -> ReputationSummary {
    let active: Vec<&FeedbackEntry> = entries.iter().filter(|e| !e.revoked).collect();

    if active.is_empty() {
        return ReputationSummary::default();
    }

    let total_transactions = active.len() as u64;
    let scores: Vec<Decimal> = active.iter().map(|e| e.score).collect();

    let successful_transactions = scores.iter().filter(|&&s| s >= dec!(3)).count() as u64;
    let disputed_transactions = scores.iter().filter(|&&s| s <= dec!(2)).count() as u64;
    let average_score = average_scores(&scores);

    // Aggregate dimension scores
    let dimension_scores = aggregate_dimensions(&active);

    // Determine trust tier
    let dispute_rate = if total_transactions > 0 {
        Decimal::from(disputed_transactions)
            / Decimal::from(total_transactions)
    } else {
        Decimal::ZERO
    };
    let trust_tier =
        TrustTier::compute_tier(total_transactions, average_score, disputed_transactions, dispute_rate);

    ReputationSummary {
        total_transactions,
        successful_transactions,
        disputed_transactions,
        average_score,
        dimension_scores,
        trust_tier,
    }
}

/// Aggregate per-dimension scores from feedback entries.
fn aggregate_dimensions(entries: &[&FeedbackEntry]) -> DimensionScores {
    let dimensions = [
        ScoreDimension::Reliability,
        ScoreDimension::Quality,
        ScoreDimension::Speed,
        ScoreDimension::Communication,
    ];

    let mut result = DimensionScores::default();

    for &dim in &dimensions {
        let dim_scores: Vec<Decimal> = entries
            .iter()
            .filter_map(|e| e.dimensions.as_ref())
            .map(|d| *d.get(dim))
            .filter(|&s| s > Decimal::ZERO)
            .collect();

        if !dim_scores.is_empty() {
            result.set(dim, average_scores(&dim_scores));
        }
    }

    result
}

/// Compute the dispute rate (0.0–1.0) from transaction counts.
#[must_use]
pub fn dispute_rate(disputed: u64, total: u64) -> Decimal {
    if total == 0 {
        return Decimal::ZERO;
    }
    (Decimal::from(disputed) / Decimal::from(total)).round_dp(4)
}

/// Check whether a score counts as "successful" (>= 3).
#[must_use]
pub fn is_successful(score: Decimal) -> bool {
    score >= dec!(3)
}

/// Check whether a score counts as "disputed" (<= 2).
#[must_use]
pub fn is_disputed(score: Decimal) -> bool {
    score <= dec!(2)
}

/// Compute a score distribution histogram (counts for scores 1–5).
#[must_use]
pub fn score_distribution(scores: &[Decimal]) -> [u64; 5] {
    let mut dist = [0u64; 5];
    for &s in scores {
        if let Some(idx) = s.to_u64() {
            if (1..=5).contains(&idx) {
                dist[(idx - 1) as usize] += 1;
            }
        }
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn validate_score_valid_range() {
        assert!(validate_score(dec!(1)).is_ok());
        assert!(validate_score(dec!(3)).is_ok());
        assert!(validate_score(dec!(5)).is_ok());
        assert!(validate_score(dec!(2.5)).is_ok());
    }

    #[test]
    fn validate_score_out_of_range_low() {
        let err = validate_score(dec!(0)).unwrap_err();
        assert!(matches!(err, A2AError::ScoreOutOfRange { .. }));
    }

    #[test]
    fn validate_score_out_of_range_high() {
        let err = validate_score(dec!(6)).unwrap_err();
        assert!(matches!(err, A2AError::ScoreOutOfRange { .. }));
    }

    #[test]
    fn validate_score_negative() {
        let err = validate_score(dec!(-1)).unwrap_err();
        assert!(matches!(err, A2AError::ScoreOutOfRange { .. }));
    }

    #[test]
    fn average_scores_empty() {
        assert_eq!(average_scores(&[]), Decimal::ZERO);
    }

    #[test]
    fn average_scores_single() {
        assert_eq!(average_scores(&[dec!(4)]), dec!(4));
    }

    #[test]
    fn average_scores_multiple() {
        let scores = vec![dec!(4), dec!(5), dec!(3)];
        assert_eq!(average_scores(&scores), dec!(4));
    }

    #[test]
    fn average_scores_fractional() {
        let scores = vec![dec!(4), dec!(3)];
        assert_eq!(average_scores(&scores), dec!(3.5));
    }

    #[test]
    fn average_scores_rounds_to_2dp() {
        let scores = vec![dec!(1), dec!(1), dec!(2)];
        // 4/3 = 1.333...
        assert_eq!(average_scores(&scores), dec!(1.33));
    }

    #[test]
    fn is_successful_boundary() {
        assert!(is_successful(dec!(3)));
        assert!(is_successful(dec!(4)));
        assert!(is_successful(dec!(5)));
        assert!(!is_successful(dec!(2)));
        assert!(!is_successful(dec!(1)));
    }

    #[test]
    fn is_disputed_boundary() {
        assert!(is_disputed(dec!(2)));
        assert!(is_disputed(dec!(1)));
        assert!(!is_disputed(dec!(3)));
        assert!(!is_disputed(dec!(4)));
    }

    #[test]
    fn dispute_rate_zero_total() {
        assert_eq!(dispute_rate(0, 0), Decimal::ZERO);
    }

    #[test]
    fn dispute_rate_no_disputes() {
        assert_eq!(dispute_rate(0, 100), Decimal::ZERO);
    }

    #[test]
    fn dispute_rate_calculation() {
        assert_eq!(dispute_rate(2, 100), dec!(0.02));
    }

    #[test]
    fn dispute_rate_all_disputed() {
        assert_eq!(dispute_rate(10, 10), dec!(1));
    }

    #[test]
    fn score_distribution_empty() {
        assert_eq!(score_distribution(&[]), [0, 0, 0, 0, 0]);
    }

    #[test]
    fn score_distribution_all_fives() {
        let scores = vec![dec!(5), dec!(5), dec!(5)];
        assert_eq!(score_distribution(&scores), [0, 0, 0, 0, 3]);
    }

    #[test]
    fn score_distribution_mixed() {
        let scores = vec![dec!(1), dec!(2), dec!(3), dec!(4), dec!(5), dec!(5)];
        assert_eq!(score_distribution(&scores), [1, 1, 1, 1, 2]);
    }

    #[test]
    fn dimension_scores_get_set() {
        let mut ds = DimensionScores::default();
        ds.set(ScoreDimension::Quality, dec!(4.5));
        assert_eq!(*ds.get(ScoreDimension::Quality), dec!(4.5));
        assert_eq!(*ds.get(ScoreDimension::Speed), Decimal::ZERO);
    }

    #[test]
    fn dimension_display() {
        assert_eq!(ScoreDimension::Reliability.to_string(), "reliability");
        assert_eq!(ScoreDimension::Quality.to_string(), "quality");
        assert_eq!(ScoreDimension::Speed.to_string(), "speed");
        assert_eq!(ScoreDimension::Communication.to_string(), "communication");
    }

    #[test]
    fn aggregate_feedback_empty() {
        let summary = aggregate_feedback(&[]);
        assert_eq!(summary.total_transactions, 0);
        assert_eq!(summary.average_score, Decimal::ZERO);
        assert_eq!(summary.trust_tier, TrustTier::Sandbox);
    }

    #[test]
    fn aggregate_feedback_skips_revoked() {
        let entries = vec![
            FeedbackEntry { score: dec!(5), dimensions: None, revoked: false },
            FeedbackEntry { score: dec!(1), dimensions: None, revoked: true },
        ];
        let summary = aggregate_feedback(&entries);
        assert_eq!(summary.total_transactions, 1);
        assert_eq!(summary.average_score, dec!(5));
    }

    #[test]
    fn aggregate_feedback_counts_successful_and_disputed() {
        let entries = vec![
            FeedbackEntry { score: dec!(5), dimensions: None, revoked: false },
            FeedbackEntry { score: dec!(4), dimensions: None, revoked: false },
            FeedbackEntry { score: dec!(1), dimensions: None, revoked: false },
            FeedbackEntry { score: dec!(2), dimensions: None, revoked: false },
        ];
        let summary = aggregate_feedback(&entries);
        assert_eq!(summary.total_transactions, 4);
        assert_eq!(summary.successful_transactions, 2); // 5, 4
        assert_eq!(summary.disputed_transactions, 2); // 1, 2
    }

    #[test]
    fn aggregate_feedback_with_dimensions() {
        let dims = DimensionScores {
            reliability: dec!(4),
            quality: dec!(5),
            speed: dec!(3),
            communication: dec!(4),
        };
        let entries = vec![
            FeedbackEntry { score: dec!(4), dimensions: Some(dims.clone()), revoked: false },
            FeedbackEntry { score: dec!(4), dimensions: Some(dims), revoked: false },
        ];
        let summary = aggregate_feedback(&entries);
        assert_eq!(*summary.dimension_scores.get(ScoreDimension::Quality), dec!(5));
        assert_eq!(*summary.dimension_scores.get(ScoreDimension::Speed), dec!(3));
    }

    #[test]
    fn aggregate_feedback_dimension_averaging() {
        let d1 = DimensionScores {
            reliability: dec!(4),
            quality: dec!(5),
            speed: dec!(2),
            communication: dec!(3),
        };
        let d2 = DimensionScores {
            reliability: dec!(2),
            quality: dec!(3),
            speed: dec!(4),
            communication: dec!(5),
        };
        let entries = vec![
            FeedbackEntry { score: dec!(4), dimensions: Some(d1), revoked: false },
            FeedbackEntry { score: dec!(3), dimensions: Some(d2), revoked: false },
        ];
        let summary = aggregate_feedback(&entries);
        // reliability: (4+2)/2 = 3, quality: (5+3)/2 = 4, speed: (2+4)/2 = 3, comm: (3+5)/2 = 4
        assert_eq!(*summary.dimension_scores.get(ScoreDimension::Reliability), dec!(3));
        assert_eq!(*summary.dimension_scores.get(ScoreDimension::Quality), dec!(4));
        assert_eq!(*summary.dimension_scores.get(ScoreDimension::Speed), dec!(3));
        assert_eq!(*summary.dimension_scores.get(ScoreDimension::Communication), dec!(4));
    }

    #[test]
    fn reputation_summary_default() {
        let summary = ReputationSummary::default();
        assert_eq!(summary.total_transactions, 0);
        assert_eq!(summary.trust_tier, TrustTier::Sandbox);
    }

    #[test]
    fn aggregate_feedback_standard_tier_promotion() {
        // 6 transactions with avg >= 3.5 → standard
        let entries: Vec<FeedbackEntry> = (0..6)
            .map(|_| FeedbackEntry { score: dec!(4), dimensions: None, revoked: false })
            .collect();
        let summary = aggregate_feedback(&entries);
        assert_eq!(summary.trust_tier, TrustTier::Standard);
    }

    #[test]
    fn aggregate_feedback_verified_tier_promotion() {
        // 26 transactions, avg 4.0+, 0 disputes → verified
        let entries: Vec<FeedbackEntry> = (0..26)
            .map(|_| FeedbackEntry { score: dec!(4), dimensions: None, revoked: false })
            .collect();
        let summary = aggregate_feedback(&entries);
        assert_eq!(summary.trust_tier, TrustTier::Verified);
    }

    #[test]
    fn aggregate_feedback_enterprise_tier_promotion() {
        // 101 transactions, avg 4.5+, <2% dispute rate → enterprise
        let entries: Vec<FeedbackEntry> = (0..101)
            .map(|_| FeedbackEntry { score: dec!(5), dimensions: None, revoked: false })
            .collect();
        let summary = aggregate_feedback(&entries);
        assert_eq!(summary.trust_tier, TrustTier::Enterprise);
    }
}
