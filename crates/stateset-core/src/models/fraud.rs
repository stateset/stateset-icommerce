//! Fraud detection domain models
//!
//! Provides signal-based fraud assessment with configurable rules and
//! manual review workflows.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stateset_primitives::{FraudRuleId, OrderId};
use strum::{Display, EnumString};

/// Types of fraud signals that can be detected
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum FraudSignalType {
    /// Unusually high order velocity for this customer
    VelocitySpike,
    /// Billing and shipping addresses don't match
    AddressMismatch,
    /// First order from this customer is unusually high value
    HighValueFirstOrder,
    /// Customer IP geolocates to a different country than billing address
    GeoIpAnomaly,
    /// Card BIN country doesn't match billing country
    BinCountryMismatch,
    /// Known suspicious device fingerprint
    DeviceFingerprint,
    /// Connection through proxy or VPN
    ProxyVpn,
    /// Email address matches disposable email pattern
    DisposableEmail,
    /// Multiple failed payment attempts
    PaymentRetries,
    /// Unusual time of day for this customer's locale
    UnusualTime,
}

/// Fraud assessment decision
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, Display, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case", ascii_case_insensitive)]
#[non_exhaustive]
pub enum FraudDecision {
    /// Order is considered safe, proceed normally
    #[default]
    Accept,
    /// Order needs manual review before proceeding
    Review,
    /// Order is considered fraudulent, reject it
    Reject,
}

/// A single fraud signal detected for an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudSignal {
    /// Order this signal relates to
    pub order_id: OrderId,
    /// Type of signal detected
    pub signal_type: FraudSignalType,
    /// Confidence score (0.0 = no confidence, 1.0 = certain)
    pub score: f64,
    /// Human-readable details about the signal
    pub details: String,
    /// When the signal was detected
    pub detected_at: DateTime<Utc>,
}

/// Aggregate fraud assessment for an order
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudAssessment {
    /// Order being assessed
    pub order_id: OrderId,
    /// Overall risk score (0.0 = safe, 1.0 = high risk)
    pub risk_score: f64,
    /// Individual signals that contributed to the assessment
    pub signals: Vec<FraudSignal>,
    /// Final decision
    pub decision: FraudDecision,
    /// Who reviewed (None if automated)
    pub reviewed_by: Option<String>,
    /// Optional reviewer notes
    pub review_notes: Option<String>,
    /// When the assessment was created
    pub created_at: DateTime<Utc>,
    /// When the assessment was last updated
    pub updated_at: DateTime<Utc>,
}

/// A configurable fraud detection rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudRule {
    /// Unique rule ID
    pub id: FraudRuleId,
    /// Rule name
    pub name: String,
    /// Description of what this rule detects
    pub description: Option<String>,
    /// Signal type this rule evaluates
    pub signal_type: FraudSignalType,
    /// Score threshold to trigger this rule (0.0-1.0)
    pub threshold: f64,
    /// Action to take when rule triggers
    pub action: FraudDecision,
    /// Whether this rule is currently active
    pub enabled: bool,
    /// When the rule was created
    pub created_at: DateTime<Utc>,
    /// When the rule was last updated
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a fraud assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFraudAssessment {
    /// Order to assess
    pub order_id: OrderId,
    /// Signals detected
    pub signals: Vec<CreateFraudSignal>,
}

/// Input for creating a fraud signal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFraudSignal {
    /// Signal type
    pub signal_type: FraudSignalType,
    /// Confidence score
    pub score: f64,
    /// Details
    pub details: String,
}

/// Input for creating a fraud rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFraudRule {
    /// Rule name
    pub name: String,
    /// Description
    pub description: Option<String>,
    /// Signal type
    pub signal_type: FraudSignalType,
    /// Threshold
    pub threshold: f64,
    /// Action
    pub action: FraudDecision,
}

/// Input for updating a fraud rule
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateFraudRule {
    /// Updated name
    pub name: Option<String>,
    /// Updated description
    pub description: Option<Option<String>>,
    /// Updated threshold
    pub threshold: Option<f64>,
    /// Updated action
    pub action: Option<FraudDecision>,
    /// Updated enabled status
    pub enabled: Option<bool>,
}

