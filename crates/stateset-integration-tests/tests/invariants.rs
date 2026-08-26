//! Financial / inventory invariant harness.
//!
//! Drives random-but-valid sequences of commerce operations through the
//! embedded SQLite engine and, after EVERY operation, asserts a set of global
//! invariants over the whole database: no over-refund, captured ≤ order total,
//! returned ≤ ordered, inventory balances reconcile to the movement ledger,
//! every posted journal entry balances, the trial balance nets to zero, the AR
//! control account equals the sum of open invoice balances, and no money value
//! is stored with more decimals than its currency allows.
//!
//! Every operation must either succeed or return a typed [`CommerceError`] —
//! never panic — and a failed operation must leave the books untouched, which
//! is checked by comparing the database against an in-memory reference model
//! after each step.
//!
//! Run with `PROPTEST_CASES=<n>` to override the default of 64 cases.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use chrono::NaiveDate;
use proptest::prelude::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    CommerceError, CreateAutoPostingConfig, CreateGlPeriod, CreateInventoryItem, CreateInvoice,
    CreateInvoiceItem, CreateOrder, CreateOrderItem, CreatePayment, CreateRefund, CreateReturn,
    CreateReturnItem, CustomerId, InvoiceId, JournalEntryFilter, JournalEntryStatus, OrderId,
    OrderItemId, OrderStatus, PaymentId, PaymentTransactionStatus, ProductId, RecordInvoicePayment,
    RefundStatus, ReservationStatus, ReturnId, ReturnReason, ReturnStatus,
};
use stateset_embedded::Commerce;
use stateset_test_utils::fixtures;
use uuid::Uuid;

/// Minor units for the harness currency (USD).
const MONEY_SCALE: u32 = 2;
const SKUS: [&str; 3] = ["INV-SKU-A", "INV-SKU-B", "INV-SKU-C"];
const INITIAL_STOCK: i64 = 10;
const OPS_MIN: usize = 32;
const OPS_MAX: usize = 48;

// ===========================================================================
// Op alphabet
// ===========================================================================

/// One random operation. Indices are resolved modulo the current collection
/// length at execution time so that shrinking never produces an out-of-range
/// reference (an empty collection makes the op a no-op).
#[derive(Debug, Clone)]
enum Op {
    /// Receive `qty` units of a SKU into stock (positive adjustment).
    ReceiveStock {
        sku: u8,
        qty: u8,
    },
    /// Remove `qty` units from stock (negative adjustment; may be rejected).
    RemoveStock {
        sku: u8,
        qty: u8,
    },
    /// Create an order; each line is `(sku, qty, unit_price_cents)`.
    CreateOrder {
        lines: Vec<(u8, u8, u16)>,
    },
    /// Capture `pct`% of the order's remaining uncaptured total.
    CapturePayment {
        order: u8,
        pct: u8,
    },
    Ship {
        order: u8,
    },
    Deliver {
        order: u8,
    },
    CancelOrder {
        order: u8,
    },
    /// Request a return of `qty` units of one order line.
    RequestReturn {
        order: u8,
        line: u8,
        qty: u8,
    },
    /// Advance a return one step along approve → in-transit → received → completed.
    AdvanceReturn {
        ret: u8,
    },
    /// Reject (if requested) or cancel (if approved) a return.
    RejectReturn {
        ret: u8,
    },
    /// Create a pending refund for `pct`% of the payment's remaining refundable balance.
    RequestRefund {
        payment: u8,
        pct: u8,
    },
    CompleteRefund {
        refund: u8,
    },
    FailRefund {
        refund: u8,
    },
    /// Invoice an order (create + send + auto-post AR / revenue).
    PostInvoice {
        order: u8,
    },
    /// Pay `pct`% of an invoice's balance (payment + record + auto-post cash / AR).
    PayInvoice {
        invoice: u8,
        pct: u8,
    },
    /// Try to capture MORE than the order's remaining total (`extra_cents` over);
    /// the engine must reject it with a typed error and write nothing.
    OverCapture {
        order: u8,
        extra_cents: u16,
    },
    /// Try to create an order whose `unit_price` carries more decimal places
    /// than USD allows (`cents` plus `sub_cents`/1000 of a cent). The engine
    /// must reject it with `commerce.money.scale_exceeds_currency` and write
    /// nothing — this is what makes the M1 scale check in `check_orders`
    /// reachable, since the ordinary order generator only ever emits 2-dp
    /// prices.
    OverScaledOrder {
        sku: u8,
        qty: u8,
        cents: u16,
        /// 1..=9 thousandths of a currency unit added to the price, making it
        /// genuinely three-scale (never a trailing zero).
        sub_cents: u8,
    },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    let line = (0u8..3, 1u8..=5, 1u16..=20_000);
    prop_oneof![
        3 => (0u8..3, 1u8..=50).prop_map(|(sku, qty)| Op::ReceiveStock { sku, qty }),
        2 => (0u8..3, 1u8..=50).prop_map(|(sku, qty)| Op::RemoveStock { sku, qty }),
        4 => proptest::collection::vec(line, 1..=3).prop_map(|lines| Op::CreateOrder { lines }),
        4 => (any::<u8>(), 1u8..=100).prop_map(|(order, pct)| Op::CapturePayment { order, pct }),
        3 => any::<u8>().prop_map(|order| Op::Ship { order }),
        1 => any::<u8>().prop_map(|order| Op::Deliver { order }),
        1 => any::<u8>().prop_map(|order| Op::CancelOrder { order }),
        3 => (any::<u8>(), any::<u8>(), 1u8..=6)
            .prop_map(|(order, line, qty)| Op::RequestReturn { order, line, qty }),
        3 => any::<u8>().prop_map(|ret| Op::AdvanceReturn { ret }),
        1 => any::<u8>().prop_map(|ret| Op::RejectReturn { ret }),
        3 => (any::<u8>(), 1u8..=100).prop_map(|(payment, pct)| Op::RequestRefund { payment, pct }),
        3 => any::<u8>().prop_map(|refund| Op::CompleteRefund { refund }),
        1 => any::<u8>().prop_map(|refund| Op::FailRefund { refund }),
        2 => any::<u8>().prop_map(|order| Op::PostInvoice { order }),
        2 => (any::<u8>(), 1u8..=100).prop_map(|(invoice, pct)| Op::PayInvoice { invoice, pct }),
        2 => (any::<u8>(), 1u16..=5_000)
            .prop_map(|(order, extra_cents)| Op::OverCapture { order, extra_cents }),
        2 => (0u8..3, 1u8..=5, 1u16..=20_000, 1u8..=9).prop_map(
            |(sku, qty, cents, sub_cents)| Op::OverScaledOrder { sku, qty, cents, sub_cents }
        ),
    ]
}

// ===========================================================================
// Reference model
// ===========================================================================

#[derive(Debug, Clone)]
struct MLine {
    item_id: OrderItemId,
}

#[derive(Debug, Clone)]
struct MOrder {
    id: OrderId,
    total: Decimal,
    /// Σ completed order payments.
    captured: Decimal,
    lines: Vec<MLine>,
    invoiced: bool,
    /// Set once the engine accepted `ship`/`deliver`; returns are only valid after this.
    shipped: bool,
}

#[derive(Debug, Clone)]
struct MPayment {
    id: PaymentId,
    amount: Decimal,
    refunded: Decimal,
    in_flight: Decimal,
}

