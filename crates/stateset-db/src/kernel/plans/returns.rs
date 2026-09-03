//! `returns.transition` plan.

use crate::kernel::envelope::GuardRejection;
use stateset_core::TransitionReturn;

/// Static payload checks for `returns.transition`.
#[must_use]
pub fn transition_return_guard(input: &TransitionReturn) -> Option<GuardRejection> {
    input.return_id.into_uuid().is_nil().then(|| {
        GuardRejection::never("commerce.return_validation_failed", "return_id must not be nil")
    })
}
