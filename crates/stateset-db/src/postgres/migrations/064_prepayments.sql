-- Prepayments: cash paid to suppliers in advance, drawn down by applying to
-- AP bills or payment obligations; unused balance refundable.
--
-- Repository: crates/stateset-db/src/postgres/prepayments.rs

CREATE TABLE IF NOT EXISTS prepayments (
    id          UUID PRIMARY KEY,
    number      TEXT NOT NULL,
    supplier_id UUID NOT NULL,
    amount      NUMERIC(19,4) NOT NULL,
    remaining   NUMERIC(19,4) NOT NULL,
    currency    TEXT NOT NULL DEFAULT 'USD',
    status      TEXT NOT NULL DEFAULT 'open',
    method      TEXT,
    reference   TEXT,
    memo        TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT prepayments_amount_positive CHECK (amount > 0),
    CONSTRAINT prepayments_remaining_non_negative CHECK (remaining >= 0)
);
CREATE INDEX IF NOT EXISTS idx_prepayments_supplier ON prepayments (supplier_id);
CREATE INDEX IF NOT EXISTS idx_prepayments_status ON prepayments (status);

CREATE TABLE IF NOT EXISTS prepayment_applications (
    id            UUID PRIMARY KEY,
    prepayment_id UUID NOT NULL REFERENCES prepayments(id) ON DELETE CASCADE,
    target_type   TEXT NOT NULL DEFAULT 'bill',
    target_id     UUID NOT NULL,
    amount        NUMERIC(19,4) NOT NULL,
    reversed      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_prepayment_apps_prepayment ON prepayment_applications (prepayment_id);
