-- Additive storage for the separate MCP CONTROL_PLANE_ADMIN principal.
-- Do not apply to AAA without a reviewed backup and schema preflight.

ALTER TABLE onts ALTER COLUMN created_by_technician DROP NOT NULL;
ALTER TABLE onts ADD COLUMN IF NOT EXISTS created_by_control_plane_admin TEXT;

ALTER TABLE modems ALTER COLUMN created_by_technician DROP NOT NULL;
ALTER TABLE modems ADD COLUMN IF NOT EXISTS created_by_control_plane_admin TEXT;

ALTER TABLE ont_modem_links ALTER COLUMN created_by_technician DROP NOT NULL;
ALTER TABLE ont_modem_links ADD COLUMN IF NOT EXISTS created_by_control_plane_admin TEXT;

ALTER TABLE roes ADD COLUMN IF NOT EXISTS created_by_control_plane_admin TEXT;

CREATE TABLE IF NOT EXISTS control_plane_admin_operations (
    operation_id TEXT PRIMARY KEY,
    principal_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    input_sha256 TEXT NOT NULL CHECK (length(input_sha256) = 64),
    status TEXT NOT NULL CHECK (status IN ('PENDING', 'APPLIED')),
    resource_id TEXT,
    result JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((status = 'PENDING' AND result IS NULL) OR (status = 'APPLIED' AND result IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS control_plane_admin_audit (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    operation_id TEXT NOT NULL UNIQUE REFERENCES control_plane_admin_operations(operation_id),
    principal_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    sanitized_input JSONB NOT NULL,
    input_sha256 TEXT NOT NULL CHECK (length(input_sha256) = 64),
    validation JSONB NOT NULL,
    result JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_control_plane_admin_audit_resource
    ON control_plane_admin_audit(resource_type, resource_id, created_at);

INSERT INTO schema_migrations (version)
VALUES ('011_control_plane_admin.sql')
ON CONFLICT (version) DO NOTHING;
