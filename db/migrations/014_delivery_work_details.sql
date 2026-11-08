ALTER TABLE activation_requests ALTER COLUMN technician_id DROP NOT NULL;
ALTER TABLE activation_requests ALTER COLUMN ont_serial DROP NOT NULL;

CREATE TABLE IF NOT EXISTS delivery_teams (
    team_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('ACTIVE','INACTIVE')),
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS delivery_team_members (
    team_id TEXT NOT NULL REFERENCES delivery_teams(team_id),
    technician_id TEXT NOT NULL REFERENCES technicians(technician_id),
    PRIMARY KEY(team_id,technician_id)
);
CREATE TABLE IF NOT EXISTS delivery_work_details (
    activation_id TEXT PRIMARY KEY REFERENCES activation_requests(activation_id),
    work_order_id TEXT NOT NULL UNIQUE,
    operator_name TEXT NOT NULL,
    customer_id TEXT REFERENCES customers(customer_id),
    customer_phone TEXT,
    address JSONB NOT NULL CHECK(jsonb_typeof(address)='object'),
    geolocation JSONB,
    appointment_at TIMESTAMPTZ,
    team_id TEXT REFERENCES delivery_teams(team_id),
    work_status TEXT NOT NULL CHECK(work_status IN ('NEW','ASSIGNED','SCHEDULED','IN_PROGRESS','COMPLETED','CANCELLED','KO')),
    ko_reason TEXT,
    notes TEXT,
    revision BIGINT NOT NULL DEFAULT 1 CHECK(revision>0),
    created_by TEXT NOT NULL,
    updated_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS delivery_appointment_idx ON delivery_work_details(appointment_at);
INSERT INTO schema_migrations(version) VALUES('014_delivery_work_details.sql') ON CONFLICT(version) DO NOTHING;