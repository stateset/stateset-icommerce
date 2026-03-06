//! Comprehensive integration tests for the `stateset-a2a` crate.
//!
//! These tests exercise the public API surface as an external consumer would,
//! covering split payment calculations, subscription billing date arithmetic,
//! conditional escrow evaluation, HMAC webhook signing/verification, and
//! SSRF URL validation.

use chrono::{DateTime, Datelike, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use stateset_a2a::A2AError;
use stateset_a2a::escrow::conditions::{
    evaluate_all_conditions, evaluate_buyer_confirmed, evaluate_condition, evaluate_milestone,
    evaluate_seller_fulfilled, evaluate_time_lock,
};
use stateset_a2a::escrow::{Condition, ConditionType};
use stateset_a2a::events::matches_event_filter;
use stateset_a2a::notifications::{sign_webhook, validate_url, verify_webhook};
use stateset_a2a::splits::{Recipient, calculate_fixed_split, calculate_percentage_split};
use stateset_a2a::subscriptions::{BillingInterval, compute_next_billing_date};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn pct_recipient(address: &str, percent: Decimal) -> Recipient {
    Recipient { address: address.into(), percent: Some(percent), amount: None }
}

fn fixed_recipient(address: &str, amount: Decimal) -> Recipient {
    Recipient { address: address.into(), percent: None, amount: Some(amount) }
}

fn dt(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, 12, 0, 0).unwrap()
}

// ===========================================================================
// 1. PERCENTAGE SPLIT CALCULATIONS
// ===========================================================================

#[test]
fn pct_split_three_recipients_50_30_20_shares_sum_to_total() {
    let recipients = vec![
        pct_recipient("0xAlice", dec!(50)),
        pct_recipient("0xBob", dec!(30)),
        pct_recipient("0xCharlie", dec!(20)),
    ];
    let result = calculate_percentage_split(dec!(100), dec!(2.5), &recipients).unwrap();

    let share_sum: Decimal = result.shares.iter().map(|s| s.amount).sum();
    assert_eq!(
        share_sum + result.platform_fee,
        result.total_distributed,
        "shares + platform_fee must equal total_distributed"
    );
    assert_eq!(result.total_distributed, dec!(100));
    assert_eq!(result.shares.len(), 3);
}

#[test]
fn pct_split_platform_fee_matches_expected_percentage() {
    let recipients = vec![pct_recipient("0xA", dec!(60)), pct_recipient("0xB", dec!(40))];
    let result = calculate_percentage_split(dec!(200), dec!(5), &recipients).unwrap();

    // 200 * 5% = 10
    assert_eq!(result.platform_fee, dec!(10.000000));
    assert_eq!(result.total_distributed, dec!(200));
}

#[test]
fn pct_split_rounding_drift_last_recipient_absorbs_remainder() {
    // 33.333 + 33.333 + 33.334 = 100.000 — but rounding means the last
    // recipient absorbs any fractional remainder.
    let recipients = vec![
        pct_recipient("0xA", dec!(33.333)),
        pct_recipient("0xB", dec!(33.333)),
        pct_recipient("0xC", dec!(33.334)),
    ];
    let result = calculate_percentage_split(dec!(100), dec!(0), &recipients).unwrap();

    // Regardless of per-share rounding, total must be exact.
    assert_eq!(result.total_distributed, dec!(100));

    // First two shares are computed via round_dp; the last gets the remainder.
    let first_two_sum = result.shares[0].amount + result.shares[1].amount;
    assert_eq!(
        result.shares[2].amount,
        dec!(100) - first_two_sum,
        "last recipient should get exact remainder"
    );
}

#[test]
fn pct_split_equal_two_way_50_50() {
    let recipients = vec![pct_recipient("0xA", dec!(50)), pct_recipient("0xB", dec!(50))];
    let result = calculate_percentage_split(dec!(100), dec!(0), &recipients).unwrap();

    assert_eq!(result.shares[0].amount, dec!(50));
    assert_eq!(result.shares[1].amount, dec!(50));
    assert_eq!(result.total_distributed, dec!(100));
}

#[test]
fn pct_split_edge_very_small_total() {
    let recipients = vec![pct_recipient("0xA", dec!(60)), pct_recipient("0xB", dec!(40))];
    let result = calculate_percentage_split(dec!(0.01), dec!(0), &recipients).unwrap();

    assert_eq!(result.total_distributed, dec!(0.01));
    // 0.01 * 60% = 0.006, 0.01 * 40% = remainder
    let share_sum: Decimal = result.shares.iter().map(|s| s.amount).sum();
    assert_eq!(share_sum, dec!(0.01));
}

#[test]
fn pct_split_edge_large_total() {
    let recipients = vec![pct_recipient("0xA", dec!(70)), pct_recipient("0xB", dec!(30))];
    let result = calculate_percentage_split(dec!(1_000_000.00), dec!(1), &recipients).unwrap();

    assert_eq!(result.platform_fee, dec!(10_000.000000));
    assert_eq!(result.total_distributed, dec!(1_000_000.00));
}

#[test]
fn pct_split_zero_platform_fee() {
    let recipients = vec![pct_recipient("0xA", dec!(80)), pct_recipient("0xB", dec!(20))];
    let result = calculate_percentage_split(dec!(500), dec!(0), &recipients).unwrap();

    assert_eq!(result.platform_fee, Decimal::ZERO);
    assert_eq!(result.total_distributed, dec!(500));
}

