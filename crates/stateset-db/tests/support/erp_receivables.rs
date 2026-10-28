use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;
fn customer(db: &dyn Database) -> CustomerId {
    db.customers()
        .create(CreateCustomer {
            email: format!("ar-{}@example.com", Uuid::new_v4()),
            first_name: "AR".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .unwrap()
        .id
}
fn invoice(db: &dyn Database, customer: CustomerId, amount: Decimal) -> Invoice {
    let inv = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer,
            items: vec![CreateInvoiceItem {
                description: "Service".into(),
                quantity: dec!(1),
                unit_price: amount,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.invoices().send(inv.id).unwrap()
}
fn payment(db: &dyn Database, customer: CustomerId, amount: Decimal) -> Payment {
    let p = db
        .payments()
        .create(CreatePayment { customer_id: Some(customer), amount, ..Default::default() })
        .unwrap();
    db.payments().mark_completed(p.id).unwrap()
}
fn apply(
    db: &dyn Database,
    payment: PaymentId,
    invoice: InvoiceId,
    amount: Decimal,
) -> Result<Vec<ArPaymentApplication>> {
    db.accounts_receivable().apply_payment_to_invoices(ApplyPaymentToInvoices {
        payment_id: payment.into(),
        applications: vec![PaymentApplicationLine { invoice_id: invoice.into(), amount }],
    })
}
fn write_off(db: &dyn Database, invoice: InvoiceId, amount: Decimal) -> WriteOff {
    db.accounts_receivable()
        .create_write_off(CreateWriteOff {
            invoice_id: invoice.into(),
            amount,
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: None,
        })
        .unwrap()
}

pub(crate) fn funding_guards(db: &dyn Database) {
    let other = customer(db);
    let customer = customer(db);
    let inv = invoice(db, customer, dec!(1000));
    let pending = db
        .payments()
        .create(CreatePayment {
            customer_id: Some(customer),
            amount: dec!(100),
            ..Default::default()
        })
        .unwrap();
    assert!(apply(db, pending.id, inv.id, dec!(10)).is_err());
    db.payments().mark_failed(pending.id, "test", None).unwrap();
    assert!(apply(db, pending.id, inv.id, dec!(10)).is_err());
    let wrong = payment(db, other, dec!(100));
    assert!(apply(db, wrong.id, inv.id, dec!(10)).is_err());
    let foreign = db
        .payments()
        .create(CreatePayment {
            customer_id: Some(customer),
            currency: Some(CurrencyCode::EUR),
            amount: dec!(100),
            ..Default::default()
        })
        .unwrap();
    db.payments().mark_completed(foreign.id).unwrap();
    assert!(apply(db, foreign.id, inv.id, dec!(10)).is_err());
    let p = payment(db, customer, dec!(100));
    let r = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: p.id,
            amount: Some(dec!(90)),
            ..Default::default()
        })
        .unwrap();
    db.payments().complete_refund(r.id).unwrap();
    assert!(apply(db, p.id, inv.id, dec!(11)).is_err());
    apply(db, p.id, inv.id, dec!(10)).unwrap();
    assert!(apply(db, p.id, inv.id, dec!(1)).is_err());
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(990));
    let final_refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: p.id,
            amount: Some(dec!(10)),
            ..Default::default()
        })
        .unwrap();
    assert!(db.payments().complete_refund(final_refund.id).is_err());
    let allocation = db.accounts_receivable().get_payment_applications(p.id.into()).unwrap()[0].id;
    db.accounts_receivable().unapply_payment(allocation).unwrap();
    db.payments().complete_refund(final_refund.id).unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(1000));

    let p = payment(db, customer, dec!(100));
    let r = db
        .payments()
        .create_refund(CreateRefund { payment_id: p.id, ..Default::default() })
        .unwrap();
    db.payments().complete_refund(r.id).unwrap();
    assert!(apply(db, p.id, inv.id, dec!(1)).is_err());
}

