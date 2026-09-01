#![cfg(feature = "sqlite")]

//! Money-safety guards for Accounts Receivable:
//! - a payment cannot be applied beyond its own amount (over-application)
//! - payments and credit memos cannot be applied to voided / written-off invoices
//! - write-offs must be positive and bounded by the invoice's balance due
//! - credit memos must be created with a positive amount

use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_embedded::{
    ApplyCreditMemo, ApplyPaymentToInvoices, Commerce, CreateCreditMemo, CreateCustomer,
    CreateInvoice, CreateInvoiceItem, CreatePayment, CreateWriteOff, CreditMemoReason,
    PaymentApplicationLine, RecordInvoicePayment, WriteOffReason,
};
use uuid::Uuid;

fn new_commerce() -> Commerce {
    Commerce::new(":memory:").expect("Failed to create in-memory Commerce")
}

fn create_test_customer(commerce: &Commerce) -> stateset_embedded::CustomerId {
    commerce
        .customers()
        .create(CreateCustomer {
            email: format!("ar-guards-{}@example.com", Uuid::new_v4()),
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

/// Create a payment of the given amount and return its id.
fn create_payment(
    commerce: &Commerce,
    customer_id: stateset_embedded::CustomerId,
    amount: Decimal,
) -> Uuid {
    commerce
        .payments()
        .create(CreatePayment { customer_id: Some(customer_id), amount, ..Default::default() })
        .expect("Failed to create payment")
        .id
        .into()
}

// ============================================================================
// FIX 1 — payment over-application guard
// ============================================================================

#[test]
fn payment_application_beyond_payment_amount_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(300.00));
    let payment_id = create_payment(&commerce, customer_id, dec!(100.00));

    let result = commerce.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
        payment_id,
        applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(150.00) }],
    });

    let err = result.expect_err("applying 150 of a 100 payment must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );
}

#[test]
fn payment_application_of_exact_amount_succeeds_then_further_application_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(300.00));
    let payment_id = create_payment(&commerce, customer_id, dec!(100.00));

    // Exact-amount application must still succeed.
    let apps = commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(100.00) }],
        })
        .expect("applying the exact payment amount must succeed");
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].applied_amount, dec!(100.00));

    // The payment is now fully applied: even one more cent must be rejected.
    let second = commerce.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
        payment_id,
        applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(0.01) }],
    });
    let err = second.expect_err("applying beyond the payment amount must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );
}

#[test]
fn payment_application_summed_across_lines_beyond_payment_amount_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_a = create_invoice(&commerce, customer_id, dec!(80.00));
    let invoice_b = create_invoice(&commerce, customer_id, dec!(80.00));
    let payment_id = create_payment(&commerce, customer_id, dec!(100.00));

    // Each line fits its invoice balance, but the sum (160) exceeds the payment (100).
    let result = commerce.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
        payment_id,
        applications: vec![
            PaymentApplicationLine { invoice_id: invoice_a, amount: dec!(80.00) },
            PaymentApplicationLine { invoice_id: invoice_b, amount: dec!(80.00) },
        ],
    });
    let err = result.expect_err("summed applications beyond the payment amount must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );
}

#[test]
fn payment_application_for_nonexistent_payment_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(50.00));

    let result = commerce.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
        payment_id: Uuid::new_v4(),
        applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(10.00) }],
    });
    let err = result.expect_err("applying a nonexistent payment must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );
}

// ============================================================================
// FIX 2 — status guards on application targets
// ============================================================================

#[test]
fn payment_application_to_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    let payment_id = create_payment(&commerce, customer_id, dec!(100.00));

    commerce.invoices().void(invoice_id).expect("void invoice");

    let result = commerce.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
        payment_id,
        applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(10.00) }],
    });
    let err = result.expect_err("applying a payment to a voided invoice must fail");
    match err {
        stateset_embedded::CommerceError::ValidationError(msg) => {
            assert!(msg.contains("voided"), "error must name the status: {msg}");
            assert!(msg.contains(&invoice_id.to_string()), "error must name the invoice: {msg}");
        }
        other => panic!("expected ValidationError, got: {other:?}"),
    }
}

#[test]
fn payment_application_to_written_off_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    let payment_id = create_payment(&commerce, customer_id, dec!(100.00));

    commerce.invoices().write_off(invoice_id).expect("write off invoice");

    let result = commerce.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
        payment_id,
        applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(10.00) }],
    });
    let err = result.expect_err("applying a payment to a written-off invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );
}

#[test]
fn credit_memo_application_to_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let memo = commerce
        .accounts_receivable()
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer_id.into(),
            amount: dec!(50.00),
            reason: CreditMemoReason::ServiceCredit,
            original_invoice_id: None,
            notes: None,
        })
        .expect("create credit memo");

    commerce.invoices().void(invoice_id).expect("void invoice");

    let result = commerce.accounts_receivable().apply_credit_memo(ApplyCreditMemo {
        credit_memo_id: memo.id,
        invoice_id,
        amount: dec!(25.00),
    });
    let err = result.expect_err("applying a credit memo to a voided invoice must fail");
    match err {
        stateset_embedded::CommerceError::ValidationError(msg) => {
            assert!(msg.contains("voided"), "error must name the status: {msg}");
            assert!(msg.contains(&invoice_id.to_string()), "error must name the invoice: {msg}");
        }
        other => panic!("expected ValidationError, got: {other:?}"),
    }
}

// ============================================================================
// FIX 3 — write-off and credit-memo amount validation
// ============================================================================

