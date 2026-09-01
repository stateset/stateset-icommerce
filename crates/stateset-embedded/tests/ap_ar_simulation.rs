//! Randomized AP/AR burn-in simulation against the SQLite backend.
//!
//! Drives ~200 random accounts-payable and accounts-receivable operations —
//! bill create/approve/pay/item-edit/cancel/dispute, invoice create, direct
//! payments, payment applications, credit-memo applications, write-offs and
//! their reversals — and asserts after EVERY operation the money invariants
//! for every bill (`amount_due == total_amount - amount_paid`, both
//! non-negative) and every invoice (`balance_due == total - amount_paid`,
//! `0 <= amount_paid <= total`, status consistent with the amounts).
//!
//! Operations that legitimately hit an engine guard (item edits on non-draft
//! bills, payments to written-off invoices, …) are tolerated and counted, not
//! failed. At the end, each customer's statement over a window covering the
//! whole run must reconcile against an independently tracked shadow model:
//! opening balance zero and a final running balance of
//! `invoices - applied payments - applied credits - non-reversed write-offs`.
//!
//! Reproducibility: the operation stream is driven by a seeded deterministic
//! PRNG. Override the seed with the `AP_AR_SIM_SEED` env var (u64) to explore
//! other trajectories; the default is fixed so CI runs are stable.

// Uses the sync `Commerce` engine, which only exists with the sqlite backend.
#![cfg(feature = "sqlite")]

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal_macros::dec;
use stateset_embedded::{
    ApplyCreditMemo, ApplyPaymentToInvoices, BillStatus, Commerce, CommerceError, CreateBill,
    CreateBillItem, CreateBillPayment, CreateCreditMemo, CreateCustomer, CreateInvoice,
    CreateInvoiceItem, CreatePayment, CreateWriteOff, CreditMemoReason, CustomerId,
    GenerateStatementRequest, InvoiceStatus, PaymentAllocationInput, PaymentApplicationLine,
    PaymentMethodAP, RecordInvoicePayment, WriteOffReason,
};
use uuid::Uuid;

const DEFAULT_SEED: u64 = 0xAB1E_5EED_2026_0831;
const OPERATIONS: usize = 200;
const CUSTOMERS: usize = 2;

/// Deterministic splitmix64-style PRNG — no external dependency, fully
/// reproducible from the seed (same generator as `ledger_simulation.rs`).
struct Rng(u64);

impl Rng {
    const fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform value in `[0, bound)`.
    const fn below(&mut self, bound: u64) -> u64 {
        self.next_u64() % bound
    }

    /// Uniform value in `[lo, hi]` (inclusive).
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let span = u64::try_from(hi - lo).expect("valid range") + 1;
        lo + i64::try_from(self.below(span)).expect("fits")
    }

    /// Random money amount with 2 decimal places in `[lo_cents, hi_cents]`.
    fn money(&mut self, lo_cents: i64, hi_cents: i64) -> Decimal {
        Decimal::new(self.range(lo_cents, hi_cents), 2)
    }
}

/// Exact cents of a 2-decimal money amount.
fn cents(amount: Decimal) -> i64 {
    (amount * dec!(100)).to_i64().expect("money amount fits in cents")
}

/// Independently tracked statement-visible activity for one customer.
#[derive(Default)]
struct CustomerShadow {
    invoices_total: Decimal,
    applied_payments: Decimal,
    applied_credits: Decimal,
    /// Non-reversed write-off total (reversals subtract back out).
    write_offs: Decimal,
}

struct Sim {
    commerce: Commerce,
    rng: Rng,
    supplier: Uuid,
    bills: Vec<Uuid>,
    /// `(invoice id, customer index)`
    invoices: Vec<(Uuid, usize)>,
    customer_ids: Vec<CustomerId>,
    shadows: Vec<CustomerShadow>,
    /// Reversible write-offs: `(write-off id, amount, customer index)`.
    open_write_offs: Vec<(Uuid, Decimal, usize)>,
    guard_rejections: usize,
    counts: [usize; 13],
}

/// Bill statuses that accept a payment allocation.
const fn bill_is_payable(status: BillStatus) -> bool {
    matches!(status, BillStatus::Approved | BillStatus::PartiallyPaid | BillStatus::Overdue)
}

