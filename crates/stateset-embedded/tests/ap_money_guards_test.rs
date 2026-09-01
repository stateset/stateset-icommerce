#![cfg(feature = "sqlite")]

//! Regression tests for Accounts Payable status-transition and filter guards
//! (SQLite backend, sync `Commerce` engine).
//!
//! Covers:
//! - `clear_payment` refuses voided payments and clears pending ones;
//! - `cancel_bill` refuses paid bills;
//! - `dispute_bill` refuses cancelled bills;
//! - `approve_bill` errors (instead of silently succeeding) on cancelled bills;
//! - `count_payments` applies the same `from_date`/`to_date` filters as
//!   `list_payments`;
//! - `add_bill_item`/`remove_bill_item` refuse bills past draft/pending (item
//!   edits on approved/paid bills corrupted totals, e.g. negative `amount_due`
//!   on a paid bill);
//! - `get_bills_due_soon` includes a bill due exactly on the window's last day
//!   (RFC3339 vs `datetime('now')` lexical comparison excluded the boundary).

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    Bill, BillPayment, BillPaymentFilter, BillStatus, Commerce, CreateBill, CreateBillItem,
    CreateBillPayment, CreatePaymentRun, PaymentAllocationInput, PaymentMethodAP, PaymentRunStatus,
    PaymentStatusAP,
};
use uuid::Uuid;

fn commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn make_bill(commerce: &Commerce, supplier: Uuid, qty: Decimal, price: Decimal) -> Bill {
    make_bill_due(commerce, supplier, qty, price, Utc::now() + Duration::days(30))
}

fn make_bill_due(
    commerce: &Commerce,
    supplier: Uuid,
    qty: Decimal,
    price: Decimal,
    due_date: DateTime<Utc>,
) -> Bill {
    commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier,
            due_date,
            items: vec![widget_item(qty, price)],
            ..Default::default()
        })
        .expect("create bill")
}

fn widget_item(qty: Decimal, price: Decimal) -> CreateBillItem {
    CreateBillItem {
        description: "Widget".into(),
        account_code: Some("5000".into()),
        quantity: qty,
        unit_price: price,
        tax_rate: None,
        po_line_id: None,
    }
}

fn pay_bill(
    commerce: &Commerce,
    supplier: Uuid,
    bill_id: Uuid,
    amount: Decimal,
    payment_date: Option<DateTime<Utc>>,
) -> BillPayment {
    commerce
        .accounts_payable()
        .create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date,
            payment_method: PaymentMethodAP::Ach,
            amount,
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id, amount }],
        })
        .expect("create payment")
}

#[test]
fn clear_payment_succeeds_for_pending_payment() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    let payment = pay_bill(&commerce, supplier, bill.id, dec!(100), None);
    assert_eq!(payment.status, PaymentStatusAP::Pending);

    let cleared = commerce.accounts_payable().clear_payment(payment.id).expect("clear");
    assert_eq!(cleared.status, PaymentStatusAP::Cleared);
}

#[test]
fn clear_payment_rejects_voided_payment() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    let payment = pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    commerce.accounts_payable().void_payment(payment.id).expect("void");

    let result = commerce.accounts_payable().clear_payment(payment.id);
    assert!(result.is_err(), "clearing a voided payment must be rejected");

    let after = commerce
        .accounts_payable()
        .get_payment(payment.id)
        .expect("get payment")
        .expect("payment exists");
    assert_eq!(after.status, PaymentStatusAP::Voided, "status must remain voided");
}

#[test]
fn cancel_bill_rejects_paid_bill() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    let paid = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(paid.status, BillStatus::Paid);

    let result = commerce.accounts_payable().cancel_bill(bill.id);
    assert!(result.is_err(), "cancelling a paid bill must be rejected");

    let after = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Paid, "status must remain paid");
}

#[test]
fn cancel_bill_rejects_partially_paid_bill() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(40), None);

    let partial = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(partial.status, BillStatus::PartiallyPaid);

    let result = commerce.accounts_payable().cancel_bill(bill.id);
    assert!(result.is_err(), "cancelling a partially-paid bill must be rejected");
}

#[test]
fn cancel_bill_succeeds_from_draft() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    let cancelled = commerce.accounts_payable().cancel_bill(bill.id).expect("cancel");
    assert_eq!(cancelled.status, BillStatus::Cancelled);
}

#[test]
fn dispute_bill_rejects_cancelled_bill() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    commerce.accounts_payable().cancel_bill(bill.id).expect("cancel");

    let result = commerce.accounts_payable().dispute_bill(bill.id);
    assert!(result.is_err(), "disputing a cancelled bill must be rejected");

    let after = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Cancelled, "status must remain cancelled");
}

