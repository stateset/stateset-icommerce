//! `orders.transition` and `orders.ship` plans.

use super::PlanOutcome;
use crate::kernel::envelope::GuardRejection;
use rust_decimal::Decimal;
use stateset_core::{
    CommandEnvelope, DIRECT_PAYMENT_STATUS_REFUSED, Order, OrderStatus, Payment, PaymentStatus,
    ShipOrderCommand, TransitionOrder,
};
use uuid::Uuid;

/// Static payload checks for `orders.transition`.
#[must_use]
pub fn transition_order_guard(payload: &TransitionOrder) -> Option<GuardRejection> {
    if payload.order_id.into_uuid().is_nil() {
        return Some(GuardRejection::never(
            "commerce.order_validation_failed",
            "order_id must not be nil",
        ));
    }
    if matches!(payload.status, OrderStatus::Shipped | OrderStatus::PartiallyShipped) {
        return Some(GuardRejection::never(
            "commerce.shipment_command_required",
            "shipment transitions must use orders.ship",
        ));
    }
    // An order's payment status is derived from its payment ledger, never
    // declared (see `UpdateOrder::payment_status`).
    if payload.payment_status.is_some() {
        return Some(GuardRejection::never(
            "commerce.payment_status_derived",
            DIRECT_PAYMENT_STATUS_REFUSED,
        ));
    }
    None
}

/// Static payload checks for `orders.ship`.
#[must_use]
pub fn ship_order_guard(payload: &ShipOrderCommand) -> Option<GuardRejection> {
    payload.order_id.into_uuid().is_nil().then(|| {
        GuardRejection::never("commerce.order_validation_failed", "order_id must not be nil")
    })
}

/// What the backend loads (under its row lock) for an order transition.
#[derive(Debug, Clone)]
pub struct OrderTransitionSnapshot {
    /// The order with its items.
    pub order: Order,
    /// Payments that still hold captured money against the order. Only
    /// consulted for cancellations; other transitions may pass an empty list.
    pub open_captures: Vec<Payment>,
}

/// Effects of an accepted order transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderTransitionEffects {
    pub status_before: OrderStatus,
    pub payment_status_before: PaymentStatus,
    pub next_status: OrderStatus,
    pub next_payment_status: PaymentStatus,
    pub version_before: i32,
    /// Release every inventory reservation held for the order and cancel its
    /// backorders (cancellation only).
    pub release_holds: bool,
    /// Void in-flight payments before cancelling (`void_payments = true`).
    pub void_in_flight_payments: bool,
    /// Payment ids that still hold captured money after a forced cancel;
    /// they leave through a refund and are reported on the event.
    pub outstanding_capture_ids: Vec<Uuid>,
}

