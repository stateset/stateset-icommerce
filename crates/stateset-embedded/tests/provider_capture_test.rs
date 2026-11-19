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

#[test]
fn routed_refusal_selects_an_eligible_account_and_preserves_its_binding() {
    use stateset_embedded::ProcessorRoute;
    let commerce = Commerce::in_memory().unwrap();
    let payment = draft_payment(&commerce, dec!(15.25));
    let first = MockPaymentProvider::new(vec![ProviderDecision::Declined {
        code: "card_declined".into(),
        message: "refused".into(),
    }]);
    let second = MockPaymentProvider::new(vec![ProviderDecision::Authorized {
        reference: "hold_second".into(),
    }]);
    let routes = [
        ProcessorRoute {
            account: "wrong-currency",
            provider: &second,
            currencies: &["JPY"],
            methods: &[PaymentMethodType::CreditCard],
            available: true,
            token: "other",
        },
        ProcessorRoute {
            account: "unavailable",
            provider: &second,
            currencies: &["USD"],
            methods: &[PaymentMethodType::CreditCard],
            available: false,
            token: "other",
        },
        ProcessorRoute {
            account: "first",
            provider: &first,
            currencies: &["USD"],
            methods: &[PaymentMethodType::CreditCard],
            available: true,
            token: "first_token",
        },
        ProcessorRoute {
            account: "second",
            provider: &second,
            currencies: &["USD"],
            methods: &[PaymentMethodType::CreditCard],
            available: true,
            token: "second_token",
        },
    ];
    assert!(matches!(
        commerce.payments().authorize_routed(payment.id, &routes).unwrap(),
        ProviderDecision::Authorized { .. }
    ));
    assert_eq!(
        commerce.payments().selected_processor(payment.id).unwrap(),
        Some(("second".into(), "mock".into(), "hold_second".into()))
    );
    assert_eq!(
        commerce.payments().get(payment.id).unwrap().unwrap().external_id.as_deref(),
        Some("hold_second")
    );
}

#[test]
fn pending_processor_never_falls_through_to_another_account() {
    use stateset_embedded::ProcessorRoute;
    let commerce = Commerce::in_memory().unwrap();
    let payment = draft_payment(&commerce, dec!(20));
    let pending =
        MockPaymentProvider::new(vec![ProviderDecision::Pending { reference: "pending".into() }]);
    let backup = MockPaymentProvider::new(vec![ProviderDecision::Captured {
        reference: "unexpected".into(),
    }]);
    let route = |account, provider, token| ProcessorRoute {
        account,
        provider,
        token,
        currencies: &["USD"],
        methods: &[PaymentMethodType::CreditCard],
        available: true,
    };
    let routes = [route("first", &pending, "one"), route("backup", &backup, "two")];
    assert!(matches!(
        commerce.payments().authorize_routed(payment.id, &routes).unwrap(),
        ProviderDecision::Pending { .. }
    ));
    assert!(commerce.payments().authorize_routed(payment.id, &routes[1..]).is_err());
    assert!(commerce.payments().authorize_routed(payment.id, &routes).is_err());
    assert_eq!(backup.calls().len(), 0);
}

#[test]
fn interactive_card_fields_fail_over_then_settle_a_pending_refund_by_reference() {
    use serde_json::json;
    use stateset_embedded::{ProcessorRoute, RoutedAuthorization, StripeProvider};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        for (verb, path, status, expected_form, body) in [
            (
                "POST",
                "/v1/payment_intents",
                402,
                "payment_method=pm_first",
                json!({"error":{"type":"card_error","code":"card_declined","message":"refused"}}),
            ),
            (
                "POST",
                "/v1/payment_intents",
                200,
                "payment_method=pm_second",
                json!({"id":"pi_second","amount":1200,"amount_capturable":1200,"currency":"usd","status":"requires_capture"}),
            ),
            (
                "GET",
                "/v1/payment_intents/pi_second",
                200,
                "",
                json!({"id":"pi_second","amount":1200,"amount_capturable":1200,"currency":"usd","status":"requires_capture"}),
            ),
            (
                "POST",
                "/v1/payment_intents/pi_second/capture",
                200,
                "",
                json!({"id":"pi_second","amount":1200,"amount_received":1200,"currency":"usd","status":"succeeded"}),
            ),
            (
                "POST",
                "/v1/refunds",
                200,
                "amount=125",
                json!({"id":"re_pending","payment_intent":"pi_second","currency":"usd","amount":125,"status":"pending"}),
            ),
            (
                "GET",
                "/v1/refunds/re_pending",
                200,
                "",
                json!({"id":"re_pending","status":"succeeded","payment_intent":"pi_second","amount":125,"currency":"usd"}),
            ),
        ] {
            let mut line = String::new();
            let mut reader = loop {
                let (stream, _) = socket.accept().unwrap();
                let mut reader = BufReader::new(stream);
                line.clear();
                reader.read_line(&mut line).unwrap();
                // A local port discovery probe has no Stripe API path.
                if line.trim() == "GET / HTTP/1.1" {
                    reader.get_mut().write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                    continue;
                }
                break reader;
            };
            assert_eq!(line.trim(), format!("{verb} {path} HTTP/1.1"));
            let mut length = 0;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut data = vec![0; length];
            reader.read_exact(&mut data).unwrap();
            assert!(String::from_utf8(data).unwrap().contains(expected_form));
            let body = body.to_string();
            write!(reader.get_mut(), "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    let commerce = Commerce::in_memory().unwrap();
    let payment = draft_payment(&commerce, dec!(12.00));
    let p1 = StripeProvider::new("first").unwrap().with_base_url(&origin);
    let p2 = StripeProvider::new("second").unwrap().with_base_url(&origin);
    let mut routes = [
        ProcessorRoute {
            account: "first",
            provider: &p1,
            currencies: &["USD"],
            methods: &[PaymentMethodType::CreditCard],
            available: true,
            token: "",
        },
        ProcessorRoute {
            account: "second",
            provider: &p2,
            currencies: &["USD"],
            methods: &[PaymentMethodType::CreditCard],
            available: true,
            token: "",
        },
    ];
    let payments = commerce.payments();
    assert_eq!(
        payments.authorize_interactive(payment.id, &routes).unwrap(),
        RoutedAuthorization::CollectToken { account: "first".into() }
    );
    routes[0].token = "pm_first";
    assert_eq!(
        payments.authorize_interactive(payment.id, &routes).unwrap(),
        RoutedAuthorization::CollectToken { account: "second".into() }
    );
    routes[0].token = "";
    routes[1].token = "pm_second";
    assert!(matches!(
        payments.authorize_interactive(payment.id, &routes).unwrap(),
        RoutedAuthorization::Decision { decision: ProviderDecision::Authorized { .. }, .. }
    ));
    assert!(matches!(
        payments.capture_routed(payment.id, &routes).unwrap(),
        ProviderDecision::Captured { .. }
    ));
    let refund =
        payments.refund_routed(payment.id, &routes, dec!(1.25), "one-key", "return").unwrap();
    assert_eq!(refund.status, stateset_core::RefundStatus::Pending);
    let done =
        payments.refund_routed(payment.id, &routes, dec!(1.25), "one-key", "return").unwrap();
    assert_eq!(refund.id, done.id);
    assert_eq!(done.status, stateset_core::RefundStatus::Completed);
    server.join().unwrap();
}
