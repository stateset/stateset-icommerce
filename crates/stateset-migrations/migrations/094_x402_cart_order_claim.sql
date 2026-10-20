-- Database-enforced "one claiming x402 intent per cart / per order".
--
-- The accessor refused a second intent for a cart or order that already had
-- one in a claiming status (created, signed, sequenced, batched, settled) by
-- reading `for_cart` / `for_order` and then calling `create` as two separate
-- statements outside any transaction. Two concurrent `create_intent` calls
-- for one cart both passed that read and both inserted, so a cart could be
-- charged twice. Nothing at the database level stopped it: 029's cart/order
-- indexes are plain, and 082 keys only `tx_hash`.
--
-- Design notes (why keyed columns instead of a partial unique index):
--   * SQLite supports partial indexes, but the claim must also survive
--     status changes; a keyed column lets the application clear the claim in
--     the same statement that leaves the claiming state, and keeps the SQLite
--     and Postgres shapes identical.
--   * Legacy databases may already contain two claiming intents for one cart
--     (that is the bug), so the backfill keys ONLY carts/orders that have
--     exactly one claiming intent today and leaves the rest NULL. The
--     migration can therefore never fail on real data.
--   * NULLs are distinct in a unique index, so unlinked intents and intents
--     that have left the claiming set never collide.
ALTER TABLE x402_payment_intents ADD COLUMN cart_claim_key TEXT;
ALTER TABLE x402_payment_intents ADD COLUMN order_claim_key TEXT;

UPDATE x402_payment_intents
SET cart_claim_key = cart_id
WHERE cart_id IS NOT NULL
  AND status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  AND (
      SELECT COUNT(*) FROM x402_payment_intents dup
      WHERE dup.cart_id = x402_payment_intents.cart_id
        AND dup.status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  ) = 1;

UPDATE x402_payment_intents
SET order_claim_key = order_id
WHERE order_id IS NOT NULL
  AND status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  AND (
      SELECT COUNT(*) FROM x402_payment_intents dup
      WHERE dup.order_id = x402_payment_intents.order_id
        AND dup.status IN ('created', 'signed', 'sequenced', 'batched', 'settled')
  ) = 1;

CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_cart_claim_key
    ON x402_payment_intents(cart_claim_key);
CREATE UNIQUE INDEX IF NOT EXISTS ux_x402_intents_order_claim_key
    ON x402_payment_intents(order_claim_key);
