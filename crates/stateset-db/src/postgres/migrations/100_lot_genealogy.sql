-- Lot genealogy: the parent -> child links written by lot `split` and `merge`.
--
-- Postgres mirror of sqlite migration 093. A `lots` row carries a single
-- provenance (supplier_lot / supplier_id / work_order_id / purchase_order_id),
-- which cannot describe a lot merged from several receipts; before this table
-- `merge` dropped all four columns on the target, so a merged lot was
-- untraceable. Every split and every merge now writes one row per
-- (child, parent) pair and `trace()` walks the graph transitively back to the
-- original receipts.
--
-- Additive and legacy-safe: lots created before this migration have no rows.
CREATE TABLE IF NOT EXISTS lot_genealogy (
    child_lot_id  UUID NOT NULL REFERENCES lots(id) ON DELETE CASCADE,
    parent_lot_id UUID NOT NULL REFERENCES lots(id) ON DELETE CASCADE,
    -- 'split' | 'merge' (stateset_core::LotRelationship)
    relationship  TEXT NOT NULL CHECK (relationship IN ('split', 'merge')),
    -- Units that moved from the parent to the child.
    quantity      NUMERIC(12, 4) NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (child_lot_id, parent_lot_id),
    CHECK (child_lot_id <> parent_lot_id)
);

-- The (child, parent) primary key already covers child lookups.
CREATE INDEX IF NOT EXISTS idx_lot_genealogy_parent ON lot_genealogy(parent_lot_id);
