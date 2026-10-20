-- Orders could not record WHAT they charged for.
--
-- `orders` carried only `total_amount`, with no tax, shipping or discount
-- columns, and `CreateOrder` had no fields for them. Checkout therefore minted
-- an order whose total was the sum of its line amounts while charging the
-- cart's grand total (subtotal + tax + shipping - discount). A $100 merch cart
-- with $10 shipping and $8 tax charged $118 against a $100 order, so recording
-- that capture was rejected by the over-capture guard as exceeding the order
-- total — a legitimate payment could not be booked.
--
-- These columns let an order state its own money breakdown, which also makes
-- order revenue decomposable for reporting and for the general ledger's
-- shipping-revenue split.
--
-- Existing rows default to zero, which preserves today's arithmetic exactly:
-- total_amount already equals the line sum for every order created before this
-- migration.
ALTER TABLE orders ADD COLUMN tax_amount TEXT NOT NULL DEFAULT '0';
ALTER TABLE orders ADD COLUMN shipping_amount TEXT NOT NULL DEFAULT '0';
ALTER TABLE orders ADD COLUMN discount_amount TEXT NOT NULL DEFAULT '0';
