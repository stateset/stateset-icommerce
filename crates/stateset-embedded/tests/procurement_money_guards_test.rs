#![cfg(feature = "sqlite")]

//! Money-safety guards for direct invoice operations (`invoices()`):
//! - a direct payment cannot exceed the invoice's remaining balance (overpayment)
//! - a direct payment cannot be recorded against a terminal invoice
//!   (voided / written-off / cancelled)
//! - `void`, `write_off`, `dispute` and `send` are status-guarded UPDATEs rather
//!   than unconditional status flips that erase or resurrect recognized receivable
//! - `RecordInvoicePayment.payment_id` makes the payment idempotent, so a retry
//!   returns the invoice unchanged instead of double-counting the amount
//!
//! The guards must leave the AR reconciliation invariant intact:
//! `amount_paid == direct_amount_paid + SUM(payment applications) + SUM(credit memo applications)`

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    ApplyCreditMemo, Commerce, CreateCreditMemo, CreateCustomer, CreateInvoice, CreateInvoiceItem,
    CreditMemoReason, InvoiceStatus, RecordInvoicePayment,
};
use uuid::Uuid;

fn new_commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn create_test_customer(commerce: &Commerce) -> stateset_embedded::CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("procurement-guards-{}@example.com", Uuid::new_v4()),
            first_name: "Guard".into(),
            last_name: "Tester".into(),
            ..Default::default()
        })
        .expect("Failed to create test customer")
        .id
}

/// Create an invoice with a single line so `balance_due == amount`.
fn create_invoice(
    commerce: &Commerce,
    customer_id: stateset_embedded::CustomerId,
    amount: Decimal,
) -> Uuid {
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id,
            items: vec![CreateInvoiceItem {
                description: "Services".into(),
                quantity: dec!(1),
                unit_price: amount,
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("Failed to create invoice");
    invoice.id.into()
}

fn pay(amount: Decimal) -> RecordInvoicePayment {
    RecordInvoicePayment { amount, ..Default::default() }
}

fn pay_with_id(amount: Decimal, payment_id: Uuid) -> RecordInvoicePayment {
    RecordInvoicePayment { amount, payment_id: Some(payment_id), ..Default::default() }
}

fn get(commerce: &Commerce, invoice_id: Uuid) -> stateset_embedded::Invoice {
    commerce.invoices().get(invoice_id).expect("get invoice").expect("invoice exists")
}

// ============================================================================
// FIX 1 — `record_payment` overpayment and terminal-status guards
// ============================================================================

#[test]
fn record_payment_beyond_balance_due_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let err = commerce
        .invoices()
        .record_payment(invoice_id, pay(dec!(200.00)))
        .expect_err("paying 200 on a 100 invoice must fail");
    match err {
        stateset_embedded::CommerceError::ValidationError(msg) => {
            assert!(msg.contains("200"), "error must name the payment amount: {msg}");
            assert!(msg.contains("100"), "error must name the balance due: {msg}");
            assert!(msg.contains(&invoice_id.to_string()), "error must name the invoice: {msg}");
        }
        other => panic!("expected ValidationError, got: {other:?}"),
    }

    // The rejected payment must not have moved any money.
    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(0));
    assert_eq!(invoice.balance_due, dec!(100.00));
    assert!(invoice.balance_due >= Decimal::ZERO, "balance must never go negative");
}

#[test]
fn record_payment_one_cent_over_balance_due_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    commerce.invoices().record_payment(invoice_id, pay(dec!(60.00))).expect("partial payment");

    let err = commerce
        .invoices()
        .record_payment(invoice_id, pay(dec!(40.01)))
        .expect_err("a payment one cent beyond the remaining balance must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );

    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(60.00));
    assert_eq!(invoice.balance_due, dec!(40.00));
}

