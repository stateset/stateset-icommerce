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
            last_name: "Turing".into(),
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
        ProviderDecision::Captured { reference: "pi_mock_1".into(), amount: None },
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
    assert_eq!(
        captured,
        ProviderDecision::Captured { reference: "pi_mock_1".into(), amount: None }
    );

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
        ProviderDecision::Captured { reference: "pi_mock_9".into(), amount: None },
        ProviderDecision::Captured { reference: "pi_mock_9".into(), amount: None },
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
    let mock = MockPaymentProvider::new(vec![ProviderDecision::Captured {
        reference: "pi_part".into(),
        amount: None,
    }]);
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
        ProviderDecision::Captured { reference: "pi_ach".into(), amount: None },
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
fn partial_capture_records_exactly_what_the_provider_captured() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(100.00));
    let mock = MockPaymentProvider::new(vec![
        ProviderDecision::Authorized { reference: "pi_p".into() },
        // The processor reports what actually moved.
        ProviderDecision::Captured { reference: "pi_p".into(), amount: Some(dec!(60.00)) },
    ]);
    let decision = commerce
        .payments()
        .capture_with_provider(payment.id, &mock, Some(dec!(60.00)), Some("pm".into()))
        .expect("partial capture");
    assert!(matches!(decision, ProviderDecision::Captured { .. }));
    assert_eq!(mock.calls()[1], "capture:pi_p:60.00:USD");
    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.status, PaymentTransactionStatus::Completed);
    assert_eq!(stored.amount, dec!(100.00));
    assert_eq!(stored.captured_amount, Some(dec!(60.00)));

    // Refunds are bounded by the capture, not the authorization.
    let mock = MockPaymentProvider::new(vec![ProviderDecision::Captured {
        reference: "pi_p".into(),
        amount: None,
    }]);
    let err = commerce
        .payments()
        .refund_with_provider(payment.id, &mock, Some(dec!(60.01)), None)
        .expect_err("refund above the capture");
    assert!(matches!(err, stateset_embedded::CommerceError::RefundExceedsCaptured { .. }));
    assert!(mock.calls().is_empty(), "nothing was sent upstream");
    commerce.payments().refund_with_provider(payment.id, &mock, None, None).expect("refund 60");
    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.status, PaymentTransactionStatus::Refunded);
    assert_eq!(stored.amount_refunded, dec!(60.00));
}

#[test]
fn provider_reported_capture_amount_wins_over_the_request() {
    // A full capture was asked for, the processor captured less: record
    // what moved, never what was asked.
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(100.00));
    let mock = MockPaymentProvider::new(vec![
        ProviderDecision::Authorized { reference: "pi_q".into() },
        ProviderDecision::Captured { reference: "pi_q".into(), amount: Some(dec!(99.50)) },
    ]);
    commerce
        .payments()
        .capture_with_provider(payment.id, &mock, None, Some("pm".into()))
        .expect("capture");
    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.captured_amount, Some(dec!(99.50)));
}

#[test]
fn invalid_partial_capture_is_refused_before_any_provider_call() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let payment = draft_payment(&commerce, dec!(100.00));
    let mock =
        MockPaymentProvider::new(vec![ProviderDecision::Authorized { reference: "pi_p".into() }]);
    for bad in [dec!(0), dec!(100.01), dec!(1.001)] {
        let err = commerce
            .payments()
            .capture_with_provider(payment.id, &mock, Some(bad), Some("pm".into()))
            .expect_err("an unrecordable capture never goes upstream");
        assert!(
            matches!(
                err,
                stateset_embedded::CommerceError::ValidationError(_)
                    | stateset_embedded::CommerceError::MoneyScaleExceedsCurrency { .. }
            ),
            "{err:?}"
        );
    }
    assert!(mock.calls().is_empty());
}

fn captured_with_pending_refund(
    commerce: &Commerce,
    amount: rust_decimal::Decimal,
    refund: rust_decimal::Decimal,
    upstream: &str,
) -> (stateset_embedded::Payment, uuid::Uuid) {
    let payment = draft_payment(commerce, amount);
    let mock = MockPaymentProvider::new(vec![
        ProviderDecision::Captured { reference: "pi_ach".into(), amount: None },
        ProviderDecision::Pending { reference: upstream.into() },
    ]);
    commerce
        .payments()
        .capture_with_provider(payment.id, &mock, None, Some("pm".into()))
        .expect("capture");
    commerce
        .payments()
        .refund_with_provider(payment.id, &mock, Some(refund), None)
        .expect("refund accepted");
    let refunds = commerce.payments().get_refunds(payment.id).expect("refunds");
    assert_eq!(refunds[0].external_id.as_deref(), Some(upstream), "processor id kept");
    (payment, refunds[0].id)
}

