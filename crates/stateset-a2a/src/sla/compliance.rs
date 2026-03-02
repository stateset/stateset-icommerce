//! SLA compliance checking.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use super::metrics::{SlaDefinition, SlaMetricType};
use super::violations::{SlaViolation, compute_penalty_amount, determine_severity};
use crate::error::{A2AError, A2AResult};

/// Actual measured values for SLA compliance checking.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActualMetrics {
    /// Average response time in milliseconds.
    pub avg_response_time_ms: Option<Decimal>,
    /// Success rate (0.0–1.0) for uptime calculation.
    pub success_rate: Option<Decimal>,
    /// Average quality score (1–5).
    pub avg_quality_score: Option<Decimal>,
    /// Throughput in requests per second.
    pub throughput_rps: Option<Decimal>,
}

/// Result of SLA compliance checking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceResult {
    /// Whether all configured metrics are compliant.
    pub compliant: bool,
    /// List of violations (empty if compliant).
    pub violations: Vec<SlaViolation>,
    /// Total penalty amount across all violations.
    pub total_penalty: Decimal,
}

/// Check compliance of actual metrics against an SLA definition.
///
/// All configured metrics must pass for the result to be compliant.
///
/// # Errors
///
/// Returns [`A2AError::Validation`] if no metrics are configured on the SLA.
pub fn check_compliance(
    sla: &SlaDefinition,
    actual: &ActualMetrics,
    avg_transaction_value: Decimal,
) -> A2AResult<ComplianceResult> {
    if !sla.has_metrics() {
        return Err(A2AError::validation("SLA has no metrics configured"));
    }

    let mut violations = Vec::new();

    // Response time check
    if let (Some(required), Some(actual_val)) = (sla.response_time_ms, actual.avg_response_time_ms) {
        if actual_val > required {
            let severity = determine_severity(SlaMetricType::ResponseTimeMs, actual_val, required);
            violations.push(SlaViolation {
                metric: SlaMetricType::ResponseTimeMs,
                actual: actual_val,
                required,
                severity,
                penalty_amount: compute_penalty_amount(avg_transaction_value, sla.penalty_percent),
            });
        }
    }

    // Uptime check (convert success_rate 0-1 to percentage 0-100)
    if let (Some(required), Some(success_rate)) = (sla.uptime_percent, actual.success_rate) {
        let actual_percent = success_rate * dec!(100);
        if actual_percent < required {
            let severity = determine_severity(SlaMetricType::UptimePercent, actual_percent, required);
            violations.push(SlaViolation {
                metric: SlaMetricType::UptimePercent,
                actual: actual_percent,
                required,
                severity,
                penalty_amount: compute_penalty_amount(avg_transaction_value, sla.penalty_percent),
            });
        }
    }

    // Quality check
    if let (Some(required), Some(actual_val)) = (sla.quality_min_score, actual.avg_quality_score) {
        if actual_val < required {
            let severity = determine_severity(SlaMetricType::QualityMinScore, actual_val, required);
            violations.push(SlaViolation {
                metric: SlaMetricType::QualityMinScore,
                actual: actual_val,
                required,
                severity,
                penalty_amount: compute_penalty_amount(avg_transaction_value, sla.penalty_percent),
            });
        }
    }

    // Throughput check
    if let (Some(required), Some(actual_val)) = (sla.throughput_rps, actual.throughput_rps) {
        if actual_val < required {
            let severity = determine_severity(SlaMetricType::ThroughputRps, actual_val, required);
            violations.push(SlaViolation {
                metric: SlaMetricType::ThroughputRps,
                actual: actual_val,
                required,
                severity,
                penalty_amount: compute_penalty_amount(avg_transaction_value, sla.penalty_percent),
            });
        }
    }

    let total_penalty = violations.iter().map(|v| v.penalty_amount).sum();
    let compliant = violations.is_empty();

    Ok(ComplianceResult { compliant, violations, total_penalty })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::violations::ViolationSeverity;
    use rust_decimal_macros::dec;
    use uuid::Uuid;

    fn sla_with_all_metrics() -> SlaDefinition {
        SlaDefinition::new(Uuid::new_v4())
            .with_response_time(dec!(500))
            .with_uptime(dec!(99))
            .with_quality(dec!(4.0))
            .with_throughput(dec!(100))
    }

    #[test]
    fn all_compliant() {
        let sla = sla_with_all_metrics();
        let actual = ActualMetrics {
            avg_response_time_ms: Some(dec!(400)),
            success_rate: Some(dec!(0.995)),
            avg_quality_score: Some(dec!(4.5)),
            throughput_rps: Some(dec!(120)),
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(result.compliant);
        assert!(result.violations.is_empty());
        assert_eq!(result.total_penalty, Decimal::ZERO);
    }

    #[test]
    fn response_time_violation() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_response_time(dec!(500));
        let actual = ActualMetrics {
            avg_response_time_ms: Some(dec!(600)),
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].metric, SlaMetricType::ResponseTimeMs);
    }

    #[test]
    fn uptime_violation() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_uptime(dec!(99));
        let actual = ActualMetrics {
            success_rate: Some(dec!(0.95)), // 95% < 99%
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(!result.compliant);
        assert_eq!(result.violations[0].metric, SlaMetricType::UptimePercent);
        assert_eq!(result.violations[0].actual, dec!(95)); // 0.95 * 100
    }

    #[test]
    fn quality_violation() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_quality(dec!(4.0));
        let actual = ActualMetrics {
            avg_quality_score: Some(dec!(3.5)),
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(!result.compliant);
        assert_eq!(result.violations[0].metric, SlaMetricType::QualityMinScore);
    }

    #[test]
    fn throughput_violation() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_throughput(dec!(100));
        let actual = ActualMetrics {
            throughput_rps: Some(dec!(80)),
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(!result.compliant);
        assert_eq!(result.violations[0].metric, SlaMetricType::ThroughputRps);
    }

    #[test]
    fn multiple_violations() {
        let sla = sla_with_all_metrics();
        let actual = ActualMetrics {
            avg_response_time_ms: Some(dec!(800)),
            success_rate: Some(dec!(0.90)),
            avg_quality_score: Some(dec!(3.0)),
            throughput_rps: Some(dec!(50)),
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(!result.compliant);
        assert_eq!(result.violations.len(), 4);
        assert_eq!(result.total_penalty, dec!(20)); // 4 * (100 * 5%)
    }

    #[test]
    fn penalty_amount_computed() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_response_time(dec!(500));
        let actual = ActualMetrics {
            avg_response_time_ms: Some(dec!(600)),
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(200)).unwrap();
        assert_eq!(result.violations[0].penalty_amount, dec!(10)); // 200 * 5%
    }

    #[test]
    fn missing_actual_metric_is_not_violation() {
        let sla = SlaDefinition::new(Uuid::new_v4())
            .with_response_time(dec!(500))
            .with_uptime(dec!(99));
        let actual = ActualMetrics {
            avg_response_time_ms: Some(dec!(400)),
            // uptime not provided
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(result.compliant);
    }

    #[test]
    fn no_metrics_configured_error() {
        let sla = SlaDefinition::new(Uuid::new_v4());
        let actual = ActualMetrics::default();
        let err = check_compliance(&sla, &actual, dec!(100)).unwrap_err();
        assert!(matches!(err, A2AError::Validation(_)));
    }

    #[test]
    fn violation_severity_included() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_uptime(dec!(99));
        let actual = ActualMetrics {
            success_rate: Some(dec!(0.50)), // 50% < 80% of 99 → critical
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert_eq!(result.violations[0].severity, ViolationSeverity::Critical);
    }

    #[test]
    fn at_boundary_passes() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_response_time(dec!(500));
        let actual = ActualMetrics {
            avg_response_time_ms: Some(dec!(500)),
            ..ActualMetrics::default()
        };
        let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
        assert!(result.compliant);
    }
}