#[derive(Debug, Clone)]
struct MRefund {
    id: Uuid,
    payment: usize,
    amount: Decimal,
    status: RefundStatus,
}

#[derive(Debug, Clone)]
struct MInvoice {
    id: InvoiceId,
    total: Decimal,
    paid: Decimal,
}

/// Expected state, updated only when the engine reports success.
#[derive(Debug, Default)]
struct Model {
    on_hand: [Decimal; 3],
    orders: Vec<MOrder>,
    payments: Vec<MPayment>,
    refunds: Vec<MRefund>,
    returns: Vec<ReturnId>,
    invoices: Vec<MInvoice>,
    /// Number of AR-side payment records (invoice payments), for count checks.
    invoice_payments: usize,
}

impl Model {
    fn open_ar(&self) -> Decimal {
        self.invoices.iter().map(|i| i.total - i.paid).sum()
    }
}

// ===========================================================================
// Fixture
// ===========================================================================

struct Harness {
    commerce: Commerce,
    customer_id: CustomerId,
    ar_account_id: Uuid,
    item_ids: [i64; 3],
    model: Model,
}

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

/// Reporting horizon covering every entry the harness can produce.
const AS_OF: NaiveDate = date(2035, 12, 31);

impl Harness {
    fn new() -> Self {
        // `:memory:` is backed by a private temp file whose guard lives in the
        // pool, so the `Commerce` value alone keeps the database alive.
        let commerce = Commerce::in_memory().expect("in-memory commerce");
        let customer_id = commerce
            .customers()
            .create(fixtures::create_customer_input())
            .expect("create customer")
            .id;

        let mut item_ids = [0i64; 3];
        for (idx, sku) in SKUS.iter().enumerate() {
            let item = commerce
                .inventory()
                .create_item(CreateInventoryItem {
                    sku: (*sku).into(),
                    name: format!("Item {sku}"),
                    initial_quantity: Some(Decimal::from(INITIAL_STOCK)),
                    ..Default::default()
                })
                .expect("create inventory item");
            item_ids[idx] = item.id;
        }

        let gl = commerce.general_ledger();
        gl.initialize_chart_of_accounts().expect("init chart of accounts");
        let by_number =
            |n: &str| gl.get_account_by_number(n).expect("get account").expect("seeded account").id;
        let ar_account_id = by_number("1100");
        gl.set_auto_posting_config(CreateAutoPostingConfig {
            config_name: "invariants".into(),
            cash_account_id: by_number("1010"),
            accounts_receivable_account_id: ar_account_id,
            inventory_account_id: by_number("1200"),
            accounts_payable_account_id: by_number("2010"),
            unearned_revenue_account_id: None,
            sales_revenue_account_id: by_number("4010"),
            shipping_revenue_account_id: None,
            cogs_account_id: by_number("5010"),
            bad_debt_expense_account_id: None,
            fx_gain_loss_account_id: None,
            auto_post_depreciation: false,
            auto_post_revenue_recognition: false,
        })
        .expect("auto-posting config");
        // Auto-posting stamps entries with today's date, which must fall in an
        // open period.
        let period = gl
            .create_period(CreateGlPeriod {
                period_name: "INV-wide".into(),
                fiscal_year: 2026,
                period_number: 1,
                start_date: date(2020, 1, 1),
                end_date: AS_OF,
            })
            .expect("create period");
        gl.open_period(period.id).expect("open period");

        let model = Model { on_hand: [Decimal::from(INITIAL_STOCK); 3], ..Default::default() };
        Self { commerce, customer_id, ar_account_id, item_ids, model }
    }

    // -----------------------------------------------------------------------
    // Op execution. Returns Ok(()) whether the engine accepted or rejected the
    // op (rejections must be typed errors); Err only for harness-level faults.
    // -----------------------------------------------------------------------

