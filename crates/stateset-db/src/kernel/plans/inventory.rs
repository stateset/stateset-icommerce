//! `inventory.reserve` and reservation-lifecycle plans.

use crate::kernel::envelope::GuardRejection;
use rust_decimal::Decimal;
use stateset_core::ReserveInventory;
use uuid::Uuid;

const VALIDATION: &str = "commerce.inventory_validation_failed";

/// Static payload checks for `inventory.reserve`.
#[must_use]
pub fn reserve_inventory_guard(input: &ReserveInventory) -> Option<GuardRejection> {
    let message = if input.sku.trim().is_empty()
        || input.reference_type.trim().is_empty()
        || input.reference_id.trim().is_empty()
    {
        "sku, reference_type, and reference_id are required".to_string()
    } else if input.expires_in_seconds.is_some_and(|seconds| seconds <= 0) {
        "expires_in_seconds must be greater than zero".to_string()
    } else if let Err(error) = stateset_core::validate_quantity(input.quantity) {
        error.to_string()
    } else {
        return None;
    };
    Some(GuardRejection::never(VALIDATION, message))
}

/// Static payload checks shared by `inventory.reservation.confirm` and
/// `inventory.reservation.release`. `confirm_quantity` is the optional partial
/// confirmation amount; `None` means release, or confirm-in-full.
#[must_use]
pub fn reservation_lifecycle_guard(
    reservation_id: Uuid,
    confirm_quantity: Option<Decimal>,
) -> Option<GuardRejection> {
    let message = if reservation_id.is_nil() {
        "reservation_id must not be nil"
    } else if confirm_quantity.is_some_and(|quantity| quantity <= Decimal::ZERO) {
        "confirmation quantity must be greater than zero"
    } else {
        return None;
    };
    Some(GuardRejection::never(VALIDATION, message))
}