#[test]
fn dispute_bill_rejects_paid_bill() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    let result = commerce.accounts_payable().dispute_bill(bill.id);
    assert!(result.is_err(), "disputing a paid bill must be rejected");
}

#[test]
fn dispute_bill_succeeds_from_approved() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    commerce.accounts_payable().approve_bill(bill.id).expect("approve");
    let disputed = commerce.accounts_payable().dispute_bill(bill.id).expect("dispute");
    assert_eq!(disputed.status, BillStatus::Disputed);
}

#[test]
fn approve_bill_errors_on_cancelled_bill() {
    let commerce = commerce();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(1), dec!(10));
    commerce.accounts_payable().cancel_bill(bill.id).expect("cancel");

    let result = commerce.accounts_payable().approve_bill(bill.id);
    assert!(result.is_err(), "approving a cancelled bill must error, not silently succeed");

    let after = commerce.accounts_payable().get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Cancelled, "status must remain cancelled");
}

#[test]
fn approve_bill_errors_on_missing_bill() {
    let commerce = commerce();
    assert!(commerce.accounts_payable().approve_bill(Uuid::new_v4()).is_err());
}

#[test]
fn count_payments_respects_date_filters() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();

    let early = Utc.with_ymd_and_hms(2026, 1, 10, 0, 0, 0).unwrap();
    let late = Utc.with_ymd_and_hms(2026, 3, 10, 0, 0, 0).unwrap();

    let bill_a = make_bill(&commerce, supplier, dec!(1), dec!(10));
    commerce.accounts_payable().approve_bill(bill_a.id).expect("approve a");
    pay_bill(&commerce, supplier, bill_a.id, dec!(10), Some(early));

    let bill_b = make_bill(&commerce, supplier, dec!(1), dec!(20));
    commerce.accounts_payable().approve_bill(bill_b.id).expect("approve b");
    pay_bill(&commerce, supplier, bill_b.id, dec!(20), Some(late));

    let ap = commerce.accounts_payable();

    // Range covering only the early payment.
    let early_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        from_date: Some(early - Duration::days(1)),
        to_date: Some(early + Duration::days(1)),
        ..Default::default()
    };
    let listed = ap.list_payments(early_filter.clone()).expect("list early");
    let counted = ap.count_payments(early_filter).expect("count early");
    assert_eq!(listed.len(), 1, "list must return only the early payment");
    assert_eq!(counted, 1, "count must match the filtered list");

    // Open-ended from_date covering only the late payment.
    let late_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        from_date: Some(late - Duration::days(1)),
        ..Default::default()
    };
    let listed = ap.list_payments(late_filter.clone()).expect("list late");
    let counted = ap.count_payments(late_filter).expect("count late");
    assert_eq!(listed.len(), 1, "list must return only the late payment");
    assert_eq!(counted, 1, "count must match the filtered list");

    // Range covering both.
    let both_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        from_date: Some(early - Duration::days(1)),
        to_date: Some(late + Duration::days(1)),
        ..Default::default()
    };
    assert_eq!(ap.count_payments(both_filter).expect("count both"), 2);

    // Range covering neither.
    let none_filter = BillPaymentFilter {
        supplier_id: Some(supplier),
        to_date: Some(early - Duration::days(1)),
        ..Default::default()
    };
    assert_eq!(ap.count_payments(none_filter).expect("count none"), 0);
}

// ============================================================================
// Payment run state machine + real disbursement (regression: `process_payment_run`
// was a stub that flipped status to completed without creating any payments).
// ============================================================================

fn approved_bill(commerce: &Commerce, supplier: Uuid, qty: Decimal, price: Decimal) -> Bill {
    let bill = make_bill(commerce, supplier, qty, price);
    commerce.accounts_payable().approve_bill(bill.id).expect("approve bill");
    commerce.accounts_payable().get_bill(bill.id).expect("get bill").expect("bill exists")
}

fn make_run(commerce: &Commerce, bill_ids: Vec<Uuid>) -> stateset_embedded::PaymentRun {
    commerce
        .accounts_payable()
        .create_payment_run(CreatePaymentRun {
            payment_date: Utc::now(),
            payment_method: PaymentMethodAP::Ach,
            bill_ids,
            notes: None,
            created_by: Some("tester".into()),
        })
        .expect("create payment run")
}

