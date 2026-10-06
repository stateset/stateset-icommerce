//! Shared shipment validation and mutation semantics.
use chrono::{DateTime, SubsecRound, Utc};
use stateset_core::{
    AddShipmentEvent, CommerceError, CreateShipment, CreateShipmentItem, Result, Shipment,
    ShipmentEvent, ShipmentItem, ShipmentStatus, UpdateShipment,
};

/// Prepare an append with the parent locked. Carrier observations may arrive after
/// cancellation or delivery; recording history never infers a lifecycle transition.
pub(crate) fn prepare_event(
    shipment: &mut Shipment,
    input: AddShipmentEvent,
    now: DateTime<Utc>,
) -> Result<ShipmentEvent> {
    if input.event_type.trim().is_empty() {
        return Err(CommerceError::ValidationError(
            "Shipment tracking event type must not be empty".into(),
        ));
    }
    for (field, value, limit) in [
        ("event_type", Some(input.event_type.as_str()), Some(100)),
        ("location", input.location.as_deref(), Some(255)),
        ("description", input.description.as_deref(), None),
    ] {
        if let Some(value) = value {
            if value.contains('\0') {
                return Err(CommerceError::ValidationError(format!(
                    "Shipment tracking {field} must not contain NUL characters"
                )));
            }
            if let Some(limit) = limit {
                if value.chars().count() > limit {
                    return Err(CommerceError::ValidationError(format!(
                        "Shipment tracking {field} must not exceed {limit} characters"
                    )));
                }
            }
        }
    }
    let version = shipment
        .version
        .checked_add(1)
        .ok_or_else(|| CommerceError::ValidationError("Shipment version exhausted".into()))?;
    // PostgreSQL stores microseconds. Use the same precision for returned values,
    // persisted rows, and outbox payloads on both backends.
    let now = now.trunc_subsecs(6);
    let event = ShipmentEvent {
        id: uuid::Uuid::new_v4(),
        shipment_id: shipment.id,
        event_type: input.event_type,
        location: input.location,
        description: input.description,
        event_time: input.event_time.unwrap_or(now).trunc_subsecs(6),
        created_at: now,
    };
    shipment.version = version;
    shipment.updated_at = now;
    Ok(event)
}

pub(crate) fn event_fact(shipment: &Shipment, event: &ShipmentEvent) -> serde_json::Value {
    serde_json::json!({
        "id": shipment.id, "order_id": shipment.order_id, "status": shipment.status,
        "previous_version": shipment.version - 1, "version": shipment.version,
        "changed_fields": ["events"], "event": event,
    })
}

