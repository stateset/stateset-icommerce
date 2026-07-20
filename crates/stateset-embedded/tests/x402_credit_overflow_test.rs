//! Regression for x402 credit-balance overflow (SQLite).
//!
//! The SQLite Credit path added `current_balance + amount_i64` directly, which
//! panics on i64 overflow in debug and wraps in release. It now uses
//! `checked_add` and rejects the overflow with a `ValidationError`, matching the
//! Postgres backend.

#![cfg(feature = "sqlite")]

use stateset_embedded::{
    Commerce, X402Asset, X402CreditAdjustment, X402CreditDirection, X402Network,
};

#[test]
fn x402_credit_overflow_is_rejected() {
    let commerce = Commerce::new(":memory:").expect("Failed to create commerce");
    let credit = |amount: u64| X402CreditAdjustment {
        payer_address: "0xoverflow".into(),
        asset: X402Asset::Usdc,
        network: X402Network::SetChain,
        direction: X402CreditDirection::Credit,
        amount,
        reason: None,
        reference_id: None,
        metadata: None,
    };

    // Credit the maximum representable balance.
    commerce.x402().adjust_credit_balance(credit(i64::MAX as u64)).expect("credit max balance");

    // One more unit would overflow i64 — it must be rejected with a clean error,
    // not a panic (debug) or silent wrap to a negative balance (release).
    let err = commerce
        .x402()
        .adjust_credit_balance(credit(1))
        .expect_err("crediting past i64::MAX must be rejected");
    assert!(
        matches!(err, stateset_core::CommerceError::ValidationError(_)),
        "expected ValidationError, got {err:?}"
    );
}
