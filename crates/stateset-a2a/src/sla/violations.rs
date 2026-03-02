//! SLA violation severity and penalty calculation.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::metrics::SlaMetricType;

/// Severity of an SLA violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ViolationSeverity {
    /// Minor violation: actual is within 80% of target.
    Warning,
    /// Major violation: actual is below 80% of target.
    Critical,
}

impl std::fmt::Display for ViolationSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// An SLA violation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaViolation {
    /// Which metric was violated.
    pub metric: SlaMetricType,
    /// The actual measured value.
    pub actual: Decimal,
    /// The required threshold.
    pub required: Decimal,
    /// Severity of the violation.
    pub severity: ViolationSeverity,
    /// Computed penalty amount.
    pub penalty_amount: Decimal,
}

/// Determine the severity of a violation based on the ratio of actual to expected.
///
/// For `ResponseTimeMs`, higher actual is worse (inverse relationship).
/// For other metrics, lower actual is worse.
#[must_use]
pub fn determine_severity(metric: SlaMetricType, actual: Decimal, required: Decimal) -> ViolationSeverity {
    if required.is_zero() {
        return ViolationSeverity::Critical;
    }

    let ratio = match metric {
        // For response time, lower is better — ratio is required/actual
        SlaMetricType::ResponseTimeMs => {
            if actual.is_zero() {
                return ViolationSeverity::Warning;
            }
            required / actual
        }
        // For uptime, quality, throughput — higher is better
        _ => actual / required,
    };

    if ratio > dec!(0.8) {
        ViolationSeverity::Warning
    } else {
        ViolationSeverity::Critical
    }
}

/// Compute the penalty amount based on the average transaction value and penalty percentage.
///
/// `penalty_amount = avg_transaction_value * (penalty_percent / 100)`
///
/// Result is rounded to 2 decimal places.
#[must_use]
pub fn compute_penalty_amount(avg_transaction_value: Decimal, penalty_percent: Decimal) -> Decimal {
    (avg_transaction_value * penalty_percent / dec!(100)).round_dp(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn severity_display() {
        assert_eq!(ViolationSeverity::Warning.to_string(), "warning");
        assert_eq!(ViolationSeverity::Critical.to_string(), "critical");
    }

    // ===== Response time severity =====

    #[test]
    fn response_time_slight_violation_is_warning() {
        // Required: 500ms, Actual: 600ms → ratio 500/600 ≈ 0.83 > 0.8
        let severity = determine_severity(SlaMetricType::ResponseTimeMs, dec!(600), dec!(500));
        assert_eq!(severity, ViolationSeverity::Warning);
    }

    #[test]
    fn response_time_severe_violation_is_critical() {
        // Required: 500ms, Actual: 1000ms → ratio 500/1000 = 0.5 < 0.8
        let severity = determine_severity(SlaMetricType::ResponseTimeMs, dec!(1000), dec!(500));
        assert_eq!(severity, ViolationSeverity::Critical);
    }

    #[test]
    fn response_time_zero_actual_is_warning() {
        let severity = determine_severity(SlaMetricType::ResponseTimeMs, Decimal::ZERO, dec!(500));
        assert_eq!(severity, ViolationSeverity::Warning);
    }

    // ===== Uptime severity =====

    #[test]
    fn uptime_slight_violation_is_warning() {
        // Required: 99%, Actual: 95% → ratio 95/99 ≈ 0.96 > 0.8
        let severity = determine_severity(SlaMetricType::UptimePercent, dec!(95), dec!(99));
        assert_eq!(severity, ViolationSeverity::Warning);
    }

    #[test]
    fn uptime_severe_violation_is_critical() {
        // Required: 99%, Actual: 50% → ratio 50/99 ≈ 0.51 < 0.8
        let severity = determine_severity(SlaMetricType::UptimePercent, dec!(50), dec!(99));
        assert_eq!(severity, ViolationSeverity::Critical);
    }

    // ===== Quality severity =====

    #[test]
    fn quality_slight_violation_is_warning() {
        // Required: 4.0, Actual: 3.5 → ratio 3.5/4.0 = 0.875 > 0.8
        let severity = determine_severity(SlaMetricType::QualityMinScore, dec!(3.5), dec!(4.0));
        assert_eq!(severity, ViolationSeverity::Warning);
    }

    #[test]
    fn quality_severe_violation_is_critical() {
        // Required: 4.0, Actual: 2.0 → ratio 2/4 = 0.5 < 0.8
        let severity = determine_severity(SlaMetricType::QualityMinScore, dec!(2.0), dec!(4.0));
        assert_eq!(severity, ViolationSeverity::Critical);
    }

    // ===== Throughput severity =====

    #[test]
    fn throughput_violation_warning() {
        // Required: 100 rps, Actual: 85 → ratio 0.85 > 0.8
        let severity = determine_severity(SlaMetricType::ThroughputRps, dec!(85), dec!(100));
        assert_eq!(severity, ViolationSeverity::Warning);
    }

    #[test]
    fn throughput_violation_critical() {
        // Required: 100 rps, Actual: 60 → ratio 0.6 < 0.8
        let severity = determine_severity(SlaMetricType::ThroughputRps, dec!(60), dec!(100));
        assert_eq!(severity, ViolationSeverity::Critical);
    }

    // ===== Zero required =====

    #[test]
    fn zero_required_is_critical() {
        let severity = determine_severity(SlaMetricType::UptimePercent, dec!(50), Decimal::ZERO);
        assert_eq!(severity, ViolationSeverity::Critical);
    }

    // ===== Penalty computation =====

    #[test]
    fn penalty_5_percent_of_100() {
        assert_eq!(compute_penalty_amount(dec!(100), dec!(5)), dec!(5));
    }

    #[test]
    fn penalty_10_percent_of_250() {
        assert_eq!(compute_penalty_amount(dec!(250), dec!(10)), dec!(25));
    }

    #[test]
    fn penalty_rounds_to_2dp() {
        // 33.33 * 7% = 2.3331 → 2.33
        assert_eq!(compute_penalty_amount(dec!(33.33), dec!(7)), dec!(2.33));
    }

    #[test]
    fn penalty_zero_value() {
        assert_eq!(compute_penalty_amount(Decimal::ZERO, dec!(5)), Decimal::ZERO);
    }

    #[test]
    fn penalty_zero_percent() {
        assert_eq!(compute_penalty_amount(dec!(100), Decimal::ZERO), Decimal::ZERO);
    }
}
