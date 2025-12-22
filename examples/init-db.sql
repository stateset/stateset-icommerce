-- StateSet Sequencer Database Initialization
-- This runs automatically when PostgreSQL container starts

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Create schema for sequencer
CREATE SCHEMA IF NOT EXISTS sequencer;

-- Note: The sequencer will run its own migrations on startup
-- This file is for any custom initialization you need

-- Example: Create read-only user for monitoring
-- CREATE USER monitoring WITH PASSWORD 'monitoring_password';
-- GRANT CONNECT ON DATABASE sequencer TO monitoring;
-- GRANT USAGE ON SCHEMA sequencer TO monitoring;
-- GRANT SELECT ON ALL TABLES IN SCHEMA sequencer TO monitoring;

-- Log successful initialization
DO $$
BEGIN
  RAISE NOTICE 'StateSet Sequencer database initialized successfully';
END $$;
