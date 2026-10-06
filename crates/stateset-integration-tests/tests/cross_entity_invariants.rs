//! Cross-entity, model-based invariant harness.
//!
//! Every entity's own state machine is unit-tested; the defects that keep
//! escaping live *between* entities — an order whose `payment_status` no
//! longer matches its payments after a refund, a shipped order whose
//! shipments still say `pending`, a refused call that wrote half a row.
//! This harness drives random operation sequences across the customer-facing
//! lifecycle, through the public `stateset_embedded::Commerce` API only:
//!
//! customers → carts → checkout → payments (authorize / capture / fail /
//! cancel / refund) → shipments (create / add item / remove item / advance /
//! hold / cancel) → order ship / deliver / cancel → returns → inventory.
//!
//! After EVERY operation it asserts, from the database alone:
//!
//! - **refunds ≤ captured**: per payment (completed, and completed + in
//!   flight), and per order; `amount_refunded` = Σ completed refunds.
//! - **payment status is derived**: `orders.payment_status` is the fixed
//!   point of [`PaymentStatus::derive`] over the order's payment rows.
//! - **money is exact**: `total = Σ lines + tax + shipping − discount`, each
//!   line `= qty × price − discount + tax`, nothing exceeds the currency
//!   scale, and a checkout order carries its cart's totals.
//! - **fulfillment agrees with shipments**: `fulfillment_status` is what the
//!   order status implies; per-line `shipped_quantity` explains the status;
//!   shipment manifests never over-allocate a line; a fully shipped order has
//!   no shipment still waiting to ship; a frozen manifest never changes.
//! - **inventory never goes negative and reservations match open orders**:
//!   `on_hand`, `allocated` ≥ 0, `available = on_hand − allocated`,
//!   `allocated` = Σ live reservations, Σ movements = `on_hand`, and a
//!   cancelled or fully shipped order holds no live reservation.
//! - **returns ≤ shipped** per order line.
//! - **a refused operation changes nothing**: every mutating call is wrapped
//!   so that, when the engine refuses it, a snapshot of every entity the case
//!   owns is compared before/after.
//!
//! Refused operations are fine (they are a typed `CommerceError`); a panic,
//! a refusal that wrote something, or an invariant violation fails the case,
//! and proptest shrinks it to a minimal operation sequence.
//!
//! Backends: SQLite (in-memory) always; Postgres too when `DATABASE_URL` or
//! `POSTGRES_URL` is set and the crate is built with `--features postgres`.
//! Each Postgres case is scoped by its own customer and SKUs, so it shares
//! one database with other cases (and other suites) safely.
//!
//! Case counts: `CROSS_ENTITY_CASES` (SQLite, default 32) and
//! `CROSS_ENTITY_PG_CASES` (Postgres, default 12); `PROPTEST_CASES`
//! overrides both when the specific variable is unset.
//!
//! Known, tracked gaps are listed in [`KNOWN_GAPS`]; each is a product
//! decision that has not been made yet, not a harness limitation.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};

use proptest::prelude::*;
use rust_decimal::Decimal;
use serde_json::{Value, json};
use stateset_core::{
    AddCartItem, BackorderFilter, BackorderStatus, CartAddress, CartId, CartStatus, CommerceError,
    CreateCart, CreateInventoryItem, CreatePayment, CreateRefund, CreateReturn, CreateReturnItem,
    CreateShipment, CreateShipmentItem, CustomerId, FulfillmentStatus, OrderId, OrderPaymentLedger,
    OrderStatus, PaymentFilter, PaymentId, PaymentStatus, PaymentTransactionStatus, RefundStatus,
    ReservationStatus, ReturnDisposition, ReturnId, ReturnReason, ReturnStatus, SetCartPayment,
    SetCartShipping, SetReturnDisposition, ShipmentId, ShipmentLineInput, ShipmentStatus,
};
use stateset_embedded::Commerce;
use stateset_test_utils::fixtures;
use support::{MONEY_SCALE, money_scale, panic_message, pct_of};
use uuid::Uuid;

const SKU_COUNT: usize = 3;
const INITIAL_STOCK: i64 = 12;
const OPS_MIN: usize = 24;
const OPS_MAX: usize = 40;

/// Cross-entity properties the engine does not maintain today and that need
/// a product decision before they can be enforced. A violation of a listed
/// gap is recorded instead of failing the case; every other property fails.
/// Shrink-only: `known_gaps_are_still_real` replays each gap's reproducer and
/// fails once it stops reproducing, so a fixed gap must leave this list.
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "shipment-ship-does-not-ship-order",
        "Advancing a shipment record to shipped/delivered never moves the order: \
         its lines' shipped_quantity stays 0 and the order stays confirmed/unfulfilled. \
         Order → shipment propagation exists (a full order ship carries pre-ship \
         shipments to shipped) but shipment → order does not, so the two fulfillment \
         paths disagree. Needs a decision on which entity is the source of truth for \
         shipped units (and whether shipping a shipment for an unpaid order is allowed).",
    ),
    (
        "shipped-order-still-holds-stock",
        "Shipping an order (full or by lines) moves its reservations to `confirmed`, \
         which `ReservationStatus::holds_stock` still counts in `allocated`; nothing \
         ever moves them to `fulfilled` or decrements on_hand. Shipped units therefore \
         stay on hand and allocated forever, and a `restock` return disposition adds \
         the same units to on_hand a second time. Needs a decision on the stock \
         consumption point (ship vs. pick vs. deliver) for both backends.",
    ),
    (
        "open-shipment-on-shipped-order",
        "A fully shipped order can end up with a shipment still waiting to ship, two \
         ways: (a) a shipment is created (pending) after the order shipped every unit — \
         creation only refuses cancelled/refunded orders; (b) a shipment that was \
         on_hold when the order shipped (the order ship deliberately leaves holds \
         alone) is later released back to processing. A full order ship otherwise \
         carries pre-ship shipments to shipped, so the engine intends 'a shipped order \
         has no shipment waiting to ship'. Refusing (a) would break integrations that \
         record a carrier fulfillment after the order shipped (e.g. Shopify fulfillment \
         sync); for (b), releasing a hold on a shipped order could carry it to shipped. \
         Needs a decision.",
    ),
    (
        "cancelled-order-with-shipped-shipment",
        "Cancelling an order leaves its open shipments untouched, and shipment status \
         transitions never look at the order, so a shipment of a cancelled order can \
         still be packed and handed to the carrier. Either cancel should carry open \
         shipments to cancelled (the mirror of a full ship carrying them to shipped), or \
         cancel should be refused while a shipment is in progress, and shipping a \
         shipment of a cancelled order should be refused. Needs a decision.",
    ),
];

/// The minimal sequence that reproduces each tracked gap.
fn gap_reproducer(gap: &str) -> Vec<Op> {
    match gap {
        "shipment-ship-does-not-ship-order" => vec![
            checkout(&[(0, 1, 1_000)]),
            Op::CreateShipment { order: 0, line: 0, qty: 1 },
            Op::AdvanceShipment { shipment: 0 },
            Op::AdvanceShipment { shipment: 0 },
            Op::AdvanceShipment { shipment: 0 },
        ],
        "cancelled-order-with-shipped-shipment" => vec![
            checkout(&[(0, 1, 1_000)]),
            Op::CreateShipment { order: 0, line: 0, qty: 1 },
            Op::CancelOrder { order: 0 },
            Op::AdvanceShipment { shipment: 0 },
            Op::AdvanceShipment { shipment: 0 },
            Op::AdvanceShipment { shipment: 0 },
        ],
        "open-shipment-on-shipped-order" => vec![
            checkout(&[(0, 2, 1_000)]),
            // (b) on hold while the order ships, then released.
            Op::CreateShipment { order: 0, line: 0, qty: 1 },
            Op::HoldShipment { shipment: 0 },
            Op::ShipOrder { order: 0 },
            Op::AdvanceShipment { shipment: 0 },
            // (a) created after the order shipped.
            Op::CreateShipment { order: 0, line: 0, qty: 1 },
        ],
        "shipped-order-still-holds-stock" => {
            vec![checkout(&[(0, 1, 1_000)]), Op::ShipOrder { order: 0 }]
        }
        other => panic!("tracked gap {other} has no reproducer"),
    }
}