#[test]
fn record_payment_of_exact_remaining_balance_succeeds() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    commerce.invoices().record_payment(invoice_id, pay(dec!(60.00))).expect("partial payment");

    // Exact-to-the-penny payment of the remaining balance must still succeed.
    let paid = commerce
        .invoices()
        .record_payment(invoice_id, pay(dec!(40.00)))
        .expect("exact-balance payment must succeed");
    assert_eq!(paid.amount_paid, dec!(100.00));
    assert_eq!(paid.balance_due, dec!(0));
    assert_eq!(paid.status, InvoiceStatus::Paid);
    assert!(paid.paid_at.is_some(), "a fully paid invoice must be stamped paid_at");
}

#[test]
fn record_payment_on_fully_paid_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    commerce.invoices().record_payment(invoice_id, pay(dec!(100.00))).expect("full payment");

    let err = commerce
        .invoices()
        .record_payment(invoice_id, pay(dec!(0.01)))
        .expect_err("a payment on a fully paid invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );

    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(100.00));
    assert_eq!(invoice.balance_due, dec!(0));
}

#[test]
fn record_payment_on_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().void(invoice_id).expect("void invoice");

    let err = commerce
        .invoices()
        .record_payment(invoice_id, pay(dec!(10.00)))
        .expect_err("a payment on a voided invoice must fail");
    match err {
        stateset_embedded::CommerceError::ValidationError(msg) => {
            assert!(msg.contains("voided"), "error must name the status: {msg}");
            assert!(msg.contains(&invoice_id.to_string()), "error must name the invoice: {msg}");
        }
        other => panic!("expected ValidationError, got: {other:?}"),
    }

    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(0));
}

#[test]
fn record_payment_on_written_off_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().write_off(invoice_id).expect("write off invoice");

    let err = commerce
        .invoices()
        .record_payment(invoice_id, pay(dec!(10.00)))
        .expect_err("a payment on a written-off invoice must fail");
    match err {
        stateset_embedded::CommerceError::ValidationError(msg) => {
            assert!(msg.contains("written_off"), "error must name the status: {msg}");
            assert!(msg.contains(&invoice_id.to_string()), "error must name the invoice: {msg}");
        }
        other => panic!("expected ValidationError, got: {other:?}"),
    }

    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(0));
}

#[test]
fn record_payment_still_rejects_nonpositive_amount() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    assert!(commerce.invoices().record_payment(invoice_id, pay(dec!(0))).is_err());
    assert!(commerce.invoices().record_payment(invoice_id, pay(dec!(-5.00))).is_err());
}

// ============================================================================
// FIX 2 — status-guarded `void` / `write_off` / `dispute` / `send`
// ============================================================================

#[test]
fn void_of_open_invoice_succeeds() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let voided =
        commerce.invoices().void(invoice_id).expect("voiding an open invoice must succeed");
    assert_eq!(voided.status, InvoiceStatus::Voided);
    assert!(voided.voided_at.is_some(), "voiding must stamp voided_at");
}

#[test]
fn void_of_partially_paid_invoice_succeeds() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().record_payment(invoice_id, pay(dec!(25.00))).expect("partial payment");

    let voided = commerce
        .invoices()
        .void(invoice_id)
        .expect("voiding a partially paid invoice must succeed");
    assert_eq!(voided.status, InvoiceStatus::Voided);
}

#[test]
fn void_of_paid_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().record_payment(invoice_id, pay(dec!(100.00))).expect("full payment");

    let err = commerce.invoices().void(invoice_id).expect_err("voiding a paid invoice must fail");
    match err {
        stateset_embedded::CommerceError::Conflict(msg) => {
            assert!(msg.contains("paid"), "error must name the status: {msg}");
            assert!(msg.contains(&invoice_id.to_string()), "error must name the invoice: {msg}");
        }
        other => panic!("expected Conflict, got: {other:?}"),
    }

    // The recognized receivable must be untouched.
    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.status, InvoiceStatus::Paid);
    assert_eq!(invoice.amount_paid, dec!(100.00));
    assert!(invoice.voided_at.is_none());
}