#[test]
fn pct_split_rejects_single_recipient() {
    let recipients = vec![pct_recipient("0xA", dec!(100))];
    let err = calculate_percentage_split(dec!(100), dec!(0), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::Validation(_)));
}

#[test]
fn pct_split_rejects_percentages_not_summing_to_100() {
    let recipients = vec![pct_recipient("0xA", dec!(50)), pct_recipient("0xB", dec!(40))];
    let err = calculate_percentage_split(dec!(100), dec!(0), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::PercentageSumMismatch { .. }));
}

#[test]
fn pct_split_rejects_negative_platform_fee() {
    let recipients = vec![pct_recipient("0xA", dec!(50)), pct_recipient("0xB", dec!(50))];
    let err = calculate_percentage_split(dec!(100), dec!(-0.1), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::Validation(_)));
}

#[test]
fn pct_split_rejects_negative_recipient_percent() {
    let recipients = vec![pct_recipient("0xA", dec!(101)), pct_recipient("0xB", dec!(-1))];
    let err = calculate_percentage_split(dec!(100), dec!(0), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::Validation(_)));
}

// ===========================================================================
// 2. FIXED SPLIT CALCULATIONS
// ===========================================================================

#[test]
fn fixed_split_amounts_per_recipient() {
    let recipients = vec![fixed_recipient("0xA", dec!(60)), fixed_recipient("0xB", dec!(40))];
    let result = calculate_fixed_split(dec!(100), dec!(0), &recipients).unwrap();

    assert_eq!(result.shares[0].amount, dec!(60));
    assert_eq!(result.shares[1].amount, dec!(40));
    assert_eq!(result.total_distributed, dec!(100));
}

#[test]
fn fixed_split_total_distributed_equals_shares_plus_fee() {
    // 100 total, 5% fee = 5, remaining 95 split as 55 + 40
    let recipients = vec![fixed_recipient("0xA", dec!(55)), fixed_recipient("0xB", dec!(40))];
    let result = calculate_fixed_split(dec!(100), dec!(5), &recipients).unwrap();

    let share_sum: Decimal = result.shares.iter().map(|s| s.amount).sum();
    assert_eq!(share_sum + result.platform_fee, result.total_distributed);
    assert_eq!(result.platform_fee, dec!(5.000000));
    assert_eq!(result.total_distributed, dec!(100));
}

#[test]
fn fixed_split_error_when_amounts_exceed_total() {
    let recipients = vec![fixed_recipient("0xA", dec!(70)), fixed_recipient("0xB", dec!(50))];
    // Sum = 120, total = 100 => mismatch
    let err = calculate_fixed_split(dec!(100), dec!(0), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::FixedSumMismatch { .. }));
}

#[test]
fn fixed_split_error_when_amounts_under_total() {
    let recipients = vec![fixed_recipient("0xA", dec!(30)), fixed_recipient("0xB", dec!(20))];
    // Sum = 50, total = 100 => mismatch
    let err = calculate_fixed_split(dec!(100), dec!(0), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::FixedSumMismatch { .. }));
}

#[test]
fn fixed_split_rejects_negative_amount() {
    let recipients = vec![fixed_recipient("0xA", dec!(120)), fixed_recipient("0xB", dec!(-20))];
    let err = calculate_fixed_split(dec!(100), dec!(0), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::Validation(_)));
}

#[test]
fn fixed_split_rejects_platform_fee_above_100_pct() {
    let recipients = vec![fixed_recipient("0xA", dec!(60)), fixed_recipient("0xB", dec!(40))];
    let err = calculate_fixed_split(dec!(100), dec!(100.1), &recipients).unwrap_err();
    assert!(matches!(err, A2AError::Validation(_)));
}

// ===========================================================================
// 3. SUBSCRIPTION BILLING DATE ARITHMETIC
// ===========================================================================

#[test]
fn billing_monthly_jan_15_to_feb_15() {
    let start = dt(2026, 1, 15);
    let next = compute_next_billing_date(start, BillingInterval::Monthly).unwrap();
    assert_eq!(next.month(), 2);
    assert_eq!(next.day(), 15);
    assert_eq!(next.year(), 2026);
}

#[test]
fn billing_monthly_end_of_month_jan_31_to_feb_28_non_leap() {
    // 2026 is not a leap year
    let start = dt(2026, 1, 31);
    let next = compute_next_billing_date(start, BillingInterval::Monthly).unwrap();
    assert_eq!(next.month(), 2);
    assert_eq!(next.day(), 28);
}

#[test]
fn billing_monthly_end_of_month_jan_31_to_feb_29_leap() {
    // 2028 is a leap year: Jan 31 -> Feb 29
    let start = dt(2028, 1, 31);
    let next = compute_next_billing_date(start, BillingInterval::Monthly).unwrap();
    assert_eq!(next.month(), 2);
    assert_eq!(next.day(), 29);
}

#[test]
fn billing_weekly_adds_7_days() {
    let start = dt(2026, 1, 1); // Thursday
    let next = compute_next_billing_date(start, BillingInterval::Weekly).unwrap();
    assert_eq!(next.day(), 8);
    assert_eq!(next.month(), 1);
}

#[test]
fn billing_biweekly_adds_14_days() {
    let start = dt(2026, 1, 1);
    let next = compute_next_billing_date(start, BillingInterval::Biweekly).unwrap();
    assert_eq!(next.day(), 15);
    assert_eq!(next.month(), 1);
}

