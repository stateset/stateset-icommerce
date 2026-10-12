-- Fulfil legacy confirmed order reservations created by pre-sync shipping.
-- Before the shipment/order sync change, shipping moved reservations to
-- 'confirmed' without consuming stock, leaving units counted in on-hand and
-- allocated. This backfill consumes those holds exactly once and marks them
-- fulfilled. Idempotent by status: it only touches rows still 'confirmed'.

-- Collect target reservations (orders only).
WITH to_fulfil AS (
  SELECT r.id AS reservation_id,
         r.item_id,
         r.location_id,
         r.quantity,
         r.reference_id
  FROM inventory_reservations r
  WHERE r.status = 'confirmed' AND r.reference_type = 'order'
),
by_balance AS (
  SELECT item_id, location_id, SUM(quantity) AS total_qty
  FROM to_fulfil
  GROUP BY item_id, location_id
)
-- Adjust balances per (item_id, location_id).
UPDATE inventory_balances b
SET
  quantity_on_hand = b.quantity_on_hand - q.total_qty,
  quantity_allocated = GREATEST(b.quantity_allocated - q.total_qty, 0),
  quantity_available = (b.quantity_on_hand - q.total_qty) - GREATEST(b.quantity_allocated - q.total_qty, 0),
  updated_at = NOW()
FROM by_balance q
WHERE b.item_id = q.item_id AND b.location_id = q.location_id;

-- Ledger rows: one per reservation, reason tags the backfill.
INSERT INTO inventory_transactions (
  item_id, location_id, transaction_type, quantity,
  reference_type, reference_id, reason, created_at
)
SELECT tf.item_id,
       tf.location_id,
       'shipment',
       -tf.quantity,
       'order',
       tf.reference_id,
       CONCAT('Order ', tf.reference_id, ' shipment (backfill)'),
       NOW()
FROM to_fulfil tf;

-- Mark reservations fulfilled so the backfill is idempotent.
UPDATE inventory_reservations r
SET status = 'fulfilled'
WHERE r.id IN (SELECT reservation_id FROM to_fulfil);

