-- Customer segments and segment memberships
CREATE TABLE IF NOT EXISTS segments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    segment_type TEXT NOT NULL DEFAULT 'static' CHECK (segment_type IN ('static', 'dynamic')),
    rules JSONB NOT NULL DEFAULT '[]'::JSONB,
    member_count BIGINT NOT NULL DEFAULT 0 CHECK (member_count >= 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS segment_memberships (
    segment_id UUID NOT NULL REFERENCES segments(id) ON DELETE CASCADE,
    customer_id UUID NOT NULL,
    joined_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (segment_id, customer_id)
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_segments_segment_type ON segments (segment_type);
CREATE INDEX IF NOT EXISTS idx_segments_name ON segments (name);
CREATE INDEX IF NOT EXISTS idx_segments_created_at ON segments (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_segment_memberships_customer_id ON segment_memberships (customer_id);
CREATE INDEX IF NOT EXISTS idx_segment_memberships_joined_at ON segment_memberships (joined_at DESC);
