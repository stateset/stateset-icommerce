// Uses the sync `Commerce` engine, which only exists with the sqlite backend.
#![cfg(feature = "sqlite")]
#![cfg(all(feature = "sqlite", feature = "events"))]

//! Event emission tests for the finance/operations domains: fixed assets,
//! revenue recognition, cycle counts, three-way match, FX revaluation, and
//! month-end close.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::{
    AccountSubType, AccountType, CommerceEvent, CreateAutoPostingConfig, CreateBill,
    CreateBillItem, CreateCycleCount, CreateCycleCountLine, CreateFixedAsset, CreateGlAccount,
    CreateGlPeriod, CreateJournalEntry, CreateJournalEntryLine, CreatePerformanceObligation,
    CreatePurchaseOrder, CreatePurchaseOrderItem, CreateReceipt, CreateReceiptItem,
    CreateRevenueContract, CreateSupplier, CreateWarehouse, Currency, DepreciationMethod,
    FixedAssetCategory, JournalEntryType, ReceiptType, ReceiveItemLine, ReceiveItems,
    RecognitionMethod, RecordCycleCountLine, RevenueContractStatus, SetExchangeRate,
    UpdateRevenueContract, WarehouseAddress,
};
use stateset_embedded::Commerce;
use stateset_embedded::events::EventSubscription;
use uuid::Uuid;

const fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
}

/// Drain every buffered event from the subscription.
fn drain(sub: &mut EventSubscription) -> Vec<CommerceEvent> {
    let mut events = Vec::new();
    while let Some(event) = sub.try_recv() {
        events.push(event);
    }
    events
}

fn create_asset(commerce: &Commerce) -> stateset_core::FixedAsset {
    commerce
        .fixed_assets()
        .create(CreateFixedAsset {
            asset_number: None,
            name: "Espresso machine".into(),
            description: None,
            category: FixedAssetCategory::Machinery,
            acquisition_date: date(2026, 1, 1),
            acquisition_cost: dec!(1200),
            salvage_value: dec!(0),
            useful_life_months: 12,
            depreciation_method: DepreciationMethod::StraightLine,
            in_service_date: None,
            location_id: None,
            asset_account_id: None,
            accumulated_depreciation_account_id: None,
            depreciation_expense_account_id: None,
            currency: None,
        })
        .expect("create asset")
}

#[tokio::test]
async fn fixed_asset_lifecycle_emits_events() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let asset = create_asset(&commerce);
    let mut sub = commerce.events().subscribe();

    commerce.fixed_assets().place_in_service(asset.id, date(2026, 1, 1)).expect("in service");
    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::FixedAssetPlacedInService {
            asset_id,
            in_service_date,
            acquisition_cost,
            ..
        } => {
            assert_eq!(*asset_id, asset.id);
            assert_eq!(*in_service_date, date(2026, 1, 1));
            assert_eq!(*acquisition_cost, dec!(1200));
        }
        other => panic!("expected FixedAssetPlacedInService, got {}", other.event_type()),
    }

    commerce.fixed_assets().generate_schedule(asset.id).expect("schedule");
    commerce.fixed_assets().post_depreciation(asset.id, 2).expect("post depreciation");
    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::DepreciationPosted {
            asset_id,
            periods,
            amount,
            accumulated_depreciation,
            ..
        } => {
            assert_eq!(*asset_id, asset.id);
            assert_eq!(*periods, 2);
            assert_eq!(*amount, dec!(200));
            assert_eq!(*accumulated_depreciation, dec!(200));
        }
        other => panic!("expected DepreciationPosted, got {}", other.event_type()),
    }

    commerce.fixed_assets().dispose(asset.id, date(2026, 6, 30), dec!(900), None).expect("dispose");
    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::FixedAssetDisposed { asset_id, proceeds, gain_loss, .. } => {
            assert_eq!(*asset_id, asset.id);
            assert_eq!(*proceeds, dec!(900));
            // Book value at disposal: 1200 - 200 = 1000; proceeds 900 => loss 100.
            assert_eq!(*gain_loss, dec!(-100));
        }
        other => panic!("expected FixedAssetDisposed, got {}", other.event_type()),
    }
}

#[tokio::test]
async fn fixed_asset_write_off_emits_event() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let asset = create_asset(&commerce);
    commerce.fixed_assets().place_in_service(asset.id, date(2026, 1, 1)).expect("in service");
    let mut sub = commerce.events().subscribe();

    commerce.fixed_assets().write_off(asset.id, date(2026, 2, 1), None).expect("write off");
    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::FixedAssetWrittenOff { asset_id, write_off_date, loss, .. } => {
            assert_eq!(*asset_id, asset.id);
            assert_eq!(*write_off_date, date(2026, 2, 1));
            assert_eq!(*loss, dec!(1200));
        }
        other => panic!("expected FixedAssetWrittenOff, got {}", other.event_type()),
    }
}

