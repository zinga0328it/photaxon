-- FiberSpider field operations and Telegram workflow storage.
-- Additive only; do not apply to a live AAA database without an approved migration run.

CREATE TABLE IF NOT EXISTS feeder_cables (
    cable_id TEXT PRIMARY KEY,
    cabinet_id TEXT NOT NULL REFERENCES cabinets(cabinet_id),
    fiber_count INTEGER NOT NULL CHECK (fiber_count > 0),
    fibers_used INTEGER NOT NULL DEFAULT 0 CHECK (fibers_used >= 0),
    fibers_reserved INTEGER NOT NULL DEFAULT 0 CHECK (fibers_reserved >= 0),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE', 'RETIRED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (fibers_used + fibers_reserved <= fiber_count)
);

ALTER TABLE roes ADD COLUMN IF NOT EXISTS street_name TEXT;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS street_number TEXT;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS city TEXT;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS postal_code TEXT;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS latitude DOUBLE PRECISION;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS longitude DOUBLE PRECISION;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS location_accuracy_m DOUBLE PRECISION;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS geolocated_at TIMESTAMPTZ;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS geolocation_source TEXT;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS address_note TEXT;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS feeder_cable_id TEXT REFERENCES feeder_cables(cable_id);
ALTER TABLE roes ADD COLUMN IF NOT EXISTS created_by_technician TEXT REFERENCES technicians(technician_id);
ALTER TABLE roes ADD COLUMN IF NOT EXISTS gps_exception_reason TEXT;
ALTER TABLE roes ADD COLUMN IF NOT EXISTS gps_exception_authorized_by TEXT REFERENCES technicians(technician_id);

CREATE TABLE IF NOT EXISTS feeder_cable_allocations (
    allocation_id TEXT PRIMARY KEY,
    cable_id TEXT NOT NULL REFERENCES feeder_cables(cable_id),
    roe_id TEXT NOT NULL REFERENCES roes(roe_id),
    fibers_allocated INTEGER NOT NULL CHECK (fibers_allocated > 0),
    fibers_used INTEGER NOT NULL DEFAULT 0 CHECK (fibers_used >= 0),
    fibers_reserved INTEGER NOT NULL DEFAULT 0 CHECK (fibers_reserved >= 0),
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'RELEASED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (cable_id, roe_id),
    CHECK (fibers_used + fibers_reserved <= fibers_allocated)
);

