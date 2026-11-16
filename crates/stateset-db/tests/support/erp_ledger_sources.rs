use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn source_lifecycle(db: &dyn Database) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    let today = Utc::now().date_naive();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "Source lifecycles".into(),
            fiscal_year: today.year(),
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(today.year(), 12, 31).unwrap(),
        })
        .unwrap();
    gl.open_period(period.id).unwrap();
    let mut config = CreateAutoPostingConfig {
        config_name: "Sources".into(),
        cash_account_id: account("1010"),
        accounts_receivable_account_id: account("1100"),
        inventory_account_id: account("1200"),
        accounts_payable_account_id: account("2010"),
        unearned_revenue_account_id: None,
        sales_revenue_account_id: account("4010"),
        shipping_revenue_account_id: None,
        sales_tax_payable_account_id: None,
        goods_received_not_invoiced_account_id: None,
        inventory_adjustment_account_id: None,
        work_in_progress_account_id: None,
        cogs_account_id: account("5010"),
        bad_debt_expense_account_id: Some(account("5010")),
        fx_gain_loss_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    };
    gl.set_auto_posting_config(config.clone()).unwrap();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("sources-{}@example.com", Uuid::new_v4()),
            first_name: "Source".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .unwrap();
    let invoice = |kind| {
        db.invoices()
            .create(CreateInvoice {
                customer_id: customer.id,
                invoice_type: Some(kind),
                items: vec![CreateInvoiceItem {
                    description: "Service".into(),
                    quantity: dec!(1),
                    unit_price: dec!(100),
                    ..Default::default()
                }],
                ..Default::default()
            })
            .unwrap()
    };
    let first = invoice(InvoiceType::Standard);
    assert!(gl.auto_post_invoice(first.id).is_err(), "draft invoices cannot post");
    db.invoices().send(first.id).unwrap();
    let posted = gl.auto_post_invoice(first.id).unwrap();
    gl.close_period(period.id, "test").unwrap();
    assert!(db.invoices().void(first.id).is_err(), "failed reversal must roll back the void");
    assert_eq!(db.invoices().get(first.id).unwrap().unwrap().status, InvoiceStatus::Sent);
    assert_eq!(
        gl.get_journal_entry(posted.id).unwrap().unwrap().status,
        JournalEntryStatus::Posted
    );
    gl.reopen_period(period.id).unwrap();
    db.invoices().void(first.id).unwrap();
    assert!(gl.auto_post_invoice(first.id).is_err());
    let original = gl.get_journal_entry(posted.id).unwrap().unwrap();
    assert_eq!(original.status, JournalEntryStatus::Reversed);
    let reversal = gl.get_journal_entry(original.reversing_entry_id.unwrap()).unwrap().unwrap();
    assert_eq!(reversal.reversed_entry_id, Some(original.id));
    assert!(gl.reverse_journal_entry(original.id, today).is_err());
    assert_eq!(gl.get_account(account("1100")).unwrap().unwrap().current_balance, dec!(0));

    let sale = invoice(InvoiceType::Standard);
    db.invoices().send(sale.id).unwrap();
    gl.auto_post_invoice(sale.id).unwrap();
    let ar = db.accounts_receivable();
    let credit = ar
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer.id.into(),
            original_invoice_id: Some(sale.id.into()),
            reason: CreditMemoReason::PricingError,
            amount: dec!(25),
            notes: None,
        })
        .unwrap();
    let credit_entry = gl.auto_post_credit_memo(credit.id).unwrap();
    assert!(db.invoices().void(sale.id).is_err(), "Outstanding credits must be reversed first");
    assert!(
        ar.create_credit_memo(CreateCreditMemo {
            customer_id: customer.id.into(),
            original_invoice_id: Some(sale.id.into()),
            reason: CreditMemoReason::PricingError,
            amount: dec!(80),
            notes: None,
        })
        .is_err(),
        "Multiple credits cannot exceed invoice value"
    );
    assert_eq!(
        ar.list_credit_memos(CreditMemoFilter::default()).unwrap().len(),
        1,
        "Failed posting rolls back the memo"
    );
    assert_eq!(credit.gl_journal_entry_id, Some(credit_entry.id));
    assert_eq!(gl.get_account(account("1100")).unwrap().unwrap().current_balance, dec!(75));
    ar.void_credit_memo(credit.id).unwrap();
    assert!(gl.auto_post_credit_memo(credit.id).is_err());
    assert_eq!(gl.get_account(account("1100")).unwrap().unwrap().current_balance, dec!(100));

    let write_off = ar
        .create_write_off(CreateWriteOff {
            invoice_id: sale.id.into(),
            amount: dec!(100),
            reason: WriteOffReason::Uncollectible,
            notes: None,
            approved_by: Some("test".into()),
        })
        .unwrap();
    let write_off_entry = gl.auto_post_write_off(write_off.id).unwrap();
    assert_eq!(
        ar.get_write_off(write_off.id).unwrap().unwrap().gl_journal_entry_id,
        Some(write_off_entry.id)
    );
    ar.reverse_write_off(write_off.id).unwrap();
    assert!(gl.auto_post_write_off(write_off.id).is_err());
    assert_eq!(gl.get_account(account("1100")).unwrap().unwrap().current_balance, dec!(100));

    let credit_invoice = invoice(InvoiceType::CreditMemo);
    db.invoices().send(credit_invoice.id).unwrap();
    gl.auto_post_invoice(credit_invoice.id).unwrap();
    assert!(
        ar.create_credit_memo(CreateCreditMemo {
            customer_id: customer.id.into(),
            original_invoice_id: Some(credit_invoice.id.into()),
            reason: CreditMemoReason::PricingError,
            amount: dec!(10),
            notes: None,
        })
        .is_err(),
        "A credit against a credit invoice would increase receivables"
    );
    assert_eq!(gl.get_account(account("1100")).unwrap().unwrap().current_balance, dec!(0));
    let proforma = invoice(InvoiceType::Proforma);
    db.invoices().send(proforma.id).unwrap();
    assert!(gl.auto_post_invoice(proforma.id).is_err());

    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Source supplier".into(), ..Default::default() })
        .unwrap();
    let ap = db.accounts_payable();
    let bill = ap
        .create_bill(CreateBill {
            supplier_id: supplier.id,
            due_date: Utc::now(),
            items: vec![CreateBillItem {
                description: "Service".into(),
                quantity: dec!(1),
                unit_price: dec!(100),
                account_code: Some("5010".into()),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    assert!(gl.auto_post_bill(bill.id).is_err());
    ap.approve_bill(bill.id).unwrap();
    gl.auto_post_bill(bill.id).unwrap();
    let payment = ap
        .create_payment(CreateBillPayment {
            supplier_id: supplier.id,
            payment_date: None,
            payment_method: PaymentMethodAP::Check,
            amount: dec!(100),
            currency: None,
            reference_number: None,
            bank_account: None,
            check_number: None,
            memo: None,
            allocations: vec![PaymentAllocationInput { bill_id: bill.id, amount: dec!(100) }],
        })
        .unwrap();
    assert!(gl.auto_post_bill_payment(payment.id).is_err(), "pending payments cannot post");
    ap.clear_payment(payment.id).unwrap();
    gl.auto_post_bill_payment(payment.id).unwrap();
    ap.void_payment(payment.id).unwrap();
    assert!(gl.auto_post_bill_payment(payment.id).is_err());
    ap.cancel_bill(bill.id).unwrap();
    assert!(gl.auto_post_bill(bill.id).is_err());
    assert_eq!(gl.get_account(account("2010")).unwrap().unwrap().current_balance, dec!(0));
    assert_eq!(gl.get_account(account("1010")).unwrap().unwrap().current_balance, dec!(0));
    let tax = gl
        .create_account(CreateGlAccount {
            account_number: "2098".into(),
            name: "Tax".into(),
            account_type: AccountType::Liability,
            account_sub_type: None,
            description: None,
            parent_account_id: None,
            is_header: None,
            is_posting: None,
            currency: None,
        })
        .unwrap();
    let shipping = gl
        .create_account(CreateGlAccount {
            account_number: "4098".into(),
            name: "Shipping".into(),
            account_type: AccountType::Revenue,
            account_sub_type: None,
            description: None,
            parent_account_id: None,
            is_header: None,
            is_posting: None,
            currency: None,
        })
        .unwrap();
    config.sales_tax_payable_account_id = Some(tax.id);
    config.shipping_revenue_account_id = Some(shipping.id);
    gl.set_auto_posting_config(config).unwrap();
    let taxed = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            tax_amount: Some(dec!(8)),
            shipping_amount: Some(dec!(2)),
            items: vec![CreateInvoiceItem {
                description: "Taxed sale".into(),
                quantity: dec!(1),
                unit_price: dec!(100),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.invoices().send(taxed.id).unwrap();
    gl.auto_post_invoice(taxed.id).unwrap();
    let memo = ar
        .create_credit_memo(CreateCreditMemo {
            customer_id: customer.id.into(),
            original_invoice_id: Some(taxed.id.into()),
            reason: CreditMemoReason::ReturnedGoods,
            amount: dec!(55),
            notes: None,
        })
        .unwrap();
    let journal = gl.get_journal_entry(memo.gl_journal_entry_id.unwrap()).unwrap().unwrap();
    let debit = |id| {
        journal
            .lines
            .iter()
            .filter(|line| line.account_id == id)
            .map(|line| line.debit_amount)
            .sum::<rust_decimal::Decimal>()
    };
    assert_eq!(debit(account("4010")), dec!(50));
    assert_eq!(debit(tax.id), dec!(4));
    assert_eq!(debit(shipping.id), dec!(1));
    assert!(gl.get_trial_balance(today).unwrap().is_balanced());
}
