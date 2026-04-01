ALTER TABLE x402_payment_intents
    ADD COLUMN payer_signature_scheme TEXT;

ALTER TABLE x402_payment_intents
    ADD COLUMN payer_signature_bundle TEXT;

ALTER TABLE x402_payment_intents
    ADD COLUMN payer_public_key_bundle TEXT;
