//! `returns.transition` plan.

use crate::kernel::envelope::GuardRejection;
use stateset_core::{CommerceError, Return, ReturnStatus, TransitionReturn};

/// Static payload checks for `returns.transition`.
#[must_use]
pub fn transition_return_guard(input: &TransitionReturn) -> Option<GuardRejection> {
    input.return_id.into_uuid().is_nil().then(|| {
        GuardRejection::never("commerce.return_validation_failed", "return_id must not be nil")
    })
}

/// Stateful checks for `returns.transition`, applied after the state-machine
/// edge check: the rules the repository's status writes (`apply_update_*`)
/// enforce, so a governed transition refuses exactly what the repository
/// refuses — and the refusal is a sealed receipt, also under preview.
///
/// * a terminal return is immutable (`commerce.return_terminal`);
/// * rejecting or cancelling after any item was dispositioned
///   (`commerce.return_items_dispositioned`);
/// * completing while an item has no disposition
///   (`commerce.return_items_undispositioned`) — the governed command has no
///   write-off option, exactly like the repository's `complete`.
#[must_use]
pub fn transition_return_state_guard(
    current: &Return,
    next: ReturnStatus,
) -> Option<GuardRejection> {
    if current.status.is_terminal() {
        return Some(GuardRejection::never(
            "commerce.return_terminal",
            format!("Return {} is {}; terminal returns are immutable", current.id, current.status),
        ));
    }
    current.check_transition(next, false).err().map(|error| {
        let code = match error {
            CommerceError::NotPermitted(_) => "commerce.return_items_undispositioned",
            CommerceError::Conflict(_) => "commerce.return_items_dispositioned",
            _ => "commerce.invalid_return_status_transition",
        };
        GuardRejection::never(code, error.to_string())
    })
}
