//! `returns.transition` and `returns.item.disposition.set` plans.

use crate::kernel::envelope::GuardRejection;
use stateset_core::{
    CommerceError, Return, ReturnStatus, SetReturnItemDisposition, TransitionReturn,
};
use uuid::Uuid;

/// Message used with [`crate::kernel::EnvelopeGuard::unversioned`] for item
/// dispositions.
pub const RETURN_DISPOSITION_UNVERSIONED: &str =
    "return item dispositions are guarded by the return state machine, not an expected version";

/// Longest note a governed disposition may carry.
pub const DISPOSITION_NOTE_MAX_CHARS: usize = 1000;

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

/// Static payload checks for `returns.item.disposition.set`: both ids are
/// present, the disposition is in the `return_items.disposition` vocabulary
/// (`commerce.return_disposition_invalid`), and the note is bounded.
#[must_use]
pub fn set_return_item_disposition_guard(
    input: &SetReturnItemDisposition,
) -> Option<GuardRejection> {
    if input.return_id.into_uuid().is_nil() {
        return Some(GuardRejection::never(
            "commerce.return_validation_failed",
            "return_id must not be nil",
        ));
    }
    if input.return_item_id.is_nil() {
        return Some(GuardRejection::never(
            "commerce.return_validation_failed",
            "return_item_id must not be nil",
        ));
    }
    if input.parsed_disposition().is_none() {
        return Some(GuardRejection::never(
            "commerce.return_disposition_invalid",
            format!(
                "unknown return disposition '{}': expected restock, refurbish, scrap, \
                 return_to_vendor or quarantine",
                input.disposition
            ),
        ));
    }
    input.note.as_deref().filter(|note| note.chars().count() > DISPOSITION_NOTE_MAX_CHARS).map(
        |_| {
            GuardRejection::never(
                "commerce.return_validation_failed",
                format!("note must be at most {DISPOSITION_NOTE_MAX_CHARS} characters"),
            )
        },
    )
}

/// Stateful checks for `returns.item.disposition.set` on the locked return:
/// the rules the repository's `set_item_disposition` enforces, as typed
/// rejections sealed before preview or apply.
///
/// * the return must be `received` or `inspecting`
///   (`commerce.return_not_dispositionable`);
/// * the item must belong to the return (`commerce.return_item_not_found`);
/// * an item is dispositioned once
///   (`commerce.return_item_already_dispositioned`).
#[must_use]
pub fn set_return_item_disposition_state_guard(
    current: &Return,
    return_item_id: Uuid,
) -> Option<GuardRejection> {
    if !matches!(current.status, ReturnStatus::Received | ReturnStatus::Inspecting) {
        return Some(GuardRejection::never(
            "commerce.return_not_dispositionable",
            format!(
                "Return items can only be dispositioned once received (status: {})",
                current.status
            ),
        ));
    }
    let Some(item) = current.items.iter().find(|item| item.id == return_item_id) else {
        return Some(GuardRejection::never(
            "commerce.return_item_not_found",
            format!("Return item {return_item_id} not found on return {}", current.id),
        ));
    };
    item.disposition.map(|existing| {
        GuardRejection::never(
            "commerce.return_item_already_dispositioned",
            format!("Return item {return_item_id} already dispositioned as {existing}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use stateset_core::{
        CustomerId, ItemCondition, OrderId, OrderItemId, ReturnDisposition, ReturnId, ReturnItem,
        ReturnReason,
    };

    fn item(return_id: ReturnId, disposition: Option<ReturnDisposition>) -> ReturnItem {
        ReturnItem {
            id: Uuid::new_v4(),
            return_id,
            order_item_id: OrderItemId::new(),
            sku: "W-1".into(),
            name: "Widget".into(),
            quantity: 1,
            condition: ItemCondition::Opened,
            refund_amount: Decimal::new(1000, 2),
            disposition,
            disposition_at: None,
            disposition_by: None,
            lot_id: None,
            serial_ids: Vec::new(),
        }
    }

    fn received(items: Vec<ReturnItem>, id: ReturnId) -> Return {
        Return {
            id,
            order_id: OrderId::new(),
            customer_id: CustomerId::new(),
            status: ReturnStatus::Received,
            reason: ReturnReason::Defective,
            reason_details: None,
            idempotency_key: None,
            refund_amount: None,
            refund_method: None,
            tracking_number: None,
            items,
            notes: None,
            version: 3,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn disposition_guard_refuses_nil_ids_unknown_dispositions_and_long_notes() {
        let return_id = ReturnId::new();
        let mut input =
            SetReturnItemDisposition::new(return_id, Uuid::new_v4(), ReturnDisposition::Scrap);
        assert_eq!(set_return_item_disposition_guard(&input), None);
        input.note = Some("x".repeat(DISPOSITION_NOTE_MAX_CHARS));
        assert_eq!(set_return_item_disposition_guard(&input), None);
        input.note = Some("x".repeat(DISPOSITION_NOTE_MAX_CHARS + 1));
        assert_eq!(
            set_return_item_disposition_guard(&input).map(|r| r.code),
            Some("commerce.return_validation_failed")
        );
        input.note = None;
        input.disposition = "donate".into();
        assert_eq!(
            set_return_item_disposition_guard(&input).map(|r| r.code),
            Some("commerce.return_disposition_invalid")
        );
        input.disposition = "restock".into();
        input.return_item_id = Uuid::nil();
        assert_eq!(
            set_return_item_disposition_guard(&input).map(|r| r.code),
            Some("commerce.return_validation_failed")
        );
        input.return_item_id = Uuid::new_v4();
        input.return_id = ReturnId::from(Uuid::nil());
        assert_eq!(
            set_return_item_disposition_guard(&input).map(|r| r.code),
            Some("commerce.return_validation_failed")
        );
    }

    #[test]
    fn disposition_state_guard_mirrors_the_repository_rules() {
        let id = ReturnId::new();
        let open = item(id, None);
        let done = item(id, Some(ReturnDisposition::Restock));
        let mut ret = received(vec![open.clone(), done.clone()], id);
        assert_eq!(set_return_item_disposition_state_guard(&ret, open.id), None);
        assert_eq!(
            set_return_item_disposition_state_guard(&ret, done.id).map(|r| r.code),
            Some("commerce.return_item_already_dispositioned")
        );
        assert_eq!(
            set_return_item_disposition_state_guard(&ret, Uuid::new_v4()).map(|r| r.code),
            Some("commerce.return_item_not_found")
        );
        ret.status = ReturnStatus::Inspecting;
        assert_eq!(set_return_item_disposition_state_guard(&ret, open.id), None);
        for status in [
            ReturnStatus::Requested,
            ReturnStatus::Approved,
            ReturnStatus::InTransit,
            ReturnStatus::Completed,
            ReturnStatus::Rejected,
            ReturnStatus::Cancelled,
        ] {
            ret.status = status;
            assert_eq!(
                set_return_item_disposition_state_guard(&ret, open.id).map(|r| r.code),
                Some("commerce.return_not_dispositionable"),
                "{status}"
            );
        }
    }
}
