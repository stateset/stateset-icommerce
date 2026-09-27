-- Keep nanosecond-precise creation time alongside PostgreSQL's microsecond
-- TIMESTAMPTZ. Legacy rows have NULL components and expire conservatively.
ALTER TABLE http_idempotency_keys
    ADD COLUMN created_at_epoch_seconds BIGINT,
    ADD COLUMN created_at_subsec_ns BIGINT,
    ADD CONSTRAINT http_idempotency_exact_time_pair
        CHECK (
            (created_at_epoch_seconds IS NULL AND created_at_subsec_ns IS NULL)
            OR (created_at_epoch_seconds IS NOT NULL
                AND created_at_subsec_ns BETWEEN 0 AND 999999999)
        );
