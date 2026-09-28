#![cfg(feature = "sqlite")]

//! Fraud rules decide an assessment.
//!
//! `FraudAssessment::decide` applies the enabled rules -- a matching `reject`
//! rule rejects, a matching `review` rule sends the order to review -- and
//! falls back to review at a risk score of 0.8. Nothing ever called it:
//! `create_assessment` hardcoded the 0.8 cutoff, so every configured rule was
//! stored, listed, updated and ignored. An enabled reject rule on VPN traffic
//! at 0.5 let a 0.75 VPN signal through as `accept`.

use stateset_core::{
    CreateFraudAssessment, CreateFraudRule, CreateFraudSignal, FraudDecision, FraudSignalType,
    OrderId, UpdateFraudRule,
};
use stateset_embedded::Commerce;

fn signal(signal_type: FraudSignalType, score: f64) -> CreateFraudSignal {
    CreateFraudSignal { signal_type, score, details: "test".into() }
}

fn rule(
    commerce: &Commerce,
    signal_type: FraudSignalType,
    threshold: f64,
    action: FraudDecision,
) -> stateset_core::FraudRule {
    commerce
        .fraud()
        .create_rule(CreateFraudRule {
            name: format!("{signal_type:?} {action:?}"),
            description: None,
            signal_type,
            threshold,
            action,
        })
        .expect("create rule")
}

fn assess(commerce: &Commerce, signals: Vec<CreateFraudSignal>) -> FraudDecision {
    commerce
        .fraud()
        .create_assessment(CreateFraudAssessment { order_id: OrderId::new(), signals })
        .expect("assess")
        .decision
}

#[test]
fn an_enabled_reject_rule_rejects_a_matching_order() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    rule(&commerce, FraudSignalType::ProxyVpn, 0.5, FraudDecision::Reject);

    assert_eq!(
        assess(&commerce, vec![signal(FraudSignalType::ProxyVpn, 0.75)]),
        FraudDecision::Reject,
        "a VPN signal over the rule's threshold"
    );
    assert_eq!(
        assess(&commerce, vec![signal(FraudSignalType::ProxyVpn, 0.4)]),
        FraudDecision::Accept,
        "under the threshold"
    );
    assert_eq!(
        assess(&commerce, vec![signal(FraudSignalType::GeoIpAnomaly, 0.75)]),
        FraudDecision::Accept,
        "a different signal type"
    );
}

#[test]
fn a_review_rule_sends_a_low_risk_order_to_review() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    rule(&commerce, FraudSignalType::HighValueFirstOrder, 0.3, FraudDecision::Review);
    assert_eq!(
        assess(&commerce, vec![signal(FraudSignalType::HighValueFirstOrder, 0.35)]),
        FraudDecision::Review,
        "below the 0.8 fallback, but the rule matches"
    );
}

#[test]
fn a_disabled_rule_decides_nothing_and_the_fallback_still_holds() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let reject = rule(&commerce, FraudSignalType::ProxyVpn, 0.5, FraudDecision::Reject);
    commerce
        .fraud()
        .update_rule(reject.id, UpdateFraudRule { enabled: Some(false), ..Default::default() })
        .expect("disable");

    assert_eq!(
        assess(&commerce, vec![signal(FraudSignalType::ProxyVpn, 0.75)]),
        FraudDecision::Accept
    );
    assert_eq!(
        assess(&commerce, vec![signal(FraudSignalType::ProxyVpn, 0.85)]),
        FraudDecision::Review,
        "no rule, but the 0.8 fallback"
    );
}
