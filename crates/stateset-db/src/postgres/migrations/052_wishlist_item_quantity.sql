-- 052_wishlist_item_quantity: persist wishlist item quantity.
--
-- WishlistItem models `quantity` and AddWishlistItem accepts it, but the
-- Postgres wishlist_items INSERT omitted it and row_to_item hard-coded
-- quantity=1, so an item added with quantity != 1 silently lost it. Add the
-- column (SQLite gains variant_id/priority/quantity together in migration 058;
-- Postgres already stored variant_id and priority).

ALTER TABLE wishlist_items ADD COLUMN IF NOT EXISTS quantity INTEGER NOT NULL DEFAULT 1;