    fn apply(&mut self, op: &Op) -> Result<(), String> {
        let outcome = catch_unwind(AssertUnwindSafe(|| self.apply_inner(op)));
        match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(CommerceError::Internal(msg))) if msg.starts_with("HARNESS:") => {
                Err(format!("op {op:?}: {msg}"))
            }
            Ok(Err(err)) => {
                // Typed engine rejection: fine, but the model must not have moved
                // (apply_inner only mutates the model after success).
                let _typed: &CommerceError = &err;
                Ok(())
            }
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_else(|| "non-string panic".into());
                Err(format!("op {op:?} PANICKED instead of returning CommerceError: {msg}"))
            }
        }
    }

    fn pick<T>(items: &[T], idx: u8) -> Option<usize> {
        if items.is_empty() { None } else { Some(usize::from(idx) % items.len()) }
    }

    fn pct_of(amount: Decimal, pct: u8) -> Decimal {
        (amount * Decimal::from(pct) / dec!(100)).round_dp(MONEY_SCALE)
    }

    #[allow(clippy::too_many_lines)]
    fn apply_inner(&mut self, op: &Op) -> Result<(), CommerceError> {
        match op {
            Op::ReceiveStock { sku, qty } => {
                let s = usize::from(*sku) % SKUS.len();
                let q = Decimal::from(*qty);
                self.commerce.inventory().adjust(SKUS[s], q, "harness receipt")?;
                self.model.on_hand[s] += q;
            }
            Op::RemoveStock { sku, qty } => {
                let s = usize::from(*sku) % SKUS.len();
                let q = Decimal::from(*qty);
                self.commerce.inventory().adjust(SKUS[s], -q, "harness shrink")?;
                self.model.on_hand[s] -= q;
            }
            Op::CreateOrder { lines } => {
                let items: Vec<CreateOrderItem> = lines
                    .iter()
                    .map(|(sku, qty, cents)| {
                        let s = usize::from(*sku) % SKUS.len();
                        CreateOrderItem {
                            product_id: ProductId::new(),
                            sku: SKUS[s].into(),
                            name: format!("Item {}", SKUS[s]),
                            quantity: i32::from(*qty),
                            unit_price: Decimal::new(i64::from(*cents), MONEY_SCALE),
                            ..Default::default()
                        }
                    })
                    .collect();
                let order = self.commerce.orders().create(CreateOrder {
                    customer_id: self.customer_id,
                    items,
                    ..Default::default()
                })?;
                let lines = order.items.iter().map(|i| MLine { item_id: i.id }).collect();
                self.model.orders.push(MOrder {
                    id: order.id,
                    total: order.total_amount,
                    captured: Decimal::ZERO,
                    lines,
                    invoiced: false,
                    shipped: false,
                });
            }
            Op::CapturePayment { order, pct } => {
                let Some(o) = Self::pick(&self.model.orders, *order) else { return Ok(()) };
                let remaining = self.model.orders[o].total - self.model.orders[o].captured;
                let amount = Self::pct_of(remaining, *pct);
                if amount <= Decimal::ZERO {
                    return Ok(());
                }
                let payments = self.commerce.payments();
                let payment = payments.create(CreatePayment {
                    order_id: Some(self.model.orders[o].id),
                    customer_id: Some(self.customer_id),
                    amount,
                    ..Default::default()
                })?;
                // A pending payment that fails to complete is not "captured";
                // the model only counts it once completed.
                payments.mark_completed(payment.id)?;
                self.model.orders[o].captured += amount;
                self.model.payments.push(MPayment {
                    id: payment.id,
                    amount,
                    refunded: Decimal::ZERO,
                    in_flight: Decimal::ZERO,
                });
            }
            Op::Ship { order } => {
                let Some(o) = Self::pick(&self.model.orders, *order) else { return Ok(()) };
                self.commerce.orders().ship(self.model.orders[o].id, None)?;
                self.model.orders[o].shipped = true;
            }
            Op::Deliver { order } => {
                let Some(o) = Self::pick(&self.model.orders, *order) else { return Ok(()) };
                self.commerce.orders().deliver(self.model.orders[o].id)?;
                self.model.orders[o].shipped = true;
            }
            Op::CancelOrder { order } => {
                let Some(o) = Self::pick(&self.model.orders, *order) else { return Ok(()) };
                self.commerce.orders().cancel(self.model.orders[o].id)?;
            }
            Op::RequestReturn { order, line, qty } => {
                let Some(o) = Self::pick(&self.model.orders, *order) else { return Ok(()) };
                let Some(l) = Self::pick(&self.model.orders[o].lines, *line) else {
                    return Ok(());
                };
                let result = self.commerce.returns().create(CreateReturn {
                    order_id: self.model.orders[o].id,
                    reason: ReturnReason::ChangedMind,
                    items: vec![CreateReturnItem {
                        order_item_id: self.model.orders[o].lines[l].item_id,
                        quantity: i32::from(*qty),
                        condition: None,
                    }],
                    ..Default::default()
                });
                if !self.model.orders[o].shipped {
                    // Never-shipped orders have nothing to send back.
                    return match result {
                        Ok(ret) => Err(CommerceError::Internal(format!(
                            "HARNESS: return {} accepted on never-shipped order {}",
                            ret.id, self.model.orders[o].id
                        ))),
                        Err(e)
                            if e.invariant_code() == Some("commerce.return.order_not_shipped") =>
                        {
                            Ok(())
                        }
                        Err(other) => Err(other),
                    };
                }
                let ret = result?;
                self.model.returns.push(ret.id);
            }
            Op::AdvanceReturn { ret } => {
                let Some(r) = Self::pick(&self.model.returns, *ret) else { return Ok(()) };
                let id = self.model.returns[r];
                let returns = self.commerce.returns();
                let current = returns.get(id)?.ok_or(CommerceError::NotFound)?;
                match current.status {
                    ReturnStatus::Requested => {
                        returns.approve(id)?;
                    }
                    ReturnStatus::Approved => {
                        returns.add_tracking(id, &format!("RMA-{id}"))?;
                    }
                    ReturnStatus::InTransit => {
                        returns.mark_received(id)?;
                    }
                    ReturnStatus::Received | ReturnStatus::Inspecting => {
                        returns.complete(id)?;
                    }
                    _ => {}
                }
            }
            Op::RejectReturn { ret } => {
                let Some(r) = Self::pick(&self.model.returns, *ret) else { return Ok(()) };
                let id = self.model.returns[r];
                let returns = self.commerce.returns();
                let current = returns.get(id)?.ok_or(CommerceError::NotFound)?;
                match current.status {
                    ReturnStatus::Requested => {
                        returns.reject(id, "harness reject")?;
                    }
                    ReturnStatus::Approved => {
                        returns.cancel(id)?;
                    }
                    _ => {}
                }
            }
            Op::RequestRefund { payment, pct } => {
                let Some(p) = Self::pick(&self.model.payments, *payment) else { return Ok(()) };
                let mp = &self.model.payments[p];
                let remaining = mp.amount - mp.refunded - mp.in_flight;
                let amount = Self::pct_of(remaining, *pct);
                if amount <= Decimal::ZERO {
                    return Ok(());
                }
                let refund = self.commerce.payments().create_refund(CreateRefund {
                    payment_id: mp.id,
                    amount: Some(amount),
                    reason: Some("harness".into()),
                    ..Default::default()
                })?;
                self.model.payments[p].in_flight += amount;
                self.model.refunds.push(MRefund {
                    id: refund.id,
                    payment: p,
                    amount,
                    status: RefundStatus::Pending,
                });
            }
            Op::CompleteRefund { refund } => {
                let Some(r) = Self::pick(&self.model.refunds, *refund) else { return Ok(()) };
                let mr = self.model.refunds[r].clone();
                self.commerce.payments().complete_refund(mr.id)?;
                if mr.status == RefundStatus::Pending {
                    self.model.refunds[r].status = RefundStatus::Completed;
                    let mp = &mut self.model.payments[mr.payment];
                    mp.in_flight -= mr.amount;
                    mp.refunded += mr.amount;
                }
            }
            Op::FailRefund { refund } => {
                let Some(r) = Self::pick(&self.model.refunds, *refund) else { return Ok(()) };
                let mr = self.model.refunds[r].clone();
                self.commerce.payments().fail_refund(mr.id, "harness fail")?;
                if mr.status == RefundStatus::Pending {
                    self.model.refunds[r].status = RefundStatus::Failed;
                    self.model.payments[mr.payment].in_flight -= mr.amount;
                }
            }
            Op::PostInvoice { order } => {
                let Some(o) = Self::pick(&self.model.orders, *order) else { return Ok(()) };
                if self.model.orders[o].invoiced {
                    return Ok(());
                }
                let order = self
                    .commerce
                    .orders()
                    .get(self.model.orders[o].id)?
                    .ok_or(CommerceError::NotFound)?;
                let items = order
                    .items
                    .iter()
                    .map(|i| CreateInvoiceItem {
                        order_item_id: Some(i.id),
                        sku: Some(i.sku.clone()),
                        description: i.name.clone(),
                        quantity: Decimal::from(i.quantity),
                        unit_price: i.unit_price,
                        ..Default::default()
                    })
                    .collect();
                let invoices = self.commerce.invoices();
                let invoice = invoices.create(CreateInvoice {
                    customer_id: self.customer_id,
                    order_id: Some(order.id),
                    days_until_due: Some(30),
                    items,
                    ..Default::default()
                })?;
                invoices.send(invoice.id.into_uuid())?;
                self.commerce.general_ledger().auto_post_invoice(invoice.id.into_uuid())?;
                self.model.orders[o].invoiced = true;
                self.model.invoices.push(MInvoice {
                    id: invoice.id,
                    total: invoice.total,
                    paid: Decimal::ZERO,
                });
            }
            Op::OverCapture { order, extra_cents } => {
                let Some(o) = Self::pick(&self.model.orders, *order) else { return Ok(()) };
                let remaining = self.model.orders[o].total - self.model.orders[o].captured;
                let amount = remaining + Decimal::new(i64::from(*extra_cents), MONEY_SCALE);
                let result = self.commerce.payments().create(CreatePayment {
                    order_id: Some(self.model.orders[o].id),
                    customer_id: Some(self.customer_id),
                    amount,
                    ..Default::default()
                });
                return match result {
                    Ok(p) => Err(CommerceError::Internal(format!(
                        "HARNESS: over-capture {} accepted on order {} (total {}, captured {}) as payment {}",
                        amount,
                        self.model.orders[o].id,
                        self.model.orders[o].total,
                        self.model.orders[o].captured,
                        p.id
                    ))),
                    Err(e)
                        if e.invariant_code() == Some("commerce.capture.exceeds_order_total") =>
                    {
                        Ok(())
                    }
                    Err(other) => Err(other),
                };
            }
            Op::OverScaledOrder { sku, qty, cents, sub_cents } => {
                let s = usize::from(*sku) % SKUS.len();
                // e.g. 1099 cents + 7 => 10.997, three significant decimals.
                let unit_price = Decimal::new(i64::from(*cents), MONEY_SCALE)
                    + Decimal::new(i64::from(*sub_cents), MONEY_SCALE + 1);
                assert_eq!(
                    unit_price.normalize().scale(),
                    MONEY_SCALE + 1,
                    "HARNESS BUG: over-scaled price {unit_price} is not 3-scale"
                );
                let input = CreateOrder {
                    customer_id: self.customer_id,
                    items: vec![CreateOrderItem {
                        product_id: ProductId::new(),
                        sku: SKUS[s].into(),
                        name: format!("Item {}", SKUS[s]),
                        quantity: i32::from(*qty),
                        unit_price,
                        ..Default::default()
                    }],
                    ..Default::default()
                };
                return match self.commerce.orders().create(input) {
                    Ok(o) => Err(CommerceError::Internal(format!(
                        "HARNESS: over-scaled unit_price {unit_price} accepted as order {}",
                        o.id
                    ))),
                    Err(e)
                        if e.invariant_code() == Some("commerce.money.scale_exceeds_currency") =>
                    {
                        // Rejected as required; the model is untouched, and the
                        // post-step count/invariant checks assert nothing was
                        // written.
                        Ok(())
                    }
                    Err(other) => Err(other),
                };
            }
            Op::PayInvoice { invoice, pct } => {
                let Some(i) = Self::pick(&self.model.invoices, *invoice) else { return Ok(()) };
                let mi = &self.model.invoices[i];
                let amount = Self::pct_of(mi.total - mi.paid, *pct);
                if amount <= Decimal::ZERO {
                    return Ok(());
                }
                let payments = self.commerce.payments();
                let payment = payments.create(CreatePayment {
                    invoice_id: Some(mi.id.into_uuid()),
                    customer_id: Some(self.customer_id),
                    amount,
                    ..Default::default()
                })?;
                payments.mark_completed(payment.id)?;
                self.model.invoice_payments += 1;
                self.commerce.invoices().record_payment(
                    mi.id.into_uuid(),
                    RecordInvoicePayment {
                        amount,
                        payment_id: Some(payment.id.into_uuid()),
                        ..Default::default()
                    },
                )?;
                self.commerce
                    .general_ledger()
                    .auto_post_payment_received(payment.id.into_uuid())?;
                self.model.invoices[i].paid += amount;
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Invariants
    // -----------------------------------------------------------------------

    fn check_invariants(&self) -> Result<(), String> {
        self.check_payments()?;
        self.check_orders()?;
        self.check_inventory()?;
        self.check_ledger()?;
        self.check_counts()?;
        Ok(())
    }

    /// Σ refunds ≤ captured (completed AND in-flight), and the payment's
    /// `amount_refunded` equals the sum of completed refunds.
    fn check_payments(&self) -> Result<(), String> {
        let payments = self.commerce.payments();
        for mp in &self.model.payments {
            let payment = payments
                .get(mp.id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("payment {} vanished", mp.id))?;
            money_scale(&format!("payment {} amount", mp.id), payment.amount)?;
            money_scale(&format!("payment {} amount_refunded", mp.id), payment.amount_refunded)?;
            let refunds = payments.get_refunds(mp.id).map_err(|e| e.to_string())?;
            let mut completed = Decimal::ZERO;
            let mut in_flight = Decimal::ZERO;
            for r in &refunds {
                money_scale(&format!("refund {} amount", r.id), r.amount)?;
                if r.amount <= Decimal::ZERO {
                    return Err(format!("refund {} has non-positive amount {}", r.id, r.amount));
                }
                match r.status {
                    RefundStatus::Completed => completed += r.amount,
                    RefundStatus::Pending | RefundStatus::Processing => in_flight += r.amount,
                    RefundStatus::Failed | RefundStatus::Cancelled => {}
                    other => return Err(format!("unexpected refund status {other:?}")),
                }
            }
            if completed > payment.amount {
                return Err(format!(
                    "OVER-REFUND: payment {} captured {} but completed refunds total {}",
                    mp.id, payment.amount, completed
                ));
            }
            if completed + in_flight > payment.amount {
                return Err(format!(
                    "OVER-REFUND (in-flight): payment {} captured {} but completed {} + pending {} exceeds it",
                    mp.id, payment.amount, completed, in_flight
                ));
            }
            if payment.amount_refunded != completed {
                return Err(format!(
                    "payment {} amount_refunded {} != Σ completed refunds {}",
                    mp.id, payment.amount_refunded, completed
                ));
            }
            if payment.amount != mp.amount || completed != mp.refunded || in_flight != mp.in_flight
            {
                return Err(format!(
                    "payment {} drifted from model: db (amount {}, refunded {}, in-flight {}) vs model ({}, {}, {})",
                    mp.id,
                    payment.amount,
                    completed,
                    in_flight,
                    mp.amount,
                    mp.refunded,
                    mp.in_flight
                ));
            }
            if payment.status == PaymentTransactionStatus::Refunded && completed != payment.amount {
                return Err(format!(
                    "payment {} marked Refunded with only {} of {} refunded",
                    mp.id, completed, payment.amount
                ));
            }
        }
        Ok(())
    }

    /// captured ≤ total; refunded ≤ captured; returned qty per line ≤ ordered;
    /// totals foot to line items; money scale.
    fn check_orders(&self) -> Result<(), String> {
        let orders = self.commerce.orders();
        let payments = self.commerce.payments();
        let returns = self.commerce.returns();
        for mo in &self.model.orders {
            let order = orders
                .get(mo.id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("order {} vanished", mo.id))?;
            money_scale(&format!("order {} total", mo.id), order.total_amount)?;
            let mut footed = Decimal::ZERO;
            for item in &order.items {
                money_scale(&format!("order item {} unit_price", item.id), item.unit_price)?;
                money_scale(&format!("order item {} total", item.id), item.total)?;
                let expected = (item.unit_price * Decimal::from(item.quantity) - item.discount
                    + item.tax_amount)
                    .round_dp(MONEY_SCALE);
                if item.total != expected {
                    return Err(format!(
                        "order item {} total {} != qty*price-discount+tax {}",
                        item.id, item.total, expected
                    ));
                }
                footed += item.total;
            }
            if order.total_amount != footed {
                return Err(format!(
                    "order {} total {} does not foot to Σ items {}",
                    mo.id, order.total_amount, footed
                ));
            }
            if order.total_amount != mo.total {
                return Err(format!(
                    "order {} total drifted: {} vs model {}",
                    mo.id, order.total_amount, mo.total
                ));
            }

            let order_payments = payments.for_order(mo.id).map_err(|e| e.to_string())?;
            let mut captured = Decimal::ZERO;
            let mut refunded = Decimal::ZERO;
            for p in &order_payments {
                if matches!(
                    p.status,
                    PaymentTransactionStatus::Completed
                        | PaymentTransactionStatus::PartiallyRefunded
                        | PaymentTransactionStatus::Refunded
                ) {
                    captured += p.amount;
                    refunded += p.amount_refunded;
                }
            }
            if captured > order.total_amount {
                return Err(format!(
                    "OVER-CAPTURE: order {} total {} but captured {}",
                    mo.id, order.total_amount, captured
                ));
            }
            if refunded > captured {
                return Err(format!(
                    "OVER-REFUND: order {} captured {} but refunded {}",
                    mo.id, captured, refunded
                ));
            }
            if captured != mo.captured {
                return Err(format!(
                    "order {} captured drifted: {} vs model {}",
                    mo.id, captured, mo.captured
                ));
            }

            // Returned units per line (non-rejected / non-cancelled returns) ≤ ordered.
            let mut returned: BTreeMap<OrderItemId, i32> = BTreeMap::new();
            for ret in returns.list_for_order(mo.id).map_err(|e| e.to_string())? {
                if matches!(ret.status, ReturnStatus::Rejected | ReturnStatus::Cancelled) {
                    continue;
                }
                if let Some(amount) = ret.refund_amount {
                    money_scale(&format!("return {} refund_amount", ret.id), amount)?;
                    let footed: Decimal = ret.items.iter().map(|i| i.refund_amount).sum();
                    if amount != footed {
                        return Err(format!(
                            "return {} refund_amount {} does not foot to Σ items {}",
                            ret.id, amount, footed
                        ));
                    }
                }
                for item in &ret.items {
                    money_scale(
                        &format!("return item {} refund_amount", item.id),
                        item.refund_amount,
                    )?;
                    *returned.entry(item.order_item_id).or_default() += item.quantity;
                }
            }
            for item in &order.items {
                let r = returned.get(&item.id).copied().unwrap_or(0);
                if r > item.quantity {
                    return Err(format!(
                        "OVER-RETURN: order item {} ordered {} but {} units returned",
                        item.id, item.quantity, r
                    ));
                }
            }
            if order.status == OrderStatus::Cancelled {
                let live = self
                    .commerce
                    .inventory()
                    .list_reservations_by_reference("order", &mo.id.to_string())
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .filter(|r| {
                        matches!(
                            r.status,
                            ReservationStatus::Pending
                                | ReservationStatus::Confirmed
                                | ReservationStatus::Allocated
                        )
                    })
                    .count();
                if live > 0 {
                    return Err(format!(
                        "order {} is cancelled but still holds {live} live reservations",
                        mo.id
                    ));
                }
            }
        }
        Ok(())
    }

    /// `on_hand` ≥ 0; `allocated` ≥ 0; `allocated` ≤ `on_hand`; `available` =
    /// `on_hand` − `allocated`; Σ movements = `on_hand`; `allocated` = Σ live
    /// reservations; `on_hand` matches the model.
    fn check_inventory(&self) -> Result<(), String> {
        let inventory = self.commerce.inventory();
        let mut live_reserved = [Decimal::ZERO; 3];
        for mo in &self.model.orders {
            for r in inventory
                .list_reservations_by_reference("order", &mo.id.to_string())
                .map_err(|e| e.to_string())?
            {
                if matches!(
                    r.status,
                    ReservationStatus::Pending
                        | ReservationStatus::Confirmed
                        | ReservationStatus::Allocated
                ) {
                    let idx = self
                        .item_ids
                        .iter()
                        .position(|id| *id == r.item_id)
                        .ok_or_else(|| format!("reservation on unknown item {}", r.item_id))?;
                    live_reserved[idx] += r.quantity;
                }
            }
        }
        for (idx, sku) in SKUS.iter().enumerate() {
            let stock = inventory
                .get_stock(sku)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("stock for {sku} vanished"))?;
            if stock.total_on_hand < Decimal::ZERO {
                return Err(format!("{sku}: on_hand {} < 0", stock.total_on_hand));
            }
            if stock.total_allocated < Decimal::ZERO {
                return Err(format!("{sku}: allocated {} < 0", stock.total_allocated));
            }
            if stock.total_allocated > stock.total_on_hand {
                return Err(format!(
                    "{sku}: allocated {} > on_hand {}",
                    stock.total_allocated, stock.total_on_hand
                ));
            }
            if stock.total_available != stock.total_on_hand - stock.total_allocated {
                return Err(format!(
                    "{sku}: available {} != on_hand {} - allocated {}",
                    stock.total_available, stock.total_on_hand, stock.total_allocated
                ));
            }
            for loc in &stock.locations {
                if loc.available != loc.on_hand - loc.allocated || loc.on_hand < Decimal::ZERO {
                    return Err(format!(
                        "{sku}@{}: location balance inconsistent {loc:?}",
                        loc.location_id
                    ));
                }
            }
            let movements: Decimal = inventory
                .get_transactions(self.item_ids[idx], u32::MAX)
                .map_err(|e| e.to_string())?
                .iter()
                .map(|t| t.quantity)
                .sum();
            if movements != stock.total_on_hand {
                return Err(format!(
                    "{sku}: Σ movements {} != on_hand {}",
                    movements, stock.total_on_hand
                ));
            }
            if stock.total_on_hand != self.model.on_hand[idx] {
                return Err(format!(
                    "{sku}: on_hand {} drifted from model {}",
                    stock.total_on_hand, self.model.on_hand[idx]
                ));
            }
            if stock.total_allocated != live_reserved[idx] {
                return Err(format!(
                    "{sku}: allocated {} != Σ live order reservations {}",
                    stock.total_allocated, live_reserved[idx]
                ));
            }
        }
        Ok(())
    }

    /// Every posted journal entry balances; trial balance nets to zero; AR
    /// control account = Σ open invoice balances; invoice balances foot.
    fn check_ledger(&self) -> Result<(), String> {
        let gl = self.commerce.general_ledger();
        let entries = gl
            .list_journal_entries(JournalEntryFilter { limit: Some(10_000), ..Default::default() })
            .map_err(|e| e.to_string())?;
        for entry in &entries {
            let lines = gl.get_journal_entry_lines(entry.id).map_err(|e| e.to_string())?;
            let debits: Decimal = lines.iter().map(|l| l.debit_amount).sum();
            let credits: Decimal = lines.iter().map(|l| l.credit_amount).sum();
            for l in &lines {
                money_scale(
                    &format!("JE {} line {} debit", entry.entry_number, l.line_number),
                    l.debit_amount,
                )?;
                money_scale(
                    &format!("JE {} line {} credit", entry.entry_number, l.line_number),
                    l.credit_amount,
                )?;
                if !l.is_valid() {
                    return Err(format!(
                        "JE {} line {} is not a pure debit or credit: {l:?}",
                        entry.entry_number, l.line_number
                    ));
                }
            }
            if entry.status == JournalEntryStatus::Posted && debits != credits {
                return Err(format!(
                    "UNBALANCED JE {}: Σ debits {} != Σ credits {}",
                    entry.entry_number, debits, credits
                ));
            }
            if entry.total_debits != debits || entry.total_credits != credits {
                return Err(format!(
                    "JE {} header totals ({}, {}) != line sums ({}, {})",
                    entry.entry_number, entry.total_debits, entry.total_credits, debits, credits
                ));
            }
        }
        let tb = gl.get_trial_balance(AS_OF).map_err(|e| e.to_string())?;
        if tb.total_debits != tb.total_credits || !tb.is_balanced {
            return Err(format!(
                "TRIAL BALANCE does not net to zero: debits {} credits {}",
                tb.total_debits, tb.total_credits
            ));
        }
        for line in &tb.lines {
            money_scale(&format!("TB {} debit", line.account_number), line.debit_balance)?;
            money_scale(&format!("TB {} credit", line.account_number), line.credit_balance)?;
        }

        let ar = gl
            .get_account_balance(self.ar_account_id, None)
            .map_err(|e| e.to_string())?
            .unwrap_or(Decimal::ZERO);
        let invoices = self.commerce.invoices();
        let mut open = Decimal::ZERO;
        for mi in &self.model.invoices {
            let inv = invoices
                .get(mi.id.into_uuid())
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("invoice {} vanished", mi.id))?;
            money_scale(&format!("invoice {} total", mi.id), inv.total)?;
            money_scale(&format!("invoice {} amount_paid", mi.id), inv.amount_paid)?;
            money_scale(&format!("invoice {} balance_due", mi.id), inv.balance_due)?;
            if inv.balance_due != inv.total - inv.amount_paid {
                return Err(format!(
                    "invoice {} balance_due {} != total {} - paid {}",
                    mi.id, inv.balance_due, inv.total, inv.amount_paid
                ));
            }
            if inv.total != mi.total || inv.amount_paid != mi.paid {
                return Err(format!(
                    "invoice {} drifted: db (total {}, paid {}) vs model ({}, {})",
                    mi.id, inv.total, inv.amount_paid, mi.total, mi.paid
                ));
            }
            open += inv.balance_due;
        }
        if ar != open {
            return Err(format!(
                "AR CONTROL: GL 1100 balance {ar} != Σ open invoice balances {open} (model {})",
                self.model.open_ar()
            ));
        }
        Ok(())
    }

    /// Entity counts match the model — a rejected op must not leave a partial row.
    fn check_counts(&self) -> Result<(), String> {
        let c = &self.commerce;
        let orders = c.orders().count(Default::default()).map_err(|e| e.to_string())?;
        if orders != self.model.orders.len() as u64 {
            return Err(format!("order count {orders} != model {}", self.model.orders.len()));
        }
        let payments = c.payments().count(Default::default()).map_err(|e| e.to_string())?;
        let expected = (self.model.payments.len() + self.model.invoice_payments) as u64;
        if payments != expected {
            return Err(format!("payment count {payments} != model {expected}"));
        }
        let returns = c.returns().count(Default::default()).map_err(|e| e.to_string())?;
        if returns != self.model.returns.len() as u64 {
            return Err(format!("return count {returns} != model {}", self.model.returns.len()));
        }
        let invoices = c.invoices().count(Default::default()).map_err(|e| e.to_string())?;
        if invoices != self.model.invoices.len() as u64 {
            return Err(format!("invoice count {invoices} != model {}", self.model.invoices.len()));
        }
        let mut refunds = 0u64;
        for mp in &self.model.payments {
            refunds += c.payments().get_refunds(mp.id).map_err(|e| e.to_string())?.len() as u64;
        }
        if refunds != self.model.refunds.len() as u64 {
            return Err(format!("refund count {refunds} != model {}", self.model.refunds.len()));
        }
        Ok(())
    }
}

