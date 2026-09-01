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
    GenerateStatementRequest, PaymentApplicationLine, RecordInvoicePayment,
    StatementTransactionType, WriteOffReason,
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

// ============================================================================
// FIX 5 — unapply_payment atomicity and reverse_write_off status derivation
//
// `unapply_payment` used to run three autocommit statements (read, DELETE,
// recalculate); a failure after the DELETE left the application row gone but
// the invoice unchanged. It now runs in one transaction, so a failing unapply
// (e.g. nonexistent application) must change nothing.
//
// `reverse_write_off` used to force the invoice status to 'overdue'
// unconditionally; it now recalculates amount_paid/balance_due/status and only
// applies 'overdue' when the due date is actually past and the invoice is
// still open.
// ============================================================================

/// Create an invoice with a single line and an explicit due date.
fn create_invoice_due(
    commerce: &Commerce,
    customer_id: stateset_embedded::CustomerId,
    amount: Decimal,
    due_date: chrono::DateTime<chrono::Utc>,
) -> Uuid {
    let invoice = commerce
        .invoices()
        .create(CreateInvoice {
            customer_id,
            due_date: Some(due_date),
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

#[test]
fn unapply_payment_restores_invoice_amounts_and_status() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

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
    assert_eq!(invoice.amount_paid, dec!(40.00));
    assert_eq!(invoice.balance_due, dec!(60.00));
    assert_eq!(invoice.status, stateset_embedded::InvoiceStatus::PartiallyPaid);

    commerce.accounts_receivable().unapply_payment(apps[0].id).expect("unapply payment");

    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(0.00), "amount_paid restored after unapply");
    assert_eq!(invoice.balance_due, dec!(100.00), "balance_due restored after unapply");
    assert_ne!(
        invoice.status,
        stateset_embedded::InvoiceStatus::PartiallyPaid,
        "status must no longer be partially_paid after the only application is removed"
    );
}

#[test]
fn unapply_of_nonexistent_application_errors_and_changes_nothing() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let payment_id = create_payment(&commerce, customer_id, dec!(40.00));
    commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(40.00) }],
        })
        .expect("apply payment");

    let err = commerce
        .accounts_receivable()
        .unapply_payment(Uuid::new_v4())
        .expect_err("unapplying a nonexistent application must fail");
    assert!(
        matches!(err, stateset_embedded::CommerceError::NotFound),
        "expected NotFound, got: {err:?}"
    );

    // The failed unapply must not have touched the invoice or the application.
    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.amount_paid, dec!(40.00));
    assert_eq!(invoice.balance_due, dec!(60.00));
    assert_eq!(invoice.status, stateset_embedded::InvoiceStatus::PartiallyPaid);
    let apps = commerce.accounts_receivable().get_payment_applications(payment_id).expect("apps");
    assert_eq!(apps.len(), 1, "the existing application must survive a failed unapply");
}

#[test]
fn reverse_write_off_with_future_due_date_is_not_overdue() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let due = chrono::Utc::now() + chrono::Duration::days(30);
    let invoice_id = create_invoice_due(&commerce, customer_id, dec!(100.00), due);

    // Cover part of the balance with a real application, then write off the rest.
    let payment_id = create_payment(&commerce, customer_id, dec!(40.00));
    commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(40.00) }],
        })
        .expect("apply payment");

    let wo = commerce
        .accounts_receivable()
        .create_write_off(CreateWriteOff {
            invoice_id,
            amount: dec!(60.00),
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: None,
        })
        .expect("write off");
    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(invoice.status, stateset_embedded::InvoiceStatus::WrittenOff);

    let reversed = commerce.accounts_receivable().reverse_write_off(wo.id).expect("reverse");
    assert!(reversed.reversed_at.is_some());

    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_ne!(
        invoice.status,
        stateset_embedded::InvoiceStatus::Overdue,
        "an invoice due in the future must not be marked overdue on reversal"
    );
    assert_eq!(
        invoice.status,
        stateset_embedded::InvoiceStatus::PartiallyPaid,
        "the $40 application must still count after reversal"
    );
    assert_eq!(invoice.amount_paid, dec!(40.00));
    assert_eq!(invoice.balance_due, dec!(60.00), "balance_due restored after reversal");
}

#[test]
fn reverse_write_off_of_past_due_unpaid_invoice_restores_overdue() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let due = chrono::Utc::now() - chrono::Duration::days(10);
    let invoice_id = create_invoice_due(&commerce, customer_id, dec!(100.00), due);

    let wo = commerce
        .accounts_receivable()
        .create_write_off(CreateWriteOff {
            invoice_id,
            amount: dec!(100.00),
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: None,
        })
        .expect("write off");

    commerce.accounts_receivable().reverse_write_off(wo.id).expect("reverse");

    let invoice = commerce.invoices().get(invoice_id).expect("get").expect("invoice");
    assert_eq!(
        invoice.status,
        stateset_embedded::InvoiceStatus::Overdue,
        "a genuinely past-due unpaid invoice must be overdue after reversal"
    );
    assert_eq!(invoice.amount_paid, dec!(0.00));
    assert_eq!(invoice.balance_due, dec!(100.00), "balance_due restored after reversal");
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