CREATE TABLE IF NOT EXISTS onts (
    ont_id TEXT PRIMARY KEY,
    serial TEXT NOT NULL UNIQUE,
    roe_id TEXT NOT NULL REFERENCES roes(roe_id),
    vendor TEXT,
    model TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE', 'RETIRED')),
    created_by_technician TEXT NOT NULL REFERENCES technicians(technician_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS modems (
    modem_id TEXT PRIMARY KEY,
    serial TEXT NOT NULL UNIQUE,
    vendor TEXT,
    model TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE', 'RETIRED')),
    created_by_technician TEXT NOT NULL REFERENCES technicians(technician_id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS ont_modem_links (
    link_id TEXT PRIMARY KEY,
    ont_id TEXT NOT NULL REFERENCES onts(ont_id),
    modem_id TEXT NOT NULL REFERENCES modems(modem_id),
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'REPLACED', 'UNLINKED')),
    linked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ,
    replaced_by_link_id TEXT REFERENCES ont_modem_links(link_id),
    created_by_technician TEXT NOT NULL REFERENCES technicians(technician_id),
    source_event TEXT NOT NULL,
    CHECK ((status = 'ACTIVE' AND ended_at IS NULL) OR status <> 'ACTIVE')
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_ont_modem_one_active_per_ont
    ON ont_modem_links(ont_id) WHERE status = 'ACTIVE';
CREATE UNIQUE INDEX IF NOT EXISTS idx_ont_modem_one_active_per_modem
    ON ont_modem_links(modem_id) WHERE status = 'ACTIVE';

CREATE TABLE IF NOT EXISTS field_technician_sessions (
    session_id TEXT PRIMARY KEY,
    technician_id TEXT NOT NULL REFERENCES technicians(technician_id),
    telegram_user_id TEXT NOT NULL,
    telegram_chat_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('OTP_PENDING', 'ACTIVE', 'EXPIRED', 'REVOKED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    verified_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_field_sessions_technician_status
    ON field_technician_sessions(technician_id, status, expires_at);

CREATE TABLE IF NOT EXISTS field_otp_challenges (
    challenge_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES field_technician_sessions(session_id) ON DELETE CASCADE,
    otp_hash TEXT NOT NULL,
    otp_salt TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 5),
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_field_otp_one_unconsumed_per_session
    ON field_otp_challenges(session_id) WHERE consumed_at IS NULL;

CREATE TABLE IF NOT EXISTS telegram_workflows (
    workflow_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES field_technician_sessions(session_id),
    technician_id TEXT NOT NULL REFERENCES technicians(technician_id),
    workflow_type TEXT NOT NULL CHECK (workflow_type IN ('CREATE_ROE', 'REGISTER_ONT', 'REGISTER_MODEM', 'REPLACE_MODEM', 'UNLINK_MODEM', 'SEARCH_ASSET', 'NEARBY_ROE')),
    state TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('DRAFT', 'REVIEW', 'CONFIRMED', 'CANCELLED', 'EXPIRED', 'FAILED')),
    draft_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    confirmed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_telegram_one_open_workflow_per_session
    ON telegram_workflows(session_id) WHERE status IN ('DRAFT', 'REVIEW');

CREATE TABLE IF NOT EXISTS telegram_update_receipts (
    update_id BIGINT NOT NULL,
    telegram_chat_id TEXT NOT NULL,
    session_id TEXT NOT NULL REFERENCES field_technician_sessions(session_id),
    result_code TEXT NOT NULL,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (telegram_chat_id, update_id)
);

CREATE TABLE IF NOT EXISTS field_operation_audit (
    audit_id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    workflow_id TEXT REFERENCES telegram_workflows(workflow_id),
    correlation_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    actor_type TEXT NOT NULL CHECK (actor_type IN ('TECHNICIAN', 'NETWORK_DIRECTOR')),
    actor_id TEXT NOT NULL,
    key_id TEXT,
    operation_type TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    sanitized_input JSONB NOT NULL,
    before_state JSONB,
    after_state JSONB,
    validator_result JSONB NOT NULL,
    agent_decision JSONB,
    result_code TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_field_operation_audit_resource
    ON field_operation_audit(resource_type, resource_id, created_at);
CREATE INDEX IF NOT EXISTS idx_field_operation_audit_correlation
    ON field_operation_audit(correlation_id, created_at);

CREATE TABLE IF NOT EXISTS roe_selection_proposals (
    proposal_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES telegram_workflows(workflow_id),
    cabinet_id TEXT NOT NULL REFERENCES cabinets(cabinet_id),
    agent_id TEXT NOT NULL REFERENCES agents(agent_id),
    current_roe_id TEXT REFERENCES roes(roe_id),
    candidate_roe_id TEXT NOT NULL REFERENCES roes(roe_id),
    inventory_fingerprint TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('ALLOW', 'ALLOW_WITH_WARNING', 'DENY', 'NEEDS_SUPERVISOR')),
    reason_code TEXT NOT NULL,
    risk TEXT NOT NULL CHECK (risk IN ('LOW', 'MEDIUM', 'HIGH')),
    status TEXT NOT NULL CHECK (status IN ('PENDING', 'CONFIRMED', 'REJECTED', 'EXPIRED')),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    confirmed_at TIMESTAMPTZ
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_roe_selection_one_proposal_per_workflow
    ON roe_selection_proposals(workflow_id);

INSERT INTO schema_migrations (version)
VALUES ('007_telegram_field_operations.sql')
ON CONFLICT (version) DO NOTHING;