/// No money value may carry more decimal places than the currency allows.
fn money_scale(what: &str, value: Decimal) -> Result<(), String> {
    if value.normalize().scale() > MONEY_SCALE {
        return Err(format!("MONEY SCALE: {what} = {value} has more than {MONEY_SCALE} decimals"));
    }
    Ok(())
}

/// Run a sequence, checking invariants after every op. Returns the failing
/// step and message so proptest can shrink on it.
fn run_sequence(ops: &[Op]) -> Result<Harness, String> {
    let mut h = Harness::new();
    h.check_invariants().map_err(|e| format!("invariant violated before any op: {e}"))?;
    for (step, op) in ops.iter().enumerate() {
        h.apply(op)?;
        h.check_invariants().map_err(|e| format!("after step {step} {op:?}: {e}"))?;
    }
    Ok(h)
}

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES").ok().and_then(|v| v.parse().ok()).unwrap_or(64)
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), max_shrink_iters: 2_000, ..ProptestConfig::default() })]

    /// The books are right after every step of any valid op sequence.
    #[test]
    fn prop_books_balance_after_every_op(
        ops in proptest::collection::vec(op_strategy(), OPS_MIN..=OPS_MAX)
    ) {
        if let Err(msg) = run_sequence(&ops) {
            prop_assert!(false, "{msg}");
        }
    }
}

