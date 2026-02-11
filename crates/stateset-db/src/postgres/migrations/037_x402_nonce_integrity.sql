-- Harden x402 replay protection and idempotency integrity (PostgreSQL)

UPDATE x402_payment_intents
SET idempotency_key = NULL
WHERE idempotency_key = '';

-- If historical duplicate payer+nonce rows exist, reassign only duplicates to
-- nonce values above the current max for each payer.
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
    JOIN base b USING (payer_address)
    WHERE r.dup_rank > 1
)
UPDATE x402_payment_intents x
SET nonce = reassign.new_nonce
FROM reassign
WHERE x.id = reassign.id;

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
UPDATE x402_payment_intents x
SET idempotency_key = NULL
FROM ranked_idempotency r
WHERE x.id = r.id
  AND r.dup_rank > 1;

DROP INDEX IF EXISTS idx_x402_intents_nonce;
DROP INDEX IF EXISTS idx_x402_intents_idempotency;

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_payer_nonce
ON x402_payment_intents(payer_address, nonce);

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_idempotency_not_null
ON x402_payment_intents(idempotency_key)
WHERE idempotency_key IS NOT NULL;