#[test]
fn process_payment_run_creates_payments_and_pays_bills() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill_a = approved_bill(&commerce, supplier, dec!(2), dec!(50)); // 100
    let bill_b = approved_bill(&commerce, supplier, dec!(3), dec!(40)); // 120

    let run = make_run(&commerce, vec![bill_a.id, bill_b.id]);
    assert_eq!(run.status, PaymentRunStatus::Draft);
    assert_eq!(run.total_amount, dec!(220));
    assert_eq!(run.payment_count, 2);

    ap.approve_payment_run(run.id, "controller").expect("approve run");
    let processed = ap.process_payment_run(run.id).expect("process run");

    assert_eq!(processed.status, PaymentRunStatus::Completed);
    assert!(processed.processed_at.is_some(), "processed_at must be set");
    assert_eq!(processed.total_amount, dec!(220), "run total must equal disbursed amount");
    assert_eq!(processed.payment_count, 2);

    // Real ap_payments rows must exist, one per bill.
    let payments = ap
        .list_payments(BillPaymentFilter { supplier_id: Some(supplier), ..Default::default() })
        .expect("list payments");
    assert_eq!(payments.len(), 2, "processing must create one payment per bill");
    let mut amounts: Vec<Decimal> = payments.iter().map(|p| p.amount).collect();
    amounts.sort();
    assert_eq!(amounts, vec![dec!(100), dec!(120)]);
    for p in &payments {
        assert_eq!(p.status, PaymentStatusAP::Pending);
        assert_eq!(p.payment_method, PaymentMethodAP::Ach);
    }

    // Bills are paid in full.
    for bill_id in [bill_a.id, bill_b.id] {
        let bill = ap.get_bill(bill_id).expect("get bill").expect("bill exists");
        assert_eq!(bill.status, BillStatus::Paid);
        assert_eq!(bill.amount_due, Decimal::ZERO);
    }
}

#[test]
fn process_payment_run_rejects_unapproved_run() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill = approved_bill(&commerce, supplier, dec!(1), dec!(75));

    let run = make_run(&commerce, vec![bill.id]);
    let result = ap.process_payment_run(run.id);
    assert!(result.is_err(), "processing a draft run must be rejected");

    let after = ap.get_payment_run(run.id).expect("get run").expect("run exists");
    assert_eq!(after.status, PaymentRunStatus::Draft, "run must stay draft");
    assert!(
        ap.list_payments(BillPaymentFilter { supplier_id: Some(supplier), ..Default::default() })
            .expect("list payments")
            .is_empty(),
        "no payments may be created for a rejected process"
    );
}

#[test]
fn process_payment_run_rejects_double_process() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill = approved_bill(&commerce, supplier, dec!(1), dec!(75));

    let run = make_run(&commerce, vec![bill.id]);
    ap.approve_payment_run(run.id, "controller").expect("approve run");
    ap.process_payment_run(run.id).expect("first process");

    let result = ap.process_payment_run(run.id);
    assert!(result.is_err(), "a completed run must not process twice");

    let payments = ap
        .list_payments(BillPaymentFilter { supplier_id: Some(supplier), ..Default::default() })
        .expect("list payments");
    assert_eq!(payments.len(), 1, "double-processing must not duplicate payments");
}

#[test]
fn approve_payment_run_rejects_cancelled_run() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill = approved_bill(&commerce, supplier, dec!(1), dec!(75));

    let run = make_run(&commerce, vec![bill.id]);
    ap.cancel_payment_run(run.id).expect("cancel run");

    let result = ap.approve_payment_run(run.id, "controller");
    assert!(result.is_err(), "approving a cancelled run must be rejected");

    let after = ap.get_payment_run(run.id).expect("get run").expect("run exists");
    assert_eq!(after.status, PaymentRunStatus::Cancelled, "run must stay cancelled");
}

#[test]
fn cancel_payment_run_rejects_completed_run() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill = approved_bill(&commerce, supplier, dec!(1), dec!(75));

    let run = make_run(&commerce, vec![bill.id]);
    ap.approve_payment_run(run.id, "controller").expect("approve run");
    ap.process_payment_run(run.id).expect("process run");

    let result = ap.cancel_payment_run(run.id);
    assert!(result.is_err(), "cancelling a completed run must be rejected");

    let after = ap.get_payment_run(run.id).expect("get run").expect("run exists");
    assert_eq!(after.status, PaymentRunStatus::Completed, "run must stay completed");
}

#[test]
fn create_payment_run_rejects_duplicate_bill_ids() {
    let commerce = commerce();
    let supplier = Uuid::new_v4();
    let bill = approved_bill(&commerce, supplier, dec!(1), dec!(75));

    let result = commerce.accounts_payable().create_payment_run(CreatePaymentRun {
        payment_date: Utc::now(),
        payment_method: PaymentMethodAP::Ach,
        bill_ids: vec![bill.id, bill.id],
        notes: None,
        created_by: None,
    });
    assert!(result.is_err(), "duplicate bill ids in a run must be rejected");
}