// ===========================================================================
// Deterministic regressions — one hand-written sequence per invariant
// ===========================================================================

fn order(lines: &[(u8, u8, u16)]) -> Op {
    Op::CreateOrder { lines: lines.to_vec() }
}

#[test]
fn regression_refunds_never_exceed_capture_including_in_flight() {
    // Capture 100%, then request 60% + 60% (second must be rejected), complete,
    // then try 100% of what is left (40%) — never more than captured.
    let ops = [
        order(&[(0, 2, 1_999)]),
        Op::CapturePayment { order: 0, pct: 100 },
        Op::RequestRefund { payment: 0, pct: 60 },
        Op::RequestRefund { payment: 0, pct: 100 },
        Op::CompleteRefund { refund: 0 },
        Op::CompleteRefund { refund: 1 },
        Op::RequestRefund { payment: 0, pct: 100 },
        Op::CompleteRefund { refund: 2 },
        Op::FailRefund { refund: 2 },
    ];
    let h = run_sequence(&ops).unwrap_or_else(|e| panic!("{e}"));
    let p = &h.model.payments[0];
    assert_eq!(p.amount, dec!(39.98));
    assert_eq!(p.refunded, dec!(39.98), "60% + remaining 40% fully refunded");
    assert_eq!(p.in_flight, Decimal::ZERO);
}

