-- Preserve exact creation time within the millisecond stored by migration 064.
-- Existing rows remain NULL and are expired conservatively at the next
-- millisecond boundary, avoiding an early replay of a non-expired response.
ALTER TABLE http_idempotency_keys
    ADD COLUMN created_at_sub_ms_ns INTEGER
    CHECK (created_at_sub_ms_ns BETWEEN 0 AND 999999);
