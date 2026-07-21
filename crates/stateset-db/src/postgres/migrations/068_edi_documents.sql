-- EDI documents: Electronic Data Interchange documents exchanged with trading
-- partners (850 PO, 855 ack, 856 ASN, 810 invoice, etc.) with direction and
-- processing status, plus aggregate reporting.
--
-- Repository: crates/stateset-db/src/postgres/edi_documents.rs

CREATE TABLE IF NOT EXISTS edi_documents (
    id            UUID PRIMARY KEY,
    document_type TEXT NOT NULL,
    direction     TEXT NOT NULL DEFAULT 'inbound',
    status        TEXT NOT NULL DEFAULT 'pending',
    partner       TEXT,
    reference     TEXT,
    payload       TEXT,
    error_message TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_edi_documents_type ON edi_documents (document_type);
CREATE INDEX IF NOT EXISTS idx_edi_documents_status ON edi_documents (status);
CREATE INDEX IF NOT EXISTS idx_edi_documents_direction ON edi_documents (direction);
CREATE INDEX IF NOT EXISTS idx_edi_documents_partner ON edi_documents (partner) WHERE partner IS NOT NULL;
