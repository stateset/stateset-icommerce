-- Historical carrying costs remain with owned stock, including goods in transit.
ALTER TABLE gl_auto_posting_config ADD COLUMN work_in_progress_account_id TEXT REFERENCES gl_accounts(id);
ALTER TABLE cost_layers ADD COLUMN base_total_cost TEXT;
ALTER TABLE cost_layers ADD COLUMN base_remaining_cost TEXT;
ALTER TABLE cost_layers ADD COLUMN base_currency TEXT;
CREATE TABLE stock_cost_events (
    movement_id TEXT PRIMARY KEY REFERENCES inventory_movements(id),
    kind TEXT NOT NULL,
    currency TEXT NOT NULL,
    carrying_cost TEXT NOT NULL,
    actual_cost TEXT NOT NULL,
    work_order_id TEXT
);
