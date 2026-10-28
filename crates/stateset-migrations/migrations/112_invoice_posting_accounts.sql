ALTER TABLE gl_auto_posting_config ADD COLUMN sales_tax_payable_account_id TEXT REFERENCES gl_accounts(id);
