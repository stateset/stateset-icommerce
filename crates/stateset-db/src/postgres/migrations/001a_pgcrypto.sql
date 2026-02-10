-- Ensure pgcrypto extension is enabled for gen_random_uuid().
--
-- This is a separate migration (instead of editing 001_initial_schema.sql) so
-- upgrades from earlier databases will still get the extension installed.

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