// ============================================================================
// FIX 6: customer statements include credit memos and a real opening balance
// ============================================================================

#[test]
fn statement_includes_credit_memos_and_balances() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    // Apply a $30 payment and a $10 credit memo.
    let payment_id = create_payment(&commerce, customer_id, dec!(30.00));
    commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(30.00) }],
        })
        .expect("apply payment");
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

    let statement = commerce
        .accounts_receivable()
        .generate_statement(GenerateStatementRequest {
            customer_id: customer_id.into(),
            period_start: None, // default: last 30 days, covers everything
            period_end: None,
            include_paid_invoices: None,
        })
        .expect("generate statement");

    // The credit memo appears as a line item and in total_credits.
    let credit_lines: Vec<_> = statement
        .line_items
        .iter()
        .filter(|l| matches!(l.transaction_type, StatementTransactionType::CreditMemo))
        .collect();
    assert_eq!(credit_lines.len(), 1, "credit memo must appear on the statement");
    assert_eq!(credit_lines[0].credit, Some(dec!(10.00)));
    assert_eq!(statement.total_credits, dec!(10.00));
    assert_eq!(statement.total_invoices, dec!(100.00));
    assert_eq!(statement.total_payments, dec!(30.00));

    // The running balance foots to the closing balance.
    assert_eq!(statement.opening_balance, dec!(0.00));
    let last_balance = statement.line_items.last().expect("line items").balance;
    assert_eq!(last_balance, dec!(60.00), "running balance must reflect all three entries");
    assert_eq!(statement.closing_balance, dec!(60.00));
}

#[test]
fn statement_reconciles_payment_credit_memo_and_write_off_to_zero() {
    // $100 invoice, $30 payment application, $10 credit memo, write off the
    // $60 remainder: the statement must show all four entries, the running
    // balance must foot to zero, and total_credits must be exactly the memo.
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    let invoice_id = create_invoice(&commerce, customer_id, dec!(100.00));

    let payment_id = create_payment(&commerce, customer_id, dec!(30.00));
    commerce
        .accounts_receivable()
        .apply_payment_to_invoices(ApplyPaymentToInvoices {
            payment_id,
            applications: vec![PaymentApplicationLine { invoice_id, amount: dec!(30.00) }],
        })
        .expect("apply payment");

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

    commerce
        .accounts_receivable()
        .create_write_off(CreateWriteOff {
            invoice_id,
            amount: dec!(60.00),
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: None,
        })
        .expect("write off the remainder");

    let statement = commerce
        .accounts_receivable()
        .generate_statement(GenerateStatementRequest {
            customer_id: customer_id.into(),
            period_start: None, // default: last 30 days, covers everything
            period_end: None,
            include_paid_invoices: None,
        })
        .expect("generate statement");

    // All four entries appear as line items.
    assert_eq!(statement.line_items.len(), 4, "lines: {:?}", statement.line_items);
    let count = |t: StatementTransactionType| {
        statement.line_items.iter().filter(|l| l.transaction_type == t).count()
    };
    assert_eq!(count(StatementTransactionType::Invoice), 1);
    assert_eq!(count(StatementTransactionType::Payment), 1);
    assert_eq!(count(StatementTransactionType::CreditMemo), 1);
    assert_eq!(count(StatementTransactionType::WriteOff), 1);

    assert_eq!(statement.total_invoices, dec!(100.00));
    assert_eq!(statement.total_payments, dec!(30.00));
    assert_eq!(statement.total_credits, dec!(10.00), "only the credit memo counts as a credit");

    // The running balance foots from 0 through all four entries to 0, and the
    // live-aging closing balance agrees (a written-off invoice is no longer
    // outstanding).
    assert_eq!(statement.opening_balance, dec!(0.00));
    let final_balance = statement.line_items.last().expect("line items").balance;
    assert_eq!(final_balance, dec!(0.00), "100 - 30 - 10 - 60 must foot to zero");
    assert_eq!(statement.closing_balance, dec!(0.00));
}

#[test]
fn statement_opening_balance_carries_pre_period_activity() {
    let commerce = new_commerce();
    let customer_id = create_test_customer(&commerce);
    // Activity happens "now"; the statement period is entirely in the future,
    // so everything lands in the opening balance and no line items appear.
    create_invoice(&commerce, customer_id, dec!(100.00));

    let start = chrono::Utc::now() + chrono::Duration::days(10);
    let end = start + chrono::Duration::days(30);
    let statement = commerce
        .accounts_receivable()
        .generate_statement(GenerateStatementRequest {
            customer_id: customer_id.into(),
            period_start: Some(start),
            period_end: Some(end),
            include_paid_invoices: None,
        })
        .expect("generate statement");

    assert!(statement.line_items.is_empty(), "no activity inside the period");
    assert_eq!(
        statement.opening_balance,
        dec!(100.00),
        "pre-period invoice must carry into the opening balance"
    );
    assert_eq!(statement.total_invoices, dec!(0.00));
    assert_eq!(statement.closing_balance, dec!(100.00));
}
