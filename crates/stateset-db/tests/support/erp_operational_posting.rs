use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use stateset_core::*;
use stateset_db::Database;
use uuid::Uuid;

fn setup(db: &dyn Database) -> (GlPeriod, CreateAutoPostingConfig) {
    let gl = db.general_ledger();
    gl.initialize_chart_of_accounts().unwrap();
    let account = |number| gl.get_account_by_number(number).unwrap().unwrap().id;
    let year = Utc::now().year();
    let period = gl
        .create_period(CreateGlPeriod {
            period_name: "Operations".into(),
            fiscal_year: year,
            period_number: 1,
            start_date: NaiveDate::from_ymd_opt(year, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(year, 12, 31).unwrap(),
        })
        .unwrap();
    let period = gl.open_period(period.id).unwrap();
    let wip = gl
        .create_account(CreateGlAccount {
            account_number: "1211".into(),
            name: "Work in progress".into(),
            account_type: AccountType::Asset,
            account_sub_type: Some(AccountSubType::Inventory),
            description: None,
            parent_account_id: None,
            is_header: None,
            is_posting: None,
            currency: None,
        })
        .unwrap()
        .id;
    let config = CreateAutoPostingConfig {
        config_name: "Operations".into(),
        cash_account_id: account("1010"),
        accounts_receivable_account_id: account("1100"),
        inventory_account_id: account("1200"),
        accounts_payable_account_id: account("2010"),
        unearned_revenue_account_id: None,
        sales_revenue_account_id: account("4010"),
        shipping_revenue_account_id: None,
        sales_tax_payable_account_id: None,
        goods_received_not_invoiced_account_id: Some(account("2010")),
        inventory_adjustment_account_id: Some(account("5010")),
        work_in_progress_account_id: Some(wip),
        cogs_account_id: account("5010"),
        bad_debt_expense_account_id: None,
        fx_gain_loss_account_id: None,
        auto_post_depreciation: false,
        auto_post_revenue_recognition: false,
    };
    gl.set_auto_posting_config(config.clone()).unwrap();
    (period, config)
}
fn entries(db: &dyn Database, kind: &str, id: Uuid) -> Vec<JournalEntry> {
    db.general_ledger()
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some(kind.into()),
            source_document_id: Some(id),
            ..Default::default()
        })
        .unwrap()
}
fn balance(db: &dyn Database, id: Uuid) -> Decimal {
    db.general_ledger().get_account(id).unwrap().unwrap().current_balance
}

