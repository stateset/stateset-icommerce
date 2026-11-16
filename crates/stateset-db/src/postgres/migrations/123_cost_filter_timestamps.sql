-- Preserve time bounds just as SQLite does; historical day-only records are UTC midnight.
ALTER TABLE cost_layers ALTER COLUMN layer_date TYPE TIMESTAMPTZ USING layer_date::timestamp AT TIME ZONE 'UTC';
ALTER TABLE cost_variances ALTER COLUMN variance_date TYPE TIMESTAMPTZ USING variance_date::timestamp AT TIME ZONE 'UTC';