#[test]
fn pending_refund_settles_when_the_provider_reports_success() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let (payment, refund_id) =
        captured_with_pending_refund(&commerce, dec!(20.00), dec!(8.00), "re_ok");

    // Still pending upstream: nothing changes.
    let still =
        MockPaymentProvider::new(vec![ProviderDecision::Pending { reference: "re_ok".into() }]);
    let report = commerce.payments().reconcile_pending_refunds(&still, 100).expect("reconcile");
    assert_eq!(report.examined, 1);
    assert_eq!(report.still_pending, 1);
    assert_eq!(still.calls(), vec!["refund_status:re_ok".to_string()]);

    let settled = MockPaymentProvider::new(vec![ProviderDecision::Captured {
        reference: "re_ok".into(),
        amount: Some(dec!(8.00)),
    }]);
    let report = commerce.payments().reconcile_pending_refunds(&settled, 100).expect("reconcile");
    assert_eq!(report.completed, vec![refund_id]);
    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.amount_refunded, dec!(8.00));
    assert_eq!(stored.status, PaymentTransactionStatus::PartiallyRefunded);

    // Settled refunds are not looked up again, and a re-run changes nothing.
    let again = MockPaymentProvider::new(vec![ProviderDecision::Captured {
        reference: "re_ok".into(),
        amount: None,
    }]);
    let report = commerce.payments().reconcile_pending_refunds(&again, 100).expect("reconcile");
    assert_eq!(report.examined, 0);
    let refund = commerce.payments().refresh_refund_status(refund_id, &again).expect("refresh");
    assert_eq!(refund.status, stateset_embedded::RefundStatus::Completed);
    assert!(again.calls().is_empty());
    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.amount_refunded, dec!(8.00), "counted exactly once");
}

#[test]
fn pending_refund_that_fails_upstream_releases_its_reservation() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let (payment, refund_id) =
        captured_with_pending_refund(&commerce, dec!(20.00), dec!(20.00), "re_bad");
    let failed = MockPaymentProvider::new(vec![ProviderDecision::Declined {
        code: "expired_or_canceled_card".into(),
        message: "stripe refund re_bad is failed".into(),
    }]);
    let refund = commerce.payments().refresh_refund_status(refund_id, &failed).expect("refresh");
    assert_eq!(refund.status, stateset_embedded::RefundStatus::Failed);
    assert!(refund.failure_reason.as_deref().unwrap_or_default().contains("expired"));
    // The whole capture is refundable again.
    let stored = commerce.payments().get(payment.id).expect("get").expect("row");
    assert_eq!(stored.amount_refunded, dec!(0));
    commerce
        .payments()
        .create_refund(stateset_embedded::CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(20.00)),
            ..Default::default()
        })
        .expect("reservation released");
}

#[test]
fn reconciliation_reports_a_provider_outage_per_refund_and_keeps_going() {
    let commerce = Commerce::in_memory().expect("in-memory");
    let (_, first) = captured_with_pending_refund(&commerce, dec!(10.00), dec!(1.00), "re_a");
    let (_, second) = captured_with_pending_refund(&commerce, dec!(10.00), dec!(1.00), "re_b");
    // A Stripe provider pointed at an unroutable host: every lookup errors.
    let down = stateset_embedded::StripeProvider::new("sk_test_x")
        .expect("provider")
        .with_base_url("http://127.0.0.1:9");
    let report = commerce.payments().reconcile_pending_refunds(&down, 100).expect("reconcile");
    assert_eq!(report.examined, 2);
    let errored: Vec<_> = report.errors.iter().map(|e| e.refund_id).collect();
    assert!(errored.contains(&first) && errored.contains(&second), "{report:?}");
    for id in [first, second] {
        let refund = commerce.payments().get_refund(id).expect("get").expect("refund");
        assert_eq!(refund.status, stateset_embedded::RefundStatus::Pending, "left for next pass");
    }
}

#[test]
fn concurrent_reconciliation_passes_settle_each_refund_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("reconcile.db");
    let commerce =
        std::sync::Arc::new(Commerce::new(path.to_str().expect("utf8")).expect("file store"));
    let mut refunds = Vec::new();
    let mut payments = Vec::new();
    for i in 0..4 {
        let (payment, refund) =
            captured_with_pending_refund(&commerce, dec!(10.00), dec!(4.00), &format!("re_c{i}"));
        payments.push(payment.id);
        refunds.push(refund);
    }
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let commerce = std::sync::Arc::clone(&commerce);
            std::thread::spawn(move || {
                let settled = MockPaymentProvider::new(vec![ProviderDecision::Captured {
                    reference: "re".into(),
                    amount: None,
                }]);
                commerce.payments().reconcile_pending_refunds(&settled, 100)
            })
        })
        .collect();
    let mut completed = Vec::new();
    for handle in handles {
        let report = handle.join().expect("thread").expect("reconcile");
        completed.extend(report.completed);
    }
    for id in &refunds {
        assert!(completed.contains(id));
    }
    for id in payments {
        let payment = commerce.payments().get(id).expect("get").expect("row");
        assert_eq!(payment.amount_refunded, dec!(4.00), "each refund folded in exactly once");
    }
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