/// Invoice statuses that are still open for payments / credits / write-offs.
const fn invoice_is_open(status: InvoiceStatus) -> bool {
    matches!(
        status,
        InvoiceStatus::Draft
            | InvoiceStatus::Sent
            | InvoiceStatus::Viewed
            | InvoiceStatus::PartiallyPaid
            | InvoiceStatus::Overdue
    )
}

impl Sim {
    fn new(seed: u64) -> Self {
        let commerce = Commerce::new(":memory:").expect("create in-memory Commerce");
        let customer_ids: Vec<CustomerId> = (0..CUSTOMERS)
            .map(|i| {
                commerce
                    .customers()
                    .create(CreateCustomer {
                        email: format!("ap-ar-sim-{i}@example.com"),
                        first_name: "Sim".into(),
                        last_name: format!("Customer{i}"),
                        ..Default::default()
                    })
                    .expect("create customer")
                    .id
            })
            .collect();
        Self {
            commerce,
            rng: Rng(seed),
            supplier: Uuid::new_v4(),
            bills: Vec::new(),
            invoices: Vec::new(),
            customer_ids,
            shadows: (0..CUSTOMERS).map(|_| CustomerShadow::default()).collect(),
            open_write_offs: Vec::new(),
            guard_rejections: 0,
            counts: [0; 13],
        }
    }

