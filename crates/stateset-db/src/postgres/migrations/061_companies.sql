-- B2B companies (accounts) with shipping addresses, contacts and per-product
-- price overrides, previously SQLite-only.
--
-- Repository: crates/stateset-db/src/postgres/companies.rs

-- Companies: B2B customer accounts.
CREATE TABLE IF NOT EXISTS companies (
    id                 UUID PRIMARY KEY,
    name               TEXT NOT NULL,
    reference          TEXT,
    email              TEXT,
    phone              TEXT,
    currency           TEXT NOT NULL DEFAULT 'USD',
    payment_terms_days INTEGER,
    status             TEXT NOT NULL DEFAULT 'active',
    tags               JSONB NOT NULL DEFAULT '[]'::jsonb,
    metadata           JSONB NOT NULL DEFAULT 'null'::jsonb,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_companies_status ON companies (status);
CREATE INDEX IF NOT EXISTS idx_companies_name ON companies (name);

-- Company shipping addresses.
CREATE TABLE IF NOT EXISTS company_shipping_addresses (
    id          UUID PRIMARY KEY,
    company_id  UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    label       TEXT,
    name        TEXT,
    line1       TEXT NOT NULL,
    line2       TEXT,
    city        TEXT NOT NULL,
    region      TEXT,
    postal_code TEXT,
    country     TEXT NOT NULL,
    is_default  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_company_addresses_company ON company_shipping_addresses (company_id);

-- Contacts: people linked to one or more companies. company_ids is a JSONB
-- array of company UUID strings, queried with the @> containment operator.
CREATE TABLE IF NOT EXISTS contacts (
    id             UUID PRIMARY KEY,
    first_name     TEXT NOT NULL,
    last_name      TEXT,
    email          TEXT,
    phone          TEXT,
    title          TEXT,
    company_ids    JSONB NOT NULL DEFAULT '[]'::jsonb,
    portal_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    is_active      BOOLEAN NOT NULL DEFAULT TRUE,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_contacts_company_ids ON contacts USING GIN (company_ids);

-- Company-specific product price overrides.
CREATE TABLE IF NOT EXISTS company_price_overrides (
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    price      NUMERIC(19,4) NOT NULL,
    currency   TEXT NOT NULL DEFAULT 'USD',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (company_id, product_id)
);