#[test]
fn billing_quarterly_jan_15_to_apr_15() {
    let start = dt(2026, 1, 15);
    let next = compute_next_billing_date(start, BillingInterval::Quarterly).unwrap();
    assert_eq!(next.month(), 4);
    assert_eq!(next.day(), 15);
}

#[test]
fn billing_annual_feb_29_leap_to_feb_28_next_year() {
    // 2024 is a leap year; 2025 is not
    let start = dt(2024, 2, 29);
    let next = compute_next_billing_date(start, BillingInterval::Annual).unwrap();
    assert_eq!(next.year(), 2025);
    assert_eq!(next.month(), 2);
    assert_eq!(next.day(), 28);
}

#[test]
fn billing_weekly_crosses_month_boundary() {
    let start = dt(2026, 1, 28);
    let next = compute_next_billing_date(start, BillingInterval::Weekly).unwrap();
    assert_eq!(next.month(), 2);
    assert_eq!(next.day(), 4);
}

// ===========================================================================
// 4. CONDITIONAL ESCROW
// ===========================================================================

#[test]
fn escrow_seller_fulfilled_condition() {
    let quote_id = Uuid::new_v4();
    let c = Condition::seller_fulfilled(Some(quote_id));

    assert_eq!(c.condition_type, ConditionType::SellerFulfilled);
    assert!(!c.completed);
    assert_eq!(c.quote_id, Some(quote_id));
    assert!(c.release_after.is_none());
    assert!(c.description.is_none());
}

#[test]
fn escrow_buyer_confirmed_condition() {
    let c = Condition::buyer_confirmed();

    assert_eq!(c.condition_type, ConditionType::BuyerConfirmed);
    assert!(!c.completed);
    assert!(c.quote_id.is_none());
}

#[test]
fn escrow_time_lock_condition_with_future_date() {
    let future = dt(2027, 6, 15);
    let c = Condition::time_lock(future);

    assert_eq!(c.condition_type, ConditionType::TimeLock);
    assert_eq!(c.release_after, Some(future));
    assert!(!c.completed);
}

#[test]
fn escrow_milestone_condition() {
    let c = Condition::milestone("Phase 2: MVP delivery");

    assert_eq!(c.condition_type, ConditionType::Milestone);
    assert_eq!(c.description, Some("Phase 2: MVP delivery".into()));
    assert!(!c.completed);
}

#[test]
fn escrow_all_four_condition_types_evaluate_correctly() {
    let now = dt(2026, 2, 23);
    let past = dt(2026, 1, 1);
    let future = dt(2027, 1, 1);

    // SellerFulfilled: met when quote status is "fulfilled"
    assert!(evaluate_seller_fulfilled(Some("fulfilled")));
    assert!(!evaluate_seller_fulfilled(Some("pending")));
    assert!(!evaluate_seller_fulfilled(None));

    // BuyerConfirmed: met when completed
    let mut buyer = Condition::buyer_confirmed();
    assert!(!evaluate_buyer_confirmed(&buyer));
    buyer.confirm();
    assert!(evaluate_buyer_confirmed(&buyer));

    // TimeLock: met when now >= release_after
    let past_lock = Condition::time_lock(past);
    assert!(evaluate_time_lock(&past_lock, now));
    let future_lock = Condition::time_lock(future);
    assert!(!evaluate_time_lock(&future_lock, now));

    // Milestone: met when completed
    let mut milestone = Condition::milestone("Ship v1");
    assert!(!evaluate_milestone(&milestone));
    milestone.confirm();
    assert!(evaluate_milestone(&milestone));
}

#[test]
fn escrow_evaluate_all_conditions_all_met() {
    let now = dt(2026, 2, 23);
    let past = dt(2026, 1, 1);
    let quote_id = Uuid::new_v4();

    let mut buyer = Condition::buyer_confirmed();
    buyer.confirm();
    let time_lock = Condition::time_lock(past);
    let mut milestone = Condition::milestone("Done");
    milestone.confirm();
    let seller = Condition::seller_fulfilled(Some(quote_id));

    let conditions = vec![buyer, time_lock, milestone, seller];

    let (all_met, evals) = evaluate_all_conditions(&conditions, now, |id| {
        if id == Some(&quote_id) { Some("fulfilled".into()) } else { None }
    });

    assert!(all_met);
    assert_eq!(evals.len(), 4);
    assert!(evals.iter().all(|e| e.met));
}

#[test]
fn escrow_evaluate_all_conditions_one_unmet() {
    let now = dt(2026, 2, 23);
    let future = dt(2027, 1, 1);

    let mut buyer = Condition::buyer_confirmed();
    buyer.confirm();
    let future_lock = Condition::time_lock(future); // NOT met

    let (all_met, evals) = evaluate_all_conditions(&[buyer, future_lock], now, |_| None);

    assert!(!all_met);
    assert!(evals[0].met);
    assert!(!evals[1].met);
}

