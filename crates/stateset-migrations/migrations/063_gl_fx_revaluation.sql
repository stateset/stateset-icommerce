-- FX gain/loss account for period-end revaluation of foreign-currency
-- GL account balances. NULL: revaluation falls back to the first active
-- posting account with an appropriate sub-type, or fails validation.
ALTER TABLE gl_auto_posting_config ADD COLUMN fx_gain_loss_account_id TEXT;
