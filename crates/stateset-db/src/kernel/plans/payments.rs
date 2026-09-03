//! `payments.create` and `payments.create_refund` plans.

use super::PlanOutcome;
use crate::kernel::envelope::GuardRejection;
use rust_decimal::Decimal;
use stateset_core::{CreatePayment, CreateRefund, CurrencyCode, Payment, Validate};

/// Static payload checks for `payments.create` (after key normalisation).
#[must_use]
pub fn create_payment_guard(input: &CreatePayment) -> Option<GuardRejection> {
    input
        .validate()
        .err()
        .map(|error| GuardRejection::never("commerce.validation_failed", error.to_string()))
}

/// What the backend loads (under its row lock) for a refund.
#[derive(Debug, Clone)]
pub struct RefundSnapshot {
    /// The payment being refunded.
    pub payment: Payment,
    /// Sum of refunds still pending or processing against the payment.
    pub in_flight_refunds: Decimal,
    /// Escrow (and open dispute id) freezing the payment, if any.
    pub open_dispute: Option<(String, Option<String>)>,
}

/// Effects of an accepted refund.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefundEffects {
    pub amount: Decimal,
    pub currency: CurrencyCode,
}

/// Evaluate a refund. `None` means the payment does not exist.
#[must_use]
pub fn plan_refund(
    input: &CreateRefund,
    snapshot: Option<&RefundSnapshot>,
) -> PlanOutcome<RefundEffects> {
    let Some(snapshot) = snapshot else {
        return PlanOutcome::reject(GuardRejection::never(
            "commerce.payment_not_found",
            "payment does not exist",
        ));
    };
    let reject = |rejection: GuardRejection| PlanOutcome::Reject {
        rejection,
        version_before: None,
        aggregate_id: None,
    };
    let payment = &snapshot.payment;
    if let Err(error) = input.validate_for_currency(payment.currency) {
        return reject(GuardRejection::never(
            "commerce.refund_validation_failed",
            error.to_string(),
        ));
    }
    // A payment held by an A2A escrow under an open dispute cannot be
    // refunded directly: the funds are frozen until the dispute is resolved
    // (`a2a.dispute.resolve`), which settles the escrow itself.
    if let Some((escrow_id, dispute_id)) = &snapshot.open_dispute {
        return reject(GuardRejection::never(
            "commerce.refund.escrow_disputed",
            format!(
                "payment is held by escrow {escrow_id} under open dispute {}; resolve the dispute instead of refunding directly",
                dispute_id.as_deref().unwrap_or("(unfiled)")
            ),
        ));
    }
    let mut projected = payment.clone();
    projected.amount_refunded += snapshot.in_flight_refunds;
    match projected.validate_refund(input.amount) {
        Ok(amount) => PlanOutcome::Proceed(RefundEffects { amount, currency: payment.currency }),
        Err(error) => reject(GuardRejection::never(
            error.invariant_code().unwrap_or("commerce.refund_rejected"),
            error.to_string(),
        )),
    }
}
