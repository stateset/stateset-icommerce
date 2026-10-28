ALTER TABLE gl_auto_posting_config ADD COLUMN goods_received_not_invoiced_account_id TEXT REFERENCES gl_accounts(id);
ALTER TABLE gl_auto_posting_config ADD COLUMN inventory_adjustment_account_id TEXT REFERENCES gl_accounts(id);