#[test]
fn regression_engine_rejects_refund_beyond_remaining_balance() {
    let mut h = Harness::new();
    h.apply(&order(&[(0, 1, 10_000)])).unwrap_or_else(|e| panic!("{e}"));
    h.apply(&Op::CapturePayment { order: 0, pct: 100 }).unwrap_or_else(|e| panic!("{e}"));
    let payment_id = h.model.payments[0].id;
    // Pending refund reserves 70; a second 70 must fail with a typed error.
    let first = h
        .commerce
        .payments()
        .create_refund(CreateRefund { payment_id, amount: Some(dec!(70)), ..Default::default() })
        .expect("first refund");
    let err = h
        .commerce
        .payments()
        .create_refund(CreateRefund { payment_id, amount: Some(dec!(70)), ..Default::default() })
        .expect_err("second refund must be rejected while the first is in flight");
    assert!(matches!(err, CommerceError::RefundExceedsCaptured { .. }), "{err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.refund.exceeds_captured"));
    h.model.payments[0].in_flight = dec!(70);
    h.model.refunds.push(MRefund {
        id: first.id,
        payment: 0,
        amount: dec!(70),
        status: RefundStatus::Pending,
    });
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));
}

/// Regression for the `fail_refund` bug found by the harness: failing a
/// COMPLETED refund used to flip it to `failed` while the payment kept the
/// money in `amount_refunded`, so Σ completed refunds no longer matched.
#[test]
fn regression_failing_a_completed_refund_is_rejected() {
    let mut h = Harness::new();
    h.apply(&order(&[(0, 1, 5_000)])).unwrap_or_else(|e| panic!("{e}"));
    h.apply(&Op::CapturePayment { order: 0, pct: 100 }).unwrap_or_else(|e| panic!("{e}"));
    h.apply(&Op::RequestRefund { payment: 0, pct: 50 }).unwrap_or_else(|e| panic!("{e}"));
    h.apply(&Op::CompleteRefund { refund: 0 }).unwrap_or_else(|e| panic!("{e}"));
    let refund_id = h.model.refunds[0].id;
    let err = h
        .commerce
        .payments()
        .fail_refund(refund_id, "too late")
        .expect_err("a completed refund cannot fail");
    assert!(matches!(err, CommerceError::ValidationError(_)), "{err:?}");
    let payment = h.commerce.payments().get(h.model.payments[0].id).expect("get").expect("exists");
    assert_eq!(payment.amount_refunded, dec!(25.00));
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));
    // Failing an already-failed refund stays an idempotent no-op.
    h.apply(&Op::RequestRefund { payment: 0, pct: 100 }).unwrap_or_else(|e| panic!("{e}"));
    let second = h.model.refunds[1].id;
    h.commerce.payments().fail_refund(second, "first").expect("fail pending");
    h.commerce.payments().fail_refund(second, "again").expect("idempotent");
    h.model.refunds[1].status = RefundStatus::Failed;
    h.model.payments[0].in_flight = Decimal::ZERO;
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn regression_captured_never_exceeds_total_and_partial_captures_foot() {
    let ops = [
        order(&[(0, 3, 3_333), (1, 1, 1)]),
        Op::CapturePayment { order: 0, pct: 33 },
        Op::CapturePayment { order: 0, pct: 50 },
        Op::CapturePayment { order: 0, pct: 100 },
        Op::CapturePayment { order: 0, pct: 100 },
    ];
    let h = run_sequence(&ops).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.model.orders[0].total, dec!(100.00));
    assert_eq!(h.model.orders[0].captured, dec!(100.00));
    assert_eq!(h.model.payments.len(), 3, "the fourth capture has nothing left to capture");
}

