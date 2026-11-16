CREATE TABLE production_batch_work_orders (
    batch_id TEXT NOT NULL REFERENCES production_batches(id) ON DELETE CASCADE,
    work_order_id TEXT NOT NULL REFERENCES manufacturing_work_orders(id),
    PRIMARY KEY (batch_id, work_order_id)
);
CREATE INDEX idx_production_batch_work_orders_order ON production_batch_work_orders(work_order_id);
INSERT OR IGNORE INTO production_batch_work_orders SELECT DISTINCT b.id, w.id FROM production_batches b, json_each(b.work_order_ids) j JOIN manufacturing_work_orders w ON w.id = j.value;
CREATE VIEW production_batch_state AS
WITH members AS (
    SELECT l.batch_id, COUNT(*) AS total,
        SUM(CASE WHEN w.status = 'completed' THEN 1 ELSE 0 END) AS completed,
        SUM(CASE WHEN w.status = 'cancelled' THEN 1 ELSE 0 END) AS cancelled,
        SUM(CASE WHEN w.status IN ('in_progress', 'partially_completed', 'on_hold') THEN 1 ELSE 0 END) AS started,
        json_group_array(w.id) AS work_order_ids
    FROM production_batch_work_orders l JOIN manufacturing_work_orders w ON w.id = l.work_order_id GROUP BY l.batch_id
)
SELECT b.id, b.name,
    CASE WHEN b.status = 'cancelled' THEN 'cancelled'
         WHEN m.total > 0 AND m.cancelled > 0 AND m.cancelled + m.completed = m.total THEN 'cancelled'
         WHEN m.total > 0 AND m.completed = m.total THEN 'completed'
         WHEN m.started > 0 OR m.completed > 0 THEN 'in_progress'
         ELSE 'planned' END AS status,
    b.vendor_id, COALESCE(m.work_order_ids, '[]') AS work_order_ids,
    b.notes, b.scheduled_start, b.scheduled_end, b.created_at, b.updated_at,
    EXISTS(SELECT 1 FROM json_each(b.work_order_ids) j WHERE NOT EXISTS(SELECT 1 FROM manufacturing_work_orders w WHERE w.id = j.value)) AS unresolved_links
FROM production_batches b LEFT JOIN members m ON m.batch_id = b.id;
