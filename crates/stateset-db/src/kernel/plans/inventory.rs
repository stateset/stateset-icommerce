//! `inventory.reserve` and reservation-lifecycle plans.

use crate::kernel::envelope::GuardRejection;
use rust_decimal::Decimal;
use stateset_core::{EconomicCommitment, ReserveInventory};
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

/// Bind the quantity declared at the authorization boundary to the quantity
/// the inventory executor will actually reserve.
#[must_use]
pub fn economic_quantity_guard(
    commitment: Option<&EconomicCommitment>,
    observed_quantity: Decimal,
) -> Option<GuardRejection> {
    let declared = commitment.and_then(|commitment| commitment.quantity.as_deref())?;
    match declared.parse::<Decimal>() {
        Ok(quantity) if quantity == observed_quantity => None,
        Ok(_) => Some(GuardRejection::never(
            "kernel.commitment_quantity_mismatch",
            "declared economic quantity does not match the domain quantity",
        )),
        Err(_) => Some(GuardRejection::never(
            "kernel.commitment_quantity_invalid",
            "declared economic quantity is not an exact decimal",
        )),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_commitments_bind_exactly() {
        let commitment = EconomicCommitment {
            budget_id: None,
            amount: None,
            asset_amount: None,
            counterparty_id: None,
            quantity: Some("50.00".into()),
            evidence: vec![],
        };
        assert!(economic_quantity_guard(Some(&commitment), Decimal::new(5_000, 2)).is_none());
        assert_eq!(
            economic_quantity_guard(Some(&commitment), Decimal::new(5_001, 2))
                .map(|guard| guard.code),
            Some("kernel.commitment_quantity_mismatch")
        );
    }
}
