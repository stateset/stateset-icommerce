-- Auto-posting flags for fixed-asset depreciation and revenue recognition.
-- Default off: existing behavior is unchanged until explicitly enabled.
ALTER TABLE gl_auto_posting_config
    ADD COLUMN IF NOT EXISTS auto_post_depreciation BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE gl_auto_posting_config
    ADD COLUMN IF NOT EXISTS auto_post_revenue_recognition BOOLEAN NOT NULL DEFAULT FALSE;