#[test]
fn create_payment_run_rejects_empty_missing_and_unpayable_bills() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();

    let empty = ap.create_payment_run(CreatePaymentRun {
        payment_date: Utc::now(),
        payment_method: PaymentMethodAP::Ach,
        bill_ids: vec![],
        notes: None,
        created_by: None,
    });
    assert!(empty.is_err(), "a run with no bills must be rejected");

    let missing = ap.create_payment_run(CreatePaymentRun {
        payment_date: Utc::now(),
        payment_method: PaymentMethodAP::Ach,
        bill_ids: vec![Uuid::new_v4()],
        notes: None,
        created_by: None,
    });
    assert!(missing.is_err(), "a nonexistent bill must be rejected");

    // Draft (unapproved) bill is not payable.
    let draft_bill = make_bill(&commerce, supplier, dec!(1), dec!(75));
    let unpayable = ap.create_payment_run(CreatePaymentRun {
        payment_date: Utc::now(),
        payment_method: PaymentMethodAP::Ach,
        bill_ids: vec![draft_bill.id],
        notes: None,
        created_by: None,
    });
    assert!(unpayable.is_err(), "a bill not in a payable status must be rejected");
}

#[test]
fn create_payment_run_rejects_bill_already_in_active_run() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill = approved_bill(&commerce, supplier, dec!(1), dec!(75));

    let first = make_run(&commerce, vec![bill.id]);
    let second = ap.create_payment_run(CreatePaymentRun {
        payment_date: Utc::now(),
        payment_method: PaymentMethodAP::Ach,
        bill_ids: vec![bill.id],
        notes: None,
        created_by: None,
    });
    assert!(second.is_err(), "a bill already in an active run must be rejected");

    // Once the first run is cancelled the bill is free again.
    ap.cancel_payment_run(first.id).expect("cancel run");
    let third = make_run(&commerce, vec![bill.id]);
    assert_eq!(third.payment_count, 1);
}

#[test]
fn process_payment_run_skips_bill_paid_after_run_creation() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill_a = approved_bill(&commerce, supplier, dec!(2), dec!(50)); // 100
    let bill_b = approved_bill(&commerce, supplier, dec!(3), dec!(40)); // 120

    let run = make_run(&commerce, vec![bill_a.id, bill_b.id]);
    ap.approve_payment_run(run.id, "controller").expect("approve run");

    // bill_a is paid in full directly, between run approval and processing.
    pay_bill(&commerce, supplier, bill_a.id, dec!(100), None);

    let processed = ap.process_payment_run(run.id).expect("process run");
    assert_eq!(processed.status, PaymentRunStatus::Completed);
    assert_eq!(processed.total_amount, dec!(120), "only bill_b's balance is disbursed");
    assert_eq!(processed.payment_count, 1, "the fully-paid bill is skipped");
    assert!(
        processed.notes.as_deref().is_some_and(|n| n.contains("skipped")),
        "run notes must record the skipped bill, got {:?}",
        processed.notes
    );

    // Exactly two payments overall: the manual one plus the run's one for bill_b.
    let payments = ap
        .list_payments(BillPaymentFilter { supplier_id: Some(supplier), ..Default::default() })
        .expect("list payments");
    assert_eq!(payments.len(), 2, "the run must not double-pay bill_a");

    let bill_a_after = ap.get_bill(bill_a.id).expect("get").expect("bill");
    assert_eq!(bill_a_after.status, BillStatus::Paid);
    assert_eq!(bill_a_after.amount_due, Decimal::ZERO);
    let bill_b_after = ap.get_bill(bill_b.id).expect("get").expect("bill");
    assert_eq!(bill_b_after.status, BillStatus::Paid);
    assert_eq!(bill_b_after.amount_due, Decimal::ZERO);
}

// ============================================================================
// Bill line-item edit guards (regression: `add_bill_item`/`remove_bill_item`
// checked no bill status, so removing an item from a fully-paid bill drove
// `amount_due` negative while status stayed 'paid', and adding an item to a
// paid bill created owed money invisible to outstanding/aging and unpayable).
// Only draft/pending bills may have items added or removed.
// ============================================================================

