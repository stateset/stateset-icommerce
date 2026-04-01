-- Reward catalog migration for PostgreSQL
-- Loyalty program rewards with point costs and monetary values

CREATE TABLE IF NOT EXISTS rewards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    program_id UUID NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    points_cost BIGINT NOT NULL DEFAULT 0,
    reward_type VARCHAR(50) NOT NULL DEFAULT 'discount',
    value NUMERIC(12, 4),
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_rewards_program_id ON rewards(program_id);
CREATE INDEX IF NOT EXISTS idx_rewards_reward_type ON rewards(reward_type);
CREATE INDEX IF NOT EXISTS idx_rewards_is_active ON rewards(is_active);
CREATE INDEX IF NOT EXISTS idx_rewards_created_at ON rewards(created_at DESC);

-- Check constraints
ALTER TABLE rewards ADD CONSTRAINT rewards_points_cost_non_negative CHECK (points_cost >= 0);
ALTER TABLE rewards ADD CONSTRAINT rewards_value_non_negative CHECK (value IS NULL OR value >= 0);
