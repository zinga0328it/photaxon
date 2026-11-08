-- Add WR linkage and private media catalog for technician field operations.

ALTER TABLE telegram_workflows
    ADD COLUMN IF NOT EXISTS work_order_id TEXT;

CREATE TABLE IF NOT EXISTS field_operation_media (
    media_id TEXT PRIMARY KEY,
    work_order_id TEXT NOT NULL,
    workflow_id TEXT NOT NULL REFERENCES telegram_workflows(workflow_id),
    resource_type TEXT NOT NULL CHECK (resource_type IN ('ROE', 'ONT', 'MODEM')),
    resource_ref TEXT NOT NULL,
    purpose TEXT NOT NULL CHECK (purpose IN ('ROE_INSTALLATION', 'ONT_LABEL', 'MODEM_DELIVERY')),
    storage_key TEXT NOT NULL UNIQUE,
    content_type TEXT NOT NULL CHECK (content_type IN ('image/jpeg', 'image/png', 'image/webp')),
    byte_size BIGINT NOT NULL CHECK (byte_size > 0 AND byte_size <= 20971520),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    captured_by_technician TEXT NOT NULL REFERENCES technicians(technician_id),
    captured_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_field_operation_media_work_order
    ON field_operation_media(work_order_id, created_at);
CREATE INDEX IF NOT EXISTS idx_field_operation_media_resource
    ON field_operation_media(resource_type, resource_ref, created_at);

CREATE INDEX IF NOT EXISTS idx_telegram_workflows_work_order
    ON telegram_workflows(work_order_id, created_at);

INSERT INTO schema_migrations (version)
VALUES ('010_field_media_and_work_orders.sql')
ON CONFLICT (version) DO NOTHING;