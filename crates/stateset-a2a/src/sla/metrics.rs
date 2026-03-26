//! SLA metric types, penalty types, and SLA definition.

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Types of SLA metrics that can be monitored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SlaMetricType {
    /// Maximum response time in milliseconds.
    ResponseTimeMs,
    /// Minimum uptime percentage (0–100).
    UptimePercent,
    /// Minimum quality score (1–5).
    QualityMinScore,
    /// Minimum throughput in requests per second.
    ThroughputRps,
}

impl std::fmt::Display for SlaMetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResponseTimeMs => write!(f, "response_time_ms"),
            Self::UptimePercent => write!(f, "uptime_percent"),
            Self::QualityMinScore => write!(f, "quality_min_score"),
            Self::ThroughputRps => write!(f, "throughput_rps"),
        }
    }
}

/// Types of penalties for SLA violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PenaltyType {
    /// Account credit (default).
    Credit,
    /// Immediate refund to buyer.
    Refund,
    /// Temporary service suspension.
    Suspension,
}

impl Default for PenaltyType {
    fn default() -> Self {
        Self::Credit
    }
}

impl std::fmt::Display for PenaltyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Credit => write!(f, "credit"),
            Self::Refund => write!(f, "refund"),
            Self::Suspension => write!(f, "suspension"),
        }
    }
}

/// Default penalty percentage (5%).
pub const DEFAULT_PENALTY_PERCENT: Decimal = dec!(5);

/// An SLA definition attached to a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaDefinition {
    /// ID of the service this SLA applies to.
    pub service_id: Uuid,
    /// Maximum response time in milliseconds (if set).
    pub response_time_ms: Option<Decimal>,
    /// Minimum uptime percentage 0–100 (if set).
    pub uptime_percent: Option<Decimal>,
    /// Minimum quality score 1–5 (if set).
    pub quality_min_score: Option<Decimal>,
    /// Minimum throughput in RPS (if set).
    pub throughput_rps: Option<Decimal>,
    /// Penalty percentage of transaction value.
    pub penalty_percent: Decimal,
    /// Type of penalty to apply.
    pub penalty_type: PenaltyType,
    /// Whether this SLA is active.
    pub active: bool,
}

impl SlaDefinition {
    /// Create a new SLA definition with default penalty settings.
    #[must_use] 
    pub fn new(service_id: Uuid) -> Self {
        Self {
            service_id,
            response_time_ms: None,
            uptime_percent: None,
            quality_min_score: None,
            throughput_rps: None,
            penalty_percent: DEFAULT_PENALTY_PERCENT,
            penalty_type: PenaltyType::default(),
            active: true,
        }
    }

    /// Set the response time threshold.
    #[must_use] 
    pub const fn with_response_time(mut self, ms: Decimal) -> Self {
        self.response_time_ms = Some(ms);
        self
    }

    /// Set the uptime percentage threshold.
    #[must_use] 
    pub const fn with_uptime(mut self, percent: Decimal) -> Self {
        self.uptime_percent = Some(percent);
        self
    }

    /// Set the quality score threshold.
    #[must_use] 
    pub const fn with_quality(mut self, min_score: Decimal) -> Self {
        self.quality_min_score = Some(min_score);
        self
    }

    /// Set the throughput threshold.
    #[must_use] 
    pub const fn with_throughput(mut self, rps: Decimal) -> Self {
        self.throughput_rps = Some(rps);
        self
    }

    /// Set the penalty percentage.
    #[must_use] 
    pub const fn with_penalty_percent(mut self, percent: Decimal) -> Self {
        self.penalty_percent = percent;
        self
    }

    /// Set the penalty type.
    #[must_use] 
    pub const fn with_penalty_type(mut self, penalty_type: PenaltyType) -> Self {
        self.penalty_type = penalty_type;
        self
    }

    /// Check whether at least one metric is configured.
    #[must_use]
    pub const fn has_metrics(&self) -> bool {
        self.response_time_ms.is_some()
            || self.uptime_percent.is_some()
            || self.quality_min_score.is_some()
            || self.throughput_rps.is_some()
    }

