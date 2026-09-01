-- Database-enforced auto-post idempotency (see the SQLite twin,
-- 075_gl_source_document_key.sql, for the full design rationale).
--
-- One journal entry per source document for the single-entry families,
-- enforced by a unique index on a nullable key column: recognition and
-- depreciation post many entries per document and stay NULL; voiding clears
-- the key so a corrected re-post is possible; the backfill keys only
-- documents with exactly one live entry, so legacy duplicates from the
-- pre-guard era can never fail this migration.
ALTER TABLE gl_journal_entries ADD COLUMN IF NOT EXISTS source_document_key TEXT;

UPDATE gl_journal_entries je
SET source_document_key = je.source_document_type || ':' || je.source_document_id
WHERE je.status != 'voided'
  AND je.source_document_id IS NOT NULL
  AND je.source_document_type IN (
      'invoice', 'payment', 'bill', 'bill_payment',
      'cost_transaction', 'write_off', 'period_close', 'reversal'
  )
  AND (
      SELECT COUNT(*) FROM gl_journal_entries dup
      WHERE dup.source_document_type = je.source_document_type
        AND dup.source_document_id = je.source_document_id
        AND dup.status != 'voided'
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS idx_gl_je_source_document_key
    ON gl_journal_entries(source_document_key);
