-- Persist rollback intent before invoking external compensations. A crash or
-- cancellation must never allow forward execution to resume partway through
-- a rollback. Existing saga history and compensation markers are preserved.
ALTER TABLE sagas ADD COLUMN IF NOT EXISTS rollback_started_at TIMESTAMPTZ;

UPDATE sagas SET rollback_started_at = COALESCE(completed_at, started_at, created_at)
WHERE rollback_started_at IS NULL AND (
    status = 'rolled_back' OR EXISTS (
        SELECT 1 FROM saga_steps WHERE saga_steps.saga_id = sagas.id
        AND saga_steps.rollback_at IS NOT NULL
    )
);

-- Recover the completed prefix of valid legacy definitions. Older
-- coordinators recorded step results without advancing current_step.
UPDATE sagas AS saga SET current_step = (
    SELECT COUNT(*) FROM saga_steps AS step
    WHERE step.saga_id = saga.id AND step.status = 'completed' AND step.rollback_at IS NULL
    AND NOT EXISTS (
        SELECT 1 FROM saga_steps AS earlier
        WHERE earlier.saga_id = saga.id AND earlier.step_order <= step.step_order
        AND (earlier.status <> 'completed' OR earlier.rollback_at IS NOT NULL)
    )
)
WHERE saga.rollback_started_at IS NULL AND saga.current_step = 0 AND saga.total_steps > 0
AND (SELECT COUNT(*) FROM saga_steps WHERE saga_id = saga.id) = saga.total_steps
AND (SELECT MIN(step_order) FROM saga_steps WHERE saga_id = saga.id) = 1
AND (SELECT MAX(step_order) FROM saga_steps WHERE saga_id = saga.id) = saga.total_steps;