/// Filter for listing fraud assessments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FraudAssessmentFilter {
    /// Filter by decision
    pub decision: Option<FraudDecision>,
    /// Filter by minimum risk score
    pub min_risk_score: Option<f64>,
    /// Only unreviewed assessments
    pub unreviewed_only: Option<bool>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

/// Filter for listing fraud rules
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FraudRuleFilter {
    /// Filter by signal type
    pub signal_type: Option<FraudSignalType>,
    /// Filter by action
    pub action: Option<FraudDecision>,
    /// Filter by enabled status
    pub enabled: Option<bool>,
    /// Maximum results
    pub limit: Option<u32>,
    /// Offset for pagination
    pub offset: Option<u32>,
}

impl FraudAssessment {
    /// Calculate the overall risk score from signals
    pub fn calculate_risk_score(signals: &[FraudSignal]) -> f64 {
        if signals.is_empty() {
            return 0.0;
        }
        // Use max signal score as the primary risk indicator
        signals
            .iter()
            .map(|s| s.score)
            .fold(0.0_f64, f64::max)
    }

    /// Determine the decision based on risk score and rules
    pub fn decide(risk_score: f64, rules: &[FraudRule], signals: &[FraudSignal]) -> FraudDecision {
        let mut decision = FraudDecision::Accept;

        for rule in rules.iter().filter(|r| r.enabled) {
            let matching_signal = signals
                .iter()
                .find(|s| s.signal_type == rule.signal_type && s.score >= rule.threshold);

            if matching_signal.is_some() {
                match rule.action {
                    FraudDecision::Reject => return FraudDecision::Reject,
                    FraudDecision::Review if decision == FraudDecision::Accept => {
                        decision = FraudDecision::Review;
                    }
                    _ => {}
                }
            }
        }

        // Fallback: high risk score triggers review
        if risk_score >= 0.8 && decision == FraudDecision::Accept {
            decision = FraudDecision::Review;
        }

        decision
    }