#[test]
fn write_off_exceeding_balance_due_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(300.00));

    let result = commerce.accounts_receivable().create_write_off(CreateWriteOff {
        invoice_id,
        amount: dec!(400.00),
        reason: WriteOffReason::Uncollectible,
        notes: None,
        approved_by: None,
    });
    let err = result.expect_err("write-off above balance due must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );

    // The invoice must not have been flipped to written_off by the failed attempt.
    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_ne!(
        invoice.status,
        stateset_embedded::InvoiceStatus::WrittenOff,
        "a rejected write-off must not change the invoice status"
    );
}

#[test]
fn write_off_of_nonpositive_amount_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(300.00));

    for amount in [dec!(0), dec!(-5.00)] {
        let result = commerce.accounts_receivable().create_write_off(CreateWriteOff {
            invoice_id,
            amount,
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: None,
        });
        let err = result.expect_err("non-positive write-off must fail");
        assert!(
            matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
            "expected ValidationError for amount {amount}, got: {err:?}"
        );
    }
}

#[test]
fn write_off_of_full_balance_still_succeeds() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(300.00));

    let write_off = commerce
        .accounts_receivable()
        .create_write_off(CreateWriteOff {
            invoice_id,
            amount: dec!(300.00),
            reason: WriteOffReason::Uncollectible,
            notes: Some("Customer bankrupt".into()),
            approved_by: Some("Finance Manager".into()),
        })
        .expect("write-off of the full balance must succeed");
    assert_eq!(write_off.amount, dec!(300.00));

    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.status, stateset_embedded::InvoiceStatus::WrittenOff);
}

#[test]
fn write_off_of_voided_invoice_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));
    commerce.invoices().void(invoice_id).expect("void invoice");

    let result = commerce.accounts_receivable().create_write_off(CreateWriteOff {
        invoice_id,
        amount: dec!(50.00),
        reason: WriteOffReason::Uncollectible,
        notes: None,
        approved_by: None,
    });
    let err = result.expect_err("write-off of a voided invoice must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
        "expected ValidationError, got: {err:?}"
    );
}

// ============================================================================
// FIX 4 — direct payments (record_payment) must survive AR recalculation
//
// `record_payment` writes directly to the invoice without inserting an
// `ar_payment_applications` row; the AR recalculation used to REPLACE
// `amount_paid` with SUM(applications) + SUM(credit memo applications),
// silently erasing direct payments. `direct_amount_paid` now tracks them and
// recalculation adds it back in.
// ============================================================================

#[test]
fn direct_payment_survives_credit_memo_application() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    // Record a $50 direct payment (no AR application row).
    let invoice = commerce
        .invoices()
        .record_payment(
            invoice_id,
            RecordInvoicePayment { amount: dec!(50.00), ..Default::default() },
        )
        .expect("record direct payment");
    assert_eq!(invoice.amount_paid, dec!(50.00));
    assert_eq!(invoice.balance_due, dec!(50.00));

    // Apply a $10 credit memo — this triggers the AR recalculation.
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

    // The $50 direct payment must NOT vanish: 50 direct + 10 credit = 60.
    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(60.00), "recalculation must preserve the direct payment");
    assert_eq!(invoice.balance_due, dec!(40.00));
    assert_eq!(invoice.status, stateset_embedded::InvoiceStatus::PartiallyPaid);

    // A subsequent payment application must also preserve the direct payment.
    let payment_id = create_payment(&commerce, customer_id, dec!(20.00));
    commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(20.00) }],
        })
        .expect("apply payment");

    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(80.00), "50 direct + 10 credit + 20 applied");
    assert_eq!(invoice.balance_due, dec!(20.00));
}

#[test]
fn direct_payment_survives_apply_unapply_cycles() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    commerce
        .invoices()
        .record_payment(
            invoice_id,
            RecordInvoicePayment { amount: dec!(30.00), ..Default::default() },
        )
        .expect("record direct payment");

    let payment_id = create_payment(&commerce, customer_id, dec!(40.00));
    let apps = commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(40.00) }],
        })
        .expect("apply payment");
    assert_eq!(apps.len(), 1);

    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(70.00), "30 direct + 40 applied");
    assert_eq!(invoice.balance_due, dec!(30.00));

    // Unapplying the $40 must leave the $30 direct payment intact.
    commerce.accounts_receivable().unapply_payment(apps[0].id).expect("unapply payment");
    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(30.00), "direct payment survives unapply");
    assert_eq!(invoice.balance_due, dec!(70.00));

    // Re-applying restores the combined total.
    commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(40.00) }],
        })
        .expect("re-apply payment");
    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(70.00));
    assert_eq!(invoice.balance_due, dec!(30.00));
}

#[test]
fn applications_only_invoice_still_recalculates_as_before() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let payment_id = create_payment(&commerce, customer_id, dec!(60.00));
    let apps = commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(60.00) }],
        })
        .expect("apply payment");

    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(60.00));
    assert_eq!(invoice.balance_due, dec!(40.00));
    assert_eq!(invoice.status, stateset_embedded::InvoiceStatus::PartiallyPaid);

    // With no direct payment, unapplying returns the invoice to zero paid.
    commerce.accounts_receivable().unapply_payment(apps[0].id).expect("unapply payment");
    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(0.00));
    assert_eq!(invoice.balance_due, dec!(100.00));
}

#[test]
fn credit_memo_with_nonpositive_amount_is_rejected() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);

    for amount in [dec!(0), dec!(-10.00)] {
        let result = commerce.accounts_receivable().create_credit_memo(CreateCreditMemo {
            customer_id: customer_id.into(),
            amount,
            reason: CreditMemoReason::ServiceCredit,
            original_invoice_id: None,
            notes: None,
        });
        let err = result.expect_err("non-positive credit memo must fail");
        assert!(
            matches!(err, stateset_embedded::CommerceError::ValidationError(_)),
            "expected ValidationError for amount {amount}, got: {err:?}"
        );
    }
}
