-- Explicit asset journal accounts and immutable schedule revision history.
ALTER TABLE fixed_assets ADD COLUMN acquisition_offset_account_id TEXT REFERENCES gl_accounts(id);
ALTER TABLE fixed_assets ADD COLUMN disposal_gain_loss_account_id TEXT REFERENCES gl_accounts(id);
CREATE TABLE fixed_asset_schedule_revisions (id TEXT PRIMARY KEY, asset_id TEXT NOT NULL REFERENCES fixed_assets(id), previous_asset_json TEXT NOT NULL, previous_schedule_json TEXT NOT NULL, revised_at TEXT NOT NULL);
CREATE INDEX idx_asset_schedule_revisions_asset ON fixed_asset_schedule_revisions(asset_id,revised_at);