#[tokio::test]
async fn revenue_recognition_emits_recognized_and_contract_completed() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let contract = commerce
        .revenue_recognition()
        .create_contract(CreateRevenueContract {
            contract_number: None,
            customer_id: Uuid::new_v4(),
            order_id: None,
            invoice_id: None,
            transaction_price: dec!(600),
            currency: None,
            effective_date: date(2026, 1, 1),
            obligations: vec![CreatePerformanceObligation {
                description: "Q1 support".into(),
                standalone_selling_price: None,
                allocated_amount: dec!(600),
                recognition_method: RecognitionMethod::RatableOverTime {
                    start: date(2026, 1, 1),
                    end: date(2026, 3, 31),
                },
            }],
        })
        .expect("create contract");
    commerce
        .revenue_recognition()
        .update_contract(
            contract.id,
            UpdateRevenueContract {
                status: Some(RevenueContractStatus::Active),
                ..Default::default()
            },
        )
        .expect("activate contract");
    let obligation_id = contract.obligations[0].id;
    commerce.revenue_recognition().generate_schedule(obligation_id).expect("schedule");

    let mut sub = commerce.events().subscribe();

    // Recognize January only: RevenueRecognized, no completion yet.
    commerce
        .revenue_recognition()
        .recognize_period(obligation_id, date(2026, 1, 31))
        .expect("recognize january");
    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::RevenueRecognized {
            obligation_id: ob, amount, total_recognized, ..
        } => {
            assert_eq!(*ob, obligation_id);
            assert_eq!(*amount, dec!(200));
            assert_eq!(*total_recognized, dec!(200));
        }
        other => panic!("expected RevenueRecognized, got {}", other.event_type()),
    }

    // Recognize the rest: RevenueRecognized + RevenueContractCompleted.
    commerce
        .revenue_recognition()
        .recognize_period(obligation_id, date(2026, 3, 31))
        .expect("recognize rest");
    let events = drain(&mut sub);
    let types: Vec<&str> = events.iter().map(CommerceEvent::event_type).collect();
    assert_eq!(types, vec!["revenue_recognized", "revenue_contract_completed"]);
    match &events[1] {
        CommerceEvent::RevenueContractCompleted { contract_id, transaction_price, .. } => {
            assert_eq!(*contract_id, contract.id);
            assert_eq!(*transaction_price, dec!(600));
        }
        other => panic!("expected RevenueContractCompleted, got {}", other.event_type()),
    }

    // Recognizing again is a no-op and must not re-emit.
    commerce
        .revenue_recognition()
        .recognize_period(obligation_id, date(2026, 12, 31))
        .expect("recognize no-op");
    assert!(drain(&mut sub).is_empty());
}

#[tokio::test]
async fn cycle_count_completion_emits_variance_summary() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: "WH-1".into(),
            name: "Main".into(),
            warehouse_type: Default::default(),
            address: WarehouseAddress {
                street1: "1 Dock St".into(),
                street2: None,
                city: "Reno".into(),
                state: "NV".into(),
                postal_code: "89501".into(),
                country: "US".into(),
                phone: None,
            },
            timezone: None,
        })
        .expect("create warehouse");

    let count = commerce
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id: warehouse.id,
            location_id: None,
            scheduled_date: None,
            counted_by: Some("auditor".into()),
            lines: vec![CreateCycleCountLine {
                sku: "WIDGET-001".into(),
                lot_id: None,
                expected_quantity: dec!(10),
            }],
        })
        .expect("create cycle count");
    commerce.warehouse().start_cycle_count(count.id).expect("start");
    commerce
        .warehouse()
        .record_cycle_counts(
            count.id,
            vec![RecordCycleCountLine {
                sku: "WIDGET-001".into(),
                lot_id: None,
                counted_quantity: dec!(10),
            }],
        )
        .expect("record");

    let mut sub = commerce.events().subscribe();
    commerce.warehouse().complete_cycle_count(count.id).expect("complete");
    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::CycleCountCompleted {
            cycle_count_id,
            warehouse_id,
            line_count,
            variance_line_count,
            total_variance,
            ..
        } => {
            assert_eq!(*cycle_count_id, count.id);
            assert_eq!(*warehouse_id, warehouse.id);
            assert_eq!(*line_count, 1);
            assert_eq!(*variance_line_count, 0);
            assert_eq!(*total_variance, Decimal::ZERO);
        }
        other => panic!("expected CycleCountCompleted, got {}", other.event_type()),
    }
}

