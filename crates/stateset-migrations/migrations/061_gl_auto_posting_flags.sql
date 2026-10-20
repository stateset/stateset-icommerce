-- Auto-posting flags for fixed-asset depreciation and revenue recognition.
-- Default off: existing behavior is unchanged until explicitly enabled.
ALTER TABLE gl_auto_posting_config ADD COLUMN auto_post_depreciation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE gl_auto_posting_config ADD COLUMN auto_post_revenue_recognition INTEGER NOT NULL DEFAULT 0;
