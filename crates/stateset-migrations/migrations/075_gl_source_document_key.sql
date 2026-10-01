-- Database-enforced auto-post idempotency.
--
-- The application layer already guarantees one journal entry per source
-- document for the single-entry families (invoice, payment, bill,
-- bill_payment, cost_transaction, write_off, period_close, reversal): the
-- duplicate check shares the write transaction with the insert. This column
-- is the backstop that holds even for writers that bypass the application
-- layer entirely.
--
-- Design notes (why a keyed column instead of a plain unique index on
-- (source_document_type, source_document_id)):
--   * Recognition and depreciation legitimately post MANY entries per source
--     document — only the single-entry families may carry a key.
--   * A voided entry must free its document for a corrected re-post, so
--     voiding clears the key (SQLite/Postgres treat NULLs as distinct).
--   * Legacy databases may contain duplicates from the pre-guard era; the
--     backfill keys ONLY documents with exactly one live entry and leaves
--     duplicate rows NULL, so this migration can never fail on real data.
ALTER TABLE gl_journal_entries ADD COLUMN source_document_key TEXT;

-- Backfill: key every single-entry-family document that has exactly one
-- non-voided entry today. (String concatenation only — no decimal math on
-- the TEXT money columns.)
UPDATE gl_journal_entries
SET source_document_key = source_document_type || ':' || source_document_id
WHERE status != 'voided'
  AND source_document_id IS NOT NULL
  AND source_document_type IN (
      'invoice', 'payment', 'bill', 'bill_payment',
      'cost_transaction', 'write_off', 'period_close', 'reversal'
  )
  AND (
      SELECT COUNT(*) FROM gl_journal_entries dup
      WHERE dup.source_document_type = gl_journal_entries.source_document_type
        AND dup.source_document_id = gl_journal_entries.source_document_id
        AND dup.status != 'voided'
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_gl_je_source_document_key
    ON gl_journal_entries(source_document_key);
