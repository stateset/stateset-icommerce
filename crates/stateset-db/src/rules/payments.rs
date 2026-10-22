//! Backend-agnostic payments rules shared by the SQLite and Postgres repositories.

use stateset_core::{CommerceError, PaymentTransactionStatus, Result};

/// Refuse `update` writes that would move a payment INTO `Refunded` /
/// `PartiallyRefunded` as a bare status flip.
///
/// Decision: those two statuses are ledger states — they are written only by
/// `complete_refund`, which also advances `amount_refunded` and holds the
/// refund row. The state machine allows `Completed -> Refunded`,
/// `PartiallyRefunded -> Refunded` and `Disputed -> Refunded`, but taking any
/// of them through `update` left `amount_refunded` at its old value, so
/// `open_captures_for_order` still reported the money as outstanding and the
/// order could never be cancelled or deleted cleanly. A no-op write (already
/// in that status) is still allowed so metadata patches on refunded payments
/// keep working. Worded identically in the Postgres backend.
///
/// `Disputed -> Refunded` is the one exception: it is a lost chargeback, and
/// the caller records it on the ledger with `record_lost_chargeback_tx` in
/// the same transaction.
pub(crate) fn ensure_not_refund_by_status_flip(
    current: PaymentTransactionStatus,
    target: PaymentTransactionStatus,
) -> Result<()> {
    if current != target
        && !current.is_lost_chargeback(target)
        && matches!(
            target,
            PaymentTransactionStatus::Refunded | PaymentTransactionStatus::PartiallyRefunded
        )
    {
        return Err(CommerceError::ValidationError(format!(
            "Payment status cannot be set to {target} directly (from {current}); \
             refunds are recorded through create_refund + complete_refund so that \
             amount_refunded and the refund ledger stay consistent"
        )));
    }
    Ok(())
}

/// Whether a persisted status write from `from` to `to` is legal.
///
/// The rules live in the domain state machine
/// ([`PaymentTransactionStatus::can_transition_to`]); this wrapper adds exactly
/// ONE documented edge that the persistence layer needs and the enum does not
/// model: `Pending -> Completed`, the single-shot capture used by processors
/// that settle without an intermediate `Processing` step (the shipped
/// `mark_completed` contract, exercised by the invariant harness and by
/// `payments_test`). Every other answer — in particular every edge OUT of a
/// settled payment (`Completed`/`PartiallyRefunded`/`Refunded` ->
/// `Cancelled`/`Failed`, which would release the slice of the order total that
/// settled money is consuming and let the order be captured twice) — is the
/// enum's own.
pub(crate) fn payment_transition_allowed(
    from: PaymentTransactionStatus,
    to: PaymentTransactionStatus,
) -> bool {
    if from == PaymentTransactionStatus::Pending && to == PaymentTransactionStatus::Completed {
        return true;
    }
    from.can_transition_to(to)
}

/// Conflict error for a refused status write, naming the status the payment is
/// actually in. Worded identically in the Postgres backend.
pub(crate) fn transition_conflict(
    current: PaymentTransactionStatus,
    target: PaymentTransactionStatus,
) -> CommerceError {
    CommerceError::Conflict(format!("Payment is {current} and cannot transition to {target}"))
}
