//! `orders.transition` and `orders.ship` plans.

use super::PlanOutcome;
use crate::kernel::envelope::GuardRejection;
use rust_decimal::Decimal;
use stateset_core::{
    CommandEnvelope, Order, OrderStatus, Payment, PaymentStatus, ShipOrderCommand, TransitionOrder,
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
    let next_payment_status = command.payload.payment_status.unwrap_or(order.payment_status);
    if next_status == OrderStatus::Refunded
        && !matches!(
            next_payment_status,
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
    PlanOutcome::Proceed(ShipOrderEffects {
        status_before: order.status,
        resolved_status,
        deltas,
        version_before,
    })
}

/// Rejection sealed when a reservation expires while it is being confirmed.
#[must_use]
pub fn reservation_expired_during_shipment() -> GuardRejection {
    GuardRejection::never(
        "commerce.reservation_expired",
        "an inventory reservation expired during shipment",
    )
}
