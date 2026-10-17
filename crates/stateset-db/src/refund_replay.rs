//! Shared checks for an existing refund returned under a retry key.

use stateset_core::{CommerceError, CreateRefund, Refund, Result};

/// Compare immutable refund request fields. An omitted amount keeps the legacy
/// "replay the originally resolved remaining amount" behavior; an explicit
/// amount must equal the stored exact decimal. This does not reconstruct the
/// original presence/absence of an amount for historical rows.
pub(crate) fn check_refund_replay(existing: &Refund, input: &CreateRefund) -> Result<()> {
    if existing.payment_id != input.payment_id
        || input.amount.is_some_and(|amount| amount != existing.amount)
        || existing.reason != input.reason
        || existing.external_id != input.external_id
        || existing.notes != input.notes
    {
        return Err(CommerceError::Conflict(
            "refund idempotency key already belongs to different request fields".into(),
        ));
    }
    Ok(())
}
