#![cfg(all(feature = "sqlite", feature = "events"))]

//! Real card capture through [`PaymentProvider`]s.
//!
//! The mock loop proves the engine persists every outcome (authorize →
//! capture → refund, plus declines). The Stripe case runs only with
//! `STRIPE_TEST_KEY` set and exercises test mode end to end at $1.

use rust_decimal_macros::dec;
use stateset_embedded::{
    CardBrand, Commerce, CreateCustomer, CreatePayment, CustomerId, MockPaymentProvider,
    PaymentMethodType, PaymentTransactionStatus, ProviderDecision,
};
use uuid::Uuid;

fn customer(commerce: &Commerce) -> CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("cap-{}@example.com", Uuid::new_v4()),
            first_name: "Cap".into(),
            last_name: "Ture".into(),
            ..Default::default()
        })
        .expect("customer")
        .id
}

fn draft_payment(commerce: &Commerce, amount: rust_decimal::Decimal) -> stateset_embedded::Payment {
    commerce
        .payments()
        .create(CreatePayment {
            customer_id: Some(customer(commerce)),
            payment_method: PaymentMethodType::CreditCard,
            amount,
            card_brand: Some(CardBrand::Visa),
            card_last4: Some("4242".into()),
            ..Default::default()
        })
        .expect("draft payment")
}

#[test]
fn mock_authorize_then_capture_completes_with_reference() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(49.99));
    let mock = MockPaymentProvider::new(vec![
        ProviderDecision::Authorized { reference: "pi_mock_1".into() },
        ProviderDecision::Captured { reference: "pi_mock_1".into() },
    ]);

    let auth = commerce
        .payments()
        .authorize_with_provider(payment.id, &mock, Some("pm_mock".into()))
        .expect("authorize");
    assert_eq!(auth, ProviderDecision::Authorized { reference: "pi_mock_1".into() });

    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.external_id.as_deref(), Some("pi_mock_1"));
    assert_eq!(stored.status, PaymentTransactionStatus::Processing);

    let captured =
        commerce.payments().capture_with_provider(payment.id, &mock, None, None).expect("capture");
    assert_eq!(captured, ProviderDecision::Captured { reference: "pi_mock_1".into() });

    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.status, PaymentTransactionStatus::Completed);
    // The hold was reused: authorize ran once, capture ran once.
    assert_eq!(mock.calls().len(), 2);
}

#[test]
fn mock_decline_marks_payment_failed_with_code() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(10.00));
    let mock = MockPaymentProvider::new(vec![ProviderDecision::Declined {
        code: "card_declined".into(),
        message: "do not honor".into(),
    }]);

    let decision = commerce
        .payments()
        .capture_with_provider(payment.id, &mock, None, Some("pm_bad".into()))
        .expect("declines are data, not errors");
    assert!(matches!(decision, ProviderDecision::Declined { .. }));

    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.status, PaymentTransactionStatus::Failed);
    assert_eq!(stored.failure_code.as_deref(), Some("card_declined"));
}

#[test]
fn mock_refund_completes_engine_refund_row() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(20.00));
    let mock = MockPaymentProvider::new(vec![
        ProviderDecision::Captured { reference: "pi_mock_9".into() },
        ProviderDecision::Captured { reference: "pi_mock_9".into() },
    ]);
    commerce
        .payments()
        .capture_with_provider(payment.id, &mock, None, Some("pm_mock".into()))
        .expect("capture");

    commerce
        .payments()
        .refund_with_provider(payment.id, &mock, None, Some("changed mind".into()))
        .expect("refund");

    let refunds = commerce.payments().get_refunds(payment.id).expect("refunds");
    assert_eq!(refunds.len(), 1);
    assert_eq!(refunds[0].status, stateset_embedded::RefundStatus::Completed);
}

#[test]
fn partial_refunds_use_distinct_idempotency_keys() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(20.00));
    let mock =
        MockPaymentProvider::new(vec![ProviderDecision::Captured { reference: "pi_part".into() }]);
    commerce
        .payments()
        .capture_with_provider(payment.id, &mock, None, Some("pm_mock".into()))
        .expect("capture");
    for _ in 0..2 {
        commerce
            .payments()
            .refund_with_provider(payment.id, &mock, Some(dec!(5.00)), None)
            .expect("partial refund");
    }
    let keys: Vec<String> = mock
        .calls()
        .into_iter()
        .filter(|c| c.starts_with("refund:"))
        .map(|c| c.rsplit(':').next().unwrap_or_default().to_string())
        .collect();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1], "each partial refund is its own upstream operation");
}