#[test]
fn regression_returned_quantity_never_exceeds_ordered() {
    let ops = [
        order(&[(2, 4, 500)]),
        Op::Ship { order: 0 },
        Op::RequestReturn { order: 0, line: 0, qty: 3 },
        // 3 + 2 > 4: must be rejected.
        Op::RequestReturn { order: 0, line: 0, qty: 2 },
        // Rejecting the first releases its claim, so 4 becomes returnable again.
        Op::RejectReturn { ret: 0 },
        Op::RequestReturn { order: 0, line: 0, qty: 4 },
        Op::AdvanceReturn { ret: 1 },
        Op::AdvanceReturn { ret: 1 },
        Op::AdvanceReturn { ret: 1 },
        Op::AdvanceReturn { ret: 1 },
    ];
    let h = run_sequence(&ops).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.model.returns.len(), 2);
    let last = h.commerce.returns().get(h.model.returns[1]).expect("get").expect("exists");
    assert_eq!(last.status, ReturnStatus::Completed);
    assert_eq!(last.refund_amount, Some(dec!(20.00)));
}

#[test]
fn regression_inventory_reconciles_through_reserve_ship_cancel() {
    let ops = [
        Op::ReceiveStock { sku: 0, qty: 5 },
        order(&[(0, 12, 100)]),              // 15 on hand: reserves 12
        order(&[(0, 5, 100)]),               // 3 available: reserves 3, backorders 2
        Op::RemoveStock { sku: 0, qty: 15 }, // would go negative: rejected (on_hand 15, but must stay ≥ allocated? engine only guards on_hand)
        Op::Ship { order: 0 },
        Op::CancelOrder { order: 1 },
        Op::CancelOrder { order: 0 }, // shipped: rejected
        Op::RemoveStock { sku: 0, qty: 50 },
    ];
    let h = run_sequence(&ops).unwrap_or_else(|e| panic!("{e}"));
    let stock = h.commerce.inventory().get_stock(SKUS[0]).expect("stock").expect("exists");
    assert_eq!(stock.total_on_hand, h.model.on_hand[0]);
    assert!(stock.total_allocated <= stock.total_on_hand);
}

#[test]
fn regression_ledger_balances_and_ar_control_equals_open_invoices() {
    let ops = [
        order(&[(1, 2, 12_345)]),
        Op::PostInvoice { order: 0 },
        Op::PostInvoice { order: 0 }, // already invoiced: no-op
        Op::PayInvoice { invoice: 0, pct: 40 },
        order(&[(0, 1, 999)]),
        Op::PostInvoice { order: 1 },
        Op::PayInvoice { invoice: 0, pct: 100 },
        Op::PayInvoice { invoice: 0, pct: 100 }, // nothing left
    ];
    let h = run_sequence(&ops).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.model.invoices.len(), 2);
    assert_eq!(h.model.open_ar(), dec!(9.99));
    let ar = h
        .commerce
        .general_ledger()
        .get_account_balance(h.ar_account_id, None)
        .expect("balance")
        .unwrap_or_default();
    assert_eq!(ar, dec!(9.99));
}

#[test]
fn regression_money_never_exceeds_currency_scale() {
    // Prices with cents, odd quantities and percentage captures/refunds all
    // round at the write boundary.
    let ops = [
        order(&[(0, 3, 3_333), (1, 7, 1_001), (2, 1, 1)]),
        Op::CapturePayment { order: 0, pct: 33 },
        Op::CapturePayment { order: 0, pct: 67 },
        Op::RequestRefund { payment: 0, pct: 33 },
        Op::CompleteRefund { refund: 0 },
        Op::RequestReturn { order: 0, line: 1, qty: 3 },
        Op::PostInvoice { order: 0 },
        Op::PayInvoice { invoice: 0, pct: 7 },
    ];
    run_sequence(&ops).unwrap_or_else(|e| panic!("{e}"));
}

