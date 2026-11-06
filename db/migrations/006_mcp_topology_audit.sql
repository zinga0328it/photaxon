-- FiberSpider MCP topology records and audit. Additive and idempotent.
ALTER TABLE cabinets ADD COLUMN IF NOT EXISTS display_name TEXT;
ALTER TABLE cabinets ADD COLUMN IF NOT EXISTS location_reference TEXT;
ALTER TABLE cabinets ADD COLUMN IF NOT EXISTS parent_node TEXT;
ALTER TABLE cabinets ADD COLUMN IF NOT EXISTS notes TEXT;

ALTER TABLE ptes ADD COLUMN IF NOT EXISTS address_reference TEXT;
ALTER TABLE ptes ADD COLUMN IF NOT EXISTS notes TEXT;

CREATE TABLE IF NOT EXISTS roes (
    roe_id TEXT PRIMARY KEY,
    cabinet_id TEXT NOT NULL REFERENCES cabinets(cabinet_id),
    splitter_id TEXT NOT NULL REFERENCES splitters(splitter_id),
    address_reference TEXT NOT NULL,
    upstream_resource TEXT NOT NULL,
    notes TEXT,
    source_event TEXT NOT NULL,
    created_by_session TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS topology_changes (
    change_id TEXT PRIMARY KEY,
    invocation_id TEXT NOT NULL,
    technician_session TEXT NOT NULL,
    change_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    source_event TEXT NOT NULL,
    previous_state JSONB,
    next_state JSONB NOT NULL,
    validation JSONB NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS mcp_tool_audit (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    invocation_id TEXT NOT NULL,
    technician_session TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    tool_class TEXT NOT NULL CHECK (tool_class IN ('READ', 'WRITE_SAFE', 'WRITE_TOPOLOGY', 'ADMIN')),
    sanitized_input JSONB NOT NULL,
    validation JSONB NOT NULL,
    result JSONB NOT NULL,
    resource_id TEXT,
    previous_state JSONB,
    next_state JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS llm_proposals (
    proposal_id TEXT PRIMARY KEY,
    invocation_id TEXT NOT NULL,
    technician_session TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    proposal JSONB NOT NULL,
    validator_result JSONB NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_mcp_tool_audit_invocation
    ON mcp_tool_audit(invocation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_topology_changes_resource
    ON topology_changes(resource_id, created_at);

INSERT INTO schema_migrations (version)
VALUES ('006_mcp_topology_audit.sql')
ON CONFLICT (version) DO NOTHING;