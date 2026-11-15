-- Every cost-layer quantity and carrying-value change has a dated history.
CREATE TABLE stock_cost_layer_changes (
 id TEXT PRIMARY KEY, layer_id TEXT NOT NULL REFERENCES cost_layers(id),
 movement_id TEXT REFERENCES inventory_movements(id),
 quantity_delta TEXT NOT NULL, value_delta TEXT NOT NULL,
 currency TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE INDEX stock_cost_layer_changes_layer_date ON stock_cost_layer_changes(layer_id,created_at);
ALTER TABLE cost_transactions ADD COLUMN currency TEXT;
ALTER TABLE stock_cost_events ADD COLUMN settlement_currency TEXT;
ALTER TABLE stock_cost_events ADD COLUMN settlement_rate TEXT;
