-- Performance optimization indexes (PostgreSQL)

CREATE INDEX IF NOT EXISTS idx_inventory_balances_item_location
    ON inventory_balances(item_id, location_id);

CREATE INDEX IF NOT EXISTS idx_credit_reservations_customer_order
    ON credit_reservations(customer_id, order_id, status);

CREATE INDEX IF NOT EXISTS idx_serial_reservations_serial_status
    ON serial_reservations(serial_id, released_at, confirmed_at);

CREATE INDEX IF NOT EXISTS idx_pick_tasks_order_status
    ON pick_tasks(order_id, status);

CREATE INDEX IF NOT EXISTS idx_pack_tasks_order_status
    ON pack_tasks(order_id, status);

CREATE INDEX IF NOT EXISTS idx_product_variants_product_active
    ON product_variants(product_id, is_active);

CREATE INDEX IF NOT EXISTS idx_exchange_rates_currencies
    ON exchange_rates(base_currency, quote_currency);

CREATE INDEX IF NOT EXISTS idx_orders_list_cover
    ON orders(status, order_date, customer_id);

CREATE INDEX IF NOT EXISTS idx_inventory_reservations_cover
    ON inventory_reservations(item_id, status, reference_type, reference_id);

CREATE INDEX IF NOT EXISTS idx_backorders_fulfillment
    ON backorders(status, priority, expected_date, sku);

CREATE INDEX IF NOT EXISTS idx_lots_sku_status
    ON lots(sku, status);

CREATE INDEX IF NOT EXISTS idx_serial_numbers_sku_status
    ON serial_numbers(sku, status);

CREATE INDEX IF NOT EXISTS idx_location_inventory_picking
    ON location_inventory(sku, location_id, quantity_on_hand);

CREATE INDEX IF NOT EXISTS idx_cost_layers_sku_remaining
    ON cost_layers(sku, remaining_quantity, layer_date);

CREATE INDEX IF NOT EXISTS idx_ap_bills_aging
    ON ap_bills(status, due_date, supplier_id);

CREATE INDEX IF NOT EXISTS idx_invoices_aging
    ON invoices(status, due_date, customer_id);

CREATE INDEX IF NOT EXISTS idx_subscriptions_billing
    ON subscriptions(status, next_billing_date);

CREATE INDEX IF NOT EXISTS idx_quality_holds_active
    ON quality_holds(released_at, sku, location_id);

CREATE INDEX IF NOT EXISTS idx_work_orders_scheduling
    ON manufacturing_work_orders(status, scheduled_start, priority);
