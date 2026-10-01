-- Existing shipments start at version 1. All repository updates increment this
-- value in the same transaction as the shipment change and its outbox fact.
ALTER TABLE shipments ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
