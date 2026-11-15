-- Historical carrying costs remain with owned stock, including goods in transit.
ALTER TABLE gl_auto_posting_config ADD COLUMN work_in_progress_account_id UUID REFERENCES gl_accounts(id);
ALTER TABLE cost_layers ADD COLUMN base_total_cost NUMERIC;
ALTER TABLE cost_layers ADD COLUMN base_remaining_cost NUMERIC;
ALTER TABLE cost_layers ADD COLUMN base_currency TEXT;
CREATE TABLE stock_cost_events (
    movement_id UUID PRIMARY KEY REFERENCES inventory_movements(id),
    kind TEXT NOT NULL,
    currency TEXT NOT NULL,
    carrying_cost NUMERIC NOT NULL,
    actual_cost NUMERIC NOT NULL,
    work_order_id UUID
);