#[test]
fn void_of_already_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().void(invoice_id).expect("void invoice");

    let err = commerce.invoices().void(invoice_id).expect_err("re-voiding an invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::Conflict(_)),
        "expected Conflict, got: {err:?}"
    );
}

#[test]
fn void_of_written_off_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().write_off(invoice_id).expect("write off invoice");

    let err =
        commerce.invoices().void(invoice_id).expect_err("voiding a written-off invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::Conflict(_)),
        "expected Conflict, got: {err:?}"
    );
}

#[test]
fn void_of_missing_invoice_is_not_found() {
    let commerce = new_commerce();
    let err =
        commerce.invoices().void(Uuid::new_v4()).expect_err("voiding a missing invoice fails");
    assert!(
        matches!(err, stateset_embedded::CommerceError::NotFound),
        "expected NotFound, got: {err:?}"
    );
}

#[test]
fn write_off_of_open_invoice_succeeds() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let written = commerce.invoices().write_off(invoice_id).expect("write-off must succeed");
    assert_eq!(written.status, InvoiceStatus::WrittenOff);
}

#[test]
fn write_off_of_paid_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().record_payment(invoice_id, pay(dec!(100.00))).expect("full payment");

    let err = commerce
        .invoices()
        .write_off(invoice_id)
        .expect_err("writing off a paid invoice must fail");
    match err {
        stateset_embedded::CommerceError::Conflict(msg) => {
            assert!(msg.contains("paid"), "error must name the status: {msg}");
        }
        other => panic!("expected Conflict, got: {other:?}"),
    }
    assert_eq!(get(&commerce, invoice_id).status, InvoiceStatus::Paid);
}

#[test]
fn write_off_of_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().void(invoice_id).expect("void invoice");

    let err = commerce
        .invoices()
        .write_off(invoice_id)
        .expect_err("writing off a voided invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::Conflict(_)),
        "expected Conflict, got: {err:?}"
    );
    assert_eq!(get(&commerce, invoice_id).status, InvoiceStatus::Voided);
}

#[test]
fn dispute_of_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().void(invoice_id).expect("void invoice");

    // Flipping a voided invoice back to `disputed` would resurrect its full
    // balance into AR aging (aging excludes paid/voided/written_off only).
    let err =
        commerce.invoices().dispute(invoice_id).expect_err("disputing a voided invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::Conflict(_)),
        "expected Conflict, got: {err:?}"
    );
    assert_eq!(get(&commerce, invoice_id).status, InvoiceStatus::Voided);
}

#[test]
fn dispute_of_open_invoice_succeeds() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let disputed = commerce.invoices().dispute(invoice_id).expect("dispute must succeed");
    assert_eq!(disputed.status, InvoiceStatus::Disputed);
}

#[test]
fn send_of_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().void(invoice_id).expect("void invoice");

    let err = commerce.invoices().send(invoice_id).expect_err("sending a voided invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::Conflict(_)),
        "expected Conflict, got: {err:?}"
    );
    assert_eq!(get(&commerce, invoice_id).status, InvoiceStatus::Voided);
}

#[test]
fn send_of_paid_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().record_payment(invoice_id, pay(dec!(100.00))).expect("full payment");

    let err =
        commerce.invoices().send(invoice_id).expect_err("re-sending a paid invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::Conflict(_)),
        "expected Conflict, got: {err:?}"
    );
    assert_eq!(get(&commerce, invoice_id).status, InvoiceStatus::Paid);
}

#[test]
fn send_of_draft_invoice_still_succeeds() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let sent = commerce.invoices().send(invoice_id).expect("sending a draft invoice must succeed");
    assert_eq!(sent.status, InvoiceStatus::Sent);
    assert!(sent.sent_at.is_some());
}

