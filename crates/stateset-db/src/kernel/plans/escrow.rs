//! `a2a.escrow.create` and `a2a.escrow.fund` plans.

use super::PlanOutcome;
use crate::kernel::envelope::GuardRejection;
use chrono::{DateTime, Utc};
use stateset_core::{
    A2ADisputeResolutionType, A2AEscrow, A2AEscrowStatus, CreateA2AEscrow, DisputeA2AEscrow,
    FileA2ADispute, ResolveA2ADispute, SubmitA2ADisputeEvidence,
};

const VALIDATION: &str = "commerce.a2a.escrow.validation_failed";

/// Legacy integer amount stored beside the exact decimal, when representable.
#[must_use]
pub fn escrow_legacy_amount(input: &CreateA2AEscrow) -> Option<i64> {
    i64::try_from(input.amount.normalize().mantissa()).ok()
}

/// Static payload checks for `a2a.escrow.create`.
#[must_use]
pub fn create_escrow_guard(input: &CreateA2AEscrow, now: DateTime<Utc>) -> Option<GuardRejection> {
    let message = if input.buyer_address.trim().is_empty()
        || input.seller_address.trim().is_empty()
        || input.asset.trim().is_empty()
        || input.network.trim().is_empty()
    {
        "buyer_address, seller_address, asset, and network are required"
    } else if input.buyer_address == input.seller_address {
        "buyer_address and seller_address must differ"
    } else if input.amount <= rust_decimal::Decimal::ZERO {
        "amount must be greater than zero"
    } else if escrow_legacy_amount(input).is_none() {
        "amount exceeds the embedded escrow compatibility range"
    } else if input.release_conditions.len() > 20 {
        "release_conditions cannot contain more than 20 entries"
    } else if input.expires_at <= now {
        "expires_at must be in the future"
    } else if input.auto_release_after.is_some_and(|at| at <= now || at > input.expires_at) {
        "auto_release_after must be in the future and no later than expires_at"
    } else {
        return None;
    };
    Some(GuardRejection::never(VALIDATION, message))
}

/// Static payload check shared by every escrow / dispute transition.
#[must_use]
pub fn escrow_id_guard(escrow_id: &str) -> Option<GuardRejection> {
    escrow_id.trim().is_empty().then(|| GuardRejection::never(VALIDATION, "escrow_id is required"))
}

/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for escrows.
pub const ESCROW_UNVERSIONED: &str = "A2A escrows do not expose an aggregate version";

/// Evaluate funding. `None` means the escrow does not exist in scope. On
/// success the returned escrow is the post-funding projection.
#[must_use]
pub fn plan_fund_escrow(escrow: Option<A2AEscrow>, now: DateTime<Utc>) -> PlanOutcome<A2AEscrow> {
    let Some(mut escrow) = escrow else {
        return PlanOutcome::reject(GuardRejection::never(
            "commerce.a2a.escrow_not_found",
            "A2A escrow does not exist",
        ));
    };
    if escrow.status != A2AEscrowStatus::Created || escrow.expires_at <= now {
        return PlanOutcome::Reject {
            rejection: GuardRejection::never(
                "commerce.a2a.escrow_not_fundable",
                format!("cannot fund escrow in {} status or after expiry", escrow.status),
            ),
            version_before: None,
            aggregate_id: Some(escrow.id),
        };
    }
    escrow.status = A2AEscrowStatus::Active;
    escrow.funded_at = Some(now);
    escrow.updated_at = now;
    PlanOutcome::Proceed(escrow)
}

const DISPUTE_VALIDATION: &str = "commerce.a2a.dispute.validation_failed";

/// Static payload checks for `a2a.escrow.dispute`.
#[must_use]
pub fn dispute_escrow_guard(input: &DisputeA2AEscrow) -> Option<GuardRejection> {
    escrow_id_guard(&input.escrow_id).or_else(|| {
        input
            .reason
            .trim()
            .is_empty()
            .then(|| GuardRejection::never(VALIDATION, "dispute reason is required"))
    })
}

/// Static payload checks for `a2a.escrow.release` and `a2a.escrow.refund`.
#[must_use]
pub fn escrow_settlement_guard(escrow_id: &str) -> Option<GuardRejection> {
    escrow_id_guard(escrow_id)
}

/// Static payload checks for `a2a.dispute.file`. `controls_claimant` is the
/// backend-independent answer to "does the principal (or its delegator)
/// control the claimant address?".
#[must_use]
pub fn file_dispute_guard(
    input: &FileA2ADispute,
    now: DateTime<Utc>,
    controls_claimant: bool,
) -> Option<GuardRejection> {
    if let Some(rejection) = escrow_id_guard(&input.escrow_id) {
        return Some(rejection);
    }
    if input.reason.trim().is_empty()
        || input.category.trim().is_empty()
        || input.claimant_address.trim().is_empty()
    {
        return Some(GuardRejection::never(
            DISPUTE_VALIDATION,
            "claimant_address, reason, and category are required",
        ));
    }
    if input.evidence_deadline <= now || input.review_deadline <= input.evidence_deadline {
        return Some(GuardRejection::never(
            "commerce.a2a.dispute.invalid_deadlines",
            "evidence_deadline must be in the future and precede review_deadline",
        ));
    }
    (!controls_claimant).then(|| {
        GuardRejection::never(
            "kernel.actor_mismatch",
            "principal or delegator must control the claimant address",
        )
    })
}

/// Static payload checks for `a2a.dispute.evidence.submit`.
#[must_use]
pub fn submit_evidence_guard(
    input: &SubmitA2ADisputeEvidence,
    controls_submitter: bool,
) -> Option<GuardRejection> {
    if let Some(rejection) = escrow_id_guard(&input.dispute_id) {
        return Some(rejection);
    }
    if input.submitted_by.trim().is_empty()
        || input.evidence_type.trim().is_empty()
        || input.title.trim().is_empty()
        || input.content.is_empty()
    {
        return Some(GuardRejection::never(
            "commerce.a2a.dispute.evidence.validation_failed",
            "submitted_by, evidence_type, title, and content are required",
        ));
    }
    if input.title.len() > 256 || input.content.len() > 1_048_576 {
        return Some(GuardRejection::never(
            "commerce.a2a.dispute.evidence.too_large",
            "evidence title is limited to 256 bytes and content to 1 MiB",
        ));
    }
    (!controls_submitter).then(|| {
        GuardRejection::never(
            "kernel.actor_mismatch",
            "principal or delegator must control the evidence submitter address",
        )
    })
}

/// Static payload checks for `a2a.dispute.resolve`.
#[must_use]
pub fn resolve_dispute_guard(input: &ResolveA2ADispute) -> Option<GuardRejection> {
    if let Some(rejection) = escrow_id_guard(&input.dispute_id) {
        return Some(rejection);
    }
    if input.note.as_ref().is_some_and(|note| note.len() > 2_000) {
        return Some(GuardRejection::never(
            "commerce.a2a.dispute.resolution_note_too_large",
            "resolution note is limited to 2000 bytes",
        ));
    }
    let is_split = input.resolution_type == A2ADisputeResolutionType::Split;
    (is_split != (input.buyer_amount.is_some() && input.seller_amount.is_some())).then(|| {
        GuardRejection::never(
            "commerce.a2a.dispute.invalid_allocations",
            "split requires both exact allocations; other outcomes forbid allocations",
        )
    })
}
