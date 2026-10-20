-- Inventory round 6: the balance identity, enforced at the database.
--
-- 083 (SQLite) / 090 (Postgres) only guarantee NON-NEGATIVITY. Nothing has
-- ever guaranteed the identity that every reader of `inventory_balances`
-- assumes:
--
--     quantity_available == quantity_on_hand - quantity_allocated
--
-- The engine's invariant helpers check `quantity_allocated == SUM(open
-- reservations)` but never this one, so a raw-SQL write (or a code path that
-- moved on-hand without recomputing available) could leave a row that
-- oversells or undersells for ever: `reserve` reads `quantity_available`,
-- and its atomic `quantity_available >= ?` guard trusts it completely.
--
-- LEGACY-SAFE BY CONSTRUCTION, exactly like 083: the UPDATE trigger only
-- rejects a transition FROM a coherent row TO an incoherent one. A database
-- that already drifted still migrates, still loads, and can still be
-- repaired in place (the engine's clamp paths now rewrite such rows instead
-- of only logging them) — while every write that starts from a clean row is
-- held to the identity. Every insert is always held to it: a brand-new balance
-- has no legacy to protect.
--
-- The columns are TEXT decimals, so the comparison casts to REAL and allows
-- 1e-6 of slack: IEEE-754 cannot represent every decimal string exactly, and
-- the engine's own arithmetic is exact `rust_decimal::Decimal`. The trigger
-- is a backstop against raw SQL, not the primary guard.

CREATE TRIGGER IF NOT EXISTS trg_inventory_balances_identity_insert
BEFORE INSERT ON inventory_balances
WHEN ABS(CAST(NEW.quantity_available AS REAL)
         - (CAST(NEW.quantity_on_hand AS REAL) - CAST(NEW.quantity_allocated AS REAL)))
     > 0.000001
BEGIN
    SELECT RAISE(ABORT, 'inventory_balances: quantity_available must equal quantity_on_hand - quantity_allocated');
END;

CREATE TRIGGER IF NOT EXISTS trg_inventory_balances_identity_update
BEFORE UPDATE OF quantity_on_hand, quantity_allocated, quantity_available ON inventory_balances
WHEN ABS(CAST(NEW.quantity_available AS REAL)
         - (CAST(NEW.quantity_on_hand AS REAL) - CAST(NEW.quantity_allocated AS REAL)))
     > 0.000001
 AND ABS(CAST(OLD.quantity_available AS REAL)
         - (CAST(OLD.quantity_on_hand AS REAL) - CAST(OLD.quantity_allocated AS REAL)))
     <= 0.000001
BEGIN
    SELECT RAISE(ABORT, 'inventory_balances: quantity_available must equal quantity_on_hand - quantity_allocated');
END;