/// Evaluate an order transition. `None` means the order does not exist.
#[must_use]
pub fn plan_order_transition(
    command: &CommandEnvelope<TransitionOrder>,
    snapshot: Option<&OrderTransitionSnapshot>,
) -> PlanOutcome<OrderTransitionEffects> {
    let Some(snapshot) = snapshot else {
        return PlanOutcome::reject(GuardRejection::never(
            "commerce.order_not_found",
            "order does not exist",
        ));
    };
    let order = &snapshot.order;
    let version_before = order.version;
    let reject = |rejection: GuardRejection| PlanOutcome::Reject {
        rejection,
        version_before: Some(version_before),
        aggregate_id: Some(order.id.to_string()),
    };
    if command.expected_version.is_some_and(|expected| expected != version_before) {
        return reject(GuardRejection::after_conflict(
            "kernel.version_conflict",
            "order version does not match expected_version",
        ));
    }
    let next_status = command.payload.status;
    if !order.status.can_transition_to(next_status) {
        return reject(GuardRejection::never(
            "commerce.invalid_order_status_transition",
            format!("order cannot transition from {} to {}", order.status, next_status),
        ));
    }
    // The payment status is derived, never declared (a declaring command is
    // refused by `transition_order_guard`); the transition itself keeps it.
    let next_payment_status = order.payment_status;
    // Refundability is judged on the payment status the order already has.
    if next_status == OrderStatus::Refunded
        && !matches!(
            order.payment_status,
            PaymentStatus::Paid
                | PaymentStatus::PartiallyPaid
                | PaymentStatus::Refunded
                | PaymentStatus::PartiallyRefunded
        )
    {
        return reject(GuardRejection::never(
            "commerce.order_not_refundable",
            "order payment status is not refundable",
        ));
    }
    let cancelling = next_status == OrderStatus::Cancelled;
    let void_payments = command.payload.void_payments;
    let mut outstanding_capture_ids = Vec::new();
    if cancelling && !snapshot.open_captures.is_empty() {
        if !void_payments {
            let outstanding: Decimal =
                snapshot.open_captures.iter().map(|p| p.amount - p.amount_refunded).sum();
            let currency = snapshot.open_captures[0].currency;
            return reject(GuardRejection::never(
                "commerce.order.captured_money_outstanding",
                format!(
                    "order {} cannot be cancelled: {} payment(s) still hold {outstanding} {currency}; \
                     refund them first, or cancel with void_payments = true to void in-flight \
                     payments and leave settled ones for refund",
                    order.id,
                    snapshot.open_captures.len()
                ),
            ));
        }
        outstanding_capture_ids = snapshot.open_captures.iter().map(|p| p.id.into_uuid()).collect();
    }
    PlanOutcome::Proceed(OrderTransitionEffects {
        status_before: order.status,
        payment_status_before: order.payment_status,
        next_status,
        next_payment_status,
        version_before,
        release_holds: cancelling,
        void_in_flight_payments: cancelling && void_payments,
        outstanding_capture_ids,
    })
}

/// What the backend loads for a shipment. `D` is the backend's line-delta
/// type produced by its shipment planner.
#[derive(Debug, Clone)]
pub struct ShipOrderSnapshot<D> {
    pub order: Order,
    /// Resolved target status and per-line deltas, or the planner's
    /// validation message.
    pub shipment: Result<(OrderStatus, Vec<D>), String>,
    /// Whether any open reservation for the order has already expired.
    pub expired_reservation: bool,
}

/// Effects of an accepted shipment.
#[derive(Debug, Clone)]
pub struct ShipOrderEffects<D> {
    pub status_before: OrderStatus,
    pub resolved_status: OrderStatus,
    pub deltas: Vec<D>,
    pub version_before: i32,
}

/// Evaluate a shipment. `None` means the order does not exist.
#[must_use]
pub fn plan_ship_order<D: Clone>(
    command: &CommandEnvelope<ShipOrderCommand>,
    snapshot: Option<&ShipOrderSnapshot<D>>,
) -> PlanOutcome<ShipOrderEffects<D>> {
    let Some(snapshot) = snapshot else {
        return PlanOutcome::reject(GuardRejection::never(
            "commerce.order_not_found",
            "order does not exist",
        ));
    };
    let order = &snapshot.order;
    let version_before = order.version;
    let reject = |rejection: GuardRejection| PlanOutcome::Reject {
        rejection,
        version_before: Some(version_before),
        aggregate_id: Some(order.id.to_string()),
    };
    if command.expected_version.is_some_and(|expected| expected != version_before) {
        return reject(GuardRejection::after_conflict(
            "kernel.version_conflict",
            "order version does not match expected_version",
        ));
    }
    let (resolved_status, deltas) = match &snapshot.shipment {
        Ok(plan) => plan.clone(),
        Err(message) => {
            return reject(GuardRejection::never("commerce.shipment_invalid", message.clone()));
        }
    };
    if !order.status.can_transition_to(resolved_status) {
        return reject(GuardRejection::never(
            "commerce.invalid_order_status_transition",
            format!("order cannot transition from {} to {}", order.status, resolved_status),
        ));
    }
    if snapshot.expired_reservation {
        return reject(GuardRejection::never(
            "commerce.reservation_expired",
            "an inventory reservation expired before shipment",
        ));
    }
    if command.command_type == "orders.ship_reserved"
        && (command.expected_version.is_none()
            || order.payment_status != PaymentStatus::Paid
            || !matches!(order.status, OrderStatus::Processing | OrderStatus::PartiallyShipped)
            || !matches!(resolved_status, OrderStatus::PartiallyShipped | OrderStatus::Shipped))
    {
        return reject(GuardRejection::never(
            "commerce.reserved_shipment_invalid",
            "reserved shipping requires an expected version and a paid processing or partially shipped order",
        ));
    }
    PlanOutcome::Proceed(ShipOrderEffects {
        status_before: order.status,
        resolved_status,
        deltas,
        version_before,
    })
}

