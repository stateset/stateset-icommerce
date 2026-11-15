-- Preserve transaction amounts separately from functional-currency postings.
-- Legacy foreign entries are deliberately not assigned an invented rate.
ALTER TABLE gl_journal_entry_lines ADD COLUMN transaction_currency TEXT;
ALTER TABLE gl_journal_entry_lines ADD COLUMN transaction_debit_amount TEXT;
ALTER TABLE gl_journal_entry_lines ADD COLUMN transaction_credit_amount TEXT;
ALTER TABLE gl_journal_entry_lines ADD COLUMN exchange_rate TEXT;

UPDATE gl_journal_entry_lines SET transaction_currency = currency,
 transaction_debit_amount = debit_amount, transaction_credit_amount = credit_amount,
 exchange_rate = 1
WHERE currency = COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD');
UPDATE gl_journal_entry_lines SET transaction_currency = NULL,
 transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL
WHERE journal_entry_id IN (
 SELECT e.id FROM gl_journal_entries e JOIN invoices d ON d.id = e.source_document_id
 WHERE e.source_document_type = 'invoice'
 AND d.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD')
);
UPDATE gl_journal_entry_lines SET transaction_currency = NULL,
 transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL
WHERE journal_entry_id IN (
 SELECT e.id FROM gl_journal_entries e JOIN payments d ON d.id = e.source_document_id
 WHERE e.source_document_type = 'payment'
 AND d.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD')
);
UPDATE gl_journal_entry_lines SET transaction_currency = NULL,
 transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL
WHERE journal_entry_id IN (
 SELECT e.id FROM gl_journal_entries e JOIN ap_bills d ON d.id = e.source_document_id
 WHERE e.source_document_type = 'bill'
 AND d.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD')
);
UPDATE gl_journal_entry_lines SET transaction_currency = NULL,
 transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL
WHERE journal_entry_id IN (
 SELECT e.id FROM gl_journal_entries e JOIN ap_payments d ON d.id = e.source_document_id
 WHERE e.source_document_type = 'bill_payment'
 AND d.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD')
);

-- Preserve uncertainty in legacy foreign source documents and reversals.
UPDATE gl_journal_entry_lines SET transaction_currency = NULL, transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL WHERE journal_entry_id IN (SELECT e.id FROM gl_journal_entries e JOIN ar_write_offs w ON w.id = e.source_document_id JOIN invoices i ON i.id = w.invoice_id WHERE e.source_document_type = 'write_off' AND i.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD'));
UPDATE gl_journal_entry_lines SET transaction_currency = NULL, transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL WHERE journal_entry_id IN (SELECT e.id FROM gl_journal_entries e JOIN fixed_assets a ON a.id = e.source_document_id WHERE e.source_document_type = 'fixed_asset_depreciation' AND a.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD'));
UPDATE gl_journal_entry_lines SET transaction_currency = NULL, transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL WHERE journal_entry_id IN (SELECT e.id FROM gl_journal_entries e JOIN performance_obligations o ON o.id = e.source_document_id JOIN revenue_contracts c ON c.id = o.contract_id WHERE e.source_document_type = 'revenue_recognition' AND c.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD'));
UPDATE gl_journal_entry_lines SET transaction_currency = NULL, transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL WHERE journal_entry_id IN (SELECT e.id FROM gl_journal_entries e JOIN cost_transactions t ON t.id = e.source_document_id JOIN item_costs c ON c.sku = t.sku WHERE e.source_document_type = 'cost_transaction' AND c.currency != COALESCE((SELECT base_currency FROM store_currency_settings LIMIT 1), 'USD'));
UPDATE gl_journal_entry_lines SET transaction_currency = NULL, transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL WHERE journal_entry_id IN (SELECT id FROM gl_journal_entries WHERE source_document_type = 'fx_revaluation');
UPDATE gl_journal_entry_lines SET transaction_currency = NULL, transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL WHERE journal_entry_id IN (SELECT journal_entry_id FROM gl_journal_entry_lines WHERE transaction_currency IS NULL);
UPDATE gl_journal_entry_lines SET transaction_currency = NULL, transaction_debit_amount = NULL, transaction_credit_amount = NULL, exchange_rate = NULL WHERE journal_entry_id IN (SELECT e.id FROM gl_journal_entries e JOIN gl_journal_entry_lines original ON original.journal_entry_id = e.source_document_id WHERE e.source_document_type = 'reversal' AND original.transaction_currency IS NULL);

CREATE TABLE gl_document_exchange_rates (
 source_type TEXT NOT NULL,
 source_id TEXT NOT NULL,
 transaction_currency TEXT NOT NULL,
 base_currency TEXT NOT NULL,
 exchange_rate TEXT NOT NULL,
 effective_date TEXT NOT NULL,
 PRIMARY KEY (source_type, source_id)
);
CREATE TABLE gl_fx_revaluation_runs (
 run_key TEXT PRIMARY KEY,
 id TEXT NOT NULL UNIQUE,
 as_of_date TEXT NOT NULL,
 base_currency TEXT NOT NULL,
 result_json TEXT
);
CREATE INDEX idx_gl_lines_transaction_currency ON gl_journal_entry_lines(account_id, transaction_currency);

-- Keep exact balances visible even after an account is retired at zero.
DROP VIEW v_trial_balance;
CREATE VIEW v_trial_balance AS
SELECT a.id AS account_id, a.account_number, a.name AS account_name,
       a.account_type, a.normal_balance,
       CASE WHEN a.normal_balance = 'debit' THEN a.current_balance ELSE '0' END AS debit_balance,
       CASE WHEN a.normal_balance = 'credit' THEN a.current_balance ELSE '0' END AS credit_balance
FROM gl_accounts a WHERE a.is_posting = 1 ORDER BY a.account_number;
