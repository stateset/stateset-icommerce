//! Postgres parity for fraud rules deciding an assessment (SQLite:
//! stateset-embedded/tests/fraud_rules_test.rs).
//!
//! `create_assessment` hardcoded a 0.8 risk cutoff and never consulted the
//! configured rules. It now reads the enabled rules in the assessment's
//! transaction and decides with `FraudAssessment::decide`.
//!
//! Requires a live Postgres instance (`POSTGRES_URL` / `DATABASE_URL`) and is
//! skipped otherwise. Rules are global and the database is shared, so this
//! asserts only outcomes a rule must cause and removes its rules afterwards.

#![cfg(feature = "postgres")]

use stateset_core::{
    CreateFraudAssessment, CreateFraudRule, CreateFraudSignal, FraudDecision, FraudSignalType,
    OrderId,
};
use stateset_db::PostgresDatabase;

fn postgres_url() -> Option<String> {
    std::env::var("POSTGRES_URL").ok().or_else(|| std::env::var("DATABASE_URL").ok())
}

#[tokio::test]
async fn postgres_configured_rules_decide_the_assessment() {
    let Some(url) = postgres_url() else {
        eprintln!("POSTGRES_URL/DATABASE_URL not set; skipping");
        return;
    };
    let db = PostgresDatabase::connect(&url).await.expect("connect + migrate");
    let fraud = db.fraud();

    let reject = fraud
        .create_rule_async(CreateFraudRule {
            name: "vpn reject".into(),
            description: None,
            signal_type: FraudSignalType::ProxyVpn,
            threshold: 0.5,
            action: FraudDecision::Reject,
        })
        .await
        .expect("create reject rule");
    let review = fraud
        .create_rule_async(CreateFraudRule {
            name: "first order review".into(),
            description: None,
            signal_type: FraudSignalType::HighValueFirstOrder,
            threshold: 0.3,
            action: FraudDecision::Review,
        })
        .await
        .expect("create review rule");

    let assess = |signal_type, score| {
        let fraud = fraud.clone();
        async move {
            fraud
                .create_assessment_async(CreateFraudAssessment {
                    order_id: OrderId::new(),
                    signals: vec![CreateFraudSignal { signal_type, score, details: "test".into() }],
                })
                .await
                .expect("assess")
                .decision
        }
    };

    let vpn = assess(FraudSignalType::ProxyVpn, 0.75).await;
    let first_order = assess(FraudSignalType::HighValueFirstOrder, 0.35).await;

    fraud.delete_rule_async(reject.id).await.expect("delete reject rule");
    fraud.delete_rule_async(review.id).await.expect("delete review rule");

    assert_eq!(vpn, FraudDecision::Reject, "the 0.75 VPN signal crosses the reject rule");
    assert_eq!(first_order, FraudDecision::Review, "below 0.8, but the review rule matches");
}
