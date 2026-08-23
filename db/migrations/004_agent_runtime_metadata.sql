-- FS-05.4 agent runtime metadata for OpenStack mapping.
-- Additive and idempotent.

ALTER TABLE agents
    ADD COLUMN IF NOT EXISTS runtime_provider TEXT NULL,
    ADD COLUMN IF NOT EXISTS runtime_id TEXT NULL,
    ADD COLUMN IF NOT EXISTS mgmt_ip INET NULL,
    ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ NULL;

INSERT INTO schema_migrations (version)
VALUES ('004_agent_runtime_metadata.sql')
ON CONFLICT (version) DO NOTHING;