pub(crate) fn partial_writeoffs(db: &dyn Database) {
    let customer = customer(db);
    let inv = invoice(db, customer, dec!(1000));
    let first = write_off(db, inv.id, dec!(100));
    let partial = db.invoices().get(inv.id).unwrap().unwrap();
    assert_eq!(partial.balance_due, dec!(900));
    assert_ne!(partial.status, InvoiceStatus::WrittenOff);
    assert_eq!(partial.amount_paid, dec!(0));
    let p = payment(db, customer, dec!(200));
    let app = apply(db, p.id, inv.id, dec!(200)).unwrap()[0].id;
    let second = write_off(db, inv.id, dec!(400));
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(300));
    db.invoices()
        .record_payment(
            inv.id,
            RecordInvoicePayment {
                amount: dec!(100),
                payment_id: Some(Uuid::new_v4()),
                payment_method: None,
                reference: None,
                notes: None,
            },
        )
        .unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(200));
    assert!(
        db.invoices()
            .record_payment(
                inv.id,
                RecordInvoicePayment {
                    amount: dec!(201),
                    payment_id: Some(Uuid::new_v4()),
                    payment_method: None,
                    reference: None,
                    notes: None,
                }
            )
            .is_err()
    );
    let last = write_off(db, inv.id, dec!(200));
    let closed = db.invoices().get(inv.id).unwrap().unwrap();
    assert_eq!(closed.balance_due, dec!(0));
    assert_eq!(closed.status, InvoiceStatus::WrittenOff);
    assert!(db.accounts_receivable().unapply_payment(app).is_err());
    db.accounts_receivable().reverse_write_off(last.id).unwrap();
    db.accounts_receivable().unapply_payment(app).unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(400));
    db.accounts_receivable().reverse_write_off(second.id).unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(800));
    db.accounts_receivable().reverse_write_off(first.id).unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(900));
    assert!(db.accounts_receivable().reverse_write_off(first.id).is_err());
}

pub(crate) fn terminal_unapply(db: &dyn Database) {
    let customer = customer(db);
    let inv = invoice(db, customer, dec!(100));
    let p = payment(db, customer, dec!(30));
    let app = apply(db, p.id, inv.id, dec!(30)).unwrap()[0].id;
    assert!(db.invoices().void(inv.id).is_err());
    let wo = write_off(db, inv.id, dec!(70));
    assert!(db.accounts_receivable().unapply_payment(app).is_err());
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().status, InvoiceStatus::WrittenOff);
    assert_eq!(db.accounts_receivable().get_payment_applications(p.id.into()).unwrap().len(), 1);
    db.accounts_receivable().reverse_write_off(wo.id).unwrap();
    db.accounts_receivable().unapply_payment(app).unwrap();
    db.invoices().void(inv.id).unwrap();
    assert!(db.accounts_receivable().unapply_payment(app).is_err());
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().status, InvoiceStatus::Voided);
}

pub(crate) fn allocation_race(db: &dyn Database) {
    let customer = customer(db);
    let invoices = [invoice(db, customer, dec!(100)).id, invoice(db, customer, dec!(100)).id];
    let p = payment(db, customer, dec!(100));
    let barrier = std::sync::Barrier::new(2);
    let successes = std::thread::scope(|scope| {
        let handles: Vec<_> = invoices
            .into_iter()
            .map(|id| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    apply(db, p.id, id, dec!(100)).is_ok()
                })
            })
            .collect();
        handles.into_iter().map(|h| usize::from(h.join().unwrap())).sum::<usize>()
    });
    assert_eq!(successes, 1);
    let applications = db.accounts_receivable().get_payment_applications(p.id.into()).unwrap();
    assert_eq!(applications.len(), 1);
    assert_eq!(applications[0].applied_amount, dec!(100));
}

