-- 058_wishlist_item_fields: persist wishlist item variant_id, priority, quantity.
--
-- WishlistItem models variant_id, priority, and quantity, and AddWishlistItem
-- accepts all three, but the SQLite wishlist_items table only had product_id and
-- notes columns: add_item dropped the other three and row_to_item hard-coded
-- variant_id=None, priority=None, quantity=1. So an item added with a variant,
-- priority, or quantity != 1 silently lost that data. (Postgres already stored
-- variant_id and priority but likewise dropped quantity — see 052.)

ALTER TABLE wishlist_items ADD COLUMN variant_id TEXT;
ALTER TABLE wishlist_items ADD COLUMN priority INTEGER;
ALTER TABLE wishlist_items ADD COLUMN quantity INTEGER NOT NULL DEFAULT 1;