    /// Whether this assessment needs human review
    pub fn needs_review(&self) -> bool {
        self.decision == FraudDecision::Review && self.reviewed_by.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use stateset_primitives::{FraudRuleId, OrderId};

    fn make_signal(signal_type: FraudSignalType, score: f64) -> FraudSignal {
        FraudSignal {
            order_id: OrderId::new(),
            signal_type,
            score,
            details: "test signal".to_string(),
            detected_at: Utc::now(),
        }
    }

    fn make_rule(signal_type: FraudSignalType, threshold: f64, action: FraudDecision) -> FraudRule {
        FraudRule {
            id: FraudRuleId::new(),
            name: "test rule".to_string(),
            description: None,
            signal_type,
            threshold,
            action,
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_assessment(decision: FraudDecision, reviewed_by: Option<String>) -> FraudAssessment {
        FraudAssessment {
            order_id: OrderId::new(),
            risk_score: 0.5,
            signals: vec![],
            decision,
            reviewed_by,
            review_notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ---- calculate_risk_score ----

    #[test]
    fn calculate_risk_score_returns_zero_for_empty_signals() {
        assert_eq!(FraudAssessment::calculate_risk_score(&[]), 0.0);
    }

    #[test]
    fn calculate_risk_score_returns_max_signal_score() {
        let order_id = OrderId::new();
        let signals = vec![
            FraudSignal { order_id, signal_type: FraudSignalType::VelocitySpike, score: 0.3, details: String::new(), detected_at: Utc::now() },
            FraudSignal { order_id, signal_type: FraudSignalType::AddressMismatch, score: 0.7, details: String::new(), detected_at: Utc::now() },
            FraudSignal { order_id, signal_type: FraudSignalType::GeoIpAnomaly, score: 0.5, details: String::new(), detected_at: Utc::now() },
        ];
        assert!((FraudAssessment::calculate_risk_score(&signals) - 0.7).abs() < f64::EPSILON);
    }

    // ---- decide ----

    #[test]
    fn decide_returns_accept_with_no_rules() {
        let signals = vec![make_signal(FraudSignalType::VelocitySpike, 0.5)];
        let decision = FraudAssessment::decide(0.3, &[], &signals);
        assert_eq!(decision, FraudDecision::Accept);
    }

    #[test]
    fn decide_returns_reject_when_rule_triggers_reject() {
        let signals = vec![make_signal(FraudSignalType::VelocitySpike, 0.9)];
        let rules = vec![make_rule(FraudSignalType::VelocitySpike, 0.8, FraudDecision::Reject)];
        let decision = FraudAssessment::decide(0.9, &rules, &signals);
        assert_eq!(decision, FraudDecision::Reject);
    }

    #[test]
    fn decide_returns_review_when_rule_triggers_review() {
        let signals = vec![make_signal(FraudSignalType::AddressMismatch, 0.6)];
        let rules = vec![make_rule(FraudSignalType::AddressMismatch, 0.5, FraudDecision::Review)];
        let decision = FraudAssessment::decide(0.6, &rules, &signals);
        assert_eq!(decision, FraudDecision::Review);
    }

    #[test]
    fn decide_returns_review_on_high_risk_score_fallback() {
        // No rules match, but risk_score >= 0.8 triggers review fallback
        let decision = FraudAssessment::decide(0.85, &[], &[]);
        assert_eq!(decision, FraudDecision::Review);
    }

    #[test]
    fn decide_disabled_rule_is_ignored() {
        let signals = vec![make_signal(FraudSignalType::VelocitySpike, 0.9)];
        let mut rule = make_rule(FraudSignalType::VelocitySpike, 0.8, FraudDecision::Reject);
        rule.enabled = false;
        let decision = FraudAssessment::decide(0.5, &[rule], &signals);
        // Disabled rule should not trigger; risk_score < 0.8 so no fallback review
        assert_eq!(decision, FraudDecision::Accept);
    }

    // ---- needs_review ----

    #[test]
    fn needs_review_returns_true_when_review_decision_and_no_reviewer() {
        let assessment = make_assessment(FraudDecision::Review, None);
        assert!(assessment.needs_review());
    }

    #[test]
    fn needs_review_returns_false_when_already_reviewed() {
        let assessment = make_assessment(FraudDecision::Review, Some("admin".to_string()));
        assert!(!assessment.needs_review());
    }

    #[test]
    fn needs_review_returns_false_when_decision_is_accept() {
        let assessment = make_assessment(FraudDecision::Accept, None);
        assert!(!assessment.needs_review());
    }

    // ---- enum Display / FromStr round-trips ----

    #[test]
    fn fraud_signal_type_display_fromstr_roundtrip() {
        for signal_type in [
            FraudSignalType::VelocitySpike,
            FraudSignalType::AddressMismatch,
            FraudSignalType::HighValueFirstOrder,
            FraudSignalType::GeoIpAnomaly,
            FraudSignalType::BinCountryMismatch,
            FraudSignalType::DeviceFingerprint,
            FraudSignalType::ProxyVpn,
            FraudSignalType::DisposableEmail,
            FraudSignalType::PaymentRetries,
            FraudSignalType::UnusualTime,
        ] {
            let s = signal_type.to_string();
            let parsed: FraudSignalType = s.parse().unwrap();
            assert_eq!(parsed, signal_type, "round-trip failed for {s}");
        }
    }

    #[test]
    fn fraud_decision_display_fromstr_roundtrip() {
        for decision in [FraudDecision::Accept, FraudDecision::Review, FraudDecision::Reject] {
            let s = decision.to_string();
            let parsed: FraudDecision = s.parse().unwrap();
            assert_eq!(parsed, decision, "round-trip failed for {s}");
        }
    }

    // ---- Defaults ----

    #[test]
    fn fraud_decision_default_is_accept() {
        assert_eq!(FraudDecision::default(), FraudDecision::Accept);
    }
}