    /// Get all configured metric types.
    #[must_use]
    pub fn configured_metrics(&self) -> Vec<SlaMetricType> {
        let mut metrics = Vec::new();
        if self.response_time_ms.is_some() {
            metrics.push(SlaMetricType::ResponseTimeMs);
        }
        if self.uptime_percent.is_some() {
            metrics.push(SlaMetricType::UptimePercent);
        }
        if self.quality_min_score.is_some() {
            metrics.push(SlaMetricType::QualityMinScore);
        }
        if self.throughput_rps.is_some() {
            metrics.push(SlaMetricType::ThroughputRps);
        }
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn metric_type_display() {
        assert_eq!(SlaMetricType::ResponseTimeMs.to_string(), "response_time_ms");
        assert_eq!(SlaMetricType::UptimePercent.to_string(), "uptime_percent");
        assert_eq!(SlaMetricType::QualityMinScore.to_string(), "quality_min_score");
        assert_eq!(SlaMetricType::ThroughputRps.to_string(), "throughput_rps");
    }

    #[test]
    fn penalty_type_display() {
        assert_eq!(PenaltyType::Credit.to_string(), "credit");
        assert_eq!(PenaltyType::Refund.to_string(), "refund");
        assert_eq!(PenaltyType::Suspension.to_string(), "suspension");
    }

    #[test]
    fn penalty_type_default() {
        assert_eq!(PenaltyType::default(), PenaltyType::Credit);
    }

    #[test]
    fn sla_definition_defaults() {
        let sla = SlaDefinition::new(Uuid::new_v4());
        assert_eq!(sla.penalty_percent, dec!(5));
        assert_eq!(sla.penalty_type, PenaltyType::Credit);
        assert!(sla.active);
        assert!(!sla.has_metrics());
    }

    #[test]
    fn sla_definition_builder() {
        let sla = SlaDefinition::new(Uuid::new_v4())
            .with_response_time(dec!(500))
            .with_uptime(dec!(99.9))
            .with_quality(dec!(4.0))
            .with_throughput(dec!(100))
            .with_penalty_percent(dec!(10))
            .with_penalty_type(PenaltyType::Refund);

        assert_eq!(sla.response_time_ms, Some(dec!(500)));
        assert_eq!(sla.uptime_percent, Some(dec!(99.9)));
        assert_eq!(sla.quality_min_score, Some(dec!(4.0)));
        assert_eq!(sla.throughput_rps, Some(dec!(100)));
        assert_eq!(sla.penalty_percent, dec!(10));
        assert_eq!(sla.penalty_type, PenaltyType::Refund);
    }

    #[test]
    fn has_metrics_none() {
        let sla = SlaDefinition::new(Uuid::new_v4());
        assert!(!sla.has_metrics());
    }

    #[test]
    fn has_metrics_with_one() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_response_time(dec!(500));
        assert!(sla.has_metrics());
    }

    #[test]
    fn configured_metrics_all() {
        let sla = SlaDefinition::new(Uuid::new_v4())
            .with_response_time(dec!(500))
            .with_uptime(dec!(99))
            .with_quality(dec!(4))
            .with_throughput(dec!(50));
        let metrics = sla.configured_metrics();
        assert_eq!(metrics.len(), 4);
    }

    #[test]
    fn configured_metrics_partial() {
        let sla = SlaDefinition::new(Uuid::new_v4()).with_uptime(dec!(99));
        let metrics = sla.configured_metrics();
        assert_eq!(metrics, vec![SlaMetricType::UptimePercent]);
    }

    #[test]
    fn sla_serde_roundtrip() {
        let sla = SlaDefinition::new(Uuid::new_v4())
            .with_response_time(dec!(200))
            .with_uptime(dec!(99.5));
        let json = serde_json::to_string(&sla).unwrap();
        let parsed: SlaDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.response_time_ms, sla.response_time_ms);
        assert_eq!(parsed.uptime_percent, sla.uptime_percent);
    }
}
