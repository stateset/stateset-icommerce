ALTER TABLE x402_payment_intents
    ADD COLUMN IF NOT EXISTS payer_signature_scheme TEXT;

ALTER TABLE x402_payment_intents
    ADD COLUMN IF NOT EXISTS payer_signature_bundle JSONB;

ALTER TABLE x402_payment_intents
    ADD COLUMN IF NOT EXISTS payer_public_key_bundle JSONB;