pub(crate) fn refund_allocation_race(db: &dyn Database) {
    let customer = customer(db);
    let inv = invoice(db, customer, dec!(100));
    let p = payment(db, customer, dec!(100));
    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: p.id,
            amount: Some(dec!(50)),
            ..Default::default()
        })
        .unwrap();
    let barrier = std::sync::Barrier::new(2);
    let successes = std::thread::scope(|scope| {
        let barrier = &barrier;
        let allocate = scope.spawn(move || {
            barrier.wait();
            apply(db, p.id, inv.id, dec!(100)).is_ok()
        });
        let refund = scope.spawn(move || {
            barrier.wait();
            db.payments().complete_refund(refund.id).is_ok()
        });
        usize::from(allocate.join().unwrap()) + usize::from(refund.join().unwrap())
    });
    assert_eq!(successes, 1);
    let p = db.payments().get(p.id).unwrap().unwrap();
    let allocated: Decimal = db
        .accounts_receivable()
        .get_payment_applications(p.id.into())
        .unwrap()
        .iter()
        .map(|a| a.applied_amount)
        .sum();
    assert!(allocated <= p.amount - p.amount_refunded);
}

pub(crate) fn draft_edits(db: &dyn Database) {
    let customer = customer(db);
    let input = || CreateInvoiceItem {
        description: "Draft line".into(),
        quantity: dec!(2),
        unit_price: dec!(5),
        ..Default::default()
    };
    let inv = db
        .invoices()
        .create(CreateInvoice { customer_id: customer, items: vec![input()], ..Default::default() })
        .unwrap();
    assert_eq!(db.accounts_receivable().get_aging_summary().unwrap().total, dec!(0));
    assert_eq!(
        db.accounts_receivable()
            .get_customer_aging(customer.into())
            .unwrap()
            .unwrap()
            .total_outstanding,
        dec!(0)
    );
    assert_eq!(db.accounts_receivable().get_dso(30).unwrap(), dec!(0));
    let added = db
        .invoices()
        .add_item(inv.id, CreateInvoiceItem { quantity: dec!(3), unit_price: dec!(2), ..input() })
        .unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().total, dec!(16));
    db.invoices()
        .update_item(inv.items[0].id, CreateInvoiceItem { quantity: dec!(1), ..input() })
        .unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().total, dec!(11));
    db.invoices().remove_item(added.id).unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().total, dec!(5));
    db.invoices()
        .update(
            inv.id,
            UpdateInvoice {
                discount_amount: Some(dec!(1)),
                tax_amount: Some(dec!(2)),
                shipping_amount: Some(dec!(3)),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(db.invoices().recalculate(inv.id).unwrap().total, dec!(9));
    db.invoices().send(inv.id).unwrap();
    assert_eq!(db.accounts_receivable().get_aging_summary().unwrap().total, dec!(9));
    for paid in [false, true] {
        if paid {
            db.invoices()
                .record_payment(
                    inv.id,
                    RecordInvoicePayment { amount: dec!(9), ..Default::default() },
                )
                .unwrap();
        }
        assert!(
            db.invoices()
                .update(
                    inv.id,
                    UpdateInvoice { shipping_amount: Some(dec!(8)), ..Default::default() }
                )
                .is_err()
        );
        assert!(db.invoices().add_item(inv.id, input()).is_err());
        assert!(
            db.invoices()
                .update_batch_atomic(vec![(
                    inv.id,
                    UpdateInvoice { shipping_amount: Some(dec!(8)), ..Default::default() }
                )])
                .is_err()
        );
        let batch = db
            .invoices()
            .update_batch(vec![(
                inv.id,
                UpdateInvoice { shipping_amount: Some(dec!(8)), ..Default::default() },
            )])
            .unwrap();
        assert!(batch.succeeded.is_empty());
        assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().total, dec!(9));
        assert!(db.invoices().update_item(inv.items[0].id, input()).is_err());
        assert!(db.invoices().remove_item(inv.items[0].id).is_err());
        assert!(db.invoices().recalculate(inv.id).is_err());
        let saved = db.invoices().get(inv.id).unwrap().unwrap();
        assert_eq!(saved.total, dec!(9));
        assert_eq!(saved.items.len(), 1);
        assert_eq!(saved.balance_due, if paid { dec!(0) } else { dec!(9) });
    }
}