    /// Tolerate an engine guard rejection (`ValidationError` / `Conflict`) by
    /// counting it; any other error is a real failure. Returns the success
    /// payload when the operation went through.
    fn tolerate<T>(&mut self, context: &str, result: Result<T, CommerceError>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(CommerceError::ValidationError(_) | CommerceError::Conflict(_)) => {
                self.guard_rejections += 1;
                None
            }
            Err(e) => panic!("{context}: unexpected error {e:?}"),
        }
    }

    fn random_item(&mut self) -> CreateBillItem {
        CreateBillItem {
            description: "Sim widget".into(),
            account_code: Some("5000".into()),
            quantity: Decimal::from(self.rng.range(1, 5)),
            unit_price: self.rng.money(100, 20_000), // $1 .. $200
            tax_rate: None,
            po_line_id: None,
        }
    }

    fn pick_bill(&mut self) -> Option<Uuid> {
        if self.bills.is_empty() {
            return None;
        }
        let i = usize::try_from(self.rng.below(self.bills.len() as u64)).expect("index");
        Some(self.bills[i])
    }

    fn pick_invoice(&mut self) -> Option<(Uuid, usize)> {
        if self.invoices.is_empty() {
            return None;
        }
        let i = usize::try_from(self.rng.below(self.invoices.len() as u64)).expect("index");
        Some(self.invoices[i])
    }

    fn get_bill(&self, id: Uuid) -> stateset_embedded::Bill {
        self.commerce.accounts_payable().get_bill(id).expect("get bill").expect("bill exists")
    }

    fn get_invoice(&self, id: Uuid) -> stateset_embedded::Invoice {
        self.commerce.invoices().get(id).expect("get invoice").expect("invoice exists")
    }

    // ==================================================================
    // AP operations
    // ==================================================================

    fn op_create_bill(&mut self) {
        let item_count = self.rng.range(1, 3);
        let items = (0..item_count).map(|_| self.random_item()).collect();
        let bill = self
            .commerce
            .accounts_payable()
            .create_bill(CreateBill {
                supplier_id: self.supplier,
                due_date: Utc::now() + Duration::days(30),
                items,
                ..Default::default()
            })
            .expect("create bill");
        self.bills.push(bill.id);
    }

    fn op_approve_bill(&mut self) {
        let Some(id) = self.pick_bill() else { return self.op_create_bill() };
        let result = self.commerce.accounts_payable().approve_bill(id);
        self.tolerate("approve_bill", result);
    }

    fn op_pay_bill(&mut self) {
        let Some(id) = self.pick_bill() else { return self.op_create_bill() };
        let bill = self.get_bill(id);
        if !bill_is_payable(bill.status) || bill.amount_due <= Decimal::ZERO {
            return;
        }
        let due_cents = cents(bill.amount_due);
        // Full payoff a third of the time, otherwise a random partial amount.
        let amount = if self.rng.below(3) == 0 {
            bill.amount_due
        } else {
            Decimal::new(self.rng.range(1, due_cents), 2)
        };
        let result = self.commerce.accounts_payable().create_payment(CreateBillPayment {
            supplier_id: self.supplier,
            payment_date: None,
            payment_method: PaymentMethodAP::Ach,
            amount,
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: id, amount }],
        });
        self.tolerate("create_payment (AP)", result);
    }

    fn op_add_bill_item(&mut self) {
        let Some(id) = self.pick_bill() else { return self.op_create_bill() };
        let item = self.random_item();
        let result = self.commerce.accounts_payable().add_bill_item(id, item);
        self.tolerate("add_bill_item", result);
    }

    fn op_remove_bill_item(&mut self) {
        let Some(id) = self.pick_bill() else { return self.op_create_bill() };
        let items = self.commerce.accounts_payable().get_bill_items(id).expect("get bill items");
        if items.is_empty() {
            return;
        }
        let i = usize::try_from(self.rng.below(items.len() as u64)).expect("index");
        let result = self.commerce.accounts_payable().remove_bill_item(items[i].id);
        self.tolerate("remove_bill_item", result);
    }

    fn op_cancel_bill(&mut self) {
        let Some(id) = self.pick_bill() else { return self.op_create_bill() };
        let result = self.commerce.accounts_payable().cancel_bill(id);
        self.tolerate("cancel_bill", result);
    }

    fn op_dispute_bill(&mut self) {
        let Some(id) = self.pick_bill() else { return self.op_create_bill() };
        let result = self.commerce.accounts_payable().dispute_bill(id);
        self.tolerate("dispute_bill", result);
    }

    // ==================================================================
    // AR operations
    // ==================================================================

    fn op_create_invoice(&mut self) {
        let cust = usize::try_from(self.rng.below(CUSTOMERS as u64)).expect("index");
        let item_count = self.rng.range(1, 3);
        let items = (0..item_count)
            .map(|i| CreateInvoiceItem {
                description: format!("Sim line {i}"),
                quantity: Decimal::from(self.rng.range(1, 5)),
                unit_price: self.rng.money(100, 20_000),
                ..Default::default()
            })
            .collect();
        let invoice = self
            .commerce
            .invoices()
            .create(CreateInvoice {
                customer_id: self.customer_ids[cust],
                items,
                ..Default::default()
            })
            .expect("create invoice");
        // Shadow from the engine-reported total (covers any default tax or
        // rounding the engine applies to the raw line inputs).
        self.shadows[cust].invoices_total += invoice.total;
        self.invoices.push((invoice.id.into(), cust));
    }

    fn op_direct_payment(&mut self) {
        let Some((id, _)) = self.pick_invoice() else { return self.op_create_invoice() };
        let invoice = self.get_invoice(id);
        if !invoice_is_open(invoice.status) || invoice.balance_due <= Decimal::ZERO {
            return;
        }
        let amount = Decimal::new(self.rng.range(1, cents(invoice.balance_due)), 2);
        let result = self
            .commerce
            .invoices()
            .record_payment(id, RecordInvoicePayment { amount, ..Default::default() });
        self.tolerate("record_payment (direct)", result);
        // NOTE: direct payments are deliberately NOT statement-visible (no
        // ar_payment_applications row), so they do not enter the shadow.
    }

    fn op_apply_payment(&mut self) {
        let Some((id, cust)) = self.pick_invoice() else { return self.op_create_invoice() };
        let invoice = self.get_invoice(id);
        if !invoice_is_open(invoice.status) || invoice.balance_due <= Decimal::ZERO {
            return;
        }
        let amount = Decimal::new(self.rng.range(1, cents(invoice.balance_due)), 2);
        let payment_id: Uuid = self
            .commerce
            .payments()
            .create(CreatePayment {
                customer_id: Some(self.customer_ids[cust]),
                amount,
                ..Default::default()
            })
            .expect("create AR payment")
            .id
            .into();
        let result =
            self.commerce.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
                payment_id,
                applications: vec![PaymentApplicationLine { invoice_id: id, amount }],
            });
        if self.tolerate("apply_payment_to_invoices", result).is_some() {
            self.shadows[cust].applied_payments += amount;
        }
    }

    fn op_apply_credit_memo(&mut self) {
        let Some((id, cust)) = self.pick_invoice() else { return self.op_create_invoice() };
        let invoice = self.get_invoice(id);
        if !invoice_is_open(invoice.status) || invoice.balance_due <= Decimal::ZERO {
            return;
        }
        let amount = Decimal::new(self.rng.range(1, cents(invoice.balance_due)), 2);
        let memo = self
            .commerce
            .accounts_receivable()
            .create_credit_memo(CreateCreditMemo {
                customer_id: self.customer_ids[cust].into(),
                amount,
                reason: CreditMemoReason::ServiceCredit,
                original_invoice_id: None,
                notes: None,
            })
            .expect("create credit memo");
        let result = self.commerce.accounts_receivable().apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: memo.id,
            invoice_id: id,
            amount,
        });
        if self.tolerate("apply_credit_memo", result).is_some() {
            self.shadows[cust].applied_credits += amount;
        }
    }

    fn op_write_off(&mut self) {
        let Some((id, cust)) = self.pick_invoice() else { return self.op_create_invoice() };
        let invoice = self.get_invoice(id);
        if invoice.balance_due <= Decimal::ZERO
            || matches!(invoice.status, InvoiceStatus::Voided | InvoiceStatus::WrittenOff)
        {
            return;
        }
        // Full balance half the time, otherwise a random partial amount.
        let amount = if self.rng.below(2) == 0 {
            invoice.balance_due
        } else {
            Decimal::new(self.rng.range(1, cents(invoice.balance_due)), 2)
        };
        let result = self.commerce.accounts_receivable().create_write_off(CreateWriteOff {
            invoice_id: id,
            amount,
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: None,
        });
        if let Some(wo) = self.tolerate("create_write_off", result) {
            self.shadows[cust].write_offs += amount;
            self.open_write_offs.push((wo.id, amount, cust));
        }
    }

    fn op_reverse_write_off(&mut self) {
        if self.open_write_offs.is_empty() {
            return;
        }
        let i = usize::try_from(self.rng.below(self.open_write_offs.len() as u64)).expect("index");
        let (wo_id, amount, cust) = self.open_write_offs.swap_remove(i);
        self.commerce.accounts_receivable().reverse_write_off(wo_id).expect("reverse write-off");
        self.shadows[cust].write_offs -= amount;
    }

    // ==================================================================
    // Invariants
    // ==================================================================

    fn assert_invariants(&self, context: &str) {
        for &bill_id in &self.bills {
            let b = self.get_bill(bill_id);
            assert_eq!(
                b.amount_due,
                b.total_amount - b.amount_paid,
                "{context}: bill {bill_id} amount_due != total - paid \
                 (total {}, paid {}, due {}, status {:?})",
                b.total_amount,
                b.amount_paid,
                b.amount_due,
                b.status
            );
            assert!(
                b.amount_due >= Decimal::ZERO,
                "{context}: bill {bill_id} has negative amount_due {}",
                b.amount_due
            );
            assert!(
                b.amount_paid >= Decimal::ZERO,
                "{context}: bill {bill_id} has negative amount_paid {}",
                b.amount_paid
            );
            if b.status == BillStatus::Paid {
                assert_eq!(
                    b.amount_due,
                    Decimal::ZERO,
                    "{context}: bill {bill_id} is paid but still owes {}",
                    b.amount_due
                );
            }
            if b.status == BillStatus::PartiallyPaid {
                assert!(
                    b.amount_paid > Decimal::ZERO && b.amount_due > Decimal::ZERO,
                    "{context}: bill {bill_id} partially_paid but paid {} / due {}",
                    b.amount_paid,
                    b.amount_due
                );
            }
        }

        for &(invoice_id, _) in &self.invoices {
            let inv = self.get_invoice(invoice_id);
            assert_eq!(
                inv.balance_due,
                inv.total - inv.amount_paid,
                "{context}: invoice {invoice_id} balance_due != total - paid \
                 (total {}, paid {}, balance {}, status {:?})",
                inv.total,
                inv.amount_paid,
                inv.balance_due,
                inv.status
            );
            assert!(
                inv.amount_paid >= Decimal::ZERO,
                "{context}: invoice {invoice_id} has negative amount_paid {}",
                inv.amount_paid
            );
            assert!(
                inv.amount_paid <= inv.total,
                "{context}: invoice {invoice_id} over-paid: paid {} > total {}",
                inv.amount_paid,
                inv.total
            );
            // What the engine promises (see recalculate_invoice_with_conn and
            // record_payment): paid <=> balance cleared with money received;
            // partially_paid requires money received AND money still owed.
            if inv.status == InvoiceStatus::Paid {
                assert!(
                    inv.balance_due == Decimal::ZERO && inv.amount_paid > Decimal::ZERO,
                    "{context}: invoice {invoice_id} paid but balance {} / paid {}",
                    inv.balance_due,
                    inv.amount_paid
                );
            }
            if inv.status == InvoiceStatus::PartiallyPaid {
                assert!(
                    inv.amount_paid > Decimal::ZERO && inv.balance_due > Decimal::ZERO,
                    "{context}: invoice {invoice_id} partially_paid but paid {} / balance {}",
                    inv.amount_paid,
                    inv.balance_due
                );
            }
        }
    }

    fn step(&mut self, index: usize) {
        // Weighted op selection over 13 operation kinds.
        let roll = self.rng.below(100);
        let op = match roll {
            0..=9 => 0,    // create bill
            10..=19 => 1,  // approve bill
            20..=31 => 2,  // pay bill
            32..=37 => 3,  // add bill item
            38..=43 => 4,  // remove bill item
            44..=47 => 5,  // cancel bill
            48..=51 => 6,  // dispute bill
            52..=63 => 7,  // create invoice
            64..=73 => 8,  // direct payment
            74..=85 => 9,  // create+apply payment
            86..=93 => 10, // create+apply credit memo
            94..=97 => 11, // write off
            _ => 12,       // reverse write off
        };
        self.counts[op] += 1;
        match op {
            0 => self.op_create_bill(),
            1 => self.op_approve_bill(),
            2 => self.op_pay_bill(),
            3 => self.op_add_bill_item(),
            4 => self.op_remove_bill_item(),
            5 => self.op_cancel_bill(),
            6 => self.op_dispute_bill(),
            7 => self.op_create_invoice(),
            8 => self.op_direct_payment(),
            9 => self.op_apply_payment(),
            10 => self.op_apply_credit_memo(),
            11 => self.op_write_off(),
            _ => self.op_reverse_write_off(),
        }
        self.assert_invariants(&format!("after operation {index} (op kind {op})"));
    }
}