/// Stock evidence read under the backend's shipment transaction locks.
#[derive(Debug)]
pub struct ReservedShipmentStock {
    pub id: Uuid,
    pub order_item_id: Option<String>,
    pub sku: String,
    pub quantity: Decimal,
    pub item_id: i64,
    pub location_id: i32,
    pub on_hand: Decimal,
    pub allocated: Decimal,
    pub available: Decimal,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The exact portion of a hold consumed by this dispatch.
#[derive(Debug)]
pub struct ReservedShipmentConsumption {
    pub id: Uuid,
    pub quantity: Decimal,
}

/// Validate remaining line coverage before selecting any stock. A prior native
/// receipt must account for all already-shipped units: legacy shipping only
/// confirms holds, and must never be treated as physical stock consumption.
/// Unkeyed legacy holds remain supported for wholly unshipped full orders only.
pub fn plan_reserved_stock(
    order: &Order,
    tracked_skus: &[String],
    reservations: &[ReservedShipmentStock],
    deltas: &[(String, i32)],
    previous: Option<&Order>,
) -> Result<Vec<ReservedShipmentConsumption>, GuardRejection> {
    use std::collections::BTreeMap;
    let reject = || {
        GuardRejection::never(
            "commerce.shipment_inventory_invalid",
            "open reservations must exactly cover remaining tracked order lines and backed inventory balances",
        )
    };
    if order.items.iter().any(|item| {
        item.quantity <= 0 || item.shipped_quantity < 0 || item.shipped_quantity > item.quantity
    }) || !deltas.iter().any(|(_, quantity)| *quantity > 0)
    {
        return Err(reject());
    }
    let already_shipped = order.items.iter().any(|item| item.shipped_quantity > 0);
    if already_shipped
        && !previous.is_some_and(|prior| {
            prior.id == order.id
                && prior.items.len() == order.items.len()
                && prior.items.iter().all(|item| {
                    order.items.iter().any(|current| {
                        current.id == item.id
                            && current.sku == item.sku
                            && current.quantity == item.quantity
                            && current.shipped_quantity == item.shipped_quantity
                    })
                })
        })
    {
        return Err(GuardRejection::never(
            "commerce.shipment_stock_provenance_missing",
            "previous shipped quantities must match a succeeded reserved-shipping receipt",
        ));
    }
    let mut demand = BTreeMap::<String, Decimal>::new();
    let mut sku_demand = BTreeMap::<String, Decimal>::new();
    let mut requested = BTreeMap::<String, Decimal>::new();
    for item in &order.items {
        let remaining = item.quantity - item.shipped_quantity;
        if tracked_skus.contains(&item.sku) && remaining > 0 {
            demand.insert(item.id.to_string(), Decimal::from(remaining));
            *sku_demand.entry(item.sku.clone()).or_default() += Decimal::from(remaining);
        }
    }
    for (id, quantity) in deltas {
        let Some(item) = order.items.iter().find(|item| item.id.to_string() == *id) else {
            return Err(reject());
        };
        if *quantity < 0 {
            return Err(reject());
        }
        let total = requested.entry(id.clone()).or_default();
        *total += Decimal::from(*quantity);
        if *total > Decimal::from(item.quantity - item.shipped_quantity) {
            return Err(reject());
        }
    }
    let legacy = reservations.iter().any(|row| row.order_item_id.is_none());
    if legacy
        && (already_shipped
            || reservations.iter().any(|row| row.order_item_id.is_some())
            || order.items.iter().any(|item| {
                requested.get(&item.id.to_string()).copied().unwrap_or_default()
                    != Decimal::from(item.quantity)
            }))
    {
        return Err(reject());
    }
    let mut held = BTreeMap::<String, Decimal>::new();
    let mut balances = BTreeMap::<(i64, i32), (Decimal, Decimal, Decimal)>::new();
    for row in reservations {
        if row.available != row.on_hand - row.allocated || row.quantity <= Decimal::ZERO {
            return Err(reject());
        }
        if row.expires_at.is_some_and(|expiry| expiry <= chrono::Utc::now()) {
            return Err(GuardRejection::never(
                "commerce.reservation_expired",
                "reservation expired before stock consumption",
            ));
        }
        let key = if let Some(id) = &row.order_item_id {
            if !order.items.iter().any(|item| item.id.to_string() == *id && item.sku == row.sku) {
                return Err(reject());
            }
            id
        } else {
            &row.sku
        };
        *held.entry(key.clone()).or_default() += row.quantity;
        let balance = balances.entry((row.item_id, row.location_id)).or_insert((
            Decimal::ZERO,
            row.on_hand,
            row.allocated,
        ));
        if balance.1 != row.on_hand || balance.2 != row.allocated {
            return Err(reject());
        }
        balance.0 += row.quantity;
    }
    if held != if legacy { sku_demand } else { demand }
        || balances.values().any(|(needed, on_hand, allocated)| {
            needed > on_hand || needed > allocated || allocated > on_hand
        })
    {
        return Err(reject());
    }
    let mut consumption = Vec::new();
    // Backends supply stable item/location/reservation ordering while holding
    // their locks, so splitting a line across holds is deterministic.
    for row in reservations {
        let quantity = if let Some(id) = &row.order_item_id {
            let remaining = requested.entry(id.clone()).or_default();
            let take = row.quantity.min(*remaining);
            *remaining -= take;
            take
        } else {
            row.quantity
        };
        if quantity > Decimal::ZERO {
            consumption.push(ReservedShipmentConsumption { id: row.id, quantity });
        }
    }
    Ok(consumption)
}

/// Exact units a planned shipment will move, summed over its line deltas.
///
/// Only positive deltas count: a plan that leaves a line untouched moves
/// nothing, and no shipment ever un-ships units. This is the figure a
/// declared `commitment.quantity` is bound against, so both backends must
/// derive it the same way.
#[must_use]
pub fn shipped_units(deltas: impl IntoIterator<Item = i32>) -> Decimal {
    Decimal::from(deltas.into_iter().filter(|delta| *delta > 0).map(i64::from).sum::<i64>())
}

/// The projection both backends use to observe the units a checkout moves.
///
/// `checkout.commit` carries no quantity of its own, so this SUM *is* the
/// observed figure a declared `commitment.quantity` binds against — the
/// checkout twin of [`shipped_units`]. The two backends must read exactly the
/// same thing, and only the placeholder dialect differs, so both spellings are
/// generated from this one text.
macro_rules! cart_units_sql {
    ($placeholder:literal) => {
        concat!("SELECT COALESCE(SUM(quantity), 0) FROM cart_items WHERE cart_id = ", $placeholder)
    };
}

/// The cart-units projection in SQLite placeholder dialect.
pub const CART_UNITS_SQL_SQLITE: &str = cart_units_sql!("?");

/// The cart-units projection in Postgres placeholder dialect.
pub const CART_UNITS_SQL_POSTGRES: &str = cart_units_sql!("$1");

/// Rejection sealed when a reservation expires while it is being confirmed.
#[must_use]
pub fn reservation_expired_during_shipment() -> GuardRejection {
    GuardRejection::never(
        "commerce.reservation_expired",
        "an inventory reservation expired during shipment",
    )
}

#[cfg(test)]
#[path = "orders_ownership_formal_tests.rs"]
mod ownership_formal_tests;