pub(crate) fn refunds(db: &dyn Database) {
    let (period, config) = setup(db);
    let payment =
        db.payments().create(CreatePayment { amount: dec!(100), ..Default::default() }).unwrap();
    db.general_ledger().close_period(period.id, "test").unwrap();
    assert!(db.payments().mark_completed(payment.id).is_err());
    assert_eq!(
        db.payments().get(payment.id).unwrap().unwrap().status,
        PaymentTransactionStatus::Pending
    );
    assert!(entries(db, "payment", payment.id.into()).is_empty());
    db.general_ledger().reopen_period(period.id).unwrap();
    db.payments().mark_completed(payment.id).unwrap();
    let entry = entries(db, "payment", payment.id.into());
    assert_eq!(entry.len(), 1);
    assert_eq!(
        db.general_ledger().auto_post_payment_received(payment.id.into()).unwrap().id,
        entry[0].id
    );
    let refund = db
        .payments()
        .create_refund(CreateRefund {
            payment_id: payment.id,
            amount: Some(dec!(30)),
            ..Default::default()
        })
        .unwrap();
    db.general_ledger().close_period(period.id, "test").unwrap();
    assert!(db.payments().complete_refund(refund.id).is_err());
    assert_eq!(db.payments().get_refund(refund.id).unwrap().unwrap().status, RefundStatus::Pending);
    assert_eq!(db.payments().get(payment.id).unwrap().unwrap().amount_refunded, dec!(0));
    db.general_ledger().reopen_period(period.id).unwrap();
    db.payments().complete_refund(refund.id).unwrap();
    db.payments().complete_refund(refund.id).unwrap();
    assert_eq!(entries(db, "refund", refund.id).len(), 1);
    assert_eq!(balance(db, config.cash_account_id), dec!(70));
    assert_eq!(balance(db, config.accounts_receivable_account_id), dec!(-70));
    db.payments()
        .update(
            payment.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Disputed),
                ..Default::default()
            },
        )
        .unwrap();
    db.payments()
        .update(
            payment.id,
            UpdatePayment {
                status: Some(PaymentTransactionStatus::Refunded),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(balance(db, config.cash_account_id), dec!(0));
    assert_eq!(balance(db, config.accounts_receivable_account_id), dec!(0));
}
fn destination(db: &dyn Database) -> (i32, i32) {
    let warehouse = db
        .warehouse()
        .create_warehouse(CreateWarehouse {
            code: Uuid::new_v4().to_string(),
            name: "Costed stock".into(),
            ..Default::default()
        })
        .unwrap();
    let location = db
        .warehouse()
        .create_location(CreateLocation { warehouse_id: warehouse.id, ..Default::default() })
        .unwrap();
    (warehouse.id, location.id)
}
fn putaway(
    db: &dyn Database,
    warehouse: i32,
    location: i32,
    sku: &str,
    qty: Decimal,
    cost: Decimal,
) -> Uuid {
    let repo = db.receiving();
    let receipt = repo
        .create_receipt(CreateReceipt {
            warehouse_id: warehouse,
            items: vec![CreateReceiptItem {
                sku: sku.into(),
                expected_quantity: qty,
                unit_cost: Some(cost),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    let item = repo.get_receipt_items(receipt.id).unwrap()[0].id;
    repo.start_receiving(receipt.id).unwrap();
    repo.receive_items(ReceiveItems {
        receipt_id: receipt.id,
        receiving_location_id: Some(location),
        received_by: None,
        items: vec![ReceiveItemLine {
            receipt_item_id: item,
            quantity_received: qty,
            quantity_rejected: None,
            rejection_reason: None,
            lot_number: None,
            serial_numbers: None,
            expiration_date: None,
            notes: None,
        }],
    })
    .unwrap();
    repo.complete_receiving(receipt.id).unwrap();
    repo.create_put_away(CreatePutAway {
        receipt_id: receipt.id,
        receipt_item_id: item,
        sku: sku.into(),
        from_location_id: None,
        to_location_id: location,
        quantity: qty,
        lot_id: None,
        assigned_to: None,
        notes: None,
    })
    .unwrap()
    .id
}
fn finish(db: &dyn Database, id: Uuid) -> Result<PutAway> {
    db.receiving().complete_put_away(CompletePutAway {
        put_away_id: id,
        actual_location_id: None,
        completed_by: None,
        notes: None,
    })
}

pub(crate) fn stock(db: &dyn Database) {
    let (period, config) = setup(db);
    let (warehouse, location) = destination(db);
    let sku = "COSTED";
    db.cost_accounting()
        .set_item_cost(SetItemCost {
            sku: sku.into(),
            cost_method: Some(CostMethod::Fifo),
            ..Default::default()
        })
        .unwrap();
    let first = putaway(db, warehouse, location, sku, dec!(4), dec!(3));
    let mut broken = config.clone();
    broken.goods_received_not_invoiced_account_id = None;
    db.general_ledger().set_auto_posting_config(broken).unwrap();
    assert!(finish(db, first).is_err());
    assert!(db.warehouse().get_location_inventory(location).unwrap().is_empty());
    assert!(db.cost_accounting().list_cost_layers(CostLayerFilter::default()).unwrap().is_empty());
    db.general_ledger().set_auto_posting_config(config.clone()).unwrap();
    finish(db, first).unwrap();
    assert!(finish(db, first).is_err());
    let second = putaway(db, warehouse, location, sku, dec!(4), dec!(5));
    finish(db, second).unwrap();
    assert_eq!(balance(db, config.inventory_account_id), dec!(32));
    let _order = ship(db, sku, 5);
    assert_eq!(balance(db, config.inventory_account_id), dec!(15));
    assert_eq!(balance(db, config.cogs_account_id), dec!(17));
    let layers = db
        .cost_accounting()
        .list_cost_layers(CostLayerFilter {
            sku: Some(sku.into()),
            has_remaining: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(layers.iter().map(|l| l.remaining_quantity).sum::<Decimal>(), dec!(3));
    let count = db
        .warehouse()
        .create_cycle_count(CreateCycleCount {
            warehouse_id: warehouse,
            location_id: Some(location),
            lines: vec![CreateCycleCountLine {
                sku: sku.into(),
                lot_id: None,
                expected_quantity: dec!(999),
            }],
            ..Default::default()
        })
        .unwrap();
    db.warehouse().start_cycle_count(count.id).unwrap();
    db.warehouse()
        .record_cycle_counts(
            count.id,
            vec![RecordCycleCountLine { sku: sku.into(), lot_id: None, counted_quantity: dec!(2) }],
        )
        .unwrap();
    db.general_ledger().close_period(period.id, "test").unwrap();
    assert!(db.warehouse().complete_cycle_count(count.id).is_err());
    assert_eq!(db.inventory().get_stock(sku).unwrap().unwrap().total_on_hand, dec!(3));
    db.general_ledger().reopen_period(period.id).unwrap();
    db.warehouse().complete_cycle_count(count.id).unwrap();
    assert_eq!(balance(db, config.inventory_account_id), dec!(10));
    let transactions =
        db.cost_accounting().list_cost_transactions(CostTransactionFilter::default()).unwrap();
    for tx in transactions {
        assert_eq!(
            db.general_ledger().auto_post_inventory_cost(tx.id).unwrap().unwrap().id,
            entries(db, "cost_transaction", tx.id)[0].id
        );
    }
}

pub(crate) fn manufacturing(db: &dyn Database) {
    let (_, config) = setup(db);
    let (warehouse, location) = destination(db);
    let mut products = Vec::new();
    for sku in ["RAW", "FINISHED"] {
        products.push(
            db.products()
                .create(CreateProduct {
                    name: sku.into(),
                    variants: Some(vec![CreateProductVariant {
                        sku: sku.into(),
                        price: dec!(10),
                        ..Default::default()
                    }]),
                    ..Default::default()
                })
                .unwrap()
                .id,
        );
        db.cost_accounting()
            .set_item_cost(SetItemCost {
                sku: sku.into(),
                cost_method: Some(CostMethod::Fifo),
                ..Default::default()
            })
            .unwrap();
    }
    finish(db, putaway(db, warehouse, location, "RAW", dec!(6), dec!(3))).unwrap();
    let bom = db
        .bom()
        .create(CreateBom {
            product_id: products[1],
            name: "Assembly".into(),
            components: Some(vec![CreateBomComponent {
                component_sku: Some("RAW".into()),
                name: "Raw".into(),
                quantity: dec!(2),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    db.bom().activate(bom.id).unwrap();
    let wo = db
        .work_orders()
        .create(CreateWorkOrder {
            product_id: products[1],
            bom_id: Some(bom.id),
            quantity_to_build: dec!(3),
            stock: Some(WorkOrderStock {
                source_location_id: location,
                output_location_id: location,
                output_sku: "FINISHED".into(),
            }),
            ..Default::default()
        })
        .unwrap();
    db.work_orders().start(wo.id).unwrap();
    let mut broken = config.clone();
    broken.work_in_progress_account_id = None;
    db.general_ledger().set_auto_posting_config(broken).unwrap();
    assert!(db.work_orders().complete(wo.id, dec!(1)).is_err());
    assert_eq!(db.inventory().get_stock("RAW").unwrap().unwrap().total_on_hand, dec!(6));
    assert_eq!(db.work_orders().get(wo.id).unwrap().unwrap().quantity_completed, dec!(0));
    db.general_ledger().set_auto_posting_config(config.clone()).unwrap();
    db.work_orders().consume_material(wo.materials[0].id, dec!(3)).unwrap();
    assert_eq!(balance(db, config.work_in_progress_account_id.unwrap()), dec!(9));
    db.work_orders().complete(wo.id, dec!(1)).unwrap();
    assert_eq!(balance(db, config.work_in_progress_account_id.unwrap()), dec!(3));
    db.work_orders().complete(wo.id, dec!(2)).unwrap();
    assert_eq!(balance(db, config.work_in_progress_account_id.unwrap()), dec!(0));
    assert_eq!(balance(db, config.inventory_account_id), dec!(18));
    assert_eq!(balance(db, config.cogs_account_id), dec!(0));
    let output = db
        .cost_accounting()
        .list_cost_layers(CostLayerFilter { sku: Some("FINISHED".into()), ..Default::default() })
        .unwrap();
    assert_eq!(output.iter().map(|l| l.quantity * l.unit_cost).sum::<Decimal>(), dec!(18));
    assert!(db.work_orders().complete(wo.id, dec!(1)).is_err());
}

pub(crate) fn restock(db: &dyn Database) {
    let (period, config) = setup(db);
    let (warehouse, location) = destination(db);
    finish(db, putaway(db, warehouse, location, "RETURNED", dec!(3), dec!(4))).unwrap();
    let order = ship(db, "RETURNED", 2);
    assert_eq!(balance(db, config.cogs_account_id), dec!(8));
    // A new receipt changes current cost but must not change the returned units' cost.
    finish(db, putaway(db, warehouse, location, "RETURNED", dec!(1), dec!(10))).unwrap();
    let returned = db
        .returns()
        .create(CreateReturn {
            order_id: order.id,
            items: vec![CreateReturnItem {
                order_item_id: order.items[0].id,
                quantity: 1,
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.returns().approve(returned.id).unwrap();
    for status in [ReturnStatus::InTransit, ReturnStatus::Received] {
        db.returns()
            .update(returned.id, UpdateReturn { status: Some(status), ..Default::default() })
            .unwrap();
    }
    let input = SetReturnDisposition { warehouse_id: Some(warehouse), ..Default::default() };
    db.general_ledger().close_period(period.id, "test").unwrap();
    assert!(
        db.returns()
            .set_item_disposition(returned.id, returned.items[0].id, input.clone())
            .is_err()
    );
    assert!(db.returns().get(returned.id).unwrap().unwrap().items[0].disposition.is_none());
    db.general_ledger().reopen_period(period.id).unwrap();
    let cogs_before = balance(db, config.cogs_account_id);
    db.returns().set_item_disposition(returned.id, returned.items[0].id, input).unwrap();
    assert_eq!(balance(db, config.cogs_account_id), cogs_before - dec!(4));
    assert_eq!(balance(db, config.inventory_account_id), dec!(18));
}

fn ship(db: &dyn Database, sku: &str, quantity: i32) -> Order {
    let customer = db
        .customers()
        .create(CreateCustomer {
            email: format!("{}@example.com", Uuid::new_v4()),
            first_name: "Cost".into(),
            last_name: "Return".into(),
            ..Default::default()
        })
        .unwrap();
    let product = db
        .products()
        .create(CreateProduct {
            name: sku.into(),
            variants: Some(vec![CreateProductVariant {
                sku: sku.into(),
                price: dec!(20),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    db.products()
        .update(
            product.id,
            UpdateProduct { status: Some(ProductStatus::Active), ..Default::default() },
        )
        .unwrap();
    let order = db
        .orders()
        .create(CreateOrder {
            customer_id: customer.id,
            items: vec![CreateOrderItem {
                product_id: product.id,
                sku: sku.into(),
                name: sku.into(),
                quantity,
                unit_price: dec!(20),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    for status in [OrderStatus::Confirmed, OrderStatus::Processing, OrderStatus::Shipped] {
        db.orders()
            .update(order.id, UpdateOrder { status: Some(status), ..Default::default() })
            .unwrap();
    }
    db.inventory().fulfill_order_stock(order.id.into()).unwrap();
    order
}

pub(crate) fn methods(db: &dyn Database) {
    let (_, config) = setup(db);
    let (warehouse, location) = destination(db);
    for (method, expected) in [
        (CostMethod::Fifo, dec!(15)),
        (CostMethod::Lifo, dec!(9)),
        (CostMethod::Average, dec!(12)),
        (CostMethod::Standard, dec!(6)),
    ] {
        let sku = format!("METHOD-{method}");
        db.cost_accounting()
            .set_item_cost(SetItemCost {
                sku: sku.clone(),
                cost_method: Some(method),
                standard_cost: Some(dec!(2)),
                ..Default::default()
            })
            .unwrap();
        finish(db, putaway(db, warehouse, location, &sku, dec!(4), dec!(3))).unwrap();
        finish(db, putaway(db, warehouse, location, &sku, dec!(4), dec!(5))).unwrap();
        ship(db, &sku, 5);
        let summary = db.cost_accounting().get_sku_cost_summary(&sku).unwrap().unwrap();
        assert_eq!(summary.quantity_on_hand, dec!(3));
        assert_eq!(summary.total_value, expected, "{method} carrying cost");
        assert!(
            db.cost_accounting()
                .set_item_cost(SetItemCost {
                    sku: sku.clone(),
                    cost_method: Some(CostMethod::Specific),
                    ..Default::default()
                })
                .is_err()
        );
        assert!(
            db.cost_accounting()
                .issue_fifo(IssueCostLayers {
                    sku,
                    quantity: dec!(1),
                    reference_type: None,
                    reference_id: None,
                    notes: None
                })
                .is_err(),
            "Managed stock cannot be relieved twice through a cost-only command"
        );
    }
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(42));
    assert_eq!(balance(db, config.inventory_account_id), dec!(42));
    let report = db.stock_snapshots().inventory_aging(Utc::now().date_naive()).unwrap();
    assert_eq!(report.total_quantity, dec!(12));
    assert_eq!(report.total_value, dec!(42));
    assert_eq!(report.buckets[0].value, dec!(42));
    assert_eq!(
        db.stock_snapshots()
            .inventory_aging(Utc::now().date_naive().pred_opt().unwrap())
            .unwrap()
            .total_value,
        dec!(0)
    );
    let adjustment = db
        .cost_accounting()
        .create_adjustment(CreateCostAdjustment {
            sku: "METHOD-standard".into(),
            adjustment_type: CostAdjustmentType::Revaluation,
            new_cost: dec!(3),
            reason: "Approved inventory revaluation".into(),
            created_by: None,
        })
        .unwrap();
    db.cost_accounting().approve_adjustment(adjustment.id, "operator").unwrap();
    db.cost_accounting().apply_adjustment(adjustment.id).unwrap();
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(45));
    assert_eq!(balance(db, config.inventory_account_id), dec!(45));
    assert_eq!(
        db.stock_snapshots().inventory_aging(Utc::now().date_naive()).unwrap().total_value,
        dec!(45)
    );
    assert!(db.cost_accounting().apply_adjustment(adjustment.id).is_err());
}

pub(crate) fn transit(db: &dyn Database) {
    let (_, config) = setup(db);
    let (warehouse, source) = destination(db);
    let (_, dest) = destination(db);
    let sku = "TRANSIT";
    let product = db
        .products()
        .create(CreateProduct {
            name: sku.into(),
            variants: Some(vec![CreateProductVariant {
                sku: sku.into(),
                price: dec!(10),
                ..Default::default()
            }]),
            ..Default::default()
        })
        .unwrap();
    finish(db, putaway(db, warehouse, source, sku, dec!(5), dec!(3))).unwrap();
    let transfer = db
        .transfer_orders()
        .create(CreateTransferOrder {
            source_warehouse_id: WarehouseId::new(),
            destination_warehouse_id: WarehouseId::new(),
            source_location_id: source,
            destination_location_id: dest,
            items: vec![CreateTransferOrderItem {
                product_id: product.id,
                sku: sku.into(),
                lot_id: None,
                quantity: dec!(4),
            }],
            expected_at: None,
            notes: None,
        })
        .unwrap();
    db.transfer_orders().ship(transfer.id).unwrap();
    assert_eq!(db.inventory().get_stock(sku).unwrap().unwrap().total_on_hand, dec!(1));
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(15));
    assert_eq!(
        db.stock_snapshots().inventory_aging(Utc::now().date_naive()).unwrap().total_quantity,
        dec!(5)
    );
    db.transfer_orders().receive_line(transfer.id, transfer.items[0].id, dec!(3)).unwrap();
    db.transfer_orders().complete_receiving(transfer.id).unwrap();
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(12));
    assert_eq!(balance(db, config.inventory_account_id), dec!(12));
    assert_eq!(
        db.stock_snapshots().inventory_aging(Utc::now().date_naive()).unwrap().total_value,
        dec!(12)
    );
}

pub(crate) fn foreign_stock(db: &dyn Database) {
    let (_, mut config) = setup(db);
    config.fx_gain_loss_account_id = Some(config.cogs_account_id);
    db.general_ledger().set_auto_posting_config(config.clone()).unwrap();
    let rate = |value| {
        db.currency()
            .set_rate(SetExchangeRate {
                base_currency: Currency::EUR,
                quote_currency: Currency::USD,
                rate: value,
                source: Some("operational test".into()),
            })
            .unwrap()
    };
    rate(dec!(1.2));
    let (warehouse, location) = destination(db);
    db.cost_accounting()
        .set_item_cost(SetItemCost {
            sku: "FOREIGN".into(),
            cost_method: Some(CostMethod::Fifo),
            currency: Some(CurrencyCode::EUR),
            ..Default::default()
        })
        .unwrap();
    finish(db, putaway(db, warehouse, location, "FOREIGN", dec!(10), dec!(6))).unwrap();
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(72));
    let entries = db
        .general_ledger()
        .list_journal_entries(JournalEntryFilter {
            source_document_type: Some("cost_transaction".into()),
            ..Default::default()
        })
        .unwrap();
    let receipt = db.general_ledger().get_journal_entry(entries[0].id).unwrap().unwrap();
    let accrual = receipt
        .lines
        .iter()
        .find(|line| line.account_id == config.goods_received_not_invoiced_account_id.unwrap())
        .unwrap();
    assert_eq!(accrual.transaction_currency, Some(CurrencyCode::EUR));
    assert_eq!(accrual.transaction_credit_amount, Some(dec!(60)));
    rate(dec!(1.5));
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(72));
    ship(db, "FOREIGN", 4);
    assert_eq!(balance(db, config.inventory_account_id), dec!(43.2));
    assert_eq!(
        db.stock_snapshots().inventory_aging(Utc::now().date_naive()).unwrap().total_value,
        dec!(43.2)
    );
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(43.2));
    let fx = db.general_ledger().revalue(Utc::now().date_naive(), None).unwrap();
    assert_eq!(fx.total_unrealized_gain_loss, dec!(-18));
    assert_eq!(balance(db, config.goods_received_not_invoiced_account_id.unwrap()), dec!(90));
    assert!(
        db.cost_accounting()
            .set_item_cost(SetItemCost {
                sku: "FOREIGN".into(),
                currency: Some(CurrencyCode::USD),
                ..Default::default()
            })
            .is_err()
    );
}

pub(crate) fn specific_lots(db: &dyn Database) {
    let (_, config) = setup(db);
    let (warehouse, location) = destination(db);
    db.cost_accounting()
        .set_item_cost(SetItemCost {
            sku: "SPECIFIC".into(),
            cost_method: Some(CostMethod::Specific),
            standard_cost: Some(dec!(3)),
            ..Default::default()
        })
        .unwrap();
    let parent = db
        .lots()
        .create(CreateLot {
            sku: "SPECIFIC".into(),
            quantity: dec!(5),
            cost_per_unit: Some(dec!(5)),
            ..Default::default()
        })
        .unwrap();
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location,
            sku: "SPECIFIC".into(),
            lot_id: Some(parent.id),
            quantity: dec!(5),
            reason: "Opening lot stock".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    let child = db
        .lots()
        .split(SplitLot { lot_id: parent.id, quantity: dec!(2), ..Default::default() })
        .unwrap();
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(25));
    assert_eq!(
        db.stock_snapshots().inventory_aging(Utc::now().date_naive()).unwrap().total_value,
        dec!(25)
    );
    assert_eq!(balance(db, config.inventory_account_id), dec!(25));
    db.warehouse()
        .adjust_inventory(AdjustLocationInventory {
            location_id: location,
            sku: "SPECIFIC".into(),
            lot_id: Some(child.id),
            quantity: dec!(-1),
            reason: "Damaged child lot".into(),
            reference_type: None,
            reference_id: None,
            performed_by: None,
        })
        .unwrap();
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(20));
    assert_eq!(
        db.stock_snapshots().inventory_aging(Utc::now().date_naive()).unwrap().total_value,
        dec!(20)
    );
    assert_eq!(
        db.inventory()
            .get_balance(db.inventory().get_item_by_sku("SPECIFIC").unwrap().unwrap().id, warehouse)
            .unwrap()
            .unwrap()
            .quantity_on_hand,
        dec!(4)
    );
}

pub(crate) fn unconfigured_cost_currency(db: &dyn Database) {
    let (warehouse, location) = destination(db);
    finish(db, putaway(db, warehouse, location, "OPENING", dec!(5), dec!(3))).unwrap();
    assert!(
        db.general_ledger().list_journal_entries(JournalEntryFilter::default()).unwrap().is_empty()
    );
    let mut settings = db.currency().get_settings().unwrap();
    settings.base_currency = Currency::EUR;
    assert!(
        db.currency().update_settings(settings).is_err(),
        "Functional currency also freezes when inventory is costed without GL configuration"
    );
    assert_eq!(db.cost_accounting().get_total_inventory_value().unwrap(), dec!(15));
}

pub(crate) fn sales_reporting(db: &dyn Database) {
    let (period, _) = setup(db);
    let (warehouse, location) = destination(db);
    finish(db, putaway(db, warehouse, location, "REPORT-SKU", dec!(3), dec!(4))).unwrap();
    let order = ship(db, "REPORT-SKU", 2);
    let invoice = db
        .invoices()
        .create(CreateInvoice {
            customer_id: order.customer_id,
            items: vec![CreateInvoiceItem {
                sku: Some("REPORT-SKU".into()),
                description: "Sale".into(),
                quantity: dec!(2),
                unit_price: dec!(20),
                ..Default::default()
            }],
            ..Default::default()
        })
        .unwrap();
    db.invoices().send(invoice.id).unwrap();
    let posted = db.general_ledger().auto_post_invoice(invoice.id).unwrap();
    let report = db.cost_accounting().transaction_cogs(period.start_date, period.end_date).unwrap();
    assert_eq!(report.total_revenue, dec!(40));
    assert_eq!(report.total_cogs, dec!(8));
    assert_eq!(report.gross_margin, dec!(32));
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].sku, "REPORT-SKU");
    assert_eq!(report.rows[0].quantity, dec!(2));
    assert!(db.cost_accounting().close_books_check().unwrap().is_ready);
    db.general_ledger().reverse_journal_entry(posted.id, Utc::now().date_naive()).unwrap();
    let reversed =
        db.cost_accounting().transaction_cogs(period.start_date, period.end_date).unwrap();
    assert_eq!(reversed.total_revenue, dec!(0));
    assert_eq!(reversed.total_cogs, dec!(8));
}