fn gap_tracked(id: &str) -> bool {
    KNOWN_GAPS.iter().any(|(gap, _)| *gap == id)
}

// ===========================================================================
// Op alphabet
// ===========================================================================

/// One random operation. Indices are resolved modulo the current collection
/// length at execution time, so shrinking never produces a dangling
/// reference (an empty collection makes the op a no-op).
#[derive(Debug, Clone)]
enum Op {
    ReceiveStock {
        sku: u8,
        qty: u8,
    },
    /// Cart → checkout. Lines are `(sku, qty, unit_price_cents)`.
    Checkout {
        lines: Vec<(u8, u8, u16)>,
        shipping_cents: u16,
        tax_cents: u16,
        external: bool,
    },
    /// Create a payment for `pct`% of what is left to capture and authorize it.
    Authorize {
        order: u8,
        pct: u8,
    },
    /// Capture (complete) an authorized or pending payment.
    Capture {
        payment: u8,
    },
    /// Create and capture in one go.
    DirectCapture {
        order: u8,
        pct: u8,
    },
    FailPayment {
        payment: u8,
    },
    CancelPayment {
        payment: u8,
    },
    RequestRefund {
        payment: u8,
        pct: u8,
    },
    /// Refund more than is left: must be refused.
    OverRefund {
        payment: u8,
        extra_cents: u16,
    },
    CompleteRefund {
        refund: u8,
    },
    FailRefund {
        refund: u8,
    },
    CreateShipment {
        order: u8,
        line: u8,
        qty: u8,
    },
    AddShipmentItem {
        shipment: u8,
        line: u8,
        qty: u8,
    },
    RemoveShipmentItem {
        shipment: u8,
        item: u8,
    },
    /// pending → processing → ready → shipped → in transit → out for
    /// delivery → delivered.
    AdvanceShipment {
        shipment: u8,
    },
    HoldShipment {
        shipment: u8,
    },
    CancelShipment {
        shipment: u8,
    },
    ShipOrder {
        order: u8,
    },
    ShipLine {
        order: u8,
        line: u8,
        qty: u8,
    },
    DeliverOrder {
        order: u8,
    },
    CancelOrder {
        order: u8,
    },
    RequestReturn {
        order: u8,
        line: u8,
        qty: u8,
    },
    AdvanceReturn {
        ret: u8,
    },
    RejectReturn {
        ret: u8,
    },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let line = (0u8..SKU_COUNT as u8, 1u8..=4, 1u16..=20_000);
    prop_oneof![
        2 => (0u8..SKU_COUNT as u8, 1u8..=20).prop_map(|(sku, qty)| Op::ReceiveStock { sku, qty }),
        5 => (proptest::collection::vec(line, 1..=3), 0u16..=1_500, 0u16..=900, proptest::bool::weighted(0.15))
            .prop_map(|(lines, shipping_cents, tax_cents, external)| Op::Checkout {
                lines, shipping_cents, tax_cents, external
            }),
        3 => (any::<u8>(), 1u8..=100).prop_map(|(order, pct)| Op::Authorize { order, pct }),
        3 => any::<u8>().prop_map(|payment| Op::Capture { payment }),
        3 => (any::<u8>(), 1u8..=100).prop_map(|(order, pct)| Op::DirectCapture { order, pct }),
        1 => any::<u8>().prop_map(|payment| Op::FailPayment { payment }),
        1 => any::<u8>().prop_map(|payment| Op::CancelPayment { payment }),
        3 => (any::<u8>(), 1u8..=100).prop_map(|(payment, pct)| Op::RequestRefund { payment, pct }),
        1 => (any::<u8>(), 1u16..=5_000).prop_map(|(payment, extra_cents)| Op::OverRefund { payment, extra_cents }),
        3 => any::<u8>().prop_map(|refund| Op::CompleteRefund { refund }),
        1 => any::<u8>().prop_map(|refund| Op::FailRefund { refund }),
        3 => (any::<u8>(), any::<u8>(), 1u8..=4).prop_map(|(order, line, qty)| Op::CreateShipment { order, line, qty }),
        2 => (any::<u8>(), any::<u8>(), 1u8..=4).prop_map(|(shipment, line, qty)| Op::AddShipmentItem { shipment, line, qty }),
        1 => (any::<u8>(), any::<u8>()).prop_map(|(shipment, item)| Op::RemoveShipmentItem { shipment, item }),
        4 => any::<u8>().prop_map(|shipment| Op::AdvanceShipment { shipment }),
        1 => any::<u8>().prop_map(|shipment| Op::HoldShipment { shipment }),
        1 => any::<u8>().prop_map(|shipment| Op::CancelShipment { shipment }),
        2 => any::<u8>().prop_map(|order| Op::ShipOrder { order }),
        2 => (any::<u8>(), any::<u8>(), 1u8..=4).prop_map(|(order, line, qty)| Op::ShipLine { order, line, qty }),
        1 => any::<u8>().prop_map(|order| Op::DeliverOrder { order }),
        1 => any::<u8>().prop_map(|order| Op::CancelOrder { order }),
        2 => (any::<u8>(), any::<u8>(), 1u8..=4).prop_map(|(order, line, qty)| Op::RequestReturn { order, line, qty }),
        3 => any::<u8>().prop_map(|ret| Op::AdvanceReturn { ret }),
        1 => any::<u8>().prop_map(|ret| Op::RejectReturn { ret }),
    ]
}

// ===========================================================================
// Model: the ids this case owns. Everything else is read back from the store.
// ===========================================================================

#[derive(Debug, Default)]
struct Model {
    carts: Vec<CartId>,
    /// Orders minted by checkout, with the cart they came from.
    orders: Vec<(OrderId, CartId)>,
    payments: Vec<PaymentId>,
    refunds: Vec<Uuid>,
    shipments: Vec<ShipmentId>,
    /// Shipments the order ship could not carry along: created after the
    /// order had shipped every unit, or on hold when it did (tracked gap
    /// `open-shipment-on-shipped-order`).
    late_shipments: BTreeSet<ShipmentId>,
    returns: Vec<ReturnId>,
}

struct Harness<'c> {
    commerce: &'c Commerce,
    customer_id: CustomerId,
    customer_email: String,
    skus: [String; SKU_COUNT],
    item_ids: [i64; SKU_COUNT],
    model: Model,
    /// Violations of [`KNOWN_GAPS`] seen during the run (for the gap test).
    gaps_seen: BTreeSet<&'static str>,
}

/// A harness-level failure (not an engine refusal).
fn harness_err(msg: impl Into<String>) -> CommerceError {
    CommerceError::Internal(format!("HARNESS: {}", msg.into()))
}

fn address() -> CartAddress {
    CartAddress {
        first_name: "Ada".into(),
        last_name: "Lovelace".into(),
        company: None,
        line1: "1 Analytical Way".into(),
        line2: None,
        city: "San Francisco".into(),
        state: Some("CA".into()),
        postal_code: "94102".into(),
        country: "US".into(),
        phone: None,
        email: None,
    }
}

impl<'c> Harness<'c> {
    fn new(commerce: &'c Commerce) -> Self {
        let customer = commerce
            .customers()
            .create(fixtures::create_customer_input())
            .expect("create customer");
        // Unique per case, so cases can share one Postgres database.
        let tag = Uuid::new_v4().simple().to_string()[..10].to_uppercase();
        let skus: [String; SKU_COUNT] = std::array::from_fn(|i| format!("XE-{tag}-{i}"));
        let mut item_ids = [0i64; SKU_COUNT];
        for (idx, sku) in skus.iter().enumerate() {
            item_ids[idx] = commerce
                .inventory()
                .create_item(CreateInventoryItem {
                    sku: sku.clone(),
                    name: format!("Item {sku}"),
                    initial_quantity: Some(Decimal::from(INITIAL_STOCK)),
                    ..Default::default()
                })
                .expect("create inventory item")
                .id;
        }
        Self {
            commerce,
            customer_id: customer.id,
            customer_email: customer.email,
            skus,
            item_ids,
            model: Model::default(),
            gaps_seen: BTreeSet::new(),
        }
    }

