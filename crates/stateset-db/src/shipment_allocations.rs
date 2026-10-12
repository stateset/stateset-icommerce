//! Manifest allocation rules shared by both backends. The caller locks the order.
use stateset_core::{CommerceError, CreateShipmentItem, ProductId, Result};
use std::collections::BTreeMap;
use uuid::Uuid;

pub(crate) struct Line {
    pub id: Uuid,
    pub product_id: ProductId,
    pub sku: String,
    pub quantity: i32,
}

pub(crate) struct Assignment {
    pub order_item_id: Option<Uuid>,
    pub product_id: Option<ProductId>,
    pub sku: String,
    pub quantity: i32,
}

/// Refuse shipment manifests (a new shipment, or a new line on one) for an
/// order that is closed to fulfilment: every unit already shipped
/// (`shipped`, `delivered`) or the order is `cancelled`/`refunded`. A fully
/// shipped order keeps no package waiting to leave.
pub(crate) fn validate_order_status(status: &str) -> Result<()> {
    let status = status
        .parse::<stateset_core::OrderStatus>()
        .map_err(|error| CommerceError::DatabaseError(format!("Invalid order status: {error}")))?;
    if matches!(
        status,
        stateset_core::OrderStatus::Shipped
            | stateset_core::OrderStatus::Delivered
            | stateset_core::OrderStatus::Cancelled
            | stateset_core::OrderStatus::Refunded
    ) {
        return Err(CommerceError::ValidationError(format!(
            "Cannot create or add shipment items for a {status} order"
        )));
    }
    Ok(())
}

fn resolve<'a>(
    lines: &'a [Line],
    id: Option<Uuid>,
    product: Option<ProductId>,
    sku: &str,
) -> Result<&'a Line> {
    let mut matches = lines.iter().filter(|line| id.map_or(line.sku == sku, |id| line.id == id));
    let line = matches.next().ok_or_else(|| {
        CommerceError::ValidationError("Shipment item does not belong to this order".into())
    })?;
    if matches.next().is_some() {
        return Err(CommerceError::ValidationError(
            "Ambiguous shipment SKU; supply an order_item_id or reconcile legacy items".into(),
        ));
    }
    if line.sku != sku || product.is_some_and(|id| id != line.product_id) {
        return Err(CommerceError::ValidationError(
            "Shipment SKU or product does not match its order item".into(),
        ));
    }
    if line.quantity <= 0 {
        return Err(CommerceError::ValidationError(
            "Invalid ordered quantity; reconcile first".into(),
        ));
    }
    Ok(line)
}

fn claim(totals: &mut BTreeMap<Uuid, i64>, line: &Line, quantity: i32) -> Result<()> {
    if quantity <= 0 {
        return Err(CommerceError::ValidationError(
            "Invalid existing shipment quantity; reconcile first".into(),
        ));
    }
    let total = totals.entry(line.id).or_default();
    *total = total
        .checked_add(i64::from(quantity))
        .ok_or_else(|| CommerceError::ValidationError("Shipment allocation overflow".into()))?;
    if *total > i64::from(line.quantity) {
        return Err(CommerceError::ValidationError(format!(
            "Shipment assignments exceed ordered quantity for item {}",
            line.id
        )));
    }
    Ok(())
}

/// Cancelled manifests are excluded by the caller. Fulfilled quantities are a
/// separate ledger and are not subtracted again from this manifest budget.
pub(crate) fn normalize(
    lines: &[Line],
    existing: &[Assignment],
    inputs: &[CreateShipmentItem],
) -> Result<Vec<CreateShipmentItem>> {
    let mut totals = BTreeMap::new();
    for assignment in existing {
        let line =
            resolve(lines, assignment.order_item_id, assignment.product_id, &assignment.sku)?;
        claim(&mut totals, line, assignment.quantity)?;
    }
    let mut normalized = Vec::with_capacity(inputs.len());
    for input in inputs {
        crate::shipment_updates::validate_item(input)?;
        let line = resolve(lines, input.order_item_id, input.product_id, &input.sku)?;
        claim(&mut totals, line, input.quantity)?;
        normalized.push(CreateShipmentItem {
            order_item_id: Some(line.id),
            product_id: Some(line.product_id),
            sku: input.sku.clone(),
            name: input.name.clone(),
            quantity: input.quantity,
        });
    }
    Ok(normalized)
}