pub(crate) fn validate_item(item: &CreateShipmentItem) -> Result<()> {
    if item.quantity <= 0 || item.sku.trim().is_empty() || item.name.trim().is_empty() {
        return Err(CommerceError::ValidationError(
            "Shipment items require a positive quantity and non-empty SKU and name".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_create_items(input: &CreateShipment) -> Result<()> {
    for item in input.items.iter().flatten() {
        validate_item(item)?;
    }
    Ok(())
}

/// Refuse an item edit unless the shipment's manifest is still editable
/// ([`ShipmentStatus::allows_item_changes`]). Call with the parent locked in
/// the same transaction as the edit, *before* any item row is written, so the
/// status that is checked is the status the edit commits against.
pub(crate) fn ensure_items_editable(shipment: &Shipment) -> Result<()> {
    if shipment.status.allows_item_changes() {
        return Ok(());
    }
    Err(CommerceError::ValidationError(format!(
        "Cannot change items for shipment in {} status",
        shipment.status
    )))
}

/// Call with the parent locked in a transaction. Contents freeze once ready to ship,
/// including after cancellation.
pub(crate) fn change_contents(shipment: &mut Shipment, now: DateTime<Utc>) -> Result<()> {
    ensure_items_editable(shipment)?;
    shipment.version = shipment
        .version
        .checked_add(1)
        .ok_or_else(|| CommerceError::ValidationError("Shipment version exhausted".into()))?;
    shipment.updated_at = now;
    Ok(())
}

pub(crate) fn item_fact(shipment: &Shipment, item: &ShipmentItem) -> serde_json::Value {
    serde_json::json!({
        "id": shipment.id, "order_id": shipment.order_id, "status": shipment.status,
        "previous_version": shipment.version - 1, "version": shipment.version,
        "changed_fields": ["items"], "item": item,
    })
}

pub(crate) fn apply(
    shipment: &mut Shipment,
    input: UpdateShipment,
    now: DateTime<Utc>,
) -> Result<Vec<&'static str>> {
    check_version(shipment, input.expected_version)?;
    if let Some(status) = input.status {
        if status != shipment.status && !shipment.status.can_transition_to(status) {
            return Err(CommerceError::ValidationError(format!(
                "Invalid shipment status transition from {} to {}",
                shipment.status, status
            )));
        }
    }
    for (field, amount) in [("shipping_cost", input.shipping_cost), ("weight_kg", input.weight_kg)]
    {
        if amount.is_some_and(|value| value.is_sign_negative()) {
            return Err(CommerceError::ValidationError(format!(
                "Shipment {field} must be non-negative"
            )));
        }
    }
    let mut changed = Vec::new();
    macro_rules! required {
        ($($field:ident),+ $(,)?) => { $(
            if let Some(value) = input.$field {
                if value != shipment.$field {
                    shipment.$field = value;
                    changed.push(stringify!($field));
                }
            }
        )+ };
    }
    macro_rules! optional {
        ($($field:ident),+ $(,)?) => { $(
            if let Some(value) = input.$field {
                if shipment.$field.as_ref() != Some(&value) {
                    shipment.$field = Some(value);
                    changed.push(stringify!($field));
                }
            }
        )+ };
    }
    required!(status, carrier, recipient_name, shipping_address);
    optional!(
        tracking_number,
        recipient_email,
        recipient_phone,
        weight_kg,
        dimensions,
        shipping_cost,
        estimated_delivery,
        notes
    );
    if changed.is_empty() {
        return Ok(changed);
    }
    shipment.tracking_url =
        shipment.tracking_number.as_ref().and_then(|number| shipment.carrier.tracking_url(number));
    if changed.contains(&"status") {
        if shipment.status == ShipmentStatus::Shipped && shipment.shipped_at.is_none() {
            shipment.shipped_at = Some(now);
        }
        if shipment.status == ShipmentStatus::Delivered && shipment.delivered_at.is_none() {
            shipment.delivered_at = Some(now);
        }
    }
    shipment.version = shipment
        .version
        .checked_add(1)
        .ok_or_else(|| CommerceError::ValidationError("Shipment version exhausted".into()))?;
    shipment.updated_at = now;
    Ok(changed)
}

pub(crate) fn fact(
    shipment: &Shipment,
    previous_status: ShipmentStatus,
    changed: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "id": shipment.id, "order_id": shipment.order_id,
        "previous_status": previous_status, "status": shipment.status,
        "version": shipment.version, "changed_fields": changed,
        "carrier": shipment.carrier, "tracking_number": shipment.tracking_number,
        "shipping_cost": shipment.shipping_cost.map(|amount| amount.to_string()),
        "shipped_at": shipment.shipped_at, "delivered_at": shipment.delivered_at,
    })
}

/// Check a caller's precondition while holding the parent mutation lock.
pub(crate) fn check_version(shipment: &Shipment, expected_version: Option<i32>) -> Result<()> {
    if let Some(expected_version) = expected_version {
        if expected_version != shipment.version {
            return Err(CommerceError::VersionConflict {
                entity: "shipment".into(),
                id: shipment.id.to_string(),
                expected_version,
            });
        }
    }
    Ok(())
}