    fn pick<T>(items: &[T], idx: u8) -> Option<usize> {
        if items.is_empty() { None } else { Some(usize::from(idx) % items.len()) }
    }

    // -----------------------------------------------------------------------
    // Snapshot of everything this case owns, for the refused-op check.
    // -----------------------------------------------------------------------

    fn snapshot(&self) -> Result<Value, String> {
        let c = self.commerce;
        let e = |err: CommerceError| err.to_string();
        let mut orders = c.orders().list_for_customer(self.customer_id).map_err(e)?;
        orders.sort_by_key(|o| o.id.to_string());
        let mut payments = c
            .payments()
            .list(PaymentFilter {
                customer_id: Some(self.customer_id),
                limit: Some(10_000),
                ..Default::default()
            })
            .map_err(e)?;
        payments.sort_by_key(|p| p.id.to_string());
        let mut refunds = Vec::new();
        for p in &payments {
            let mut r = c.payments().get_refunds(p.id).map_err(e)?;
            r.sort_by_key(|r| r.id);
            refunds.push(r);
        }
        let mut shipments = Vec::new();
        let mut reservations = Vec::new();
        for o in &orders {
            let mut s = c.shipments().for_order(o.id).map_err(e)?;
            s.sort_by_key(|s| s.id.to_string());
            shipments.push(s);
            let mut r = c
                .inventory()
                .list_reservations_by_reference("order", &o.id.to_string())
                .map_err(e)?;
            r.sort_by_key(|r| r.id);
            reservations.push(r);
        }
        let mut returns = c.returns().list_for_customer(self.customer_id).map_err(e)?;
        returns.sort_by_key(|r| r.id.to_string());
        let mut carts = c.carts().for_customer(self.customer_id).map_err(e)?;
        carts.sort_by_key(|c| c.id.to_string());
        let mut stock = Vec::new();
        for (idx, sku) in self.skus.iter().enumerate() {
            stock.push(json!({
                "level": c.inventory().get_stock(sku).map_err(e)?,
                "movements": c.inventory().get_transactions(self.item_ids[idx], u32::MAX).map_err(e)?.len(),
            }));
        }
        Ok(json!({
            "orders": orders,
            "payments": payments,
            "refunds": refunds,
            "shipments": shipments,
            "reservations": reservations,
            "returns": returns,
            "carts": carts,
            "stock": stock,
        }))
    }

    /// Run one mutating engine call. When the engine refuses it, nothing the
    /// case owns may have changed.
    fn guarded<T>(
        &self,
        what: &str,
        call: impl FnOnce(&Commerce) -> Result<T, CommerceError>,
    ) -> Result<T, CommerceError> {
        let before = self.snapshot().map_err(|e| harness_err(format!("snapshot: {e}")))?;
        let result = call(self.commerce);
        if let Err(err) = &result {
            let after = self.snapshot().map_err(|e| harness_err(format!("snapshot: {e}")))?;
            if before != after {
                return Err(harness_err(format!(
                    "REFUSED OP WROTE STATE: {what} was refused ({err}) but changed {}",
                    diff(&before, &after)
                )));
            }
        }
        result
    }

    // -----------------------------------------------------------------------
    // Op execution
    // -----------------------------------------------------------------------

