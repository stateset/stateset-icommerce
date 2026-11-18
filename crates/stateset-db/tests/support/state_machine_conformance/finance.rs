//! Finance machines: AR credit memos, supplier prepayments and vendor
//! credits, payment obligations, GL periods and journal entries.

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

use super::customer;
use super::harness::{Conformance, Report, SetStatus};

fn supplier(db: &dyn Database) -> Uuid {
    db.purchase_orders()
        .create_supplier(CreateSupplier {
            name: format!("SM supplier {}", Uuid::new_v4().simple()),
            ..Default::default()
        })
        .unwrap()
        .id
}

/// An issued invoice owing `amount` to `customer`.
fn sent_invoice(db: &dyn Database, customer: CustomerId, amount: Decimal) -> InvoiceId {
    let invoice = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer,
            items: vec![CreateInvoiceItem {
                description: "conformance".into(),
                quantity: dec!(1),
                unit_price: amount,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.invoices().send(invoice.id).unwrap().id
}

pub(crate) fn credit_memo(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let customer = customer(db);
    let invoice = sent_invoice(db, customer.id, dec!(1000000));
    let repo = db.accounts_receivable();
    let create = || {
        repo.create_credit_memo(CreateCreditMemo {
            customer_id: customer.id.into(),
            original_invoice_id: None,
            reason: CreditMemoReason::PricingError,
            amount: dec!(100),
            notes: None,
        })
        .unwrap()
        .id
    };
    let memo = |id| repo.get_credit_memo(id).unwrap().unwrap();
    let status = |id| memo(id).status.to_string();
    let apply = |id, amount| {
        repo.apply_credit_memo(ApplyCreditMemo {
            credit_memo_id: id,
            invoice_id: invoice.into(),
            amount,
        })
        .map(drop)
    };
    Conformance::new("credit_memo", create, status)
        .op("partially_applied", "apply_credit_memo (10)", |id| apply(id, dec!(10)))
        .op("fully_applied", "apply_credit_memo (the unapplied rest)", |id| {
            apply(id, memo(id).unapplied_amount)
        })
        .op("voided", "void_credit_memo", |id| repo.void_credit_memo(id).map(drop))
        .check()
}

/// A payment obligation owing `amount` to `supplier`.
fn obligation(db: &dyn Database, supplier: Uuid, amount: Decimal) -> PaymentObligationId {
    db.payment_obligations()
        .create(CreatePaymentObligation {
            supplier_id: supplier,
            purchase_order_id: None,
            amount,
            currency: Some(CurrencyCode::USD),
            due_date: Utc::now().date_naive(),
            notes: None,
        })
        .unwrap()
        .id
}

fn prepayment_of(db: &dyn Database, supplier: Uuid, amount: Decimal) -> Result<PrepaymentId> {
    db.prepayments()
        .create(CreatePrepayment {
            supplier_id: supplier,
            amount,
            currency: Some(CurrencyCode::USD),
            method: None,
            reference: None,
            memo: None,
        })
        .map(|p| p.id)
}

pub(crate) fn prepayment(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let supplier = supplier(db);
    let repo = db.prepayments();
    let create = || prepayment_of(db, supplier, dec!(100)).unwrap();
    let status = |id| repo.get(id).unwrap().unwrap().status.to_string();
    let apply = |id, amount| {
        let target = obligation(db, supplier, dec!(1000));
        repo.apply(
            id,
            ApplyPrepayment {
                target_type: PrepaymentTargetType::PaymentObligation,
                target_id: target.into(),
                amount,
            },
        )
        .map(drop)
    };
    Conformance::new("prepayment", create, status)
        .op("applied", "apply (the remaining balance)", |id| {
            apply(id, repo.get(id).unwrap().unwrap().remaining)
        })
        .op("open", "apply (part of the balance)", |id| apply(id, dec!(1)))
        .only_from(&["open"])
        .op("open", "reverse_application (the latest)", |id| {
            let latest = repo.list_applications(id)?.into_iter().rfind(|a| !a.reversed);
            let application = latest.ok_or(CommerceError::NotFound)?;
            repo.reverse_application(id, application.id).map(drop)
        })
        .run_from(&["applied"])
        .op("refunded", "refund", |id| repo.refund(id).map(drop))
        .seeded("cancelled", |id| set("prepayments", &id.to_string(), "cancelled"))
        .check()
}

pub(crate) fn vendor_credit(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let supplier = supplier(db);
    let repo = db.vendor_credits();
    let create = || {
        repo.create(CreateVendorCredit {
            supplier_id: supplier,
            vendor_return_id: None,
            amount: dec!(100),
            currency: Some(CurrencyCode::USD),
            memo: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get(id).unwrap().unwrap().status.to_string();
    let apply = |id, amount| {
        let target = obligation(db, supplier, dec!(1000));
        repo.apply(
            id,
            ApplyVendorCredit {
                target_type: VendorCreditTargetType::PaymentObligation,
                target_id: target.into(),
                amount,
            },
        )
        .map(drop)
    };
    Conformance::new("vendor_credit", create, status)
        .op("applied", "apply (the remaining balance)", |id| {
            apply(id, repo.get(id).unwrap().unwrap().remaining)
        })
        .op("open", "apply (part of the balance)", |id| apply(id, dec!(1)))
        .only_from(&["open"])
        .op("open", "reverse_application (the latest)", |id| {
            let latest = repo.list_applications(id)?.into_iter().rfind(|a| !a.reversed);
            let application = latest.ok_or(CommerceError::NotFound)?;
            repo.reverse_application(id, application.id).map(drop)
        })
        .run_from(&["applied"])
        .op("cancelled", "cancel", |id| repo.cancel(id).map(drop))
        .check()
}

pub(crate) fn payment_obligation(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let supplier = supplier(db);
    let repo = db.payment_obligations();
    let create = || obligation(db, supplier, dec!(100));
    let get = |id| repo.get(id).unwrap().unwrap();
    let status = |id| get(id).status.to_string();
    let settle = |id: PaymentObligationId, amount| {
        let prepayment = prepayment_of(db, supplier, amount)?;
        db.prepayments()
            .apply(
                prepayment,
                ApplyPrepayment {
                    target_type: PrepaymentTargetType::PaymentObligation,
                    target_id: id.into(),
                    amount,
                },
            )
            .map(drop)
    };
    let applications = |id: PaymentObligationId| -> Vec<(PrepaymentId, PrepaymentApplicationId)> {
        db.prepayments()
            .list(PrepaymentFilter { supplier_id: Some(supplier), ..Default::default() })
            .unwrap()
            .into_iter()
            .flat_map(|p| db.prepayments().list_applications(p.id).unwrap())
            .filter(|a| a.target_id == id.into_uuid() && !a.reversed)
            .map(|a| (a.prepayment_id, a.id))
            .collect()
    };
    let outstanding = |id| {
        let o = get(id);
        o.amount - o.amount_paid
    };
    let set_status = |id, value| repo.set_status(id, value).map(drop);
    Conformance::new("payment_obligation", create, status)
        .op("paid", "settle the balance in two prepayment applications", |id| {
            let half = (outstanding(id) / dec!(2)).round_dp(2);
            settle(id, half)?;
            settle(id, outstanding(id))
        })
        .refuses_same_status()
        .op("paid", "record_payment (the outstanding balance)", |id| {
            repo.record_payment(id, outstanding(id)).map(drop)
        })
        .refuses_same_status()
        .op("partially_paid", "settle part (one prepayment application)", |id| settle(id, dec!(1)))
        .only_from(&["pending", "scheduled", "partially_paid"])
        .op("partially_paid", "record_payment (one unit)", |id| {
            repo.record_payment(id, dec!(1)).map(drop)
        })
        .only_from(&["pending", "scheduled", "partially_paid"])
        .op("pending", "set_status pending", |id| set_status(id, PaymentObligationStatus::Pending))
        .only_from(&["pending", "scheduled"])
        .op("scheduled", "set_status scheduled", |id| {
            set_status(id, PaymentObligationStatus::Scheduled)
        })
        .op("partially_paid", "set_status partially_paid", |id| {
            set_status(id, PaymentObligationStatus::PartiallyPaid)
        })
        .only_from(&["partially_paid"])
        .op("paid", "set_status paid", |id| set_status(id, PaymentObligationStatus::Paid))
        .only_from(&["paid"])
        .op("cancelled", "set_status cancelled", |id| {
            set_status(id, PaymentObligationStatus::Cancelled)
        })
        .op("partially_paid", "reverse the latest settlement application", |id| {
            let (prepayment, application) =
                applications(id).pop().ok_or(CommerceError::NotFound)?;
            db.prepayments().reverse_application(prepayment, application).map(drop)
        })
        .run_from(&["paid"])
        .op("pending", "reverse every settlement application", |id| {
            for (prepayment, application) in applications(id) {
                db.prepayments().reverse_application(prepayment, application)?;
            }
            Ok(())
        })
        .run_from(&["paid", "partially_paid"])
        .check()
}

/// `count` consecutive one-day periods nobody else uses: a random far-future
/// day per call site, so reruns against a shared database do not overlap.
fn free_days(count: u32) -> impl FnMut() -> NaiveDate {
    let base = NaiveDate::from_ymd_opt(2600, 1, 1).unwrap();
    let slot = u64::try_from(Uuid::new_v4().as_u128() % 20_000).unwrap();
    let mut day = base + chrono::Days::new(slot * u64::from(count));
    move || {
        let current = day;
        day = day + chrono::Days::new(1);
        current
    }
}

fn period(db: &dyn Database, day: NaiveDate) -> GlPeriod {
    db.general_ledger()
        .create_period(CreateGlPeriod {
            period_name: format!("SM {day}"),
            fiscal_year: day.format("%Y").to_string().parse().unwrap(),
            period_number: i32::try_from(chrono::Datelike::ordinal(&day)).unwrap(),
            start_date: day,
            end_date: day,
        })
        .unwrap()
}

pub(crate) fn gl_period(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let days = std::cell::RefCell::new(free_days(64));
    let repo = db.general_ledger();
    let create = || period(db, (days.borrow_mut())()).id;
    let status = |id| repo.get_period(id).unwrap().unwrap().status.to_string();
    Conformance::new("gl_period", create, status)
        .op("open", "open_period", |id| repo.open_period(id).map(drop))
        .only_from(&["future"])
        .op("closed", "close_period", |id| repo.close_period(id, "controller").map(drop))
        .op("locked", "lock_period", |id| repo.lock_period(id, "controller").map(drop))
        .op("open", "reopen_period", |id| repo.reopen_period(id).map(drop))
        .only_from(&["closed"])
        .check()
}

fn account(db: &dyn Database, account_type: AccountType) -> Uuid {
    db.general_ledger()
        .create_account(CreateGlAccount {
            account_number: format!("SM-{}", &Uuid::new_v4().simple().to_string()[..12]),
            name: "conformance".into(),
            description: None,
            account_type,
            account_sub_type: None,
            parent_account_id: None,
            is_header: Some(false),
            is_posting: Some(true),
            currency: None,
        })
        .unwrap()
        .id
}

/// Today's period, opened (created when the store has none).
fn open_period_for_today(db: &dyn Database) -> NaiveDate {
    let today = Utc::now().date_naive();
    let gl = db.general_ledger();
    let existing = gl.get_period_for_date(today).unwrap();
    let period = match existing {
        Some(p) => p,
        None => period(db, today),
    };
    if period.status == PeriodStatus::Future {
        gl.open_period(period.id).unwrap();
    }
    assert_eq!(gl.get_period(period.id).unwrap().unwrap().status, PeriodStatus::Open);
    today
}

pub(crate) fn journal_entry(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let today = open_period_for_today(db);
    let cash = account(db, AccountType::Asset);
    let revenue = account(db, AccountType::Revenue);
    let customer = customer(db);
    let repo = db.general_ledger();
    // Each entry is the posting of its own issued invoice, so voiding that
    // invoice is the operation that voids a draft entry or reverses a posted
    // one.
    let source = std::cell::RefCell::new(std::collections::HashMap::new());
    let create = || {
        let invoice = sent_invoice(db, customer.id, dec!(25));
        let entry = repo
            .create_journal_entry(CreateJournalEntry {
                entry_date: today,
                entry_type: None,
                description: "conformance".into(),
                lines: vec![
                    CreateJournalEntryLine::debit(cash, dec!(25), None),
                    CreateJournalEntryLine::credit(revenue, dec!(25), None),
                ],
                source_document_type: Some("invoice".into()),
                source_document_id: Some(invoice.into()),
                auto_post: Some(false),
                fx: None,
            })
            .unwrap();
        source.borrow_mut().insert(entry.id, invoice);
        entry.id
    };
    let status = |id| repo.get_journal_entry(id).unwrap().unwrap().status.to_string();
    let void_source = |id| db.invoices().void(source.borrow()[&id]).map(drop);
    Conformance::new("journal_entry", create, status)
        .op("posted", "post_journal_entry", |id| {
            repo.post_journal_entry(id, "controller").map(drop)
        })
        .op("voided", "void_journal_entry", |id| repo.void_journal_entry(id).map(drop))
        .only_from(&["posted"])
        .op("reversed", "reverse_journal_entry", |id| {
            repo.reverse_journal_entry(id, today).map(drop)
        })
        .op("voided", "void the source invoice (draft entry)", void_source)
        .run_from(&["draft"])
        .op("reversed", "void the source invoice (posted entry)", void_source)
        .run_from(&["posted"])
        .seeded("pending", |id| set("gl_journal_entries", &id.to_string(), "pending"))
        .check()
}

pub(crate) fn credit_account(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let repo = db.credit();
    // One customer per account: suspend and reactivate address the account by
    // its customer.
    let owners = std::cell::RefCell::new(std::collections::HashMap::new());
    let create = || {
        let customer = customer(db).id;
        let account = repo
            .create_credit_account(CreateCreditAccount {
                customer_id: customer,
                credit_limit: dec!(1000),
                ..Default::default()
            })
            .unwrap();
        owners.borrow_mut().insert(account.id, customer);
        account.id
    };
    let status = |id| repo.get_credit_account(id).unwrap().unwrap().status.to_string();
    let owner = |id: CreditId| owners.borrow()[&id];
    let update = |id, value| {
        repo.update_credit_account(
            id,
            UpdateCreditAccount { status: Some(value), ..Default::default() },
        )
        .map(drop)
    };
    let mut check = Conformance::new("credit_account", create, status);
    for (target, label, value) in [
        ("active", "update_credit_account status=active", CreditAccountStatus::Active),
        ("suspended", "update_credit_account status=suspended", CreditAccountStatus::Suspended),
        ("on_hold", "update_credit_account status=on_hold", CreditAccountStatus::OnHold),
        ("closed", "update_credit_account status=closed", CreditAccountStatus::Closed),
        (
            "pending_review",
            "update_credit_account status=pending_review",
            CreditAccountStatus::PendingReview,
        ),
    ] {
        check = check.op(target, label, move |id| update(id, value));
    }
    check
        .op("suspended", "suspend_credit_account", |id| {
            repo.suspend_credit_account(owner(id), "conformance").map(drop)
        })
        .op("active", "reactivate_credit_account", |id| {
            repo.reactivate_credit_account(owner(id)).map(drop)
        })
        .check()
}

pub(crate) fn credit_hold(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let customer = customer(db).id;
    let repo = db.credit();
    repo.create_credit_account(CreateCreditAccount {
        customer_id: customer,
        credit_limit: dec!(1000),
        ..Default::default()
    })
    .unwrap();
    let create = || {
        repo.place_hold(PlaceCreditHold {
            customer_id: customer,
            order_id: None,
            hold_type: CreditHoldType::OverLimit,
            hold_amount: dec!(10),
            reason: "conformance".into(),
            placed_by: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get_hold(id).unwrap().unwrap().status.to_string();
    Conformance::new("credit_hold", create, status)
        .op("released", "release_hold", |id| {
            repo.release_hold(ReleaseCreditHold {
                hold_id: id,
                released_by: Some("controller".into()),
                release_notes: None,
            })
            .map(drop)
        })
        .seeded("expired", |id| set("credit_holds", &id.to_string(), "expired"))
        .check()
}

pub(crate) fn cost_adjustment(db: &dyn Database, _set: SetStatus<'_>) -> Report {
    let sku = format!("SM-COST-{}", Uuid::new_v4().simple());
    let repo = db.cost_accounting();
    let create = || {
        repo.create_adjustment(CreateCostAdjustment {
            sku: sku.clone(),
            adjustment_type: CostAdjustmentType::Revaluation,
            new_cost: dec!(3),
            reason: "conformance".into(),
            created_by: None,
        })
        .unwrap()
        .id
    };
    let status = |id| repo.get_adjustment(id).unwrap().unwrap().status.to_string();
    Conformance::new("cost_adjustment", create, status)
        .op("approved", "approve_adjustment", |id| {
            repo.approve_adjustment(id, "controller").map(drop)
        })
        .op("applied", "apply_adjustment", |id| repo.apply_adjustment(id).map(drop))
        .op("rejected", "reject_adjustment", |id| repo.reject_adjustment(id).map(drop))
        .check()
}

pub(crate) fn payment_run(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let supplier = supplier(db);
    let ap = db.accounts_payable();
    let create = || {
        let bill = ap
            .create_bill(CreateBill {
                created_by: None,
                bill_number: None,
                supplier_id: supplier,
                purchase_order_id: None,
                bill_date: None,
                due_date: Utc::now(),
                payment_terms: None,
                currency: Some(CurrencyCode::USD),
                reference_number: None,
                memo: None,
                items: vec![CreateBillItem {
                    description: "conformance".into(),
                    quantity: dec!(1),
                    unit_price: dec!(50),
                    ..Default::default()
                }],
            })
            .unwrap();
        ap.approve_bill(bill.id).unwrap();
        ap.create_payment_run(CreatePaymentRun {
            payment_date: Utc::now(),
            payment_method: PaymentMethodAP::default(),
            bill_ids: vec![bill.id],
            notes: None,
            created_by: Some("maker".into()),
        })
        .unwrap()
        .id
    };
    let status = |id| ap.get_payment_run(id).unwrap().unwrap().status.to_string();
    let seed = |state: &'static str| move |id: Uuid| set("ap_payment_runs", &id.to_string(), state);
    Conformance::new("payment_run", create, status)
        .op("approved", "approve_payment_run", |id| ap.approve_payment_run(id, "checker").map(drop))
        .op("completed", "process_payment_run", |id| ap.process_payment_run(id).map(drop))
        .op("cancelled", "cancel_payment_run", |id| ap.cancel_payment_run(id).map(drop))
        .seeded("pending", seed("pending"))
        .seeded("processing", seed("processing"))
        .check()
}

pub(crate) fn bill(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let supplier = supplier(db);
    let ap = db.accounts_payable();
    let create = || {
        ap.create_bill(CreateBill {
            created_by: None,
            bill_number: None,
            supplier_id: supplier,
            purchase_order_id: None,
            bill_date: None,
            due_date: Utc::now(),
            payment_terms: None,
            currency: Some(CurrencyCode::USD),
            reference_number: None,
            memo: None,
            items: vec![CreateBillItem {
                description: "conformance".into(),
                quantity: dec!(1),
                unit_price: dec!(100),
                ..Default::default()
            }],
        })
        .unwrap()
        .id
    };
    let status = |id| ap.get_bill(id).unwrap().unwrap().status.to_string();
    let pay = |id: Uuid, amount| {
        ap.create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date: None,
            payment_method: PaymentMethodAP::Check,
            amount,
            currency: Some(CurrencyCode::USD),
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: id, amount }],
        })
        .map(drop)
    };
    let due = |id| ap.get_bill(id).unwrap().unwrap().amount_due;
    let live_payments = |id| -> Vec<Uuid> {
        ap.get_payments_for_bill(id)
            .unwrap()
            .into_iter()
            .filter(|p| p.status != PaymentStatusAP::Voided)
            .map(|p| p.id)
            .collect()
    };
    let seed = |state: &'static str| move |id: Uuid| set("ap_bills", &id.to_string(), state);
    let resolve = |id| ap.resolve_bill_dispute(id).map(drop);
    Conformance::new("bill", create, status)
        .op("approved", "approve_bill", |id| ap.approve_bill(id).map(drop))
        .only_from(&["draft", "pending"])
        .op("cancelled", "cancel_bill", |id| ap.cancel_bill(id).map(drop))
        .op("disputed", "dispute_bill", |id| ap.dispute_bill(id).map(drop))
        .op("draft", "resolve_bill_dispute (disputed from draft)", resolve)
        .run_from(&["disputed"])
        .op("paid", "create_payment (the amount due, in two payments)", |id| {
            let half = (due(id) / dec!(2)).round_dp(2);
            pay(id, half)?;
            pay(id, due(id))
        })
        .refuses_same_status()
        .op("partially_paid", "create_payment (10)", |id| pay(id, dec!(10)))
        .only_from(&["approved", "partially_paid", "overdue"])
        .op("partially_paid", "void_payment (the latest)", |id| {
            let latest = live_payments(id).pop().ok_or(CommerceError::NotFound)?;
            ap.void_payment(latest).map(drop)
        })
        .run_from(&["paid"])
        .op("approved", "void_payment (every payment)", |id| {
            for payment in live_payments(id) {
                ap.void_payment(payment)?;
            }
            Ok(())
        })
        .run_from(&["partially_paid", "paid"])
        .seeded("pending", seed("pending"))
        .seeded("overdue", seed("overdue"))
        .scenario(&["approved", "disputed"], "resolve_bill_dispute", "approved", resolve)
        .scenario(
            &["approved", "partially_paid", "disputed"],
            "resolve_bill_dispute",
            "partially_paid",
            resolve,
        )
        .scenario(&["pending", "disputed"], "resolve_bill_dispute", "pending", resolve)
        .scenario(&["overdue", "disputed"], "resolve_bill_dispute", "overdue", resolve)
        .check()
}

pub(crate) fn bill_payment(db: &dyn Database, set: SetStatus<'_>) -> Report {
    let supplier = supplier(db);
    let ap = db.accounts_payable();
    // Each payment settles its own approved bill.
    let create = || {
        let bill = ap
            .create_bill(CreateBill {
                created_by: None,
                bill_number: None,
                supplier_id: supplier,
                purchase_order_id: None,
                bill_date: None,
                due_date: Utc::now(),
                payment_terms: None,
                currency: Some(CurrencyCode::USD),
                reference_number: None,
                memo: None,
                items: vec![CreateBillItem {
                    description: "conformance".into(),
                    quantity: dec!(1),
                    unit_price: dec!(40),
                    ..Default::default()
                }],
            })
            .unwrap();
        ap.approve_bill(bill.id).unwrap();
        ap.create_payment(CreateBillPayment {
            supplier_id: supplier,
            payment_date: None,
            payment_method: PaymentMethodAP::Check,
            amount: dec!(40),
            currency: Some(CurrencyCode::USD),
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(40) }],
        })
        .unwrap()
        .id
    };
    let status = |id| ap.get_payment(id).unwrap().unwrap().status.to_string();
    let seed = |state: &'static str| move |id: Uuid| set("ap_payments", &id.to_string(), state);
    Conformance::new("bill_payment", create, status)
        .op("cleared", "clear_payment", |id| ap.clear_payment(id).map(drop))
        .op("voided", "void_payment", |id| ap.void_payment(id).map(drop))
        .seeded("processed", seed("processed"))
        .seeded("failed", seed("failed"))
        .check()
}
