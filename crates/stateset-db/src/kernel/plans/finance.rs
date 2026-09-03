//! Static plans for the repository-delegating money commands:
//! `subscriptions.charge`, `checkout.commit`, `ledger.post`, `x402.settle`.

use crate::kernel::envelope::GuardRejection;
use stateset_core::{ChargeSubscription, CommitCheckout, PostJournalEntry, SettleX402Intent};

/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for billing cycles.
pub const BILLING_CYCLE_UNVERSIONED: &str = "billing cycles do not expose an aggregate version";
/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for carts.
pub const CART_UNVERSIONED: &str = "carts do not expose an aggregate version";
/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for journal entries.
pub const JOURNAL_ENTRY_UNVERSIONED: &str = "journal entries do not expose an aggregate version";
/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for x402 intents.
pub const X402_INTENT_UNVERSIONED: &str = "x402 payment intents do not expose an aggregate version";

/// Static payload checks for `subscriptions.charge`.
#[must_use]
pub fn charge_subscription_guard(input: &ChargeSubscription) -> Option<GuardRejection> {
    let message = if input.billing_cycle_id.is_nil() {
        "billing_cycle_id is required"
    } else if input.processor.as_deref().is_some_and(|value| value.trim().is_empty()) {
        "processor cannot be blank"
    } else {
        return None;
    };
    Some(GuardRejection::never("commerce.subscription.validation_failed", message))
}

/// Static payload checks for `checkout.commit`.
#[must_use]
pub fn commit_checkout_guard(input: &CommitCheckout) -> Option<GuardRejection> {
    input.cart_id.is_nil().then(|| {
        GuardRejection::never("commerce.checkout.validation_failed", "cart_id is required")
    })
}

/// Static payload checks for `ledger.post`.
#[must_use]
pub fn post_journal_entry_guard(input: &PostJournalEntry) -> Option<GuardRejection> {
    (input.journal_entry_id.is_nil() || input.posted_by.trim().is_empty()).then(|| {
        GuardRejection::never(
            "commerce.ledger.validation_failed",
            "journal_entry_id and posted_by are required",
        )
    })
}

/// Static payload checks for `x402.settle`.
#[must_use]
pub fn settle_x402_guard(input: &SettleX402Intent) -> Option<GuardRejection> {
    (input.intent_id.is_nil() || input.tx_hash.trim().is_empty()).then(|| {
        GuardRejection::never(
            "commerce.x402.validation_failed",
            "intent_id and tx_hash are required",
        )
    })
}