pub(crate) fn issue_edit_race(db: &dyn Database) {
    let customer = customer(db);
    let inv = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer,
            items: vec![CreateInvoiceItem {
                description: "Original".into(),
                quantity: dec!(1),
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let barrier = &barrier;
        let issue = scope.spawn(move || {
            barrier.wait();
            db.invoices().send(inv.id)
        });
        let edit = scope.spawn(move || {
            barrier.wait();
            db.invoices().add_item(
                inv.id,
                CreateInvoiceItem {
                    description: "Concurrent".into(),
                    quantity: dec!(1),
                    unit_price: dec!(6),
                    ..Default::default()
                },
            )
        });
        issue.join().unwrap().unwrap();
        let _ = edit.join().unwrap();
    });
    let saved = db.invoices().get(inv.id).unwrap().unwrap();
    assert_eq!(saved.status, InvoiceStatus::Sent);
    assert_eq!(saved.total, saved.items.iter().map(|i| i.line_total).sum::<Decimal>());
    assert_eq!(saved.total, saved.balance_due);
    assert!([dec!(10), dec!(16)].contains(&saved.total));
}

pub(crate) fn statements(db: &dyn Database) {
    use chrono::{Duration, Utc};
    let customer = customer(db);
    let inv = invoice(db, customer, dec!(1000));
    let start = Utc::now() - Duration::days(2);
    let statement = |end, currency| {
        db.accounts_receivable().generate_statement(GenerateStatementRequest {
            customer_id: customer.into(),
            period_start: Some(start),
            period_end: Some(end),
            currency,
            ..Default::default()
        })
    };
    let issued_at = Utc::now();
    db.invoices()
        .record_payment(
            inv.id,
            RecordInvoicePayment {
                amount: dec!(100),
                payment_id: None,
                payment_method: None,
                reference: None,
                notes: None,
            },
        )
        .unwrap();
    let p = payment(db, customer, dec!(200));
    let app = apply(db, p.id, inv.id, dec!(200)).unwrap()[0].id;
    let credit = db
        .accounts_receivable()
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer.into(),
            original_invoice_id: Some(inv.id.into()),
            reason: CreditMemoReason::ServiceCredit,
            amount: dec!(100),
            notes: None,
        })
        .unwrap();
    db.accounts_receivable()
        .apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: credit.id,
            invoice_id: inv.id.into(),
            amount: dec!(100),
        })
        .unwrap();
    let wo = write_off(db, inv.id, dec!(50));
    let settled_at = Utc::now();
    let s = statement(settled_at, None).unwrap();
    assert_eq!(s.closing_balance, dec!(550));
    assert_eq!(s.total_payments, dec!(300));
    assert_eq!(s.line_items.last().unwrap().balance, s.closing_balance);
    assert_eq!(s.aging.total_outstanding, s.closing_balance);
    db.accounts_receivable().reverse_write_off(wo.id).unwrap();
    db.accounts_receivable().unapply_payment(app).unwrap();
    let s = statement(Utc::now(), None).unwrap();
    assert_eq!(s.closing_balance, dec!(800));
    assert_eq!(s.total_payments, dec!(100));
    assert_eq!(s.line_items.len(), 7);
    assert_eq!(
        statement(settled_at, None).unwrap().closing_balance,
        dec!(550),
        "Reversals must preserve past statements"
    );
    assert_eq!(
        statement(issued_at, None).unwrap().closing_balance,
        dec!(1000),
        "Future payments must not reduce a past statement"
    );
    let voided = invoice(db, customer, dec!(500));
    let before_void = Utc::now();
    db.invoices().void(voided.id).unwrap();
    assert_eq!(statement(before_void, None).unwrap().closing_balance, dec!(1300));
    assert_eq!(statement(Utc::now(), None).unwrap().closing_balance, dec!(800));
    let draft = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer,
            items: vec![CreateInvoiceItem {
                description: "Draft".into(),
                quantity: dec!(1),
                unit_price: dec!(9999),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(statement(Utc::now(), None).unwrap().closing_balance, dec!(800));
    db.invoices().void(draft.id).unwrap();
    assert_eq!(statement(Utc::now(), None).unwrap().closing_balance, dec!(800));
    let foreign = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer,
            currency: Some(CurrencyCode::EUR),
            items: vec![CreateInvoiceItem {
                description: "Euro".into(),
                quantity: dec!(1),
                unit_price: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.invoices().send(foreign.id).unwrap();
    assert!(statement(Utc::now(), None).is_err());
    assert_eq!(statement(Utc::now(), Some(CurrencyCode::USD)).unwrap().closing_balance, dec!(800));
    assert_eq!(statement(Utc::now(), Some(CurrencyCode::EUR)).unwrap().closing_balance, dec!(10));
    let recent = db
        .accounts_receivable()
        .generate_statement(GenerateStatementRequest {
            customer_id: customer.into(),
            currency: Some(CurrencyCode::USD),
            period_start: Some(settled_at),
            period_end: Some(Utc::now()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(recent.opening_balance, dec!(550));
    assert_eq!(recent.closing_balance, dec!(800));
}
pub(crate) fn invoice_writeoff(db: &dyn Database) {
    let customer = customer(db);
    let inv = invoice(db, customer, dec!(100));
    db.invoices()
        .record_payment(
            inv.id,
            RecordInvoicePayment {
                amount: dec!(25),
                payment_id: None,
                payment_method: None,
                reference: None,
                notes: None,
            },
        )
        .unwrap();
    let inv = db.invoices().write_off(inv.id).unwrap();
    assert_eq!(inv.balance_due, dec!(0));
    assert_eq!(inv.amount_paid, dec!(25));
    assert_eq!(inv.status, InvoiceStatus::WrittenOff);
    let records = db
        .accounts_receivable()
        .list_write_offs(WriteOffFilter { invoice_id: Some(inv.id.into()), ..Default::default() })
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].amount, dec!(75));
    assert!(db.invoices().write_off(inv.id).is_err());
    db.accounts_receivable().reverse_write_off(records[0].id).unwrap();
    assert_eq!(db.invoices().get(inv.id).unwrap().unwrap().balance_due, dec!(75));
}
pub(crate) fn credit_filters(db: &dyn Database) {
    use chrono::{Duration, Utc};
    let customer = customer(db);
    let ar = db.accounts_receivable();
    for reason in [
        CreditMemoReason::ServiceCredit,
        CreditMemoReason::PricingError,
        CreditMemoReason::ServiceCredit,
    ] {
        ar.create_credit_memo(CreateCreditMemo {
            customer_id: customer.into(),
            original_invoice_id: None,
            reason,
            amount: dec!(1),
            notes: None,
        })
        .unwrap();
    }
    let all = ar
        .list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer.into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all.len(), 3);
    let filtered = ar
        .list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer.into()),
            reason: Some(CreditMemoReason::ServiceCredit),
            has_unapplied: Some(true),
            limit: Some(1),
            offset: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(
        filtered[0].id,
        all.iter().filter(|m| m.reason == CreditMemoReason::ServiceCredit).nth(1).unwrap().id
    );
    let future = ar
        .list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer.into()),
            from_date: Some(Utc::now() + Duration::days(1)),
            ..Default::default()
        })
        .unwrap();
    assert!(future.is_empty());
    let past = ar
        .list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer.into()),
            to_date: Some(Utc::now() - Duration::days(1)),
            ..Default::default()
        })
        .unwrap();
    assert!(past.is_empty());
    let second = ar
        .list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer.into()),
            limit: Some(1),
            offset: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(second[0].id, all[1].id);
    for _ in 0..502 {
        ar.create_credit_memo(CreateCreditMemo {
            customer_id: customer.into(),
            original_invoice_id: None,
            reason: CreditMemoReason::Other,
            amount: dec!(1),
            notes: None,
        })
        .unwrap();
    }
    assert_eq!(
        ar.list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer.into()),
            ..Default::default()
        })
        .unwrap()
        .len(),
        500
    );
    assert_eq!(
        ar.list_credit_memos(CreditMemoFilter {
            customer_id: Some(customer.into()),
            has_unapplied: Some(true),
            ..Default::default()
        })
        .unwrap()
        .len(),
        500
    );
}
