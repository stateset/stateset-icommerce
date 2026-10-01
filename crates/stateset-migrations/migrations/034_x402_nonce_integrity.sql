-- Harden x402 replay protection and idempotency integrity (SQLite)

-- Normalize empty idempotency keys so uniqueness checks can ignore them.
UPDATE x402_payment_intents
SET idempotency_key = NULL
WHERE idempotency_key = '';

-- If historical duplicate payer+nonce rows exist, reassign only the duplicates
-- to nonce values above the current max for that payer.
WITH ranked AS (
    SELECT
        id,
        payer_address,
        nonce,
        ROW_NUMBER() OVER (
            PARTITION BY payer_address, nonce
            ORDER BY created_at, id
        ) AS dup_rank
    FROM x402_payment_intents
),
base AS (
    SELECT
        payer_address,
        COALESCE(MAX(nonce), -1) AS max_nonce
    FROM x402_payment_intents
    GROUP BY payer_address
),
reassign AS (
    SELECT
        r.id,
        b.max_nonce + ROW_NUMBER() OVER (
            PARTITION BY r.payer_address
            ORDER BY r.nonce, r.id
        ) AS new_nonce
    FROM ranked r
    JOIN base b ON b.payer_address = r.payer_address
    WHERE r.dup_rank > 1
)
UPDATE x402_payment_intents
SET nonce = (
    SELECT new_nonce
    FROM reassign
    WHERE reassign.id = x402_payment_intents.id
)
WHERE id IN (SELECT id FROM reassign);

-- Retain only the earliest row for each non-null idempotency key.
WITH ranked_idempotency AS (
    SELECT
        id,
        ROW_NUMBER() OVER (
            PARTITION BY idempotency_key
            ORDER BY created_at, id
        ) AS dup_rank
    FROM x402_payment_intents
    WHERE idempotency_key IS NOT NULL
)
UPDATE x402_payment_intents
SET idempotency_key = NULL
WHERE id IN (
    SELECT id
    FROM ranked_idempotency
    WHERE dup_rank > 1
);

DROP INDEX IF EXISTS idx_x402_intents_nonce;
DROP INDEX IF EXISTS idx_x402_intents_idempotency;

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_payer_nonce
ON x402_payment_intents(payer_address, nonce);

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_idempotency_not_null
ON x402_payment_intents(idempotency_key)
WHERE idempotency_key IS NOT NULL;