// ============================================================================
// FIX 3 — `RecordInvoicePayment.payment_id` is an idempotency key
// ============================================================================

#[test]
fn record_payment_with_same_payment_id_twice_counts_once() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    let payment_id = Uuid::new_v4();

    let first = commerce
        .invoices()
        .record_payment(invoice_id, pay_with_id(dec!(40.00), payment_id))
        .expect("first payment");
    assert_eq!(first.amount_paid, dec!(40.00));
    assert_eq!(first.balance_due, dec!(60.00));

    // A retry with the same payment id must return the invoice unchanged.
    let retry = commerce
        .invoices()
        .record_payment(invoice_id, pay_with_id(dec!(40.00), payment_id))
        .expect("retry must succeed and be a no-op");
    assert_eq!(retry.amount_paid, dec!(40.00), "retry must not double-count");
    assert_eq!(retry.balance_due, dec!(60.00));
    assert_eq!(retry.status, first.status);

    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(40.00));
    assert_eq!(invoice.balance_due, dec!(60.00));
}

#[test]
fn idempotent_retry_of_a_fully_paying_payment_returns_invoice_unchanged() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    let payment_id = Uuid::new_v4();

    commerce
        .invoices()
        .record_payment(invoice_id, pay_with_id(dec!(100.00), payment_id))
        .expect("full payment");

    // The invoice is now Paid, so the overpayment guard would reject a fresh
    // payment — but a retry of the SAME payment must still succeed as a no-op.
    let retry = commerce
        .invoices()
        .record_payment(invoice_id, pay_with_id(dec!(100.00), payment_id))
        .expect("retry of an already-recorded payment must be a no-op, not an error");
    assert_eq!(retry.amount_paid, dec!(100.00));
    assert_eq!(retry.balance_due, dec!(0));
    assert_eq!(retry.status, InvoiceStatus::Paid);
}

#[test]
fn record_payment_without_payment_id_is_not_deduplicated() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    // A NULL payment_id keeps today's behavior: two identical payments count twice.
    commerce.invoices().record_payment(invoice_id, pay(dec!(30.00))).expect("first payment");
    let second =
        commerce.invoices().record_payment(invoice_id, pay(dec!(30.00))).expect("second payment");
    assert_eq!(second.amount_paid, dec!(60.00));
    assert_eq!(second.balance_due, dec!(40.00));
}

#[test]
fn same_payment_id_on_two_invoices_applies_to_each() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_a = create_invoice(&commerce, customer_id, dec!(100.00));
    let invoice_b = create_invoice(&commerce, customer_id, dec!(100.00));
    let payment_id = Uuid::new_v4();

    // The idempotency key is (invoice, payment): a single payment split across
    // two invoices must land on both.
    let a = commerce
        .invoices()
        .record_payment(invoice_a, pay_with_id(dec!(40.00), payment_id))
        .expect("payment on invoice A");
    let b = commerce
        .invoices()
        .record_payment(invoice_b, pay_with_id(dec!(60.00), payment_id))
        .expect("payment on invoice B");
    assert_eq!(a.amount_paid, dec!(40.00));
    assert_eq!(b.amount_paid, dec!(60.00));

    // ... but a retry on either invoice is still a no-op.
    let a_retry = commerce
        .invoices()
        .record_payment(invoice_a, pay_with_id(dec!(40.00), payment_id))
        .expect("retry on invoice A");
    assert_eq!(a_retry.amount_paid, dec!(40.00));
}

#[test]
fn idempotency_record_is_not_written_when_the_payment_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    let payment_id = Uuid::new_v4();

    // A rejected (over-)payment must not burn the idempotency key: the caller
    // must be able to retry with a corrected amount.
    assert!(
        commerce
            .invoices()
            .record_payment(invoice_id, pay_with_id(dec!(150.00), payment_id))
            .is_err(),
        "overpayment must be rejected"
    );
    let ok = commerce
        .invoices()
        .record_payment(invoice_id, pay_with_id(dec!(100.00), payment_id))
        .expect("a corrected payment with the same id must still be accepted");
    assert_eq!(ok.amount_paid, dec!(100.00));
    assert_eq!(ok.balance_due, dec!(0));
}