#[test]
fn escrow_evaluate_dispatch_routes_correctly() {
    let now = dt(2026, 2, 23);
    let past = dt(2026, 1, 1);

    // BuyerConfirmed via dispatch
    let mut bc = Condition::buyer_confirmed();
    bc.confirm();
    assert!(evaluate_condition(&bc, now, None));

    // TimeLock via dispatch
    let tl = Condition::time_lock(past);
    assert!(evaluate_condition(&tl, now, None));

    // SellerFulfilled via dispatch
    let sf = Condition::seller_fulfilled(None);
    assert!(evaluate_condition(&sf, now, Some("fulfilled")));
    assert!(!evaluate_condition(&sf, now, Some("pending")));

    // Milestone via dispatch
    let ms = Condition::milestone("Test");
    assert!(!evaluate_condition(&ms, now, None));
}

// ===========================================================================
// 5. HMAC WEBHOOK SIGNING / VERIFICATION
// ===========================================================================

#[test]
fn hmac_sign_then_verify_same_secret() {
    let secret = b"whsec_test_secret_2026";
    let payload = br#"{"event":"payment.completed","amount":"100.00"}"#;

    let signature = sign_webhook(secret, payload);
    assert!(
        verify_webhook(secret, payload, &signature),
        "verification must succeed with matching secret"
    );
}

#[test]
fn hmac_verify_fails_with_wrong_secret() {
    let payload = b"test payload";
    let signature = sign_webhook(b"correct_secret", payload);

    assert!(
        !verify_webhook(b"wrong_secret", payload, &signature),
        "verification must fail with wrong secret"
    );
}

#[test]
fn hmac_verify_fails_with_tampered_payload() {
    let secret = b"my_secret";
    let signature = sign_webhook(secret, b"original payload");

    assert!(
        !verify_webhook(secret, b"tampered payload", &signature),
        "verification must fail with tampered payload"
    );
}

#[test]
fn hmac_signature_is_deterministic() {
    let secret = b"deterministic_key";
    let payload = b"deterministic_payload";

    let sig1 = sign_webhook(secret, payload);
    let sig2 = sign_webhook(secret, payload);

    assert_eq!(sig1, sig2, "same inputs must produce identical signatures");
}

#[test]
fn hmac_signature_is_64_hex_chars() {
    let sig = sign_webhook(b"any_key", b"any_data");

    assert_eq!(sig.len(), 64, "HMAC-SHA256 hex output must be 64 chars");
    assert!(sig.chars().all(|c| c.is_ascii_hexdigit()), "signature must be valid hex");
    // 64 hex chars = 32 bytes
    let decoded = hex::decode(&sig).expect("must be valid hex");
    assert_eq!(decoded.len(), 32);
}

#[test]
fn hmac_known_test_vector() {
    // HMAC-SHA256("key", "The quick brown fox jumps over the lazy dog")
    // is a well-known test vector.
    let sig = sign_webhook(b"key", b"The quick brown fox jumps over the lazy dog");
    assert_eq!(sig, "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8");
}

#[test]
fn hmac_empty_payload() {
    let secret = b"secret";
    let sig = sign_webhook(secret, b"");
    assert!(verify_webhook(secret, b"", &sig));
}

#[test]
fn hmac_verify_rejects_invalid_hex() {
    assert!(!verify_webhook(b"secret", b"payload", "zzzz_not_hex"));
}

// ===========================================================================
// 6. SSRF URL VALIDATION
// ===========================================================================

#[test]
fn ssrf_valid_https_passes() {
    assert!(validate_url("https://example.com/webhooks").is_ok());
}

#[test]
fn ssrf_valid_http_passes() {
    assert!(validate_url("http://8.8.8.8/hooks").is_ok());
}