    fn apply(&mut self, op: &Op) -> Result<(), String> {
        let outcome = catch_unwind(AssertUnwindSafe(|| self.apply_inner(op)));
        let result = match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(CommerceError::Internal(msg))) if msg.starts_with("HARNESS:") => {
                Err(format!("op {op:?}: {msg}"))
            }
            // A typed engine refusal: fine (the guarded wrapper already
            // checked that it wrote nothing).
            Ok(Err(_)) => Ok(()),
            Err(payload) => Err(format!(
                "op {op:?} PANICKED instead of returning CommerceError: {}",
                panic_message(payload.as_ref())
            )),
        };
        self.adopt_engine_refunds();
        result
    }

    /// Track refunds the engine created on its own (return completion).
    fn adopt_engine_refunds(&mut self) {
        for p in &self.model.payments {
            if let Ok(refunds) = self.commerce.payments().get_refunds(*p) {
                for r in refunds {
                    if !self.model.refunds.contains(&r.id) {
                        self.model.refunds.push(r.id);
                    }
                }
            }
        }
    }

    fn order(&self, idx: u8) -> Option<OrderId> {
        Self::pick(&self.model.orders, idx).map(|i| self.model.orders[i].0)
    }

    /// Remaining capturable amount on an order: total − Σ captures that are
    /// completed or still in flight.
    fn uncaptured(&self, order_id: OrderId) -> Result<Decimal, CommerceError> {
        let order = self.commerce.orders().get(order_id)?.ok_or(CommerceError::NotFound)?;
        let committed: Decimal = self
            .commerce
            .payments()
            .for_order(order_id)?
            .iter()
            .filter(|p| {
                !matches!(
                    p.status,
                    PaymentTransactionStatus::Failed | PaymentTransactionStatus::Cancelled
                )
            })
            .map(|p| p.amount)
            .sum();
        Ok(order.total_amount - committed)
    }

    #[allow(clippy::too_many_lines)]
    fn apply_inner(&mut self, op: &Op) -> Result<(), CommerceError> {
        match op {
            Op::ReceiveStock { sku, qty } => {
                let sku = self.skus[usize::from(*sku) % SKU_COUNT].clone();
                self.guarded("receive stock", |c| {
                    c.inventory().adjust(&sku, Decimal::from(*qty), "harness receipt")
                })?;
            }
            Op::Checkout { lines, shipping_cents, tax_cents, external } => {
                let items = lines
                    .iter()
                    .map(|(sku, qty, cents)| {
                        let sku = self.skus[usize::from(*sku) % SKU_COUNT].clone();
                        AddCartItem {
                            name: format!("Item {sku}"),
                            sku,
                            quantity: i32::from(*qty),
                            unit_price: Decimal::new(i64::from(*cents), MONEY_SCALE),
                            ..Default::default()
                        }
                    })
                    .collect();
                let cart = self.guarded("create cart", |c| {
                    c.carts().create(CreateCart {
                        customer_id: Some(self.customer_id),
                        customer_email: Some(self.customer_email.clone()),
                        customer_name: Some("Ada Lovelace".into()),
                        items: Some(items),
                        ..Default::default()
                    })
                })?;
                self.model.carts.push(cart.id);
                let shipping = Decimal::new(i64::from(*shipping_cents), MONEY_SCALE);
                self.guarded("set cart shipping", |c| {
                    c.carts().set_shipping(
                        cart.id,
                        SetCartShipping {
                            shipping_address: address(),
                            shipping_method: Some("standard".into()),
                            shipping_carrier: None,
                            shipping_amount: Some(shipping),
                        },
                    )
                })?;
                let tax = Decimal::new(i64::from(*tax_cents), MONEY_SCALE);
                self.guarded("set cart tax", |c| c.carts().set_tax(cart.id, tax))?;
                self.guarded("set cart payment", |c| {
                    c.carts().set_payment(
                        cart.id,
                        SetCartPayment {
                            payment_method: "credit_card".into(),
                            payment_token: Some("tok_harness".into()),
                            billing_address: None,
                        },
                    )
                })?;
                let result = self.guarded("complete checkout", |c| {
                    if *external {
                        c.carts().complete_settled_externally(cart.id)
                    } else {
                        c.carts().complete(cart.id)
                    }
                })?;
                self.model.orders.push((result.order_id, cart.id));
                // Checkout is idempotent: completing again returns the same order.
                let again =
                    self.guarded("re-complete checkout", |c| c.carts().complete(cart.id))?;
                if again.order_id != result.order_id {
                    return Err(harness_err(format!(
                        "checkout of cart {} is not idempotent: {} then {}",
                        cart.id, result.order_id, again.order_id
                    )));
                }
            }
            Op::Authorize { order, pct } | Op::DirectCapture { order, pct } => {
                let Some(order_id) = self.order(*order) else { return Ok(()) };
                let amount = pct_of(self.uncaptured(order_id)?, *pct);
                if amount <= Decimal::ZERO {
                    return Ok(());
                }
                let payment = self.guarded("create payment", |c| {
                    c.payments().create(CreatePayment {
                        order_id: Some(order_id),
                        customer_id: Some(self.customer_id),
                        amount,
                        ..Default::default()
                    })
                })?;
                self.model.payments.push(payment.id);
                if matches!(op, Op::Authorize { .. }) {
                    self.guarded("authorize payment", |c| {
                        c.payments().mark_processing(payment.id)
                    })?;
                } else {
                    self.guarded("capture payment", |c| c.payments().mark_completed(payment.id))?;
                }
            }
            Op::Capture { payment } => {
                let Some(p) = Self::pick(&self.model.payments, *payment) else { return Ok(()) };
                let id = self.model.payments[p];
                self.guarded("capture payment", |c| c.payments().mark_completed(id))?;
            }
            Op::FailPayment { payment } => {
                let Some(p) = Self::pick(&self.model.payments, *payment) else { return Ok(()) };
                let id = self.model.payments[p];
                self.guarded("fail payment", |c| c.payments().mark_failed(id, "harness", None))?;
            }
            Op::CancelPayment { payment } => {
                let Some(p) = Self::pick(&self.model.payments, *payment) else { return Ok(()) };
                let id = self.model.payments[p];
                self.guarded("cancel payment", |c| c.payments().cancel(id))?;
            }
            Op::RequestRefund { payment, pct } => {
                let Some(p) = Self::pick(&self.model.payments, *payment) else { return Ok(()) };
                let id = self.model.payments[p];
                let refundable = self.refundable(id)?;
                let amount = pct_of(refundable, *pct);
                if amount <= Decimal::ZERO {
                    return Ok(());
                }
                let refund = self.guarded("create refund", |c| {
                    c.payments().create_refund(CreateRefund {
                        payment_id: id,
                        amount: Some(amount),
                        reason: Some("harness".into()),
                        ..Default::default()
                    })
                })?;
                self.model.refunds.push(refund.id);
            }
            Op::OverRefund { payment, extra_cents } => {
                let Some(p) = Self::pick(&self.model.payments, *payment) else { return Ok(()) };
                let id = self.model.payments[p];
                let amount =
                    self.refundable(id)? + Decimal::new(i64::from(*extra_cents), MONEY_SCALE);
                let result = self.guarded("over-refund", |c| {
                    c.payments().create_refund(CreateRefund {
                        payment_id: id,
                        amount: Some(amount),
                        ..Default::default()
                    })
                });
                if let Ok(refund) = result {
                    self.model.refunds.push(refund.id);
                    return Err(harness_err(format!(
                        "refund of {amount} accepted on payment {id} beyond its refundable balance"
                    )));
                }
            }
            Op::CompleteRefund { refund } => {
                let Some(r) = Self::pick(&self.model.refunds, *refund) else { return Ok(()) };
                let id = self.model.refunds[r];
                self.guarded("complete refund", |c| c.payments().complete_refund(id))?;
            }
            Op::FailRefund { refund } => {
                let Some(r) = Self::pick(&self.model.refunds, *refund) else { return Ok(()) };
                let id = self.model.refunds[r];
                self.guarded("fail refund", |c| c.payments().fail_refund(id, "harness"))?;
            }
            Op::CreateShipment { order, line, qty } => {
                let Some(order_id) = self.order(*order) else { return Ok(()) };
                let Some(item) = self.line_item(order_id, *line, *qty)? else { return Ok(()) };
                let order_shipped = matches!(
                    self.commerce.orders().get(order_id)?.ok_or(CommerceError::NotFound)?.status,
                    OrderStatus::Shipped | OrderStatus::Delivered
                );
                let shipment = self.guarded("create shipment", |c| {
                    c.shipments().create(CreateShipment {
                        order_id,
                        recipient_name: "Ada Lovelace".into(),
                        shipping_address: "1 Analytical Way, San Francisco, CA 94102".into(),
                        items: Some(vec![item]),
                        ..Default::default()
                    })
                })?;
                self.model.shipments.push(shipment.id);
                if order_shipped {
                    self.model.late_shipments.insert(shipment.id);
                }
            }
            Op::AddShipmentItem { shipment, line, qty } => {
                let Some(s) = Self::pick(&self.model.shipments, *shipment) else { return Ok(()) };
                let id = self.model.shipments[s];
                let current = self.commerce.shipments().get(id)?.ok_or(CommerceError::NotFound)?;
                let Some(item) = self.line_item(current.order_id, *line, *qty)? else {
                    return Ok(());
                };
                let result =
                    self.guarded("add shipment item", |c| c.shipments().add_item(id, item));
                if result.is_ok() && !current.status.allows_item_changes() {
                    return Err(harness_err(format!(
                        "FROZEN MANIFEST CHANGED: item added to shipment {id} in {} status",
                        current.status
                    )));
                }
                result?;
            }
            Op::RemoveShipmentItem { shipment, item } => {
                let Some(s) = Self::pick(&self.model.shipments, *shipment) else { return Ok(()) };
                let id = self.model.shipments[s];
                let current = self.commerce.shipments().get(id)?.ok_or(CommerceError::NotFound)?;
                let Some(i) = Self::pick(&current.items, *item) else { return Ok(()) };
                let item_id = current.items[i].id;
                let result =
                    self.guarded("remove shipment item", |c| c.shipments().remove_item(item_id));
                if result.is_ok() && !current.status.allows_item_changes() {
                    return Err(harness_err(format!(
                        "FROZEN MANIFEST CHANGED: item removed from shipment {id} in {} status",
                        current.status
                    )));
                }
                result?;
            }
            Op::AdvanceShipment { shipment } => {
                let Some(s) = Self::pick(&self.model.shipments, *shipment) else { return Ok(()) };
                let id = self.model.shipments[s];
                let current = self.commerce.shipments().get(id)?.ok_or(CommerceError::NotFound)?;
                let tracking = format!("TRK-{}", id.to_string()[..8].to_uppercase());
                self.guarded("advance shipment", |c| {
                    let s = c.shipments();
                    match current.status {
                        ShipmentStatus::Pending | ShipmentStatus::OnHold => s.mark_processing(id),
                        ShipmentStatus::Processing => s.mark_ready(id),
                        ShipmentStatus::ReadyToShip => s.ship(id, Some(tracking)),
                        ShipmentStatus::Shipped => s.mark_in_transit(id),
                        ShipmentStatus::InTransit => s.mark_out_for_delivery(id),
                        ShipmentStatus::OutForDelivery => s.mark_delivered(id),
                        _ => Ok(current.clone()),
                    }
                })?;
            }
            Op::HoldShipment { shipment } => {
                let Some(s) = Self::pick(&self.model.shipments, *shipment) else { return Ok(()) };
                let id = self.model.shipments[s];
                self.guarded("hold shipment", |c| c.shipments().hold(id))?;
            }
            Op::CancelShipment { shipment } => {
                let Some(s) = Self::pick(&self.model.shipments, *shipment) else { return Ok(()) };
                let id = self.model.shipments[s];
                self.guarded("cancel shipment", |c| c.shipments().cancel(id))?;
            }
            Op::ShipOrder { order } => {
                let Some(order_id) = self.order(*order) else { return Ok(()) };
                self.guarded("ship order", |c| c.orders().ship(order_id, Some("TRK-ORDER")))?;
                self.note_held_shipments(order_id)?;
            }
            Op::ShipLine { order, line, qty } => {
                let Some(order_id) = self.order(*order) else { return Ok(()) };
                let current =
                    self.commerce.orders().get(order_id)?.ok_or(CommerceError::NotFound)?;
                let Some(l) = Self::pick(&current.items, *line) else { return Ok(()) };
                let lines = vec![ShipmentLineInput {
                    order_item_id: current.items[l].id,
                    quantity: i32::from(*qty),
                }];
                self.guarded("ship order lines", |c| {
                    c.orders().ship_lines(order_id, None, Some(lines))
                })?;
                self.note_held_shipments(order_id)?;
            }
            Op::DeliverOrder { order } => {
                let Some(order_id) = self.order(*order) else { return Ok(()) };
                self.guarded("deliver order", |c| c.orders().deliver(order_id))?;
            }
            Op::CancelOrder { order } => {
                let Some(order_id) = self.order(*order) else { return Ok(()) };
                self.guarded("cancel order", |c| c.orders().cancel(order_id))?;
            }
            Op::RequestReturn { order, line, qty } => {
                let Some(order_id) = self.order(*order) else { return Ok(()) };
                let current =
                    self.commerce.orders().get(order_id)?.ok_or(CommerceError::NotFound)?;
                let Some(l) = Self::pick(&current.items, *line) else { return Ok(()) };
                let ret = self.guarded("create return", |c| {
                    c.returns().create(CreateReturn {
                        order_id,
                        reason: ReturnReason::Defective,
                        items: vec![CreateReturnItem {
                            order_item_id: current.items[l].id,
                            quantity: i32::from(*qty),
                            condition: None,
                        }],
                        ..Default::default()
                    })
                })?;
                self.model.returns.push(ret.id);
            }
            Op::AdvanceReturn { ret } => {
                let Some(r) = Self::pick(&self.model.returns, *ret) else { return Ok(()) };
                let id = self.model.returns[r];
                let current = self.commerce.returns().get(id)?.ok_or(CommerceError::NotFound)?;
                match current.status {
                    ReturnStatus::Requested => {
                        self.guarded("approve return", |c| c.returns().approve(id))?;
                    }
                    ReturnStatus::Approved => {
                        self.guarded("return tracking", |c| {
                            c.returns().add_tracking(id, &format!("RMA-{id}"))
                        })?;
                    }
                    ReturnStatus::InTransit => {
                        self.guarded("receive return", |c| c.returns().mark_received(id))?;
                    }
                    ReturnStatus::Received | ReturnStatus::Inspecting => {
                        for item in &current.items {
                            if item.disposition.is_none() {
                                self.guarded("disposition return item", |c| {
                                    c.returns().set_item_disposition(
                                        id,
                                        item.id,
                                        SetReturnDisposition {
                                            disposition: ReturnDisposition::Restock,
                                            ..Default::default()
                                        },
                                    )
                                })?;
                            }
                        }
                        self.guarded("complete return", |c| c.returns().complete(id))?;
                    }
                    _ => {}
                }
            }
            Op::RejectReturn { ret } => {
                let Some(r) = Self::pick(&self.model.returns, *ret) else { return Ok(()) };
                let id = self.model.returns[r];
                let current = self.commerce.returns().get(id)?.ok_or(CommerceError::NotFound)?;
                match current.status {
                    ReturnStatus::Requested => {
                        self.guarded("reject return", |c| c.returns().reject(id, "harness"))?;
                    }
                    ReturnStatus::Approved => {
                        self.guarded("cancel return", |c| c.returns().cancel(id))?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// After a ship that left the order fully shipped, remember the shipments
    /// it deliberately did not carry along (on hold): releasing one later
    /// reopens it on a shipped order (tracked gap).
    fn note_held_shipments(&mut self, order_id: OrderId) -> Result<(), CommerceError> {
        let order = self.commerce.orders().get(order_id)?.ok_or(CommerceError::NotFound)?;
        if matches!(order.status, OrderStatus::Shipped | OrderStatus::Delivered) {
            for s in self.commerce.shipments().for_order(order_id)? {
                if s.status == ShipmentStatus::OnHold {
                    self.model.late_shipments.insert(s.id);
                }
            }
        }
        Ok(())
    }

    /// What is still refundable on a payment: amount − completed − in flight.
    fn refundable(&self, id: PaymentId) -> Result<Decimal, CommerceError> {
        let payment = self.commerce.payments().get(id)?.ok_or(CommerceError::NotFound)?;
        let pending: Decimal = self
            .commerce
            .payments()
            .get_refunds(id)?
            .iter()
            .filter(|r| matches!(r.status, RefundStatus::Pending | RefundStatus::Processing))
            .map(|r| r.amount)
            .sum();
        Ok(payment.amount - payment.amount_refunded - pending)
    }

    fn line_item(
        &self,
        order_id: OrderId,
        line: u8,
        qty: u8,
    ) -> Result<Option<CreateShipmentItem>, CommerceError> {
        let order = self.commerce.orders().get(order_id)?.ok_or(CommerceError::NotFound)?;
        let Some(l) = Self::pick(&order.items, line) else { return Ok(None) };
        let item = &order.items[l];
        Ok(Some(CreateShipmentItem {
            order_item_id: Some(item.id.into_uuid()),
            product_id: None,
            sku: item.sku.clone(),
            name: item.name.clone(),
            quantity: i32::from(qty),
        }))
    }

    // -----------------------------------------------------------------------
    // Invariants
    // -----------------------------------------------------------------------

    fn check_invariants(&mut self) -> Result<(), String> {
        self.check_orders()?;
        self.check_inventory()?;
        Ok(())
    }

    /// Record a violation of a tracked gap, or fail on an untracked one.
    fn gap(&mut self, id: &'static str, msg: String) -> Result<(), String> {
        if gap_tracked(id) {
            self.gaps_seen.insert(id);
            Ok(())
        } else {
            Err(format!("[{id}] {msg}"))
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_orders(&mut self) -> Result<(), String> {
        let c = self.commerce;
        let e = |err: CommerceError| err.to_string();
        let orders = self.model.orders.clone();
        for (order_id, cart_id) in orders {
            let order = c.orders().get(order_id).map_err(e)?.ok_or("order vanished")?;

            // ---- money ----------------------------------------------------
            for (what, v) in [
                ("total", order.total_amount),
                ("tax", order.tax_amount),
                ("shipping", order.shipping_amount),
                ("discount", order.discount_amount),
            ] {
                money_scale(&format!("order {order_id} {what}"), v)?;
            }
            for item in &order.items {
                money_scale(&format!("order item {} total", item.id), item.total)?;
                let expected = (item.unit_price * Decimal::from(item.quantity) - item.discount
                    + item.tax_amount)
                    .round_dp(MONEY_SCALE);
                if item.total != expected {
                    return Err(format!(
                        "MONEY: order item {} total {} != qty*price-discount+tax {expected}",
                        item.id, item.total
                    ));
                }
            }
            if order.total_amount != order.calculate_total() {
                return Err(format!(
                    "MONEY: order {order_id} total {} != Σ lines + tax {} + shipping {} − discount {} = {}",
                    order.total_amount,
                    order.tax_amount,
                    order.shipping_amount,
                    order.discount_amount,
                    order.calculate_total()
                ));
            }
            let cart = c.carts().get(cart_id).map_err(e)?.ok_or("cart vanished")?;
            if cart.status != CartStatus::Completed {
                return Err(format!(
                    "checkout cart {cart_id} minted order {order_id} but is {}",
                    cart.status
                ));
            }
            if (cart.grand_total, cart.tax_amount, cart.shipping_amount, cart.discount_amount)
                != (
                    order.total_amount,
                    order.tax_amount,
                    order.shipping_amount,
                    order.discount_amount,
                )
            {
                return Err(format!(
                    "CHECKOUT MONEY: cart {cart_id} (total {}, tax {}, shipping {}, discount {}) \
                     != order {order_id} (total {}, tax {}, shipping {}, discount {})",
                    cart.grand_total,
                    cart.tax_amount,
                    cart.shipping_amount,
                    cart.discount_amount,
                    order.total_amount,
                    order.tax_amount,
                    order.shipping_amount,
                    order.discount_amount
                ));
            }

            // ---- payments / refunds ---------------------------------------
            let payments = c.payments().for_order(order_id).map_err(e)?;
            let mut ledger = OrderPaymentLedger::default();
            let mut order_completed_refunds = Decimal::ZERO;
            for p in &payments {
                money_scale(&format!("payment {} amount", p.id), p.amount)?;
                let refunds = c.payments().get_refunds(p.id).map_err(e)?;
                let mut completed = Decimal::ZERO;
                let mut in_flight = Decimal::ZERO;
                for r in &refunds {
                    money_scale(&format!("refund {} amount", r.id), r.amount)?;
                    match r.status {
                        RefundStatus::Completed => completed += r.amount,
                        RefundStatus::Pending | RefundStatus::Processing => in_flight += r.amount,
                        _ => {}
                    }
                }
                if completed > p.amount || completed + in_flight > p.amount {
                    return Err(format!(
                        "OVER-REFUND: payment {} amount {} but completed {completed} + in flight {in_flight}",
                        p.id, p.amount
                    ));
                }
                if p.amount_refunded != completed {
                    return Err(format!(
                        "payment {} amount_refunded {} != Σ completed refunds {completed}",
                        p.id, p.amount_refunded
                    ));
                }
                let settled = matches!(
                    p.status,
                    PaymentTransactionStatus::Completed
                        | PaymentTransactionStatus::PartiallyRefunded
                        | PaymentTransactionStatus::Refunded
                        | PaymentTransactionStatus::Disputed
                );
                if completed > Decimal::ZERO && !settled {
                    return Err(format!(
                        "payment {} in {} status carries {completed} of completed refunds",
                        p.id, p.status
                    ));
                }
                let expected_status = if !settled {
                    p.status
                } else if completed == Decimal::ZERO {
                    if p.status == PaymentTransactionStatus::Disputed {
                        p.status
                    } else {
                        PaymentTransactionStatus::Completed
                    }
                } else if completed == p.amount {
                    PaymentTransactionStatus::Refunded
                } else {
                    PaymentTransactionStatus::PartiallyRefunded
                };
                if p.status != expected_status && p.status != PaymentTransactionStatus::Disputed {
                    return Err(format!(
                        "PAYMENT STATUS: payment {} is {} but amount {} / refunded {completed} implies {expected_status}",
                        p.id, p.status, p.amount
                    ));
                }
                order_completed_refunds += completed;
                ledger.record(p.status, p.amount, p.amount_refunded);
            }
            if ledger.captured > order.total_amount {
                return Err(format!(
                    "OVER-CAPTURE: order {order_id} total {} but captured {}",
                    order.total_amount, ledger.captured
                ));
            }
            if order_completed_refunds > ledger.captured {
                return Err(format!(
                    "OVER-REFUND: order {order_id} captured {} but refunded {order_completed_refunds}",
                    ledger.captured
                ));
            }
            let derived = PaymentStatus::derive(order.total_amount, &ledger, order.payment_status);
            if derived != order.payment_status {
                return Err(format!(
                    "PAYMENT STATUS DRIFT: order {order_id} payment_status {} but its payments \
                     (captured {}, refunded {}, authorized {}, awaiting {}, failed {}) derive {derived}",
                    order.payment_status,
                    ledger.captured,
                    ledger.refunded,
                    ledger.authorized,
                    ledger.awaiting,
                    ledger.failed
                ));
            }

            // ---- fulfillment ----------------------------------------------
            if let Some(expected) = FulfillmentStatus::for_order_status(order.status) {
                if order.fulfillment_status != expected {
                    return Err(format!(
                        "FULFILLMENT: order {order_id} is {} but fulfillment_status {} (expected {expected})",
                        order.status, order.fulfillment_status
                    ));
                }
            }
            let ordered: i64 = order.items.iter().map(|i| i64::from(i.quantity)).sum();
            let shipped: i64 = order.items.iter().map(|i| i64::from(i.shipped_quantity)).sum();
            for item in &order.items {
                if item.shipped_quantity < 0 || item.shipped_quantity > item.quantity {
                    return Err(format!(
                        "FULFILLMENT: order item {} shipped {} of {}",
                        item.id, item.shipped_quantity, item.quantity
                    ));
                }
            }
            let fully_shipped =
                matches!(order.status, OrderStatus::Shipped | OrderStatus::Delivered);
            if fully_shipped && shipped != ordered {
                return Err(format!(
                    "FULFILLMENT: order {order_id} is {} with {shipped} of {ordered} units shipped",
                    order.status
                ));
            }
            if order.status == OrderStatus::PartiallyShipped && !(0 < shipped && shipped < ordered)
            {
                return Err(format!(
                    "FULFILLMENT: order {order_id} is partially_shipped with {shipped} of {ordered} units shipped"
                ));
            }
            if matches!(
                order.status,
                OrderStatus::Pending | OrderStatus::Confirmed | OrderStatus::Processing
            ) && shipped != 0
            {
                return Err(format!(
                    "FULFILLMENT: order {order_id} is {} yet {shipped} units have shipped",
                    order.status
                ));
            }

            let shipments = c.shipments().for_order(order_id).map_err(e)?;
            let mut allocated: BTreeMap<Uuid, i64> = BTreeMap::new();
            for s in &shipments {
                if s.order_id != order_id {
                    return Err(format!("shipment {} listed under the wrong order", s.id));
                }
                if s.status != ShipmentStatus::Cancelled {
                    for item in &s.items {
                        if let Some(line) = item.order_item_id {
                            *allocated.entry(line).or_default() += i64::from(item.quantity);
                        }
                    }
                }
                if fully_shipped
                    && matches!(
                        s.status,
                        ShipmentStatus::Pending
                            | ShipmentStatus::Processing
                            | ShipmentStatus::ReadyToShip
                    )
                {
                    let msg = format!(
                        "FULFILLMENT: order {order_id} is {} but shipment {} is still {}",
                        order.status, s.id, s.status
                    );
                    if self.model.late_shipments.contains(&s.id) {
                        self.gap("open-shipment-on-shipped-order", msg)?;
                    } else {
                        // The order ship itself must have carried it along.
                        return Err(msg);
                    }
                }
                let left_building = matches!(
                    s.status,
                    ShipmentStatus::Shipped
                        | ShipmentStatus::InTransit
                        | ShipmentStatus::OutForDelivery
                        | ShipmentStatus::Delivered
                );
                if left_building && shipped == 0 {
                    self.gap(
                        "shipment-ship-does-not-ship-order",
                        format!(
                            "FULFILLMENT: shipment {} is {} but order {order_id} is {} with no unit shipped",
                            s.id, s.status, order.status
                        ),
                    )?;
                }
                if left_building && order.status == OrderStatus::Cancelled {
                    self.gap(
                        "cancelled-order-with-shipped-shipment",
                        format!(
                            "FULFILLMENT: order {order_id} is cancelled but shipment {} is {}",
                            s.id, s.status
                        ),
                    )?;
                }
            }
            for item in &order.items {
                let a = allocated.get(&item.id.into_uuid()).copied().unwrap_or(0);
                if a > i64::from(item.quantity) {
                    return Err(format!(
                        "OVER-ALLOCATION: order item {} ordered {} but {a} units sit on live shipments",
                        item.id, item.quantity
                    ));
                }
            }

            // ---- returns --------------------------------------------------
            let mut returned: BTreeMap<_, i32> = BTreeMap::new();
            for ret in c.returns().list_for_order(order_id).map_err(e)? {
                if matches!(ret.status, ReturnStatus::Rejected | ReturnStatus::Cancelled) {
                    continue;
                }
                if let Some(amount) = ret.refund_amount {
                    money_scale(&format!("return {} refund_amount", ret.id), amount)?;
                }
                for item in &ret.items {
                    *returned.entry(item.order_item_id).or_default() += item.quantity;
                }
            }
            for item in &order.items {
                let r = returned.get(&item.id).copied().unwrap_or(0);
                if r > item.shipped_quantity {
                    return Err(format!(
                        "OVER-RETURN: order item {} shipped {} but {r} units are on returns",
                        item.id, item.shipped_quantity
                    ));
                }
            }

            // ---- reservations ---------------------------------------------
            let live = c
                .inventory()
                .list_reservations_by_reference("order", &order_id.to_string())
                .map_err(e)?
                .into_iter()
                .filter(|r| is_live(r.status))
                .count();
            if live > 0 && matches!(order.status, OrderStatus::Cancelled) {
                return Err(format!(
                    "RESERVATION: order {order_id} is cancelled but holds {live} live reservations"
                ));
            }
            if live > 0 && fully_shipped {
                self.gap(
                    "shipped-order-still-holds-stock",
                    format!(
                        "RESERVATION: order {order_id} is {} but still holds {live} live reservations",
                        order.status
                    ),
                )?;
            }
        }
        Ok(())
    }

    /// `on_hand`, `allocated` ≥ 0; `allocated` ≤ `on_hand`; `available =
    /// on_hand − allocated`; Σ movements = `on_hand`; `allocated` = Σ live
    /// reservations of this case's orders, carts and backorders.
    fn check_inventory(&self) -> Result<(), String> {
        let c = self.commerce;
        let e = |err: CommerceError| err.to_string();
        let mut live = [Decimal::ZERO; SKU_COUNT];
        let mut refs: Vec<(&str, String)> = Vec::new();
        for (o, cart) in &self.model.orders {
            refs.push(("order", o.to_string()));
            refs.push(("cart", cart.to_string()));
        }
        for cart in &self.model.carts {
            refs.push(("cart", cart.to_string()));
        }
        for (idx, sku) in self.skus.iter().enumerate() {
            for bo in c
                .backorder()
                .list_backorders(BackorderFilter { sku: Some(sku.clone()), ..Default::default() })
                .map_err(e)?
            {
                if matches!(
                    bo.status,
                    BackorderStatus::Pending
                        | BackorderStatus::PartiallyFulfilled
                        | BackorderStatus::Allocated
                        | BackorderStatus::ReadyToShip
                ) {
                    for r in c
                        .inventory()
                        .list_reservations_by_reference("backorder", &bo.id.to_string())
                        .map_err(e)?
                    {
                        if is_live(r.status) {
                            live[idx] += r.quantity;
                        }
                    }
                }
            }
        }
        refs.sort();
        refs.dedup();
        for (kind, id) in &refs {
            for r in c.inventory().list_reservations_by_reference(kind, id).map_err(e)? {
                if !is_live(r.status) {
                    continue;
                }
                let idx = self.item_ids.iter().position(|i| *i == r.item_id).ok_or_else(|| {
                    format!("reservation {} on a foreign item {}", r.id, r.item_id)
                })?;
                live[idx] += r.quantity;
            }
        }
        for (idx, sku) in self.skus.iter().enumerate() {
            let stock = c.inventory().get_stock(sku).map_err(e)?.ok_or("stock vanished")?;
            if stock.total_on_hand < Decimal::ZERO || stock.total_allocated < Decimal::ZERO {
                return Err(format!(
                    "INVENTORY: {sku} on_hand {} allocated {} (negative)",
                    stock.total_on_hand, stock.total_allocated
                ));
            }
            if stock.total_allocated > stock.total_on_hand {
                return Err(format!(
                    "INVENTORY: {sku} allocated {} > on_hand {}",
                    stock.total_allocated, stock.total_on_hand
                ));
            }
            if stock.total_available != stock.total_on_hand - stock.total_allocated {
                return Err(format!(
                    "INVENTORY: {sku} available {} != on_hand {} − allocated {}",
                    stock.total_available, stock.total_on_hand, stock.total_allocated
                ));
            }
            let movements: Decimal = c
                .inventory()
                .get_transactions(self.item_ids[idx], u32::MAX)
                .map_err(e)?
                .iter()
                .map(|t| t.quantity)
                .sum();
            if movements != stock.total_on_hand {
                return Err(format!(
                    "INVENTORY: {sku} Σ movements {movements} != on_hand {}",
                    stock.total_on_hand
                ));
            }
            if stock.total_allocated != live[idx] {
                return Err(format!(
                    "RESERVATION: {sku} allocated {} != Σ live reservations {}",
                    stock.total_allocated, live[idx]
                ));
            }
        }
        Ok(())
    }
}

/// A reservation that still counts toward `allocated` (the engine's own
/// definition, [`ReservationStatus::holds_stock`]).
const fn is_live(status: ReservationStatus) -> bool {
    status.holds_stock()
}

/// The leaf paths that differ between two snapshots (at most a dozen).
fn diff(before: &Value, after: &Value) -> String {
    fn walk(path: &str, b: &Value, a: &Value, out: &mut Vec<String>) {
        if b == a || out.len() >= 12 {
            return;
        }
        match (b, a) {
            (Value::Object(bo), Value::Object(ao)) => {
                let keys: BTreeSet<&String> = bo.keys().chain(ao.keys()).collect();
                for k in keys {
                    let null = Value::Null;
                    walk(
                        &format!("{path}.{k}"),
                        bo.get(k).unwrap_or(&null),
                        ao.get(k).unwrap_or(&null),
                        out,
                    );
                }
            }
            (Value::Array(bl), Value::Array(al)) if bl.len() == al.len() => {
                for (i, (x, y)) in bl.iter().zip(al).enumerate() {
                    walk(&format!("{path}[{i}]"), x, y, out);
                }
            }
            (Value::Array(bl), Value::Array(al)) => {
                out.push(format!("{path}: {} → {} entries", bl.len(), al.len()));
            }
            _ => {
                let clip = |v: &Value| {
                    let s = v.to_string();
                    s.chars().take(200).collect::<String>()
                };
                out.push(format!("{path}: {} → {}", clip(b), clip(a)));
            }
        }
    }
    let mut out = Vec::new();
    walk("", before, after, &mut out);
    out.join("; ")
}

/// Run `ops` against `commerce`, checking every invariant after every step.
fn run_sequence<'c>(commerce: &'c Commerce, ops: &[Op]) -> Result<Harness<'c>, String> {
    let mut h = Harness::new(commerce);
    h.check_invariants().map_err(|e| format!("invariant violated before any op: {e}"))?;
    for (step, op) in ops.iter().enumerate() {
        h.apply(op).map_err(|e| format!("step {step}: {e}"))?;
        h.check_invariants().map_err(|e| format!("after step {step} {op:?}: {e}"))?;
    }
    Ok(h)
}

/// Shrink budget (`CROSS_ENTITY_SHRINK_ITERS` overrides): every shrink step
/// replays a whole sequence, so this bounds how long a failure takes to report.
fn shrink_iters(default: u32) -> u32 {
    std::env::var("CROSS_ENTITY_SHRINK_ITERS").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn sqlite() -> Commerce {
    Commerce::in_memory().expect("in-memory commerce")
}

#[cfg(feature = "postgres")]
fn postgres() -> Option<&'static Commerce> {
    use std::sync::OnceLock;
    static PG: OnceLock<Option<Commerce>> = OnceLock::new();
    PG.get_or_init(|| {
        let url = std::env::var("POSTGRES_URL").or_else(|_| std::env::var("DATABASE_URL")).ok()?;
        Some(Commerce::with_postgres(&url).expect("connect to postgres and migrate"))
    })
    .as_ref()
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: support::cases_from_env(&["CROSS_ENTITY_CASES"], 32),
        max_shrink_iters: shrink_iters(400),
        ..ProptestConfig::default()
    })]

    /// Cross-entity invariants hold after every step of any op sequence (SQLite).
    #[test]
    fn prop_cross_entity_invariants_sqlite(
        ops in proptest::collection::vec(op_strategy(), OPS_MIN..=OPS_MAX)
    ) {
        let commerce = sqlite();
        if let Err(msg) = run_sequence(&commerce, &ops) {
            prop_assert!(false, "{msg}");
        }
    }
}

#[cfg(feature = "postgres")]
proptest! {
    #![proptest_config(ProptestConfig {
        cases: support::cases_from_env(&["CROSS_ENTITY_PG_CASES"], 12),
        max_shrink_iters: shrink_iters(150),
        ..ProptestConfig::default()
    })]

    /// The same sequences against Postgres (skipped without a database URL).
    #[test]
    fn prop_cross_entity_invariants_postgres(
        ops in proptest::collection::vec(op_strategy(), OPS_MIN..=OPS_MAX)
    ) {
        let Some(commerce) = postgres() else {
            eprintln!("POSTGRES_URL / DATABASE_URL not set; skipping the Postgres cross-entity harness");
            return Ok(());
        };
        if let Err(msg) = run_sequence(commerce, &ops) {
            prop_assert!(false, "{msg}");
        }
    }
}