// ============================================================================
// REGRESSION — the guards must not break the AR reconciliation invariant
//   amount_paid == direct_amount_paid + payment applications + credit memos
// ============================================================================

#[test]
fn direct_payment_then_credit_memo_still_satisfies_the_invariant() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    // $50 direct payment (guarded, idempotency-keyed) ...
    let payment_id = Uuid::new_v4();
    let after_payment = commerce
        .invoices()
        .record_payment(invoice_id, pay_with_id(dec!(50.00), payment_id))
        .expect("record direct payment");
    assert_eq!(after_payment.amount_paid, dec!(50.00));
    assert_eq!(after_payment.balance_due, dec!(50.00));

    // ... followed by a $10 credit memo application, which triggers the AR
    // recalculation (amount_paid rebuilt from direct_amount_paid + sums).
    let memo = commerce
        .accounts_receivable()
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer_id.into(),
            amount: dec!(10.00),
            reason: CreditMemoReason::ServiceCredit,
            original_invoice_id: None,
            notes: None,
        })
        .expect("create credit memo");
    commerce
        .accounts_receivable()
        .apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: memo.id,
            invoice_id,
            amount: dec!(10.00),
        })
        .expect("apply credit memo");

    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(60.00), "50 direct + 10 credit");
    assert_eq!(invoice.balance_due, dec!(40.00));
    assert_eq!(invoice.status, InvoiceStatus::PartiallyPaid);

    // The overpayment guard must now bound against the RECALCULATED balance:
    // 50 would have been fine before the credit memo, but is too much now.
    assert!(
        commerce.invoices().record_payment(invoice_id, pay(dec!(50.00))).is_err(),
        "the guard must use the post-recalculation balance"
    );

    // Exactly the recalculated remainder must still be accepted, and it must
    // keep the invariant: direct 50 + 40 = 90 direct, + 10 credit = 100.
    let settled = commerce
        .invoices()
        .record_payment(invoice_id, pay(dec!(40.00)))
        .expect("exact remaining balance after the credit memo must be accepted");
    assert_eq!(settled.amount_paid, dec!(100.00));
    assert_eq!(settled.balance_due, dec!(0));
    assert_eq!(settled.status, InvoiceStatus::Paid);

    // And a retry of the original direct payment is still a no-op afterwards.
    let retry = commerce
        .invoices()
        .record_payment(invoice_id, pay_with_id(dec!(50.00), payment_id))
        .expect("retry stays a no-op");
    assert_eq!(retry.amount_paid, dec!(100.00));
    assert_eq!(retry.balance_due, dec!(0));
}

#[test]
fn credit_memo_after_a_guarded_full_payment_is_rejected_by_ar() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    commerce.invoices().record_payment(invoice_id, pay(dec!(100.00))).expect("full payment");

    let memo = commerce
        .accounts_receivable()
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer_id.into(),
            amount: dec!(10.00),
            reason: CreditMemoReason::ServiceCredit,
            original_invoice_id: None,
            notes: None,
        })
        .expect("create credit memo");

    // AR bounds applications by balance_due, which the direct payment cleared:
    // the two guards agree instead of racing amount_paid past total.
    assert!(
        commerce
            .accounts_receivable()
            .apply_credit_memo(ApplyCreditMemo {
                credit_memo_id: memo.id,
                invoice_id,
                amount: dec!(10.00),
            })
            .is_err(),
        "a credit memo must not push amount_paid past the invoice total"
    );

    let invoice = get(&commerce, invoice_id);
    assert_eq!(invoice.amount_paid, dec!(100.00));
    assert_eq!(invoice.balance_due, dec!(0));
}