#[test]
fn add_bill_item_rejects_paid_bill() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50)); // 100
    ap.approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    let result = ap.add_bill_item(bill.id, widget_item(dec!(1), dec!(25)));
    assert!(result.is_err(), "adding an item to a paid bill must be rejected");

    let after = ap.get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Paid, "status must remain paid");
    assert_eq!(after.total_amount, dec!(100), "total must be unchanged");
    assert_eq!(after.amount_due, Decimal::ZERO, "amount due must be unchanged");
    assert_eq!(ap.get_bill_items(bill.id).expect("items").len(), 1, "no item may be added");
}

#[test]
fn add_bill_item_rejects_approved_bill() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(2), dec!(50));
    ap.approve_bill(bill.id).expect("approve");

    let result = ap.add_bill_item(bill.id, widget_item(dec!(1), dec!(25)));
    assert!(result.is_err(), "adding an item to an approved bill must be rejected");
    assert_eq!(ap.get_bill_items(bill.id).expect("items").len(), 1);
}

#[test]
fn remove_bill_item_rejects_approved_bill() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(2), dec!(50));
    ap.approve_bill(bill.id).expect("approve");

    let items = ap.get_bill_items(bill.id).expect("items");
    let result = ap.remove_bill_item(items[0].id);
    assert!(result.is_err(), "removing an item from an approved bill must be rejected");

    assert_eq!(ap.get_bill_items(bill.id).expect("items").len(), 1, "item must remain");
    let after = ap.get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.total_amount, dec!(100), "total must be unchanged");
}

#[test]
fn remove_bill_item_rejects_paid_bill() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let bill = make_bill(&commerce, supplier, dec!(2), dec!(50)); // 100
    ap.approve_bill(bill.id).expect("approve");
    pay_bill(&commerce, supplier, bill.id, dec!(100), None);

    let items = ap.get_bill_items(bill.id).expect("items");
    let result = ap.remove_bill_item(items[0].id);
    assert!(result.is_err(), "removing an item from a paid bill must be rejected");

    let after = ap.get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after.status, BillStatus::Paid, "status must remain paid");
    assert_eq!(after.total_amount, dec!(100), "total must be unchanged");
    assert_eq!(after.amount_paid, dec!(100), "amount paid must be unchanged");
    assert_eq!(after.amount_due, Decimal::ZERO, "amount due must never go negative");
}

#[test]
fn add_and_remove_bill_item_on_draft_bill_recalculates_totals() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let bill = make_bill(&commerce, Uuid::new_v4(), dec!(2), dec!(50)); // 100
    assert_eq!(bill.status, BillStatus::Draft);

    let added = ap.add_bill_item(bill.id, widget_item(dec!(3), dec!(10))).expect("add item"); // +30
    let after_add = ap.get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after_add.total_amount, dec!(130), "total must include the new item");
    assert_eq!(after_add.amount_due, dec!(130), "amount due must include the new item");

    let items = ap.get_bill_items(bill.id).expect("items");
    let original = items.iter().find(|i| i.id != added.id).expect("original item");
    ap.remove_bill_item(original.id).expect("remove item"); // -100

    let after_remove = ap.get_bill(bill.id).expect("get").expect("bill");
    assert_eq!(after_remove.total_amount, dec!(30), "total must drop to the remaining item");
    assert_eq!(after_remove.amount_due, dec!(30), "amount due must drop to the remaining item");
    assert_eq!(after_remove.status, BillStatus::Draft, "status must remain draft");
    assert_eq!(ap.get_bill_items(bill.id).expect("items").len(), 1);
}

// ============================================================================
// `get_bills_due_soon` boundary (regression: `due_date` stored RFC3339 was
// compared lexically against `datetime('now', '+N days')`; 'T' > ' ' excluded
// bills due exactly on the window's last day, diverging from Postgres's
// `due_date <= CURRENT_DATE + N` date semantics).
// ============================================================================

#[test]
fn get_bills_due_soon_includes_boundary_day_and_excludes_beyond() {
    let commerce = commerce();
    let ap = commerce.accounts_payable();
    let supplier = Uuid::new_v4();
    let due_in_7 =
        make_bill_due(&commerce, supplier, dec!(1), dec!(10), Utc::now() + Duration::days(7));
    let due_in_8 =
        make_bill_due(&commerce, supplier, dec!(1), dec!(20), Utc::now() + Duration::days(8));

    let due_soon = ap.get_bills_due_soon(7).expect("due soon");
    let ids: Vec<Uuid> = due_soon.iter().map(|b| b.id).collect();
    assert!(
        ids.contains(&due_in_7.id),
        "a bill due exactly 7 days out must be inside the 7-day window"
    );
    assert!(!ids.contains(&due_in_8.id), "a bill due 8 days out must be outside the 7-day window");
}
