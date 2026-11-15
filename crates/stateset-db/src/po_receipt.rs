//! Purchase-order receipt document validation shared by database backends.
use rust_decimal::Decimal;
use stateset_core::{CommerceError, CreateReceipt, ReceivePurchaseOrderItems, Result};
use std::collections::HashSet;

pub(crate) fn validate(input: &CreateReceipt, outstanding: &CreateReceipt) -> Result<()> {
    if input.supplier_id != outstanding.supplier_id || input.items.is_empty() {
        return Err(CommerceError::ValidationError(
            "PO receipt must have the PO supplier and at least one line".into(),
        ));
    }
    let mut seen = HashSet::new();
    for item in &input.items {
        let line = outstanding
            .items
            .iter()
            .find(|line| line.po_line_id == item.po_line_id && item.po_line_id.is_some())
            .ok_or_else(|| {
                CommerceError::ValidationError(
                    "Receipt line must reference an outstanding line of its purchase order".into(),
                )
            })?;
        if !seen.insert(item.po_line_id)
            || item.sku != line.sku
            || item.expected_quantity <= Decimal::ZERO
            || item.expected_quantity > line.expected_quantity
        {
            return Err(CommerceError::ValidationError("Receipt SKU and positive quantity must match the outstanding PO line; duplicate lines are not allowed".into()));
        }
    }
    Ok(())
}

pub(crate) fn select(
    mut receipt: CreateReceipt,
    input: &ReceivePurchaseOrderItems,
) -> Result<CreateReceipt> {
    if input.warehouse_id <= 0 || input.receiving_location_id <= 0 {
        return Err(CommerceError::ValidationError(
            "An explicit warehouse and receiving location are required".into(),
        ));
    }
    let mut selected = Vec::with_capacity(input.items.len());
    for item in &input.items {
        let mut line = receipt
            .items
            .iter()
            .find(|line| line.po_line_id == Some(item.item_id))
            .cloned()
            .ok_or(CommerceError::NotFound)?;
        line.expected_quantity = item.quantity_received;
        line.notes = item.notes.clone();
        selected.push(line);
    }
    let mut result = receipt.clone();
    result.items = selected;
    validate(&result, &receipt)?;
    receipt.items = result.items;
    receipt.notes = input.notes.clone();
    Ok(receipt)
}
