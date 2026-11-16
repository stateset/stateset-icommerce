use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

pub(crate) fn invoice_posting(db: &dyn Database) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let acct = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    let extra = |number: &str, kind| {
        gl.create_account(CreateGlAccount {
            account_number: number.into(),
            name: number.into(),
            account_type: kind,
            description: None,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: None,
            currency: None,
        })
        .unwrap()
        .id
    };
    let tax = extra("2098", AccountType::Liability);
    let shipping = extra("4098", AccountType::Revenue);
    let deferred = extra("2099", AccountType::Liability);
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "2026-03".into(),
            fiscal_year: 2026,
            period_number: 3,
            start_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 3, 31).unwrap(),
        })
        .unwrap();
    gl.open_period(period.id).unwrap();
    let mut config = CreateAutoPostingConfig {
        config_name: "ERP".into(),
        cash_account_id: acct("1010"),
        accounts_receivable_account_id: acct("1100"),
        inventory_account_id: acct("1200"),
        accounts_payable_account_id: acct("2010"),
        unearned_revenue_account_id: Some(deferred),
        sales_revenue_account_id: acct("4010"),
        shipping_revenue_account_id: None,
        sales_tax_payable_account_id: None,
        goods_received_not_invoiced_account_id: None,
        inventory_adjustment_account_id: None,
        work_in_progress_account_id: None,
        cogs_account_id: acct("5010"),
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: true,
    };
    gl.set_auto_posting_config(config.clone()).unwrap();
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("invoice-{}@example.com", Uuid::new_v4()),
            first_name: "Invoice".into(),
            last_name: "Test".into(),
            ..Default::default()
        })
        .unwrap();
    let invoice = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            invoice_date: Some("2026-03-10T14:30:00Z".parse().unwrap()),
            discount_amount: Some(dec!(10)),
            tax_amount: Some(dec!(8)),
            shipping_amount: Some(dec!(12)),
            items: vec![CreateInvoiceItem {
                description: "Goods".into(),
                quantity: dec!(2),
                unit_price: dec!(50),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.invoices().send(invoice.id).unwrap();
    assert!(
        gl.auto_post_invoice(invoice.id).is_err(),
        "Missing tax account must refuse atomically"
    );
    assert_eq!(gl.get_account(acct("1100")).unwrap().unwrap().current_balance, dec!(0));
    config.sales_tax_payable_account_id = Some(tax);
    gl.set_auto_posting_config(config.clone()).unwrap();
    assert!(gl.auto_post_invoice(invoice.id).is_err(), "Shipping also needs an explicit account");
    config.shipping_revenue_account_id = Some(shipping);
    let saved = gl.set_auto_posting_config(config.clone()).unwrap();
    assert_eq!(saved.sales_tax_payable_account_id, Some(tax));
    let entry = gl.auto_post_invoice(invoice.id).unwrap();
    let credit = |account| {
        entry
            .lines
            .iter()
            .filter(|l| l.account_id == account)
            .map(|l| l.credit_amount)
            .sum::<Decimal>()
    };
    assert_eq!(entry.total_debits, dec!(110));
    assert_eq!(credit(acct("4010")), dec!(90));
    assert_eq!(credit(tax), dec!(8));
    assert_eq!(credit(shipping), dec!(12));
    assert_eq!(entry.id, gl.auto_post_invoice(invoice.id).unwrap().id);
    let make_contract = |invoice_id, amount| CreateRevenueContract {
        contract_number: None,
        customer_id: customer.id.into(),
        order_id: None,
        invoice_id,
        transaction_price: amount,
        currency: None,
        effective_date: period.start_date,
        obligations: vec![CreatePerformanceObligation {
            description: "Service".into(),
            standalone_selling_price: None,
            allocated_amount: amount,
            recognition_method: RecognitionMethod::PointInTime,
        }],
    };
    assert!(
        db.revenue_recognition()
            .create_contract(make_contract(Some(invoice.id.into()), dec!(90)))
            .is_err(),
        "Already-posted sales cannot later be linked and recognized again"
    );
    let service = db
        .invoices()
        .create(CreateInvoice {
            customer_id: customer.id,
            invoice_date: Some("2026-03-12T00:00:00Z".parse().unwrap()),
            items: vec![CreateInvoiceItem {
                description: "Service".into(),
                quantity: dec!(1),
                unit_price: dec!(1200),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.invoices().send(service.id).unwrap();
    assert!(
        db.revenue_recognition()
            .create_contract(make_contract(Some(service.id.into()), dec!(1199)))
            .is_err()
    );
    let mut wrong_customer = make_contract(Some(service.id.into()), dec!(1200));
    wrong_customer.customer_id = Uuid::new_v4();
    assert!(db.revenue_recognition().create_contract(wrong_customer).is_err());
    let contract = db
        .revenue_recognition()
        .create_contract(make_contract(Some(service.id.into()), dec!(1200)))
        .unwrap();
    assert!(
        db.revenue_recognition()
            .create_contract(make_contract(Some(service.id.into()), dec!(1200)))
            .is_err()
    );
    db.revenue_recognition()
        .update_contract(
            contract.id,
            UpdateRevenueContract {
                status: Some(RevenueContractStatus::Active),
                ..Default::default()
            },
        )
        .unwrap();
    let obligation = contract.obligations[0].id;
    db.revenue_recognition().generate_schedule(obligation).unwrap();
    assert!(
        db.revenue_recognition().recognize_period(obligation, period.end_date).is_err(),
        "Recognition must wait for the invoice to fund deferred revenue"
    );
    let entry = gl.auto_post_invoice(service.id).unwrap();
    assert!(entry.lines.iter().any(|l| l.account_id == deferred && l.credit_amount == dec!(1200)));
    assert!(!entry.lines.iter().any(|l| l.account_id == acct("4010")));
    db.revenue_recognition().recognize_period(obligation, period.end_date).unwrap();
    db.revenue_recognition().recognize_period(obligation, period.end_date).unwrap();
    assert_eq!(gl.get_account(deferred).unwrap().unwrap().current_balance, dec!(0));
    assert_eq!(gl.get_account(acct("4010")).unwrap().unwrap().current_balance, dec!(1290));
    // Receipts accrue GRNI; supplier bills clear it without debiting inventory again.
    let grni = extra("2088", AccountType::Liability);
    let adjustment = extra("5088", AccountType::Expense);
    config.goods_received_not_invoiced_account_id = Some(grni);
    config.inventory_adjustment_account_id = Some(adjustment);
    gl.set_auto_posting_config(config.clone()).unwrap();
    let today = Utc::now().date_naive();
    if today < period.start_date || today > period.end_date {
        let current = gl
            .create_period(CreateGlPeriod {
                period_name: "Cost posting day".into(),
                fiscal_year: today.year(),
                period_number: 12,
                start_date: today,
                end_date: today,
            })
            .unwrap();
        gl.open_period(current.id).unwrap();
    }
    let cost = |kind, quantity| {
        db.cost_accounting()
            .record_cost_transaction(
                "LEDGER-STOCK",
                kind,
                quantity,
                dec!(5),
                None,
                None,
                None,
                None,
            )
            .unwrap()
    };
    let receipt = cost(CostTransactionType::Receipt, dec!(20));
    let received = gl.auto_post_inventory_cost(receipt.id).unwrap().unwrap();
    assert_eq!(received.total_debits, dec!(100));
    assert_eq!(gl.get_account(grni).unwrap().unwrap().current_balance, dec!(100));
    let supplier = db
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Ledger supplier".into(), ..Default::default() })
        .unwrap();
    let bill = db
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier.id,
            bill_date: Some(Utc::now()),
            due_date: Utc::now(),
            items: vec![
                CreateBillItem {
                    description: "Received goods".into(),
                    quantity: dec!(20),
                    unit_price: dec!(5),
                    ..Default::default()
                },
                CreateBillItem {
                    description: "Other expense".into(),
                    account_code: Some("5088".into()),
                    quantity: dec!(1),
                    unit_price: dec!(25),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })
        .unwrap();
    db.accounts_payable().approve_bill(bill.id).unwrap();
    let billed = gl.auto_post_bill(bill.id).unwrap();
    assert_eq!(billed.total_debits, dec!(125));
    assert_eq!(gl.get_account(grni).unwrap().unwrap().current_balance, dec!(0));
    assert_eq!(gl.get_account(acct("1200")).unwrap().unwrap().current_balance, dec!(100));
    assert_eq!(gl.get_account(adjustment).unwrap().unwrap().current_balance, dec!(25));
    assert_eq!(gl.get_account(acct("5010")).unwrap().unwrap().current_balance, dec!(0));
    let transfer = cost(CostTransactionType::Transfer, dec!(10));
    assert!(gl.auto_post_inventory_cost(transfer.id).unwrap().is_none());
    let loss = cost(CostTransactionType::Adjustment, dec!(-2));
    gl.auto_post_inventory_cost(loss.id).unwrap().unwrap();
    assert_eq!(gl.get_account(adjustment).unwrap().unwrap().current_balance, dec!(35));
    assert_eq!(gl.get_account(acct("1200")).unwrap().unwrap().current_balance, dec!(90));
    let issue = cost(CostTransactionType::Issue, dec!(4));
    gl.auto_post_inventory_cost(issue.id).unwrap().unwrap();
    assert_eq!(gl.get_account(acct("5010")).unwrap().unwrap().current_balance, dec!(20));
    assert_eq!(gl.get_account(acct("1200")).unwrap().unwrap().current_balance, dec!(70));
    // A late run retains each month's own date; no open period for the run day is needed.
    let accumulated = extra("1599", AccountType::Asset);
    let depreciation = extra("5099", AccountType::Expense);
    config.auto_post_depreciation = true;
    gl.set_auto_posting_config(config).unwrap();
    let april = gl
        .create_period(CreateGlPeriod {
            period_name: "2026-04".into(),
            fiscal_year: 2026,
            period_number: 4,
            start_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
        })
        .unwrap();
    gl.open_period(april.id).unwrap();
    let asset_input = CreateFixedAsset {
        asset_number: None,
        name: "Press".into(),
        description: None,
        category: FixedAssetCategory::Equipment,
        acquisition_date: period.start_date,
        acquisition_cost: dec!(1200),
        salvage_value: dec!(0),
        useful_life_months: 12,
        depreciation_method: DepreciationMethod::StraightLine,
        in_service_date: Some(period.start_date),
        location_id: None,
        asset_account_id: Some(acct("1200")),
        accumulated_depreciation_account_id: Some(accumulated),
        acquisition_offset_account_id: None,
        disposal_gain_loss_account_id: None,
        depreciation_expense_account_id: Some(depreciation),
        currency: None,
    };
    let asset = db.fixed_assets().create(asset_input.clone()).unwrap();
    db.fixed_assets().generate_schedule(asset.id).unwrap();
    db.fixed_assets().post_depreciation(asset.id, 2).unwrap();
    let entries = gl
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some("fixed_asset_depreciation".into()),
            source_document_id: Some(asset.id),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.entry_date == period.end_date && e.total_debits == dec!(100)));
    assert!(entries.iter().any(|e| e.entry_date == april.end_date && e.total_debits == dec!(100)));
    let mut future_input = asset_input;
    future_input.in_service_date = Some(today.checked_add_months(chrono::Months::new(1)).unwrap());
    let future = db.fixed_assets().create(future_input).unwrap();
    db.fixed_assets().generate_schedule(future.id).unwrap();
    assert!(db.fixed_assets().post_depreciation(future.id, 1).is_err());
    assert_eq!(
        db.fixed_assets().get(future.id).unwrap().unwrap().accumulated_depreciation,
        dec!(0)
    );
}
