-- 056_rewards: loyalty reward catalog.
--
-- The `rewards` table shipped on PostgreSQL (postgres/migrations/045_rewards.sql)
-- and is exercised by sqlite/rewards.rs, but was never created by the SQLite
-- migration set — so `loyalty().create_reward(...)` failed at runtime with
-- "no such table: rewards" on the default embedded backend. Columns mirror
-- sqlite/rewards.rs.

CREATE TABLE IF NOT EXISTS rewards (
    id TEXT PRIMARY KEY,
    program_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    points_cost INTEGER NOT NULL DEFAULT 0,
    reward_type TEXT NOT NULL DEFAULT 'discount',
    value TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_rewards_program_id ON rewards(program_id);
CREATE INDEX IF NOT EXISTS idx_rewards_reward_type ON rewards(reward_type);
CREATE INDEX IF NOT EXISTS idx_rewards_is_active ON rewards(is_active);