/// M1 has an engine guard on order creation: an over-scaled `unit_price` is
/// refused with `commerce.money.scale_exceeds_currency` and nothing is written.
///
/// Trailing zeros are insignificant, so `10.9900` must still be accepted for
/// USD — the scale bound is on precision, not on characters.
#[test]
fn regression_engine_rejects_over_scaled_order_money() {
    let h = Harness::new();
    let before = h.commerce.orders().list(Default::default()).expect("list").len();

    for (field, item) in [
        ("unit_price", CreateOrderItem { unit_price: dec!(10.999), ..Default::default() }),
        (
            "discount",
            CreateOrderItem {
                unit_price: dec!(10.00),
                discount: Some(dec!(0.005)),
                ..Default::default()
            },
        ),
        (
            "tax_amount",
            CreateOrderItem {
                unit_price: dec!(10.00),
                tax_amount: Some(dec!(0.875)),
                ..Default::default()
            },
        ),
    ] {
        let item = CreateOrderItem {
            product_id: ProductId::new(),
            sku: SKUS[0].into(),
            name: "Over-scaled".into(),
            quantity: 1,
            ..item
        };
        let err = h
            .commerce
            .orders()
            .create(CreateOrder {
                customer_id: h.customer_id,
                items: vec![item],
                ..Default::default()
            })
            .expect_err("over-scaled money must be rejected");
        assert_eq!(
            err.invariant_code(),
            Some("commerce.money.scale_exceeds_currency"),
            "over-scaled {field} rejected with the wrong code: {err:?}"
        );
    }

    // A1: nothing was written by any of the three rejections.
    assert_eq!(
        h.commerce.orders().list(Default::default()).expect("list").len(),
        before,
        "a rejected over-scaled order still wrote an order row"
    );
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));

    // Insignificant trailing zeros are fine: 10.9900 is two-scale USD.
    let order = h
        .commerce
        .orders()
        .create(CreateOrder {
            customer_id: h.customer_id,
            items: vec![CreateOrderItem {
                product_id: ProductId::new(),
                sku: SKUS[0].into(),
                name: "Trailing zeros".into(),
                quantity: 2,
                unit_price: dec!(10.9900),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("10.9900 is two significant decimals and must be accepted for USD");
    assert_eq!(order.total_amount, dec!(21.98));
}

#[test]
fn regression_failed_ops_are_typed_errors_and_leave_books_intact() {
    let mut h = Harness::new();
    h.apply(&order(&[(0, 1, 5_000)])).unwrap_or_else(|e| panic!("{e}"));
    h.apply(&Op::Ship { order: 0 }).unwrap_or_else(|e| panic!("{e}"));
    let before = h.commerce.orders().get(h.model.orders[0].id).expect("get").expect("exists");

    // Each of these must be a typed rejection, not a panic, and must not move the books.
    let err = h.commerce.orders().cancel(before.id).expect_err("shipped order cannot be cancelled");
    assert!(matches!(err, CommerceError::OrderCannotBeCancelled(_)), "{err:?}");
    let err = h
        .commerce
        .inventory()
        .adjust(SKUS[0], dec!(-1000), "impossible")
        .expect_err("cannot go negative");
    assert!(matches!(err, CommerceError::InsufficientStock { .. }), "{err:?}");
    let err = h
        .commerce
        .returns()
        .create(CreateReturn {
            order_id: before.id,
            reason: ReturnReason::Damaged,
            items: vec![CreateReturnItem {
                order_item_id: before.items[0].id,
                quantity: 2,
                condition: None,
            }],
            ..Default::default()
        })
        .expect_err("cannot return more than ordered");
    assert!(err.invariant_code().is_some_and(|c| c.starts_with("commerce.return.")), "{err:?}");
    let err = h
        .commerce
        .payments()
        .create_refund(CreateRefund { payment_id: PaymentId::new(), ..Default::default() })
        .expect_err("unknown payment");
    assert!(matches!(err, CommerceError::NotFound), "{err:?}");

    let after = h.commerce.orders().get(before.id).expect("get").expect("exists");
    assert_eq!(after.version, before.version, "rejected ops must not bump the order version");
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));
}

/// Returns require shipped goods: a never-shipped (Pending) order rejects a
/// return with a typed error and no row is written; the same return succeeds
/// once the order ships.
#[test]
fn regression_returns_require_shipped_units() {
    let mut h = Harness::new();
    h.apply(&order(&[(0, 2, 1_000)])).unwrap_or_else(|e| panic!("{e}"));
    let order_id = h.model.orders[0].id;
    let item_id = h.model.orders[0].lines[0].item_id;
    let input = CreateReturn {
        order_id,
        reason: ReturnReason::ChangedMind,
        items: vec![CreateReturnItem { order_item_id: item_id, quantity: 2, condition: None }],
        ..Default::default()
    };
    let err = h
        .commerce
        .returns()
        .create(input.clone())
        .expect_err("return against a never-shipped order must be rejected");
    assert!(matches!(err, CommerceError::ReturnOrderNotShipped { .. }), "{err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.return.order_not_shipped"));
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));

    // Cancelled orders are closed too.
    h.apply(&order(&[(1, 1, 500)])).unwrap_or_else(|e| panic!("{e}"));
    h.apply(&Op::CancelOrder { order: 1 }).unwrap_or_else(|e| panic!("{e}"));
    let cancelled = CreateReturn {
        order_id: h.model.orders[1].id,
        items: vec![CreateReturnItem {
            order_item_id: h.model.orders[1].lines[0].item_id,
            quantity: 1,
            condition: None,
        }],
        ..input.clone()
    };
    assert!(h.commerce.returns().create(cancelled).is_err());

    // Once shipped, the very same return is accepted.
    h.apply(&Op::Ship { order: 0 }).unwrap_or_else(|e| panic!("{e}"));
    let ret = h.commerce.returns().create(input).expect("return after shipment");
    h.model.returns.push(ret.id);
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));
    // The random generator must also see this: unshipped -> rejected, shipped -> accepted.
    h.apply(&Op::RequestReturn { order: 1, line: 0, qty: 1 }).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(h.model.returns.len(), 1, "return on the cancelled order must not be recorded");
}

/// Over-capture guard: Σ(in-flight + completed) captures can never exceed the
/// order total, at create time and at completion time, and a rejected capture
/// writes nothing.
#[test]
fn regression_engine_rejects_capture_beyond_order_total() {
    let mut h = Harness::new();
    h.apply(&order(&[(0, 1, 1_000)])).unwrap_or_else(|e| panic!("{e}"));
    let order_id = h.model.orders[0].id;
    let payments = h.commerce.payments();
    let capture =
        |amount: Decimal| CreatePayment { order_id: Some(order_id), amount, ..Default::default() };

    // 25.00 against a 10.00 order: rejected at create.
    let err = payments.create(capture(dec!(25.00))).expect_err("over-capture must be rejected");
    assert!(matches!(err, CommerceError::CaptureExceedsOrderTotal { .. }), "{err:?}");
    assert_eq!(err.invariant_code(), Some("commerce.capture.exceeds_order_total"));
    assert!(payments.for_order(order_id).expect("list").is_empty(), "rejected capture wrote a row");

    // 6.00 pending (in flight) + 5.00 > 10.00: rejected even though nothing is completed yet.
    let first = payments.create(capture(dec!(6.00))).expect("6.00 fits");
    let err = payments.create(capture(dec!(5.00))).expect_err("in-flight captures count");
    assert_eq!(err.invariant_code(), Some("commerce.capture.exceeds_order_total"), "{err:?}");
    // Exactly the remainder fits.
    let second = payments.create(capture(dec!(4.00))).expect("4.00 fits exactly");
    payments.mark_completed(first.id).expect("complete first");
    payments.mark_completed(second.id).expect("complete second");

    // A failed payment releases its slice; re-completing it later must re-check.
    let third = payments
        .create(capture(dec!(0.01)))
        .expect_err("order fully captured; even a cent is over");
    assert_eq!(third.invariant_code(), Some("commerce.capture.exceeds_order_total"), "{third:?}");
    payments.mark_failed(first.id, "declined", None).expect("fail first");
    let refill = payments.create(capture(dec!(6.00))).expect("released slice is reusable");
    payments.mark_completed(refill.id).expect("complete refill");
    let err = payments
        .mark_completed(first.id)
        .expect_err("re-completing the failed one would over-capture");
    assert_eq!(err.invariant_code(), Some("commerce.capture.exceeds_order_total"), "{err:?}");

    h.model.orders[0].captured = dec!(10.00);
    for p in [second, refill] {
        h.model.payments.push(MPayment {
            id: p.id,
            amount: p.amount,
            refunded: Decimal::ZERO,
            in_flight: Decimal::ZERO,
        });
    }
    // `first` is Failed; it is excluded from captured but still a row.
    h.model.invoice_payments += 1;
    h.check_invariants().unwrap_or_else(|e| panic!("{e}"));

    // The random generator's over-capture op is rejected too.
    h.apply(&Op::OverCapture { order: 0, extra_cents: 1 }).unwrap_or_else(|e| panic!("{e}"));
}
