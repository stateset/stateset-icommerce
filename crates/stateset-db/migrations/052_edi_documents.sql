-- EDI documents: Electronic Data Interchange documents exchanged with trading
-- partners (850 PO, 855 ack, 856 ASN, 810 invoice, etc.) with direction and
-- processing status, plus aggregate reporting.
--
-- Repository: crates/stateset-db/src/sqlite/edi_documents.rs
-- REST:       crates/stateset-http/src/routes/edi_documents.rs
--
-- Timestamps RFC3339 TEXT.

CREATE TABLE IF NOT EXISTS edi_documents (
    id TEXT PRIMARY KEY,
    document_type TEXT NOT NULL,
    direction TEXT NOT NULL DEFAULT 'inbound',
    status TEXT NOT NULL DEFAULT 'pending',
    partner TEXT,
    reference TEXT,
    payload TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_edi_documents_type ON edi_documents(document_type);
CREATE INDEX IF NOT EXISTS idx_edi_documents_status ON edi_documents(status);
CREATE INDEX IF NOT EXISTS idx_edi_documents_direction ON edi_documents(direction);
CREATE INDEX IF NOT EXISTS idx_edi_documents_partner ON edi_documents(partner);
