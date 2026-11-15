//! Read-query smoke coverage on a freshly migrated store. Domain refusals for
//! absent fixtures are allowed; every database/query error fails with its call.
use stateset_core::*;
use stateset_db::Database;

pub(crate) fn run(db: &dyn Database) {
    let now = chrono::Utc::now();
    let day = now.date_naive();
    let mut failures = Vec::new();
    macro_rules! check {
        ($call:expr) => {
            if let Err(error) = $call {
                if matches!(
                    error,
                    CommerceError::DatabaseError(_)
                        | CommerceError::Database(_)
                        | CommerceError::NotPermitted(_)
                ) {
                    failures.push(format!("{}: {error}", stringify!($call)));
                }
            }
        };
    }
    // OrderRepository
    check!(db.orders().get(OrderId::from(uuid::Uuid::nil())));
    check!(db.orders().get_by_number("smoke-missing"));
    check!(db.orders().list(Default::default()));
    check!(db.orders().count(Default::default()));
    check!(db.orders().get_batch(vec![OrderId::from(uuid::Uuid::nil())]));
    // InventoryRepository
    check!(db.inventory().get_item(1));
    check!(db.inventory().get_item_by_sku("smoke-missing"));
    check!(db.inventory().get_stock("smoke-missing"));
    check!(db.inventory().get_balance(1, 1));
    check!(db.inventory().get_reservation(uuid::Uuid::nil()));
    check!(db.inventory().list_reservations_by_reference("smoke-missing", "smoke-missing"));
    check!(db.inventory().list(Default::default()));
    check!(db.inventory().get_reorder_needed());
    check!(db.inventory().get_transactions(1, 1));
    check!(db.inventory().get_item_batch(vec![1]));
    check!(db.inventory().get_stock_batch(vec!["smoke-missing".into()]));
    // CustomerRepository
    check!(db.customers().get(CustomerId::from(uuid::Uuid::nil())));
    check!(db.customers().get_by_email("smoke-missing"));
    check!(db.customers().list(Default::default()));
    check!(db.customers().get_addresses(CustomerId::from(uuid::Uuid::nil())));
    check!(db.customers().count(Default::default()));
    check!(db.customers().get_batch(vec![CustomerId::from(uuid::Uuid::nil())]));
    // ProductRepository
    check!(db.products().get(ProductId::from(uuid::Uuid::nil())));
    check!(db.products().get_by_slug("smoke-missing"));
    check!(db.products().list(Default::default()));
    check!(db.products().get_variant(uuid::Uuid::nil()));
    check!(db.products().get_variant_by_sku("smoke-missing"));
    check!(db.products().get_variants(ProductId::from(uuid::Uuid::nil())));
    check!(db.products().count(Default::default()));
    check!(db.products().get_batch(vec![ProductId::from(uuid::Uuid::nil())]));
    // CustomObjectRepository
    check!(db.custom_objects().get_type(uuid::Uuid::nil()));
    check!(db.custom_objects().get_type_by_handle("smoke-missing"));
    check!(db.custom_objects().list_types(Default::default()));
    check!(db.custom_objects().get_object(uuid::Uuid::nil()));
    check!(db.custom_objects().get_object_by_handle("smoke-missing", "smoke-missing"));
    check!(db.custom_objects().list_objects(Default::default()));
    // ReturnRepository
    check!(db.returns().get(ReturnId::from(uuid::Uuid::nil())));
    check!(db.returns().list(Default::default()));
    check!(db.returns().count(Default::default()));
    check!(db.returns().get_batch(vec![ReturnId::from(uuid::Uuid::nil())]));
    // BomRepository
    check!(db.bom().get(uuid::Uuid::nil()));
    check!(db.bom().get_by_number("smoke-missing"));
    check!(db.bom().list(Default::default()));
    check!(db.bom().get_components(uuid::Uuid::nil()));
    check!(db.bom().count(Default::default()));
    check!(db.bom().get_batch(vec![uuid::Uuid::nil()]));
    // WorkOrderRepository
    check!(db.work_orders().get(uuid::Uuid::nil()));
    check!(db.work_orders().get_by_number("smoke-missing"));
    check!(db.work_orders().list(Default::default()));
    check!(db.work_orders().get_tasks(uuid::Uuid::nil()));
    check!(db.work_orders().get_materials(uuid::Uuid::nil()));
    check!(db.work_orders().count(Default::default()));
    check!(db.work_orders().get_batch(vec![uuid::Uuid::nil()]));
    // ShipmentRepository
    check!(db.shipments().get(ShipmentId::from(uuid::Uuid::nil())));
    check!(db.shipments().get_by_number("smoke-missing"));
    check!(db.shipments().get_by_tracking("smoke-missing"));
    check!(db.shipments().list(Default::default()));
    check!(db.shipments().for_order(OrderId::from(uuid::Uuid::nil())));
    check!(db.shipments().get_items(ShipmentId::from(uuid::Uuid::nil())));
    check!(db.shipments().get_events(ShipmentId::from(uuid::Uuid::nil())));
    check!(db.shipments().count(Default::default()));
    check!(db.shipments().get_batch(vec![ShipmentId::from(uuid::Uuid::nil())]));
    // PaymentRepository
    check!(db.payments().get(PaymentId::from(uuid::Uuid::nil())));
    check!(db.payments().get_by_number("smoke-missing"));
    check!(db.payments().get_by_external_id("smoke-missing"));
    check!(db.payments().list(Default::default()));
    check!(db.payments().for_order(OrderId::from(uuid::Uuid::nil())));
    check!(db.payments().for_invoice(InvoiceId::from(uuid::Uuid::nil())));
    check!(db.payments().get_refund(uuid::Uuid::nil()));
    check!(db.payments().get_refunds(PaymentId::from(uuid::Uuid::nil())));
    check!(db.payments().get_payment_methods(CustomerId::from(uuid::Uuid::nil())));
    check!(db.payments().count(Default::default()));
    check!(db.payments().get_batch(vec![PaymentId::from(uuid::Uuid::nil())]));
    // WarrantyRepository
    check!(db.warranties().get(WarrantyId::from(uuid::Uuid::nil())));
    check!(db.warranties().get_by_number("smoke-missing"));
    check!(db.warranties().get_by_serial("smoke-missing"));
    check!(db.warranties().list(Default::default()));
    check!(db.warranties().for_customer(CustomerId::from(uuid::Uuid::nil())));
    check!(db.warranties().for_order(OrderId::from(uuid::Uuid::nil())));
    check!(db.warranties().get_claim(uuid::Uuid::nil()));
    check!(db.warranties().get_claim_by_number("smoke-missing"));
    check!(db.warranties().list_claims(Default::default()));
    check!(db.warranties().get_claims(WarrantyId::from(uuid::Uuid::nil())));
    check!(db.warranties().count(Default::default()));
    check!(db.warranties().count_claims(Default::default()));
    check!(db.warranties().get_batch(vec![WarrantyId::from(uuid::Uuid::nil())]));
    // PurchaseOrderRepository
    check!(db.purchase_orders().get_supplier(uuid::Uuid::nil()));
    check!(db.purchase_orders().get_supplier_by_code("smoke-missing"));
    check!(db.purchase_orders().list_suppliers(Default::default()));
    check!(db.purchase_orders().get(PurchaseOrderId::from(uuid::Uuid::nil())));
    check!(db.purchase_orders().get_by_number("smoke-missing"));
    check!(db.purchase_orders().list(Default::default()));
    check!(db.purchase_orders().for_supplier(uuid::Uuid::nil()));
    check!(db.purchase_orders().get_items(PurchaseOrderId::from(uuid::Uuid::nil())));
    check!(db.purchase_orders().count(Default::default()));
    check!(db.purchase_orders().count_suppliers(Default::default()));
    check!(db.purchase_orders().get_batch(vec![PurchaseOrderId::from(uuid::Uuid::nil())]));
    // InvoiceRepository
    check!(db.invoices().get(InvoiceId::from(uuid::Uuid::nil())));
    check!(db.invoices().get_by_number("smoke-missing"));
    check!(db.invoices().list(Default::default()));
    check!(db.invoices().for_customer(CustomerId::from(uuid::Uuid::nil())));
    check!(db.invoices().for_order(OrderId::from(uuid::Uuid::nil())));
    check!(db.invoices().get_items(InvoiceId::from(uuid::Uuid::nil())));
    check!(db.invoices().get_overdue());
    check!(db.invoices().count(Default::default()));
    check!(db.invoices().get_batch(vec![InvoiceId::from(uuid::Uuid::nil())]));
    // CartRepository
    check!(db.carts().get(CartId::from(uuid::Uuid::nil())));
    check!(db.carts().get_by_number("smoke-missing"));
    check!(db.carts().list(Default::default()));
    check!(db.carts().for_customer(CustomerId::from(uuid::Uuid::nil())));
    check!(db.carts().get_items(CartId::from(uuid::Uuid::nil())));
    check!(db.carts().get_item(uuid::Uuid::nil()));
    check!(db.carts().get_shipping_rates(CartId::from(uuid::Uuid::nil())));
    check!(db.carts().get_abandoned());
    check!(db.carts().get_expired());
    check!(db.carts().count(Default::default()));
    check!(db.carts().get_batch(vec![CartId::from(uuid::Uuid::nil())]));
    // AnalyticsRepository
    check!(db.analytics().get_sales_summary(Default::default()));
    check!(db.analytics().get_revenue_by_period(Default::default()));
    check!(db.analytics().get_top_products(Default::default()));
    check!(db.analytics().get_product_performance(Default::default()));
    check!(db.analytics().get_customer_metrics(Default::default()));
    check!(db.analytics().get_top_customers(Default::default()));
    check!(db.analytics().get_inventory_health());
    check!(db.analytics().get_low_stock_items(None));
    check!(db.analytics().get_inventory_movement(Default::default()));
    check!(db.analytics().get_order_status_breakdown(Default::default()));
    check!(db.analytics().get_fulfillment_metrics(Default::default()));
    check!(db.analytics().get_return_metrics(Default::default()));
    check!(db.analytics().get_demand_forecast(None, 1));
    check!(db.analytics().get_revenue_forecast(1, Default::default()));
    check!(db.analytics().get_sales_summary_batch(vec![Default::default()]));
    // CurrencyRepository
    check!(db.currency().get_rate(Default::default(), Default::default()));
    check!(db.currency().get_rates_for(Default::default()));
    check!(db.currency().list_rates(Default::default()));
    check!(db.currency().get_settings());
    // TaxRepository
    check!(db.tax().get_jurisdiction(uuid::Uuid::nil()));
    check!(db.tax().get_jurisdiction_by_code("smoke-missing"));
    check!(db.tax().list_jurisdictions(Default::default()));
    check!(db.tax().get_rate(uuid::Uuid::nil()));
    check!(db.tax().list_rates(Default::default()));
    check!(db.tax().get_rates_for_address(&Default::default(), Default::default(), day));
    check!(db.tax().get_exemption(uuid::Uuid::nil()));
    check!(db.tax().get_customer_exemptions(uuid::Uuid::nil()));
    check!(db.tax().get_settings());
    check!(db.tax().calculate_tax(Default::default()));
    // PromotionRepository
    check!(db.promotions().get(PromotionId::from(uuid::Uuid::nil())));
    check!(db.promotions().get_by_code("smoke-missing"));
    check!(db.promotions().list(Default::default()));
    check!(db.promotions().get_coupon(uuid::Uuid::nil()));
    check!(db.promotions().get_coupon_by_code("smoke-missing"));
    check!(db.promotions().list_coupons(Default::default()));
    check!(db.promotions().list_usage(Default::default()));
    // SubscriptionRepository
    check!(db.subscriptions().get_plan(uuid::Uuid::nil()));
    check!(db.subscriptions().get_plan_by_code("smoke-missing"));
    check!(db.subscriptions().list_plans(Default::default()));
    check!(db.subscriptions().get_subscription(SubscriptionId::from(uuid::Uuid::nil())));
    check!(db.subscriptions().get_subscription_by_number("smoke-missing"));
    check!(db.subscriptions().list_subscriptions(Default::default()));
    check!(db.subscriptions().get_billing_cycle(uuid::Uuid::nil()));
    check!(db.subscriptions().list_billing_cycles(Default::default()));
    check!(db.subscriptions().get_due_for_billing(now, None));
    check!(db.subscriptions().get_subscription_events(SubscriptionId::from(uuid::Uuid::nil())));
    // QualityRepository
    check!(db.quality().get_inspection(uuid::Uuid::nil()));
    check!(db.quality().get_inspection_by_number("smoke-missing"));
    check!(db.quality().list_inspections(Default::default()));
    check!(db.quality().get_inspection_items(uuid::Uuid::nil()));
    check!(db.quality().count_inspections(Default::default()));
    check!(db.quality().get_ncr(uuid::Uuid::nil()));
    check!(db.quality().get_ncr_by_number("smoke-missing"));
    check!(db.quality().list_ncrs(Default::default()));
    check!(db.quality().count_ncrs(Default::default()));
    check!(db.quality().get_hold(uuid::Uuid::nil()));
    check!(db.quality().list_holds(Default::default()));
    check!(db.quality().get_active_holds_for_sku("smoke-missing"));
    check!(db.quality().get_active_holds_for_lot("smoke-missing"));
    check!(db.quality().count_active_holds());
    check!(db.quality().get_defect_code("smoke-missing"));
    check!(db.quality().list_defect_codes(None));
    // LotRepository
    check!(db.lots().get(uuid::Uuid::nil()));
    check!(db.lots().get_by_number("smoke-missing"));
    check!(db.lots().list(Default::default()));
    check!(db.lots().get_transactions(uuid::Uuid::nil(), 1));
    check!(db.lots().get_quantity_at_location(uuid::Uuid::nil(), 1));
    check!(db.lots().get_lot_locations(uuid::Uuid::nil()));
    check!(db.lots().get_lot_parents(uuid::Uuid::nil()));
    check!(db.lots().get_lot_children(uuid::Uuid::nil()));
    check!(db.lots().get_certificates(uuid::Uuid::nil()));
    check!(db.lots().get_expiring_lots(1));
    check!(db.lots().get_expired_lots());
    check!(db.lots().get_available_lots_for_sku("smoke-missing"));
    check!(db.lots().count(Default::default()));
    check!(db.lots().get_batch(vec![uuid::Uuid::nil()]));
    // SerialRepository
    check!(db.serials().get(uuid::Uuid::nil()));
    check!(db.serials().get_by_serial("smoke-missing"));
    check!(db.serials().list(Default::default()));
    check!(db.serials().get_reservation(uuid::Uuid::nil()));
    check!(db.serials().get_history(uuid::Uuid::nil(), Default::default()));
    check!(db.serials().validate("smoke-missing"));
    check!(db.serials().get_available_for_sku("smoke-missing", 1));
    check!(db.serials().get_for_lot(uuid::Uuid::nil()));
    check!(db.serials().get_for_customer(uuid::Uuid::nil()));
    check!(db.serials().count(Default::default()));
    check!(db.serials().get_batch(vec![uuid::Uuid::nil()]));
    check!(db.serials().get_batch_by_serial(vec!["smoke-missing".into()]));
    // WarehouseRepository
    check!(db.warehouse().get_warehouse(1));
    check!(db.warehouse().get_warehouse_by_code("smoke-missing"));
    check!(db.warehouse().list_warehouses(Default::default()));
    check!(db.warehouse().count_warehouses(Default::default()));
    check!(db.warehouse().get_zone(1));
    check!(db.warehouse().get_zones(1));
    check!(db.warehouse().get_location(1));
    check!(db.warehouse().get_location_by_code(1, "smoke-missing"));
    check!(db.warehouse().list_locations(Default::default()));
    check!(db.warehouse().count_locations(Default::default()));
    check!(db.warehouse().get_locations_for_warehouse(1));
    check!(db.warehouse().get_pickable_locations(1, "smoke-missing"));
    check!(db.warehouse().get_receivable_locations(1));
    check!(db.warehouse().get_location_inventory(1));
    check!(db.warehouse().get_inventory_for_sku(1, "smoke-missing"));
    check!(db.warehouse().list_location_inventory(Default::default()));
    check!(db.warehouse().get_movements(Default::default()));
    check!(db.warehouse().count_movements(Default::default()));
    check!(db.warehouse().get_locations_batch(vec![1]));
    check!(db.warehouse().get_cycle_count(uuid::Uuid::nil()));
    check!(db.warehouse().list_cycle_counts(Default::default()));
    // BinRepository
    check!(db.bins().get_bin(1));
    check!(db.bins().get_bin_by_code(1, "smoke-missing"));
    check!(db.bins().list_bins(Default::default()));
    check!(db.bins().count_bins(Default::default()));
    check!(db.bins().get_bin_levels(1));
    check!(db.bins().get_bin_levels_for_sku(1, "smoke-missing"));
    // ReceivingRepository
    check!(db.receiving().get_receipt(uuid::Uuid::nil()));
    check!(db.receiving().get_receipt_by_number("smoke-missing"));
    check!(db.receiving().list_receipts(Default::default()));
    check!(db.receiving().get_receipt_items(uuid::Uuid::nil()));
    check!(db.receiving().count_receipts(Default::default()));
    check!(db.receiving().get_put_away(uuid::Uuid::nil()));
    check!(db.receiving().list_put_aways(Default::default()));
    check!(db.receiving().get_pending_put_aways(uuid::Uuid::nil()));
    check!(db.receiving().count_put_aways(Default::default()));
    check!(db.receiving().get_receipts_batch(vec![uuid::Uuid::nil()]));
    // FulfillmentRepository
    check!(db.fulfillment().get_wave(FulfillmentId::from(uuid::Uuid::nil())));
    check!(db.fulfillment().get_wave_by_number("smoke-missing"));
    check!(db.fulfillment().list_waves(Default::default()));
    check!(db.fulfillment().get_wave_orders(FulfillmentId::from(uuid::Uuid::nil())));
    check!(db.fulfillment().count_waves(Default::default()));
    check!(db.fulfillment().get_pick(uuid::Uuid::nil()));
    check!(db.fulfillment().list_picks(Default::default()));
    check!(db.fulfillment().get_picks_for_order(OrderId::from(uuid::Uuid::nil())));
    check!(db.fulfillment().get_picks_for_wave(FulfillmentId::from(uuid::Uuid::nil())));
    check!(db.fulfillment().count_picks(Default::default()));
    check!(db.fulfillment().get_pack(uuid::Uuid::nil()));
    check!(db.fulfillment().list_packs(Default::default()));
    check!(db.fulfillment().get_cartons(uuid::Uuid::nil()));
    check!(db.fulfillment().get_carton_items(uuid::Uuid::nil()));
    check!(db.fulfillment().count_packs(Default::default()));
    check!(db.fulfillment().get_ship(uuid::Uuid::nil()));
    check!(db.fulfillment().list_ships(Default::default()));
    check!(db.fulfillment().count_ships(Default::default()));
    check!(db.fulfillment().get_picks_batch(vec![uuid::Uuid::nil()]));
    // AccountsPayableRepository
    check!(db.accounts_payable().get_bill(uuid::Uuid::nil()));
    check!(db.accounts_payable().get_bill_by_number("smoke-missing"));
    check!(db.accounts_payable().list_bills(Default::default()));
    check!(db.accounts_payable().get_bill_items(uuid::Uuid::nil()));
    check!(db.accounts_payable().count_bills(Default::default()));
    check!(db.accounts_payable().get_overdue_bills());
    check!(db.accounts_payable().get_bills_due_soon(1));
    check!(db.accounts_payable().get_payment(uuid::Uuid::nil()));
    check!(db.accounts_payable().get_payment_by_number("smoke-missing"));
    check!(db.accounts_payable().list_payments(Default::default()));
    check!(db.accounts_payable().get_payment_allocations(uuid::Uuid::nil()));
    check!(db.accounts_payable().get_payments_for_bill(uuid::Uuid::nil()));
    check!(db.accounts_payable().count_payments(Default::default()));
    check!(db.accounts_payable().get_payment_run(uuid::Uuid::nil()));
    check!(db.accounts_payable().list_payment_runs(Default::default()));
    check!(db.accounts_payable().get_payment_run_bills(uuid::Uuid::nil()));
    check!(db.accounts_payable().get_aging_summary());
    check!(db.accounts_payable().get_supplier_summary(uuid::Uuid::nil()));
    check!(db.accounts_payable().get_total_outstanding());
    check!(db.accounts_payable().get_bills_batch(vec![uuid::Uuid::nil()]));
    // CostAccountingRepository
    check!(db.cost_accounting().get_item_cost("smoke-missing"));
    check!(db.cost_accounting().list_item_costs(Default::default()));
    check!(db.cost_accounting().get_cost_layer(uuid::Uuid::nil()));
    check!(db.cost_accounting().list_cost_layers(Default::default()));
    check!(db.cost_accounting().get_layers_remaining("smoke-missing"));
    check!(db.cost_accounting().list_cost_transactions(Default::default()));
    check!(db.cost_accounting().list_variances(Default::default()));
    check!(db.cost_accounting().get_variance_summary(now, now));
    check!(db.cost_accounting().get_adjustment(uuid::Uuid::nil()));
    check!(db.cost_accounting().list_adjustments(Default::default()));
    check!(db.cost_accounting().get_rollup("smoke-missing"));
    check!(db.cost_accounting().get_inventory_valuation(Default::default()));
    check!(db.cost_accounting().get_sku_cost_summary("smoke-missing"));
    check!(db.cost_accounting().get_total_inventory_value());
    check!(db.cost_accounting().transaction_cogs(now.date_naive(), now.date_naive()));
    check!(db.cost_accounting().close_books_check());
    // CreditRepository
    check!(db.credit().get_credit_account(CreditId::from(uuid::Uuid::nil())));
    check!(db.credit().get_credit_account_by_customer(CustomerId::from(uuid::Uuid::nil())));
    check!(db.credit().list_credit_accounts(Default::default()));
    check!(
        db.credit().check_credit(CustomerId::from(uuid::Uuid::nil()), rust_decimal::Decimal::ONE)
    );
    check!(db.credit().get_hold(uuid::Uuid::nil()));
    check!(db.credit().list_holds(Default::default()));
    check!(db.credit().get_active_holds(CustomerId::from(uuid::Uuid::nil())));
    check!(db.credit().get_holds_for_order(OrderId::from(uuid::Uuid::nil())));
    check!(db.credit().get_application(uuid::Uuid::nil()));
    check!(db.credit().list_applications(Default::default()));
    check!(db.credit().list_transactions(Default::default()));
    check!(db.credit().get_customer_summary(CustomerId::from(uuid::Uuid::nil())));
    check!(db.credit().get_aging_report());
    check!(db.credit().get_over_limit_customers());
    // BackorderRepository
    check!(db.backorder().get_backorder(uuid::Uuid::nil()));
    check!(db.backorder().get_backorder_by_number("smoke-missing"));
    check!(db.backorder().list_backorders(Default::default()));
    check!(db.backorder().get_backorders_for_order(uuid::Uuid::nil()));
    check!(db.backorder().get_backorders_for_customer(uuid::Uuid::nil()));
    check!(db.backorder().get_backorders_for_sku("smoke-missing"));
    check!(db.backorder().get_fulfillment_history(uuid::Uuid::nil()));
    check!(db.backorder().get_allocations(uuid::Uuid::nil()));
    check!(db.backorder().get_summary());
    check!(db.backorder().get_sku_summary("smoke-missing"));
    check!(db.backorder().get_overdue_backorders());
    check!(db.backorder().count_pending());
    // AccountsReceivableRepository
    check!(db.accounts_receivable().get_aging_summary());
    check!(db.accounts_receivable().get_customer_aging(uuid::Uuid::nil()));
    check!(db.accounts_receivable().get_aging_report(Default::default()));
    check!(db.accounts_receivable().list_collection_activities(Default::default()));
    check!(db.accounts_receivable().get_invoices_due_for_dunning());
    check!(db.accounts_receivable().get_write_off(uuid::Uuid::nil()));
    check!(db.accounts_receivable().list_write_offs(Default::default()));
    check!(db.accounts_receivable().get_credit_memo(uuid::Uuid::nil()));
    check!(db.accounts_receivable().get_credit_memo_by_number("smoke-missing"));
    check!(db.accounts_receivable().list_credit_memos(Default::default()));
    check!(db.accounts_receivable().get_unapplied_credits(uuid::Uuid::nil()));
    check!(db.accounts_receivable().get_payment_applications(uuid::Uuid::nil()));
    check!(db.accounts_receivable().get_customer_summary(uuid::Uuid::nil()));
    check!(db.accounts_receivable().get_total_outstanding());
    check!(db.accounts_receivable().get_dso(1));
    check!(db.accounts_receivable().get_average_days_to_pay(uuid::Uuid::nil()));
    check!(db.accounts_receivable().get_customers_batch(vec![uuid::Uuid::nil()]));
    // GeneralLedgerRepository
    check!(db.general_ledger().get_account(uuid::Uuid::nil()));
    check!(db.general_ledger().get_account_by_number("smoke-missing"));
    check!(db.general_ledger().list_accounts(Default::default()));
    check!(db.general_ledger().get_account_hierarchy());
    check!(db.general_ledger().get_period(uuid::Uuid::nil()));
    check!(db.general_ledger().get_current_period());
    check!(db.general_ledger().get_period_for_date(day));
    check!(db.general_ledger().list_periods(Default::default()));
    check!(db.general_ledger().get_journal_entry(uuid::Uuid::nil()));
    check!(db.general_ledger().get_journal_entry_by_number("smoke-missing"));
    check!(db.general_ledger().list_journal_entries(Default::default()));
    check!(db.general_ledger().get_journal_entry_lines(uuid::Uuid::nil()));
    check!(db.general_ledger().get_auto_posting_config());
    check!(db.general_ledger().get_trial_balance(day));
    check!(db.general_ledger().get_balance_sheet(day));
    check!(db.general_ledger().get_income_statement(day, day));
    check!(db.general_ledger().get_account_balance(uuid::Uuid::nil(), None));
    check!(db.general_ledger().get_account_transactions(uuid::Uuid::nil(), Default::default()));
    check!(db.general_ledger().get_accounts_batch(vec![uuid::Uuid::nil()]));
    // X402PaymentIntentRepository
    check!(db.x402_payment_intents().get(uuid::Uuid::nil()));
    check!(db.x402_payment_intents().get_by_idempotency_key("smoke-missing"));
    check!(db.x402_payment_intents().for_cart(uuid::Uuid::nil()));
    check!(db.x402_payment_intents().for_order(uuid::Uuid::nil()));
    check!(db.x402_payment_intents().get_next_nonce("smoke-missing"));
    check!(db.x402_payment_intents().list(Default::default()));
    check!(db.x402_payment_intents().count(Default::default()));
    check!(db.x402_payment_intents().get_batch(vec![uuid::Uuid::nil()]));
    // X402CreditRepository
    check!(db.x402_credits().get_account("smoke-missing", Default::default(), Default::default()));
    check!(db.x402_credits().get_balance("smoke-missing", Default::default(), Default::default()));
    check!(db.x402_credits().list_transactions(Default::default()));
    // A2ACommerceRepository
    check!(db.a2a_quotes().get_quote(uuid::Uuid::nil()));
    check!(db.a2a_quotes().get_quote_by_number("smoke-missing"));
    check!(db.a2a_quotes().list_quotes(Default::default()));
    check!(db.a2a_quotes().count_quotes(Default::default()));
    check!(db.a2a_quotes().get_purchase(uuid::Uuid::nil()));
    check!(db.a2a_quotes().get_purchase_by_number("smoke-missing"));
    check!(db.a2a_quotes().list_purchases(Default::default()));
    check!(db.a2a_quotes().count_purchases(Default::default()));
    // A2ACommerceRepository
    check!(db.a2a_purchases().get_quote(uuid::Uuid::nil()));
    check!(db.a2a_purchases().get_quote_by_number("smoke-missing"));
    check!(db.a2a_purchases().list_quotes(Default::default()));
    check!(db.a2a_purchases().count_quotes(Default::default()));
    check!(db.a2a_purchases().get_purchase(uuid::Uuid::nil()));
    check!(db.a2a_purchases().get_purchase_by_number("smoke-missing"));
    check!(db.a2a_purchases().list_purchases(Default::default()));
    check!(db.a2a_purchases().count_purchases(Default::default()));
    // A2ACreditTermsRepository
    check!(db.a2a_credit_terms().get_terms("smoke-missing", uuid::Uuid::nil()));
    check!(db.a2a_credit_terms().list_terms(Default::default()));
    check!(db.a2a_credit_terms().list_entries("smoke-missing", uuid::Uuid::nil()));
    // A2AMessagingRepository
    check!(db.a2a_messages().get_message("smoke-missing", uuid::Uuid::nil()));
    check!(db.a2a_messages().list_messages(Default::default()));
    // AgentCardRepository
    check!(db.agent_cards().get(uuid::Uuid::nil()));
    check!(db.agent_cards().get_by_wallet("smoke-missing"));
    check!(db.agent_cards().list(Default::default()));
    check!(db.agent_cards().count(Default::default()));
    check!(db.agent_cards().get_batch(vec![uuid::Uuid::nil()]));
    // AgentIdentityRepository
    check!(db.agent_identities().get("smoke-missing", "smoke-missing"));
    check!(db.agent_identities().get_by_wallet("smoke-missing"));
    check!(db.agent_identities().list(Default::default()));
    check!(db.agent_identities().count(Default::default()));
    check!(db.agent_identities().get_metadata("smoke-missing", "smoke-missing", "smoke-missing"));
    // AgentReputationRepository
    check!(db.agent_reputation().get_summary(
        "smoke-missing",
        "smoke-missing",
        vec!["smoke-missing".into()],
        None,
        None
    ));
    check!(db.agent_reputation().get_response_count(
        "smoke-missing",
        "smoke-missing",
        "smoke-missing",
        1,
        None
    ));
    check!(db.agent_reputation().get_clients("smoke-missing", "smoke-missing"));
    check!(db.agent_reputation().get_last_index("smoke-missing", "smoke-missing", "smoke-missing"));
    // AgentValidationRepository
    check!(db.agent_validation().get_validation_status("smoke-missing"));
    check!(db.agent_validation().get_summary("smoke-missing", "smoke-missing", None, None));
    check!(db.agent_validation().get_agent_validations("smoke-missing", "smoke-missing"));
    check!(db.agent_validation().get_validator_requests("smoke-missing"));
    // GiftCardRepository
    check!(db.gift_cards().get(GiftCardId::from(uuid::Uuid::nil())));
    check!(db.gift_cards().get_by_code("smoke-missing"));
    check!(db.gift_cards().list(Default::default()));
    check!(db.gift_cards().get_transactions(GiftCardId::from(uuid::Uuid::nil())));
    // StoreCreditRepository
    check!(db.store_credits().get(StoreCreditId::from(uuid::Uuid::nil())));
    check!(db.store_credits().list(Default::default()));
    check!(db.store_credits().get_transactions(StoreCreditId::from(uuid::Uuid::nil())));
    // SegmentRepository
    check!(db.segments().get(SegmentId::from(uuid::Uuid::nil())));
    check!(db.segments().list(Default::default()));
    check!(db.segments().list_members(SegmentId::from(uuid::Uuid::nil()), None, None));
    check!(db.segments().count_members(SegmentId::from(uuid::Uuid::nil())));
    // ShippingZoneRepository
    check!(db.shipping_zones().get(ShippingZoneId::from(uuid::Uuid::nil())));
    check!(db.shipping_zones().list(Default::default()));
    check!(db.shipping_zones().find_matching_zones("smoke-missing", None, None));
    // ZoneShippingMethodRepository
    check!(db.zone_shipping_methods().get(ShippingMethodId::from(uuid::Uuid::nil())));
    check!(db.zone_shipping_methods().list(Default::default()));
    check!(db.zone_shipping_methods().calculate_rates(ZoneShippingRateRequest {
        country: "US".into(),
        region: None,
        postal_code: None,
        weight: None,
        order_total: None,
        currency: CurrencyCode::USD
    }));
    // ReviewRepository
    check!(db.reviews().get(ReviewId::from(uuid::Uuid::nil())));
    check!(db.reviews().list(Default::default()));
    check!(db.reviews().get_summary(ProductId::from(uuid::Uuid::nil())));
    // WishlistRepository
    check!(db.wishlists().get(WishlistId::from(uuid::Uuid::nil())));
    check!(db.wishlists().list(Default::default()));
    // LoyaltyProgramRepository
    check!(db.loyalty_programs().get(LoyaltyProgramId::from(uuid::Uuid::nil())));
    check!(db.loyalty_programs().list());
    check!(db.loyalty_programs().get_account(LoyaltyAccountId::from(uuid::Uuid::nil())));
    check!(db.loyalty_programs().get_account_by_customer(
        CustomerId::from(uuid::Uuid::nil()),
        LoyaltyProgramId::from(uuid::Uuid::nil())
    ));
    check!(db.loyalty_programs().list_accounts(Default::default()));
    check!(db.loyalty_programs().get_transactions(LoyaltyAccountId::from(uuid::Uuid::nil()), None));
    // RewardRepository
    check!(db.rewards().get(RewardId::from(uuid::Uuid::nil())));
    check!(db.rewards().list(Default::default()));
    // FraudRepository
    check!(db.fraud().get_assessment(OrderId::from(uuid::Uuid::nil())));
    check!(db.fraud().list_assessments(Default::default()));
    check!(db.fraud().get_rule(FraudRuleId::from(uuid::Uuid::nil())));
    check!(db.fraud().list_rules(Default::default()));
    check!(db.fraud().get_active_rules());
    // SearchConfigRepository
    check!(db.search_configs().get(SearchConfigId::from(uuid::Uuid::nil())));
    check!(db.search_configs().list(Default::default()));
    check!(db.search_configs().get_active());
    // ChannelRepository
    check!(db.channels().get(ChannelId::from(uuid::Uuid::nil())));
    check!(db.channels().list(Default::default()));
    check!(db.channels().list_product_mappings(ChannelId::from(uuid::Uuid::nil())));
    // CompanyRepository
    check!(db.companies().get(CompanyId::from(uuid::Uuid::nil())));
    check!(db.companies().list(Default::default()));
    check!(db.companies().list_addresses(CompanyId::from(uuid::Uuid::nil())));
    check!(db.companies().list_price_overrides(CompanyId::from(uuid::Uuid::nil())));
    check!(db.companies().get_contact(ContactId::from(uuid::Uuid::nil())));
    check!(db.companies().list_contacts(CompanyId::from(uuid::Uuid::nil())));
    // TransferOrderRepository
    check!(db.transfer_orders().get(TransferOrderId::from(uuid::Uuid::nil())));
    check!(db.transfer_orders().list(Default::default()));
    // UnitOfMeasureRepository
    check!(db.units_of_measure().list_classes());
    check!(db.units_of_measure().list_uoms(Default::default()));
    check!(db.units_of_measure().list_rules());
    // ProductionBatchRepository
    check!(db.production_batches().get(ProductionBatchId::from(uuid::Uuid::nil())));
    check!(db.production_batches().list(Default::default()));
    // SupplierSkuRepository
    check!(db.supplier_skus().get(SupplierSkuId::from(uuid::Uuid::nil())));
    check!(db.supplier_skus().list(Default::default()));
    // VendorReturnRepository
    check!(db.vendor_returns().get(VendorReturnId::from(uuid::Uuid::nil())));
    check!(db.vendor_returns().list(Default::default()));
    // VendorCreditRepository
    check!(db.vendor_credits().get(VendorCreditId::from(uuid::Uuid::nil())));
    check!(db.vendor_credits().list(Default::default()));
    check!(db.vendor_credits().list_applications(VendorCreditId::from(uuid::Uuid::nil())));
    // PaymentObligationRepository
    check!(db.payment_obligations().get(PaymentObligationId::from(uuid::Uuid::nil())));
    check!(db.payment_obligations().list(Default::default()));
    check!(db.payment_obligations().dashboard(day));
    // PriceLevelRepository
    check!(db.price_levels().get(PriceLevelId::from(uuid::Uuid::nil())));
    check!(db.price_levels().list(Default::default()));
    check!(db.price_levels().list_entries(PriceLevelId::from(uuid::Uuid::nil())));
    // PrepaymentRepository
    check!(db.prepayments().get(PrepaymentId::from(uuid::Uuid::nil())));
    check!(db.prepayments().list(Default::default()));
    check!(db.prepayments().list_applications(PrepaymentId::from(uuid::Uuid::nil())));
    // PriceScheduleRepository
    check!(db.price_schedules().get(PriceScheduleId::from(uuid::Uuid::nil())));
    check!(db.price_schedules().list(Default::default()));
    check!(db.price_schedules().list_entries(PriceScheduleId::from(uuid::Uuid::nil())));
    // ActivityLogRepository
    check!(db.activity_logs().get(ActivityLogId::from(uuid::Uuid::nil())));
    check!(db.activity_logs().list(Default::default()));
    // IntegrationMappingRepository
    check!(db.integration_mappings().get(IntegrationMappingId::from(uuid::Uuid::nil())));
    check!(db.integration_mappings().list(Default::default()));
    // InboundShipmentRepository
    check!(db.inbound_shipments().get(InboundShipmentId::from(uuid::Uuid::nil())));
    check!(db.inbound_shipments().list(Default::default()));
    // PurgatoryRepository
    check!(db.purgatory().get(PurgatoryOrderId::from(uuid::Uuid::nil())));
    check!(db.purgatory().list(Default::default()));
    // PrintStationRepository
    check!(db.print_stations().list_stations());
    check!(db.print_stations().get_station(PrintStationId::from(uuid::Uuid::nil())));
    check!(
        db.print_stations().list_jobs(PrintStationId::from(uuid::Uuid::nil()), Default::default())
    );
    // EdiDocumentRepository
    check!(db.edi_documents().get(EdiDocumentId::from(uuid::Uuid::nil())));
    check!(db.edi_documents().list(Default::default()));
    check!(db.edi_documents().summary());
    // IntegrationFieldMappingRepository
    check!(db.integration_field_mappings().get(IntegrationFieldMappingId::from(uuid::Uuid::nil())));
    check!(db.integration_field_mappings().list(Default::default()));
    // TopologySnapshotRepository
    check!(db.topology_snapshots().get(TopologySnapshotId::from(uuid::Uuid::nil())));
    check!(db.topology_snapshots().list(Default::default()));
    // StockSnapshotRepository
    check!(db.stock_snapshots().get(StockSnapshotId::from(uuid::Uuid::nil())));
    check!(db.stock_snapshots().list(Default::default()));
    // FixedAssetRepository
    check!(db.fixed_assets().get(Default::default()));
    check!(db.fixed_assets().list(Default::default()));
    check!(db.fixed_assets().get_schedule(Default::default()));
    // RevenueRecognitionRepository
    check!(db.revenue_recognition().get_contract(Default::default()));
    check!(db.revenue_recognition().list_contracts(Default::default()));
    check!(db.revenue_recognition().list_obligations(Default::default()));
    check!(db.revenue_recognition().get_schedule(Default::default()));
    assert!(failures.is_empty(), "repository query failures:\n{}", failures.join("\n"));
}