#[tokio::test]
async fn three_way_match_variance_emits_event() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let supplier = commerce
        .purchase_orders()
        .create_supplier(CreateSupplier { name: "Acme Supplies".into(), ..Default::default() })
        .expect("supplier");
    let po = commerce
        .purchase_orders()
        .create(CreatePurchaseOrder {
            supplier_id: supplier.id,
            items: vec![CreatePurchaseOrderItem {
                sku: "WIDGET-001".into(),
                name: "Widget".into(),
                quantity: dec!(10),
                unit_cost: dec!(5),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("po");
    let po_id: Uuid = po.id.into();
    let po_line_id = po.items[0].id;

    let warehouse = commerce
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: "WH-1".into(),
            name: "Main".into(),
            warehouse_type: Default::default(),
            address: WarehouseAddress {
                street1: "1 Dock St".into(),
                street2: None,
                city: "Reno".into(),
                state: "NV".into(),
                postal_code: "89501".into(),
                country: "US".into(),
                phone: None,
            },
            timezone: None,
        })
        .expect("warehouse");

    let receipt = commerce
        .receiving()
        .create_receipt(CreateReceipt {
            receipt_type: ReceiptType::PurchaseOrder,
            reference_type: Some("purchase_order".into()),
            reference_id: Some(po_id),
            supplier_id: Some(supplier.id),
            warehouse_id: warehouse.id,
            items: vec![CreateReceiptItem {
                sku: "WIDGET-001".into(),
                po_line_id: Some(po_line_id),
                expected_quantity: dec!(10),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("receipt");
    let receipt_items = commerce.receiving().get_receipt_items(receipt.id).expect("items");
    commerce.receiving().start_receiving(receipt.id).expect("start receiving");
    commerce
        .receiving()
        .receive_items(ReceiveItems {
            receipt_id: receipt.id,
            items: vec![ReceiveItemLine {
                receipt_item_id: receipt_items[0].id,
                quantity_received: dec!(10),
                quantity_rejected: None,
                rejection_reason: None,
                lot_number: None,
                serial_numbers: None,
                expiration_date: None,
                notes: None,
            }],
            receiving_location_id: None,
            received_by: None,
        })
        .expect("receive");

    // Over-billed bill: quantity 12 vs 10 ordered/received.
    let bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier.id,
            purchase_order_id: Some(po_id),
            due_date: chrono::Utc::now() + chrono::Duration::days(30),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                quantity: dec!(12),
                unit_price: dec!(5),
                po_line_id: Some(po_line_id),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("bill");

    let mut sub = commerce.events().subscribe();
    let result = commerce
        .accounts_payable()
        .three_way_match(bill.id, Some(dec!(5)))
        .expect("three way match");
    assert!(matches!(result.match_status, stateset_core::MatchStatus::Variance { .. }));

    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::ThreeWayMatchVarianceDetected {
            bill_id,
            purchase_order_id,
            variance_line_count,
            tolerance_percent,
            ..
        } => {
            assert_eq!(*bill_id, bill.id);
            assert_eq!(*purchase_order_id, po_id);
            assert_eq!(*variance_line_count, 1);
            assert_eq!(*tolerance_percent, dec!(5));
        }
        other => panic!("expected ThreeWayMatchVarianceDetected, got {}", other.event_type()),
    }

    // Matched bill emits nothing.
    let matched_bill = commerce
        .accounts_payable()
        .create_bill(CreateBill {
            supplier_id: supplier.id,
            purchase_order_id: Some(po_id),
            due_date: chrono::Utc::now() + chrono::Duration::days(30),
            items: vec![CreateBillItem {
                description: "Widget".into(),
                quantity: dec!(10),
                unit_price: dec!(5),
                po_line_id: Some(po_line_id),
                ..Default::default()
            }],
            ..Default::default()
        })
        .expect("matched bill");
    commerce
        .accounts_payable()
        .three_way_match(matched_bill.id, Some(dec!(5)))
        .expect("matched three way match");
    assert!(drain(&mut sub).is_empty());
}

/// Chart of accounts, FX gain/loss config, an EUR balance, and an open wide
/// period — enough for `revalue` to post and `close_month` to run.
fn setup_gl_with_fx(commerce: &Commerce) -> Uuid {
    let gl = commerce.general_ledger();
    gl.initialize_chart_of_accounts().expect("init chart");

    let sub = |number: &str, name: &str, ty: AccountType, sub_type: AccountSubType| {
        gl.create_account(CreateGlAccount {
            account_number: number.into(),
            name: name.into(),
            description: None,
            account_type: ty,
            account_sub_type: Some(sub_type),
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: None,
        })
        .expect("create account")
        .id
    };
    let fx_id = sub("7900", "FX Gain/Loss", AccountType::Expense, AccountSubType::OtherExpense);

    let eur_id = gl
        .create_account(CreateGlAccount {
            account_number: "1015".into(),
            name: "EUR Cash".into(),
            description: None,
            account_type: AccountType::Asset,
            account_sub_type: None,
            parent_account_id: None,
            is_header: None,
            is_posting: Some(true),
            currency: Some("EUR".parse().expect("EUR")),
        })
        .expect("create EUR account")
        .id;

    let by_number =
        |n: &str| gl.get_account_by_number(n).expect("get account").expect("account exists").id;
    gl.set_auto_posting_config(CreateAutoPostingConfig {
        config_name: "Financial events test".into(),
        cash_account_id: by_number("1010"),
        accounts_receivable_account_id: by_number("1100"),
        inventory_account_id: by_number("1200"),
        accounts_payable_account_id: by_number("2010"),
        unearned_revenue_account_id: None,
        sales_revenue_account_id: by_number("4010"),
        shipping_revenue_account_id: None,
        cogs_account_id: by_number("5010"),
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: Some(fx_id),
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    })
    .expect("set auto posting config");

    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "FY2026-wide".into(),
            fiscal_year: 2026,
            period_number: 1,
            start_date: date(2020, 1, 1),
            end_date: date(2030, 12, 31),
        })
        .expect("create period");
    gl.open_period(period.id).expect("open period");

    commerce
        .currency()
        .set_rate(SetExchangeRate {
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            rate: dec!(1.00),
            source: Some("test".into()),
        })
        .expect("set rate");
    gl.create_journal_entry(CreateJournalEntry {
        entry_date: date(2026, 6, 15),
        entry_type: Some(JournalEntryType::Standard),
        description: "Seed EUR balance".into(),
        lines: vec![
            CreateJournalEntryLine {
                account_id: eur_id,
                description: None,
                debit_amount: dec!(1000),
                credit_amount: Decimal::ZERO,
                reference_type: None,
                reference_id: None,
            },
            CreateJournalEntryLine {
                account_id: by_number("4010"),
                description: None,
                debit_amount: Decimal::ZERO,
                credit_amount: dec!(1000),
                reference_type: None,
                reference_id: None,
            },
        ],
        source_document_type: None,
        source_document_id: None,
        auto_post: Some(true),
    })
    .expect("seed EUR entry");

    period.id
}

#[tokio::test]
async fn fx_revaluation_and_month_end_close_emit_events() {
    let commerce = Commerce::new(":memory:").expect("commerce");
    let period_id = setup_gl_with_fx(&commerce);

    // Move the rate so the revaluation has a non-zero adjustment.
    commerce
        .currency()
        .set_rate(SetExchangeRate {
            base_currency: Currency::EUR,
            quote_currency: Currency::USD,
            rate: dec!(1.10),
            source: Some("test".into()),
        })
        .expect("set rate");

    let mut sub = commerce.events().subscribe();
    let result = commerce.general_ledger().revalue(date(2026, 6, 30), None).expect("revalue");
    assert!(result.journal_entry.is_some());

    let events = drain(&mut sub);
    assert_eq!(events.len(), 1);
    match &events[0] {
        CommerceEvent::FxRevaluationPosted {
            as_of_date,
            total_unrealized_gain_loss,
            journal_entry_id,
            ..
        } => {
            assert_eq!(*as_of_date, date(2026, 6, 30));
            assert_eq!(*total_unrealized_gain_loss, result.total_unrealized_gain_loss);
            assert_eq!(*journal_entry_id, result.journal_entry.as_ref().map(|e| e.id));
        }
        other => panic!("expected FxRevaluationPosted, got {}", other.event_type()),
    }

    // Month-end close (skipping nothing) emits a summary event; the FX step
    // runs through `revalue`, so an FX event may accompany it.
    let report = commerce
        .general_ledger()
        .close_month(period_id, stateset_core::CloseMonthOptions::default())
        .expect("close month");
    assert!(!report.dry_run);

    let events = drain(&mut sub);
    let close_event = events
        .iter()
        .find(|e| e.event_type() == "month_end_close_completed")
        .expect("month_end_close_completed emitted");
    match close_event {
        CommerceEvent::MonthEndCloseCompleted { period_id: pid, period_name, .. } => {
            assert_eq!(*pid, period_id);
            assert_eq!(period_name, "FY2026-wide");
        }
        other => panic!("expected MonthEndCloseCompleted, got {}", other.event_type()),
    }

    // Dry-run close must not emit.
    let gl = commerce.general_ledger();
    gl.reopen_period(period_id).expect("reopen");
    drain(&mut sub);
    gl.close_month(
        period_id,
        stateset_core::CloseMonthOptions { dry_run: true, ..Default::default() },
    )
    .expect("dry run close");
    assert!(
        !drain(&mut sub).iter().any(|e| e.event_type() == "month_end_close_completed"),
        "dry-run close must not emit month_end_close_completed"
    );
}
