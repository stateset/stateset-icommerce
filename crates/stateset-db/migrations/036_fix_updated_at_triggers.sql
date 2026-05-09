-- Fix updated_at triggers to write RFC3339-formatted timestamps.
--
-- Original triggers used datetime('now'), which produces 'YYYY-MM-DD HH:MM:SS'
-- (no T separator, no timezone). The row parsers across the workspace expect
-- RFC3339, so any UPDATE on these tables would silently corrupt updated_at and
-- cause subsequent reads to fail with a chrono parse error.
--
-- We DROP and recreate each trigger to write strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
-- which yields '2026-05-07T08:55:23.123Z' — a parseable RFC3339 string.
--
-- Coverage: 22 triggers across warehouse, receiving, fulfillment, accounts_payable,
-- cost_accounting, credit, backorder, accounts_receivable, general_ledger.

-- 016_warehouse.sql
DROP TRIGGER IF EXISTS update_warehouses_timestamp;
CREATE TRIGGER update_warehouses_timestamp
AFTER UPDATE ON warehouses
BEGIN
    UPDATE warehouses SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS update_locations_timestamp;
CREATE TRIGGER update_locations_timestamp
AFTER UPDATE ON locations
BEGIN
    UPDATE locations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS update_location_inventory_timestamp;
CREATE TRIGGER update_location_inventory_timestamp
AFTER UPDATE ON location_inventory
BEGIN
    UPDATE location_inventory SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 017_receiving.sql
DROP TRIGGER IF EXISTS receipts_updated_at;
CREATE TRIGGER receipts_updated_at
AFTER UPDATE ON receipts
BEGIN
    UPDATE receipts SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS receipt_items_updated_at;
CREATE TRIGGER receipt_items_updated_at
AFTER UPDATE ON receipt_items
BEGIN
    UPDATE receipt_items SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 018_fulfillment.sql
DROP TRIGGER IF EXISTS waves_updated_at;
CREATE TRIGGER waves_updated_at
AFTER UPDATE ON waves
BEGIN
    UPDATE waves SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS pick_tasks_updated_at;
CREATE TRIGGER pick_tasks_updated_at
AFTER UPDATE ON pick_tasks
BEGIN
    UPDATE pick_tasks SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS pack_tasks_updated_at;
CREATE TRIGGER pack_tasks_updated_at
AFTER UPDATE ON pack_tasks
BEGIN
    UPDATE pack_tasks SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS ship_tasks_updated_at;
CREATE TRIGGER ship_tasks_updated_at
AFTER UPDATE ON ship_tasks
BEGIN
    UPDATE ship_tasks SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 019_accounts_payable.sql
DROP TRIGGER IF EXISTS ap_bills_updated_at;
CREATE TRIGGER ap_bills_updated_at
AFTER UPDATE ON ap_bills
BEGIN
    UPDATE ap_bills SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS ap_payments_updated_at;
CREATE TRIGGER ap_payments_updated_at
AFTER UPDATE ON ap_payments
BEGIN
    UPDATE ap_payments SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS ap_payment_runs_updated_at;
CREATE TRIGGER ap_payment_runs_updated_at
AFTER UPDATE ON ap_payment_runs
BEGIN
    UPDATE ap_payment_runs SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 020_cost_accounting.sql
DROP TRIGGER IF EXISTS item_costs_updated_at;
CREATE TRIGGER item_costs_updated_at
AFTER UPDATE ON item_costs
BEGIN
    UPDATE item_costs SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 021_credit.sql
DROP TRIGGER IF EXISTS credit_accounts_updated_at;
CREATE TRIGGER credit_accounts_updated_at
AFTER UPDATE ON credit_accounts
BEGIN
    UPDATE credit_accounts SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS credit_applications_updated_at;
CREATE TRIGGER credit_applications_updated_at
AFTER UPDATE ON credit_applications
BEGIN
    UPDATE credit_applications SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 022_backorder.sql
DROP TRIGGER IF EXISTS backorders_updated_at;
CREATE TRIGGER backorders_updated_at
AFTER UPDATE ON backorders
BEGIN
    UPDATE backorders SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 023_accounts_receivable.sql
DROP TRIGGER IF EXISTS ar_credit_memos_updated_at;
CREATE TRIGGER ar_credit_memos_updated_at
AFTER UPDATE ON ar_credit_memos
BEGIN
    UPDATE ar_credit_memos SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

-- 024_general_ledger.sql
DROP TRIGGER IF EXISTS gl_accounts_updated_at;
CREATE TRIGGER gl_accounts_updated_at
AFTER UPDATE ON gl_accounts
BEGIN
    UPDATE gl_accounts SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS gl_periods_updated_at;
CREATE TRIGGER gl_periods_updated_at
AFTER UPDATE ON gl_periods
BEGIN
    UPDATE gl_periods SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS gl_journal_entries_updated_at;
CREATE TRIGGER gl_journal_entries_updated_at
AFTER UPDATE ON gl_journal_entries
BEGIN
    UPDATE gl_journal_entries SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS gl_auto_posting_config_updated_at;
CREATE TRIGGER gl_auto_posting_config_updated_at
AFTER UPDATE ON gl_auto_posting_config
BEGIN
    UPDATE gl_auto_posting_config SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;

DROP TRIGGER IF EXISTS gl_account_balances_updated_at;
CREATE TRIGGER gl_account_balances_updated_at
AFTER UPDATE ON gl_account_balances
BEGIN
    UPDATE gl_account_balances SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
END;
