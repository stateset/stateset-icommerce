//! `inventory.item.create` and `products.create` plans.

use crate::kernel::envelope::GuardRejection;
use stateset_core::{CreateInventoryItem, CreateProduct, Validate};

const VALIDATION: &str = "commerce.validation_failed";

/// Static payload checks for `inventory.item.create`.
#[must_use]
pub fn create_inventory_item_guard(input: &CreateInventoryItem) -> Option<GuardRejection> {
    input.validate().err().map(|error| GuardRejection::never(VALIDATION, error.to_string()))
}

/// Static payload checks for `products.create`.
#[must_use]
pub fn create_product_guard(input: &CreateProduct) -> Option<GuardRejection> {
    input.validate().err().map(|error| GuardRejection::never(VALIDATION, error.to_string()))
}
