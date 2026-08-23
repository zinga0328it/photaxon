-- FS-05.3 generic resource runtime state, additive only.
-- Does not modify or replace 001_initial_schema.sql / 002_seed_lab_topology.sql.

CREATE TABLE IF NOT EXISTS resource_states (
    resource_id TEXT PRIMARY KEY,
    resource_kind TEXT NOT NULL,
    resource_state TEXT NOT NULL CHECK (resource_state IN ('FREE', 'RESERVED', 'CONNECTED', 'VERIFIED', 'OCCUPIED', 'FAULT')),
    owner_transaction_id TEXT REFERENCES transactions(transaction_id),
    revision BIGINT NOT NULL DEFAULT 1,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO schema_migrations (version)
VALUES ('003_resource_runtime_state.sql')
ON CONFLICT (version) DO NOTHING;