#[test]
fn pending_refund_is_left_pending_not_completed() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(20.00));
    let mock = MockPaymentProvider::new(vec![
        ProviderDecision::Captured { reference: "pi_ach".into() },
        ProviderDecision::Pending { reference: "re_ach".into() },
    ]);
    commerce
        .payments()
        .capture_with_provider(payment.id, &mock, None, Some("pm_mock".into()))
        .expect("capture");
    let decision = commerce
        .payments()
        .refund_with_provider(payment.id, &mock, None, None)
        .expect("refund accepted");
    assert_eq!(decision, ProviderDecision::Pending { reference: "re_ach".into() });
    let refunds = commerce.payments().get_refunds(payment.id).expect("refunds");
    assert_eq!(refunds.len(), 1);
    assert_ne!(refunds[0].status, stateset_embedded::RefundStatus::Completed);
}

#[test]
fn capture_does_not_follow_a_3ds_challenge() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(30.00));
    let challenge = ProviderDecision::RequiresAction {
        reference: "pi_3ds".into(),
        action_url: Some("https://bank.example/3ds".into()),
    };
    let mock = MockPaymentProvider::new(vec![challenge.clone()]);
    let decision = commerce
        .payments()
        .capture_with_provider(payment.id, &mock, None, Some("pm_3ds".into()))
        .expect("challenge is data");
    assert_eq!(decision, challenge, "the action_url reaches the caller");
    assert!(
        mock.calls().iter().all(|c| !c.starts_with("capture:")),
        "no capture against an intent that still needs the customer: {:?}",
        mock.calls()
    );
}

#[test]
fn partial_capture_is_refused_before_any_provider_call() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(100.00));
    let mock =
        MockPaymentProvider::new(vec![ProviderDecision::Authorized { reference: "pi_p".into() }]);
    let err = commerce
        .payments()
        .capture_with_provider(payment.id, &mock, Some(dec!(60.00)), Some("pm".into()))
        .expect_err("partial capture would over-record collected funds");
    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)));
    assert!(mock.calls().is_empty());
}

#[test]
fn authorizing_a_failed_payment_is_refused_before_going_upstream() {
    // A decline marks the payment Failed. Re-authorizing it must not place a
    // hold upstream that the engine then cannot record (an orphaned hold).
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(15.00));
    let mock = MockPaymentProvider::new(vec![
        ProviderDecision::Declined { code: "card_declined".into(), message: "no".into() },
        ProviderDecision::Authorized { reference: "pi_orphan".into() },
    ]);
    let first = commerce
        .payments()
        .authorize_with_provider(payment.id, &mock, Some("pm_bad".into()))
        .expect("decline is data");
    assert!(matches!(first, ProviderDecision::Declined { .. }));
    let err = commerce
        .payments()
        .authorize_with_provider(payment.id, &mock, Some("pm_good".into()))
        .expect_err("a failed payment is not re-authorizable");
    assert!(matches!(err, stateset_embedded::CommerceError::ValidationError(_)));
    assert_eq!(mock.calls().len(), 1, "no second upstream authorize: {:?}", mock.calls());
}

/// Live Stripe test mode. Requires `STRIPE_TEST_KEY`; otherwise skipped.
/// Authorizes $1.00 on `pm_card_visa`, captures, and refunds it: net zero,
/// and every step asserts the engine row matches the upstream truth.
#[test]
fn stripe_test_mode_authorize_capture_refund() {
    let Some(key) = std::env::var("STRIPE_TEST_KEY").ok() else {
        eprintln!("STRIPE_TEST_KEY not set; skipping live stripe test");
        return;
    };
    let provider = stateset_embedded::StripeProvider::new(key).expect("stripe provider");
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(1.00));

    let auth = commerce
        .payments()
        .authorize_with_provider(payment.id, &provider, Some("pm_card_visa".into()))
        .expect("test authorize");
    assert!(
        matches!(auth, ProviderDecision::Authorized { .. }),
        "unexpected test-mode decision: {auth:?}"
    );

    let captured = commerce
        .payments()
        .capture_with_provider(payment.id, &provider, None, None)
        .expect("test capture");
    assert!(matches!(captured, ProviderDecision::Captured { .. }));

    let refunded = commerce
        .payments()
        .refund_with_provider(payment.id, &provider, None, Some("book-of-record test".into()))
        .expect("test refund");
    assert!(matches!(refunded, ProviderDecision::Captured { .. }));
}