#[test]
fn ssrf_blocks_localhost() {
    let err = validate_url("http://localhost/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_localhost_with_port() {
    let err = validate_url("http://localhost:8080/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_127_0_0_1() {
    let err = validate_url("http://127.0.0.1/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_10_x_x_x() {
    let err = validate_url("http://10.0.0.1/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));

    let err2 = validate_url("http://10.255.255.255/hook").unwrap_err();
    assert!(matches!(err2, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_172_16_through_31() {
    for second_octet in 16..=31 {
        let url = format!("http://172.{second_octet}.0.1/hook");
        let err = validate_url(&url).unwrap_err();
        assert!(
            matches!(err, A2AError::SsrfBlocked(_)),
            "172.{second_octet}.x.x should be blocked"
        );
    }
}

#[test]
fn ssrf_allows_172_15_and_172_32() {
    assert!(validate_url("http://172.15.0.1/hook").is_ok());
    assert!(validate_url("http://172.32.0.1/hook").is_ok());
}

#[test]
fn ssrf_blocks_192_168_x_x() {
    let err = validate_url("http://192.168.0.1/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));

    let err2 = validate_url("http://192.168.255.255/hook").unwrap_err();
    assert!(matches!(err2, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_dot_local_tld() {
    let err = validate_url("http://myhost.local/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_dot_internal_tld() {
    let err = validate_url("http://service.internal/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_dot_localhost_tld() {
    let err = validate_url("http://app.localhost/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_ipv6_loopback() {
    let err = validate_url("http://[::1]/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_ftp_scheme() {
    let err = validate_url("ftp://example.com/file").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_file_scheme() {
    let err = validate_url("file:///etc/passwd").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_blocks_gopher_scheme() {
    let err = validate_url("gopher://evil.com/").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_rejects_empty_url() {
    let err = validate_url("").unwrap_err();
    assert!(matches!(err, A2AError::Validation(_)));
}

#[test]
fn ssrf_rejects_url_without_scheme() {
    let err = validate_url("example.com/hook").unwrap_err();
    assert!(matches!(err, A2AError::Validation(_)));
}

#[test]
fn ssrf_allows_public_ip() {
    assert!(validate_url("https://8.8.8.8/hook").is_ok());
}

#[test]
fn ssrf_blocks_reserved_documentation_ip() {
    let err = validate_url("https://203.0.113.1/hook").unwrap_err();
    assert!(matches!(err, A2AError::SsrfBlocked(_)));
}

#[test]
fn ssrf_allows_subdomain() {
    assert!(validate_url("https://www.example.com/a2a").is_ok());
}

// ===========================================================================
// 7. CROSS-MODULE INTEGRATION SCENARIOS
// ===========================================================================

/// End-to-end: compute a split, then validate the webhook URL for each
/// recipient, sign a notification payload, and verify it.
#[test]
fn integration_split_then_webhook_notify() {
    // Step 1: compute a split
    let recipients = vec![
        pct_recipient("https://8.8.8.8/hook", dec!(60)),
        pct_recipient("https://1.1.1.1/hook", dec!(40)),
    ];
    let split = calculate_percentage_split(dec!(500), dec!(2), &recipients).unwrap();
    assert_eq!(split.total_distributed, dec!(500));

    // Step 2: validate each recipient URL (they happen to be valid URLs)
    for share in &split.shares {
        validate_url(&share.address).expect("recipient URL should be safe");
    }

    // Step 3: sign + verify a webhook notification per share
    let secret = b"whsec_integration_test";
    for share in &split.shares {
        let payload = format!(r#"{{"recipient":"{}","amount":"{}"}}"#, share.address, share.amount);
        let sig = sign_webhook(secret, payload.as_bytes());
        assert!(verify_webhook(secret, payload.as_bytes(), &sig));
    }
}

/// End-to-end: create a subscription, compute billing dates across several
/// months, verify each date is correct.
#[test]
fn integration_subscription_billing_chain() {
    let mut current = dt(2026, 1, 15);

    // Chain 6 monthly billing cycles
    for expected_month in [2, 3, 4, 5, 6, 7] {
        current = compute_next_billing_date(current, BillingInterval::Monthly).unwrap();
        assert_eq!(current.month(), expected_month);
        assert_eq!(current.day(), 15);
    }
}

/// End-to-end: create escrow conditions, evaluate them partially, then
/// complete all and verify release.
#[test]
fn integration_escrow_progressive_completion() {
    let now = dt(2026, 2, 23);
    let past = dt(2026, 1, 1);
    let quote_id = Uuid::new_v4();

    let mut buyer = Condition::buyer_confirmed();
    let time_lock = Condition::time_lock(past);
    let mut milestone = Condition::milestone("Deliver prototype");
    let seller = Condition::seller_fulfilled(Some(quote_id));

    let quote_lookup = |id: Option<&Uuid>| -> Option<String> {
        if id == Some(&quote_id) { Some("fulfilled".into()) } else { None }
    };

    // Initially: buyer not confirmed, milestone not done
    let (all_met, _) = evaluate_all_conditions(
        &[buyer.clone(), time_lock.clone(), milestone.clone(), seller.clone()],
        now,
        quote_lookup,
    );
    assert!(!all_met);

    // Confirm buyer
    buyer.confirm();
    let (all_met, _) = evaluate_all_conditions(
        &[buyer.clone(), time_lock.clone(), milestone.clone(), seller.clone()],
        now,
        quote_lookup,
    );
    assert!(!all_met, "milestone still incomplete");

    // Complete milestone
    milestone.confirm();
    let (all_met, evals) =
        evaluate_all_conditions(&[buyer, time_lock, milestone, seller], now, quote_lookup);
    assert!(all_met, "all conditions should now be met");
    assert!(evals.iter().all(|e| e.met));
}

/// Verify event filtering works with the split/escrow event types a real
/// system would emit.
#[test]
fn integration_event_filter_realistic_patterns() {
    let filters = vec![
        "a2a_split.*".to_string(),
        "escrow.released".to_string(),
        "subscription.cancelled".to_string(),
    ];

    // Should match
    assert!(matches_event_filter("a2a_split.created", &filters));
    assert!(matches_event_filter("a2a_split.completed", &filters));
    assert!(matches_event_filter("escrow.released", &filters));
    assert!(matches_event_filter("subscription.cancelled", &filters));

    // Should not match
    assert!(!matches_event_filter("escrow.funded", &filters));
    assert!(!matches_event_filter("subscription.paused", &filters));
    assert!(!matches_event_filter("payment.completed", &filters));
}

// ===========================================================================
// 8. DISPUTE LIFECYCLE
// ===========================================================================

#[test]
fn dispute_full_lifecycle_to_resolution() {
    use stateset_a2a::disputes::{
        DisputeCategory, DisputeRecord, DisputeStatus, DisputeTransition, ResolutionType,
        resolution_to_escrow_action,
    };

    // File a dispute
    let record = DisputeRecord::new(
        Uuid::new_v4(),
        "0xBuyer",
        "0xSeller",
        DisputeCategory::NonDelivery,
        "Item was never shipped",
    )
    .with_amount(dec!(100));

    assert_eq!(record.status, DisputeStatus::Filed);
    assert_eq!(record.amount, Some(dec!(100)));

    // Transition through the lifecycle
    let t1 = DisputeTransition::new(DisputeStatus::Filed, DisputeStatus::EvidencePeriod).unwrap();
    let t2 = DisputeTransition::new(t1.to, DisputeStatus::UnderReview).unwrap();
    let t3 = DisputeTransition::new(t2.to, DisputeStatus::Resolved).unwrap();
    assert_eq!(t3.to, DisputeStatus::Resolved);

    // Resolution maps to escrow action
    let action = resolution_to_escrow_action(ResolutionType::FullRefund, dec!(100));
    assert_eq!(action.refund_amount, Some(dec!(100)));
    assert_eq!(action.release_amount, None);
}

#[test]
fn dispute_evidence_hashing_and_verification() {
    use stateset_a2a::disputes::EvidenceType;
    use stateset_a2a::disputes::{Evidence, evidence::verify_evidence_hash, hash_evidence};

    let content = b"Transaction log: payment of 100 USDC on 2026-02-15";
    let evidence = Evidence::new(
        Uuid::new_v4(),
        "0xBuyer",
        EvidenceType::TransactionLog,
        "Payment proof",
        content,
    );

    // Hash is deterministic and verifiable
    assert_eq!(evidence.content_hash, hash_evidence(content));
    assert!(verify_evidence_hash(content, &evidence.content_hash));
    assert!(!verify_evidence_hash(b"tampered content", &evidence.content_hash));
}

#[test]
fn dispute_escalation_path() {
    use stateset_a2a::disputes::{
        DisputeStatus, DisputeTransition, ResolutionType, resolution_to_escrow_action,
    };

    // Dispute can be escalated from under_review
    let t1 = DisputeTransition::new(DisputeStatus::Filed, DisputeStatus::EvidencePeriod).unwrap();
    let t2 = DisputeTransition::new(t1.to, DisputeStatus::UnderReview).unwrap();
    let t3 = DisputeTransition::new(t2.to, DisputeStatus::Escalated).unwrap();
    assert!(t3.to.is_terminal());

    // Escalated resolution holds funds
    let action = resolution_to_escrow_action(ResolutionType::Escalated, dec!(500));
    assert!(action.hold);
    assert_eq!(action.refund_amount, None);
    assert_eq!(action.release_amount, None);
}

// ===========================================================================
// 9. REPUTATION AND TRUST TIERS
// ===========================================================================

#[test]
fn reputation_scoring_and_tier_promotion() {
    use stateset_a2a::reputation::scoring::{FeedbackEntry, aggregate_feedback, validate_score};
    use stateset_a2a::reputation::tiers::TrustTier;

    // Validate score boundaries
    assert!(validate_score(dec!(1)).is_ok());
    assert!(validate_score(dec!(5)).is_ok());
    assert!(validate_score(dec!(0)).is_err());
    assert!(validate_score(dec!(6)).is_err());

    // Build up reputation: 6 good transactions → standard tier
    let entries: Vec<FeedbackEntry> = (0..6)
        .map(|_| FeedbackEntry { score: dec!(4), dimensions: None, revoked: false })
        .collect();
    let summary = aggregate_feedback(&entries);
    assert_eq!(summary.trust_tier, TrustTier::Standard);
    assert_eq!(summary.total_transactions, 6);
    assert_eq!(summary.average_score, dec!(4));
}

#[test]
fn reputation_tier_progression() {
    use stateset_a2a::reputation::tiers::TrustTier;

    // Sandbox → Standard (5 txns, 3.5 avg)
    assert_eq!(TrustTier::compute_tier(5, dec!(3.5), 0, Decimal::ZERO), TrustTier::Standard);

    // Standard → Verified (25 txns, 4.0 avg, 0 disputes)
    assert_eq!(TrustTier::compute_tier(25, dec!(4.0), 0, Decimal::ZERO), TrustTier::Verified);

    // Verified → Enterprise (100 txns, 4.5 avg, <2% disputes)
    assert_eq!(TrustTier::compute_tier(100, dec!(4.5), 0, dec!(0.01)), TrustTier::Enterprise);

    // Enterprise blocked by high dispute rate
    assert_ne!(TrustTier::compute_tier(100, dec!(4.5), 0, dec!(0.05)), TrustTier::Enterprise);
}

// ===========================================================================
// 10. CIRCUIT BREAKER
// ===========================================================================

#[test]
fn circuit_breaker_lifecycle() {
    use stateset_a2a::circuit_breaker::limits::{
        LimitCheckResult, SpendingLimits, check_spending_limits,
    };
    use stateset_a2a::circuit_breaker::{CircuitBreakerConfig, CircuitState, CircuitTransition};

    // Normal operation
    let cfg = CircuitBreakerConfig::default();
    let spending = SpendingLimits::default();
    let result =
        check_spending_limits(&cfg, CircuitState::Closed, dec!(100), &spending, Decimal::ZERO);
    assert_eq!(result, LimitCheckResult::Allowed);

    // Circuit trips on failure rate
    let result =
        check_spending_limits(&cfg, CircuitState::Closed, dec!(100), &spending, dec!(0.35));
    assert_eq!(result, LimitCheckResult::FailureRateExceeded);

    // Open state blocks all transactions
    let result =
        check_spending_limits(&cfg, CircuitState::Open, dec!(100), &spending, Decimal::ZERO);
    assert_eq!(result, LimitCheckResult::CircuitOpen);

    // Recovery: open → half_open → closed
    let t1 = CircuitTransition::new(CircuitState::Open, CircuitState::HalfOpen).unwrap();
    let t2 = CircuitTransition::new(t1.to, CircuitState::Closed).unwrap();
    assert_eq!(t2.to, CircuitState::Closed);
}

#[test]
fn circuit_breaker_spending_limits() {
    use stateset_a2a::circuit_breaker::limits::{
        LimitCheckResult, SpendingLimits, check_spending_limits,
    };
    use stateset_a2a::circuit_breaker::{CircuitBreakerConfig, CircuitState};

    let cfg = CircuitBreakerConfig::default();

    // Per-transaction limit: 1000 (default)
    let result = check_spending_limits(
        &cfg,
        CircuitState::Closed,
        dec!(1001),
        &SpendingLimits::default(),
        Decimal::ZERO,
    );
    assert_eq!(result, LimitCheckResult::PerTransactionExceeded);

    // Daily limit: 10000 (default)
    let spending = SpendingLimits { daily_spent: dec!(9500), monthly_spent: dec!(9500) };
    let result =
        check_spending_limits(&cfg, CircuitState::Closed, dec!(600), &spending, Decimal::ZERO);
    assert_eq!(result, LimitCheckResult::DailyLimitExceeded);

    // Monthly limit: 100000 (default)
    let spending = SpendingLimits { daily_spent: Decimal::ZERO, monthly_spent: dec!(99500) };
    let result =
        check_spending_limits(&cfg, CircuitState::Closed, dec!(600), &spending, Decimal::ZERO);
    assert_eq!(result, LimitCheckResult::MonthlyLimitExceeded);
}

// ===========================================================================
// 11. SLA COMPLIANCE
// ===========================================================================

#[test]
fn sla_compliance_all_pass() {
    use stateset_a2a::sla::compliance::ActualMetrics;
    use stateset_a2a::sla::{SlaDefinition, check_compliance};

    let sla =
        SlaDefinition::new(Uuid::new_v4()).with_response_time(dec!(500)).with_uptime(dec!(99));

    let actual = ActualMetrics {
        avg_response_time_ms: Some(dec!(400)),
        success_rate: Some(dec!(0.995)),
        ..ActualMetrics::default()
    };

    let result = check_compliance(&sla, &actual, dec!(100)).unwrap();
    assert!(result.compliant);
    assert!(result.violations.is_empty());
}

#[test]
fn sla_compliance_violations_and_penalties() {
    use stateset_a2a::sla::compliance::ActualMetrics;
    use stateset_a2a::sla::{SlaDefinition, SlaMetricType, check_compliance};

    let sla = SlaDefinition::new(Uuid::new_v4())
        .with_response_time(dec!(500))
        .with_uptime(dec!(99))
        .with_penalty_percent(dec!(10));

    let actual = ActualMetrics {
        avg_response_time_ms: Some(dec!(800)),
        success_rate: Some(dec!(0.90)),
        ..ActualMetrics::default()
    };

    let result = check_compliance(&sla, &actual, dec!(200)).unwrap();
    assert!(!result.compliant);
    assert_eq!(result.violations.len(), 2);
    assert_eq!(result.violations[0].metric, SlaMetricType::ResponseTimeMs);
    assert_eq!(result.violations[1].metric, SlaMetricType::UptimePercent);

    // Each violation: 200 * 10% = 20, total = 40
    assert_eq!(result.total_penalty, dec!(40));
}

// ===========================================================================
// 12. MARKETPLACE / RFQ
// ===========================================================================

#[test]
fn marketplace_rfq_scoring_and_ranking() {
    use stateset_a2a::marketplace::{RfqResponse, ScoringCriteria, rank_responses};

    let responses = vec![
        RfqResponse::new("Expensive", dec!(300)).with_reputation(dec!(5)),
        RfqResponse::new("Cheap", dec!(100)).with_reputation(dec!(3)),
        RfqResponse::new("Medium", dec!(200)).with_reputation(dec!(4)),
    ];

    // Cheapest: cheapest seller wins
    let ranked = rank_responses(&responses, ScoringCriteria::Cheapest);
    assert_eq!(ranked[0].seller, "Cheap");
    assert_eq!(ranked[0].rank, Some(1));

    // Best value: blended price + reputation
    let ranked = rank_responses(&responses, ScoringCriteria::BestValue);
    // The cheap one with decent rep should still be top due to 60% price weight
    assert!(ranked[0].score > ranked[1].score);
}

#[test]
fn marketplace_rfq_state_machine() {
    use stateset_a2a::marketplace::{RfqStatus, RfqTransition};

    // Open → Awarded
    let t = RfqTransition::new(RfqStatus::Open, RfqStatus::Awarded).unwrap();
    assert_eq!(t.to, RfqStatus::Awarded);
    assert!(RfqStatus::Awarded.is_terminal());

    // Open → Expired
    let t = RfqTransition::new(RfqStatus::Open, RfqStatus::Expired).unwrap();
    assert!(t.to.is_terminal());

    // Terminal state cannot transition
    assert!(RfqTransition::new(RfqStatus::Awarded, RfqStatus::Open).is_err());
}

// ===========================================================================
// 13. AGENT CARDS
// ===========================================================================

#[test]
fn agent_card_validation_and_discovery() {
    use stateset_a2a::agent_cards::types::filter_agents;
    use stateset_a2a::agent_cards::{AgentCard, AgentSkill, DiscoveryFilter, validate_agent_card};
    use stateset_a2a::reputation::TrustTier;

    // Valid card
    let card = AgentCard::new("TestBot", "0xABC123", "An AI commerce agent");
    assert!(validate_agent_card(&card).is_ok());

    // Invalid card (empty name)
    let mut bad_card = card;
    bad_card.name = String::new();
    assert!(validate_agent_card(&bad_card).is_err());

    // Discovery filtering
    let cards = vec![
        AgentCard::new("Alice", "0x1", "desc")
            .with_trust_tier(TrustTier::Verified)
            .with_skills(vec![AgentSkill::Sell, AgentSkill::Quote]),
        AgentCard::new("Bob", "0x2", "desc")
            .with_trust_tier(TrustTier::Standard)
            .with_skills(vec![AgentSkill::Buy]),
    ];

    // Filter by skill
    let filter = DiscoveryFilter { skill: Some(AgentSkill::Sell), ..Default::default() };
    let results = filter_agents(&cards, &filter);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Alice");

    // Filter by trust tier
    let filter =
        DiscoveryFilter { min_trust_tier: Some(TrustTier::Verified), ..Default::default() };
    let results = filter_agents(&cards, &filter);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Alice");
}

// ===========================================================================
// 14. CROSS-MODULE INTEGRATION: DISPUTE + REPUTATION + CIRCUIT BREAKER
// ===========================================================================

#[test]
fn cross_module_dispute_impacts_reputation() {
    use stateset_a2a::reputation::TrustTier;
    use stateset_a2a::reputation::scoring::{FeedbackEntry, aggregate_feedback};

    // Agent has mostly good transactions but one dispute
    let mut entries: Vec<FeedbackEntry> = (0..24)
        .map(|_| FeedbackEntry { score: dec!(4.5), dimensions: None, revoked: false })
        .collect();
    // One disputed transaction
    entries.push(FeedbackEntry { score: dec!(1), dimensions: None, revoked: false });

    let summary = aggregate_feedback(&entries);
    assert_eq!(summary.total_transactions, 25);
    assert_eq!(summary.disputed_transactions, 1); // score <= 2
    assert!(summary.average_score > dec!(4)); // Still high overall

    // With 1 unresolved dispute, can't reach Verified (requires 0)
    let tier = TrustTier::compute_tier(25, summary.average_score, 1, Decimal::ZERO);
    assert_eq!(tier, TrustTier::Standard); // Blocked by unresolved dispute

    // After resolving the dispute
    let tier = TrustTier::compute_tier(25, summary.average_score, 0, Decimal::ZERO);
    assert_eq!(tier, TrustTier::Verified);
}

#[test]
fn cross_module_circuit_breaker_blocks_then_recovers() {
    use stateset_a2a::circuit_breaker::limits::{
        LimitCheckResult, SpendingLimits, check_spending_limits,
    };
    use stateset_a2a::circuit_breaker::{CircuitBreakerConfig, CircuitState, CircuitTransition};

    let cfg = CircuitBreakerConfig::default();
    let spending = SpendingLimits::default();

    // Normal operation
    let result =
        check_spending_limits(&cfg, CircuitState::Closed, dec!(500), &spending, Decimal::ZERO);
    assert!(result.is_allowed());

    // Failure rate spikes — would trip to open
    let result = check_spending_limits(&cfg, CircuitState::Closed, dec!(500), &spending, dec!(0.4));
    assert_eq!(result, LimitCheckResult::FailureRateExceeded);

    // Now in open state — blocked
    let result =
        check_spending_limits(&cfg, CircuitState::Open, dec!(100), &spending, Decimal::ZERO);
    assert!(!result.is_allowed());

    // Cooldown passes → half_open
    let t1 = CircuitTransition::new(CircuitState::Open, CircuitState::HalfOpen).unwrap();

    // Half-open allows limited transactions
    let result = check_spending_limits(&cfg, t1.to, dec!(100), &spending, Decimal::ZERO);
    assert!(result.is_allowed());

    // Successes in half-open → close
    let t2 = CircuitTransition::new(CircuitState::HalfOpen, CircuitState::Closed).unwrap();
    assert!(t2.to.is_normal());
}

#[test]
fn cross_module_sla_violation_with_penalty() {
    use stateset_a2a::sla::compliance::ActualMetrics;
    use stateset_a2a::sla::violations::ViolationSeverity;
    use stateset_a2a::sla::{SlaDefinition, SlaMetricType, check_compliance};

    let sla = SlaDefinition::new(Uuid::new_v4())
        .with_response_time(dec!(500))
        .with_quality(dec!(4.0))
        .with_penalty_percent(dec!(5));

    // Severe quality violation (< 80% of target)
    let actual = ActualMetrics {
        avg_response_time_ms: Some(dec!(400)), // OK
        avg_quality_score: Some(dec!(2.0)),    // 2.0/4.0 = 50% → critical
        ..ActualMetrics::default()
    };

    let result = check_compliance(&sla, &actual, dec!(1000)).unwrap();
    assert!(!result.compliant);
    assert_eq!(result.violations.len(), 1);
    assert_eq!(result.violations[0].metric, SlaMetricType::QualityMinScore);
    assert_eq!(result.violations[0].severity, ViolationSeverity::Critical);
    assert_eq!(result.violations[0].penalty_amount, dec!(50)); // 1000 * 5%
}
