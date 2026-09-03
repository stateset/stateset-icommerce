-- Database-enforced "one claiming x402 intent per cart / per order" (see the
-- SQLite twin, 094_x402_cart_order_claim.sql, for the full rationale).
--
-- Two concurrent `create_intent` calls for one cart both passed the
-- read-then-create duplicate check and both inserted, double-charging the
-- cart. `cart_claim_key` / `order_claim_key` carry the cart/order id while an
-- intent is in a claiming status (created, signed, sequenced, batched,
-- settled) and are cleared when it leaves that set, so the unique indexes
-- below are the backstop even for writers that bypass the application check.
-- The backfill keys only carts/orders with exactly one claiming intent today,
-- so this migration can never fail on real data.
ALTER TABLE x402_payment_intents ADD COLUMN IF NOT EXISTS cart_claim_key TEXT;
ALTER TABLE x402_payment_intents ADD COLUMN IF NOT EXISTS order_claim_key TEXT;

UPDATE x402_payment_intents pi
SET cart_claim_key = pi.cart_id::text
WHERE pi.cart_id IS NOT NULL
  AND pi.cart_claim_key IS NULL
  AND pi.status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  AND (
      SELECT COUNT(*) FROM x402_payment_intents dup
      WHERE dup.cart_id = pi.cart_id
        AND dup.status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  ) = 1;

UPDATE x402_payment_intents pi
SET order_claim_key = pi.order_id::text
WHERE pi.order_id IS NOT NULL
  AND pi.order_claim_key IS NULL
  AND pi.status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  AND (
      SELECT COUNT(*) FROM x402_payment_intents dup
      WHERE dup.order_id = pi.order_id
        AND dup.status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_cart_claim_key
    ON x402_payment_intents(cart_claim_key);
CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_order_claim_key
    ON x402_payment_intents(order_claim_key);