// ===========================================================================
// Deterministic sequences — the full lifecycle, on every available backend.
// ===========================================================================

/// Opens a backend, or `None` when it is not configured.
type OpenBackend = Box<dyn FnOnce() -> Option<BackendHandle>>;

fn backends() -> Vec<(&'static str, OpenBackend)> {
    #[cfg_attr(not(feature = "postgres"), allow(unused_mut))]
    let mut v: Vec<(&'static str, OpenBackend)> =
        vec![("sqlite", Box::new(|| Some(BackendHandle::Owned(sqlite()))))];
    #[cfg(feature = "postgres")]
    v.push(("postgres", Box::new(|| postgres().map(BackendHandle::Shared))));
    v
}

enum BackendHandle {
    Owned(Commerce),
    #[cfg_attr(not(feature = "postgres"), allow(dead_code))]
    Shared(&'static Commerce),
}

impl BackendHandle {
    const fn get(&self) -> &Commerce {
        match self {
            Self::Owned(c) => c,
            Self::Shared(c) => c,
        }
    }
}

fn run_everywhere(ops: &[Op]) {
    for (name, make) in backends() {
        let Some(handle) = make() else {
            eprintln!("{name}: not configured; skipped");
            continue;
        };
        run_sequence(handle.get(), ops).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

fn checkout(lines: &[(u8, u8, u16)]) -> Op {
    Op::Checkout { lines: lines.to_vec(), shipping_cents: 599, tax_cents: 123, external: false }
}

#[test]
fn lifecycle_checkout_authorize_capture_ship_return_refund() {
    run_everywhere(&[
        checkout(&[(0, 2, 1_999), (1, 1, 500)]),
        Op::Authorize { order: 0, pct: 100 },
        Op::Capture { payment: 0 },
        Op::CreateShipment { order: 0, line: 0, qty: 2 },
        Op::AddShipmentItem { shipment: 0, line: 1, qty: 1 },
        Op::AdvanceShipment { shipment: 0 },
        Op::AdvanceShipment { shipment: 0 },
        Op::AdvanceShipment { shipment: 0 },
        // Shipped: the manifest is frozen now.
        Op::AddShipmentItem { shipment: 0, line: 1, qty: 1 },
        Op::RemoveShipmentItem { shipment: 0, item: 0 },
        Op::ShipOrder { order: 0 },
        Op::DeliverOrder { order: 0 },
        Op::RequestReturn { order: 0, line: 0, qty: 1 },
        Op::AdvanceReturn { ret: 0 },
        Op::AdvanceReturn { ret: 0 },
        Op::AdvanceReturn { ret: 0 },
        Op::AdvanceReturn { ret: 0 },
        Op::CompleteRefund { refund: 0 },
        Op::RequestRefund { payment: 0, pct: 100 },
        Op::CompleteRefund { refund: 1 },
        Op::OverRefund { payment: 0, extra_cents: 1 },
    ]);
}

#[test]
fn lifecycle_partial_captures_partial_ship_and_cancel() {
    run_everywhere(&[
        checkout(&[(0, 3, 1_000)]),
        checkout(&[(1, 2, 2_500), (2, 1, 999)]),
        Op::DirectCapture { order: 0, pct: 40 },
        Op::Authorize { order: 0, pct: 100 },
        Op::FailPayment { payment: 1 },
        Op::DirectCapture { order: 0, pct: 100 },
        Op::ShipLine { order: 0, line: 0, qty: 1 },
        Op::RequestRefund { payment: 0, pct: 50 },
        Op::FailRefund { refund: 0 },
        Op::RequestRefund { payment: 2, pct: 100 },
        Op::CompleteRefund { refund: 1 },
        Op::CancelOrder { order: 1 },
        Op::CreateShipment { order: 1, line: 0, qty: 1 },
        Op::ShipLine { order: 0, line: 0, qty: 2 },
        Op::Authorize { order: 1, pct: 100 },
        Op::CancelPayment { payment: 3 },
    ]);
}

#[test]
fn shipment_manifest_freezes_from_ready_to_ship_on_every_backend() {
    for (name, make) in backends() {
        let Some(handle) = make() else { continue };
        let mut h = Harness::new(handle.get());
        h.apply(&checkout(&[(0, 3, 1_000)])).unwrap_or_else(|e| panic!("{name}: {e}"));
        h.apply(&Op::CreateShipment { order: 0, line: 0, qty: 1 })
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let shipment_id = h.model.shipments[0];
        let shipments = h.commerce.shipments();
        let order = h.commerce.orders().get(h.model.orders[0].0).unwrap().unwrap();
        let line = CreateShipmentItem {
            order_item_id: Some(order.items[0].id.into_uuid()),
            product_id: None,
            sku: order.items[0].sku.clone(),
            name: order.items[0].name.clone(),
            quantity: 1,
        };
        // Pending → processing: still editable.
        shipments.mark_processing(shipment_id).unwrap();
        let added = shipments.add_item(shipment_id, line.clone()).expect("processing is editable");
        shipments.remove_item(added.id).expect("processing is editable");
        // Ready → shipped → delivered, and cancelled: frozen.
        shipments.mark_ready(shipment_id).unwrap();
        for step in 0..4 {
            let before = shipments.get(shipment_id).unwrap().unwrap();
            let err = shipments
                .add_item(shipment_id, line.clone())
                .expect_err(&format!("{name}: add in {} must be refused", before.status));
            assert!(
                matches!(err, CommerceError::ValidationError(_)),
                "{name}: {} → {err:?}",
                before.status
            );
            shipments
                .remove_item(before.items[0].id)
                .expect_err(&format!("{name}: remove in {} must be refused", before.status));
            let after = shipments.get(shipment_id).unwrap().unwrap();
            assert_eq!(
                serde_json::to_value(&after).unwrap(),
                serde_json::to_value(&before).unwrap(),
                "{name}: a refused item edit in {} changed the shipment",
                before.status
            );
            match step {
                0 => shipments.ship(shipment_id, Some("TRK-1".into())).map(drop).unwrap(),
                1 => shipments.mark_in_transit(shipment_id).map(drop).unwrap(),
                2 => shipments.mark_out_for_delivery(shipment_id).map(drop).unwrap(),
                _ => shipments.mark_delivered(shipment_id).map(drop).unwrap(),
            }
        }
        h.check_invariants().unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

/// Every tracked gap must still reproduce; a gap that no longer does has been
/// fixed and must leave [`KNOWN_GAPS`].
#[test]
fn known_gaps_are_still_real() {
    for (gap, _reason) in KNOWN_GAPS {
        for (name, make) in backends() {
            let Some(handle) = make() else { continue };
            let h = run_sequence(handle.get(), &gap_reproducer(gap))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(
                h.gaps_seen.contains(gap),
                "{name}: gap {gap} no longer reproduces; remove it from KNOWN_GAPS"
            );
        }
    }
}
