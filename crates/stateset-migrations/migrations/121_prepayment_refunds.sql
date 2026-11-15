ALTER TABLE prepayments ADD COLUMN refunded_amount TEXT DEFAULT '0';
ALTER TABLE prepayments ADD COLUMN refunded_at TEXT;
-- Exact legacy refund amounts are reconstructed from active applications on
-- read, without SQLite's floating point subtraction in a migration.
UPDATE prepayments SET refunded_amount=NULL,refunded_at=updated_at WHERE status='refunded';
