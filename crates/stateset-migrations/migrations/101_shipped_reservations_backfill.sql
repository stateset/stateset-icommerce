-- Fulfil legacy confirmed order reservations created by pre-sync shipping.
-- Before the shipment/order sync change, shipping moved reservations to
-- 'confirmed' without consuming stock, leaving units counted in on-hand and
-- allocated. This backfill consumes those holds exactly once and marks them
-- fulfilled. Idempotent by status: it only touches rows still 'confirmed'.

-- Reservations to fulfil: only confirmed holds belonging to orders.
WITH to_fulfil AS (
  SELECT r.id AS reservation_id,
         r.item_id,
         r.location_id,
         r.quantity,
         r.reference_id
  FROM inventory_reservations r
  WHERE r.status = 'confirmed' AND r.reference_type = 'order'
)
-- Adjust balances per (item_id, location_id).
UPDATE inventory_balances AS b
SET
  quantity_on_hand = b.quantity_on_hand - (
    SELECT COALESCE(SUM(tf.quantity), 0)
    FROM to_fulfil tf
    WHERE tf.item_id = b.item_id AND tf.location_id = b.location_id
  ),
  quantity_allocated = MAX(
    b.quantity_allocated - (
      SELECT COALESCE(SUM(tf.quantity), 0)
      FROM to_fulfil tf
      WHERE tf.item_id = b.item_id AND tf.location_id = b.location_id
    ),
    0
  ),
  quantity_available = (
    b.quantity_on_hand - (
      SELECT COALESCE(SUM(tf.quantity), 0)
      FROM to_fulfil tf
      WHERE tf.item_id = b.item_id AND tf.location_id = b.location_id
    )
  ) - MAX(
    b.quantity_allocated - (
      SELECT COALESCE(SUM(tf.quantity), 0)
      FROM to_fulfil tf
      WHERE tf.item_id = b.item_id AND tf.location_id = b.location_id
    ),
    0
  ),
  updated_at = datetime('now')
WHERE EXISTS (
  SELECT 1 FROM to_fulfil tf WHERE tf.item_id = b.item_id AND tf.location_id = b.location_id
);

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
       'Order ' || tf.reference_id || ' shipment (backfill)',
       datetime('now')
FROM to_fulfil tf;

-- Mark the reservations fulfilled so the backfill is idempotent.
UPDATE inventory_reservations
SET status = 'fulfilled'
WHERE id IN (SELECT reservation_id FROM to_fulfil);