#[test]
fn randomized_ap_ar_simulation_keeps_money_invariants() {
    let seed = std::env::var("AP_AR_SIM_SEED")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SEED);
    let mut sim = Sim::new(seed);
    sim.assert_invariants("after setup");

    for i in 0..OPERATIONS {
        sim.step(i);
    }

    // Statement reconciliation: for each customer, a statement over a window
    // covering the whole run must open at zero and foot to the shadow model's
    // net (invoices - applied payments - applied credits - net write-offs).
    // Direct payments are intentionally absent from both sides: they are not
    // statement-visible activity.
    let start = Utc::now() - Duration::days(2);
    let end = Utc::now() + Duration::days(2);
    for (cust, shadow) in sim.shadows.iter().enumerate() {
        let statement = sim
            .commerce
            .accounts_receivable()
            .generate_statement(GenerateStatementRequest {
                customer_id: sim.customer_ids[cust].into(),
                period_start: Some(start),
                period_end: Some(end),
                include_paid_invoices: None,
            })
            .expect("generate statement");

        assert_eq!(
            statement.opening_balance,
            Decimal::ZERO,
            "customer {cust}: nothing predates the window (seed {seed})"
        );
        let expected = shadow.invoices_total
            - shadow.applied_payments
            - shadow.applied_credits
            - shadow.write_offs;
        let final_balance =
            statement.line_items.last().map_or(statement.opening_balance, |l| l.balance);
        assert_eq!(
            final_balance,
            expected,
            "customer {cust}: statement running balance != shadow model \
             (invoices {} - payments {} - credits {} - write-offs {}, seed {seed})",
            shadow.invoices_total,
            shadow.applied_payments,
            shadow.applied_credits,
            shadow.write_offs
        );
        assert_eq!(
            statement.total_invoices, shadow.invoices_total,
            "customer {cust}: statement invoice total != shadow (seed {seed})"
        );
        assert_eq!(
            statement.total_payments, shadow.applied_payments,
            "customer {cust}: statement payment total != shadow (seed {seed})"
        );
        assert_eq!(
            statement.total_credits, shadow.applied_credits,
            "customer {cust}: statement credit total != shadow (seed {seed})"
        );
    }

    eprintln!(
        "ap/ar simulation seed={seed}: counts by op kind {:?}, {} guard rejections, \
         {} bills, {} invoices, {} open write-offs",
        sim.counts,
        sim.guard_rejections,
        sim.bills.len(),
        sim.invoices.len(),
        sim.open_write_offs.len()
    );
}
