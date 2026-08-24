CREATE TABLE IF NOT EXISTS technicians (
    technician_id text PRIMARY KEY,
    display_name text NOT NULL,
    status text NOT NULL CHECK (status IN ('ACTIVE', 'INACTIVE'))
);
CREATE TABLE IF NOT EXISTS demo_resources (
    resource_id text PRIMARY KEY,
    cabinet_id text NOT NULL,
    splitter_id text NOT NULL,
    port_position integer NOT NULL,
    state text NOT NULL CHECK (state IN ('FREE', 'RESERVED', 'CONNECTED', 'VERIFIED', 'OCCUPIED', 'FAULT')),
    owner_transaction_id text,
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS telemetry_events (
    event_id text PRIMARY KEY,
    cabinet_id text NOT NULL,
    event_type text NOT NULL,
    observed_at timestamptz NOT NULL,
    payload jsonb NOT NULL,
    source_mode text NOT NULL CHECK (source_mode IN ('SIMULATED', 'PHYSICAL')),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS demo_runs (
    run_id text PRIMARY KEY,
    technician_id text NOT NULL REFERENCES technicians(technician_id),
    event_id text NOT NULL REFERENCES telemetry_events(event_id),
    agent_proposal jsonb NOT NULL,
    validation_result jsonb NOT NULL,
    transaction_id text,
    final_state text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO technicians (technician_id, display_name, status)
VALUES ('TECH-DEMO-001', 'FiberSpider Demo Technician', 'ACTIVE')
ON CONFLICT (technician_id) DO UPDATE SET status = EXCLUDED.status;
INSERT INTO demo_resources (resource_id, cabinet_id, splitter_id, port_position, state)
VALUES ('PORT-A-01', 'CAB-A', 'SPL-A-01', 1, 'FREE')
ON CONFLICT (resource_id) DO NOTHING;

