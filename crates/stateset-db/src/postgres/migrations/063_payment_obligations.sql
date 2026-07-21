-- Payment obligations: scheduled amounts owed to suppliers, generated from
-- purchase-order terms, linkable to AP bills, with a dashboard rollup.
--
-- Repository: crates/stateset-db/src/postgres/payment_obligations.rs

CREATE TABLE IF NOT EXISTS payment_obligations (
    id                UUID PRIMARY KEY,
    number            TEXT NOT NULL,
    supplier_id       UUID NOT NULL,
    purchase_order_id UUID,
    amount            NUMERIC(19,4) NOT NULL,
    amount_paid       NUMERIC(19,4) NOT NULL DEFAULT 0,
    currency          TEXT NOT NULL DEFAULT 'USD',
    due_date          DATE NOT NULL,
    status            TEXT NOT NULL DEFAULT 'pending',
    linked_bill_ids   JSONB NOT NULL DEFAULT '[]'::jsonb,
    notes             TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT payment_obligations_amount_positive CHECK (amount > 0),
    CONSTRAINT payment_obligations_amount_paid_non_negative CHECK (amount_paid >= 0)
);
CREATE INDEX IF NOT EXISTS idx_payment_obligations_supplier ON payment_obligations (supplier_id);
CREATE INDEX IF NOT EXISTS idx_payment_obligations_status ON payment_obligations (status);
CREATE INDEX IF NOT EXISTS idx_payment_obligations_due ON payment_obligations (due_date);
