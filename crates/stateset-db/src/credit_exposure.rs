//! Unpaid documents contribute to credit exposure without double counting
//! the invoiced part of an order or its existing credit reservations.
use rust_decimal::Decimal;
use stateset_core::{CommerceError, Result};
use std::collections::BTreeMap;
use uuid::Uuid;
#[derive(Default)]
struct Exposure {
    order: Decimal,
    invoiced: Decimal,
    due: Decimal,
    reserved: Decimal,
    charged: Decimal,
}
pub(crate) fn additional(
    currency: &str,
    balance: Decimal,
    orders: Vec<(Uuid, Decimal, String, String)>,
    invoices: Vec<(Uuid, Option<Uuid>, Decimal, Decimal, String)>,
    reservations: Vec<(Uuid, Decimal)>,
    charges: Vec<(Option<Uuid>, Decimal)>,
    payments: Vec<(Uuid, Decimal)>,
) -> Result<Decimal> {
    let mut groups: BTreeMap<Uuid, Exposure> = BTreeMap::new();
    for (id, total, denomination, status) in orders {
        if status == "paid" || status == "refunded" {
            continue;
        }
        if total > Decimal::ZERO && denomination != currency {
            return Err(CommerceError::Conflict(
                "Unpaid orders must use the customer's credit-account currency".into(),
            ));
        }
        groups.entry(id).or_default().order = total;
    }
    for (id, order, total, due, denomination) in invoices {
        if due > Decimal::ZERO && denomination != currency {
            return Err(CommerceError::Conflict(
                "Issued invoices must use the customer's credit-account currency".into(),
            ));
        }
        let group = groups.entry(order.unwrap_or(id)).or_default();
        group.invoiced += total;
        group.due += due.max(Decimal::ZERO);
    }
    let mut paid: BTreeMap<Uuid, Decimal> = BTreeMap::new();
    for (order, amount) in payments {
        *paid.entry(order).or_default() += amount;
    }
    for (order, amount) in reservations {
        groups.entry(order).or_default().reserved += amount;
    }
    let total_charges: Decimal = charges.iter().map(|(_, amount)| *amount).sum();
    for (order, amount) in charges {
        if let Some(order) = order {
            groups.entry(order).or_default().charged += amount;
        }
    }
    // Historical manual repayments lack order allocation. Only subtract the
    // debt provably still outstanding for this order, keeping ambiguous legacy
    // allocations conservative rather than granting unsupported headroom.
    let reductions = (total_charges - balance).max(Decimal::ZERO);
    let mut extra = Decimal::ZERO;
    for (id, group) in groups {
        let billed_or_paid = group.invoiced.max(paid.get(&id).copied().unwrap_or_default());
        let document = (group.order - billed_or_paid).max(Decimal::ZERO) + group.due;
        let represented = group.reserved + (group.charged - reductions).max(Decimal::ZERO);
        extra += (document - represented).max(Decimal::ZERO);
    }
    Ok(extra)
}
