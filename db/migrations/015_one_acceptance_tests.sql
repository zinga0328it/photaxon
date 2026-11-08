CREATE TABLE IF NOT EXISTS optical_precheck_profiles (
    profile_id TEXT PRIMARY KEY,
    operator_name TEXT NOT NULL,
    vendor TEXT NOT NULL,
    model TEXT NOT NULL,
    rx_min_dbm DOUBLE PRECISION NOT NULL,
    rx_max_dbm DOUBLE PRECISION NOT NULL,
    revision BIGINT NOT NULL DEFAULT 1,
    updated_by TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK(rx_min_dbm<=rx_max_dbm)
);

CREATE TABLE IF NOT EXISTS ont_acceptance_tests (
    test_id TEXT PRIMARY KEY,
    activation_id TEXT NOT NULL REFERENCES activation_requests(activation_id),
    work_order_id TEXT NOT NULL,
    ont_id TEXT NOT NULL REFERENCES onts(ont_id),
    ont_serial TEXT NOT NULL,
    operator_name TEXT NOT NULL,
    test_provider TEXT NOT NULL DEFAULT 'ONE' CHECK(test_provider='ONE'),
    provider_mode TEXT NOT NULL DEFAULT 'MANUAL_RECORDED' CHECK(provider_mode='MANUAL_RECORDED'),
    test_type TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    technician_id TEXT REFERENCES technicians(technician_id),
    optical_profile_id TEXT REFERENCES optical_precheck_profiles(profile_id),
    precheck_bounds JSONB,
    measurement JSONB,
    rx_dbm DOUBLE PRECISION,
    tx_dbm DOUBLE PRECISION,
    attenuation_db DOUBLE PRECISION,
    local_precheck_status TEXT NOT NULL CHECK(local_precheck_status IN ('PENDING','PRECHECK_PASS','PRECHECK_FAIL')),
    one_status TEXT NOT NULL DEFAULT 'ONE_PENDING' CHECK(one_status IN ('ONE_PENDING','ONE_PASS','ONE_FAIL','ERROR')),
    one_result_code TEXT,
    one_test_code TEXT,
    failure_reason TEXT,
    notes TEXT,
    raw_reference TEXT,
    created_by TEXT NOT NULL,
    updated_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK(one_status<>'ONE_PASS' OR (one_test_code IS NOT NULL AND length(trim(one_test_code))>0 AND completed_at IS NOT NULL)),
    CHECK(completed_at IS NULL OR completed_at>=started_at)
);
CREATE UNIQUE INDEX IF NOT EXISTS one_test_code_unique ON ont_acceptance_tests(one_test_code) WHERE one_test_code IS NOT NULL;
CREATE INDEX IF NOT EXISTS one_tests_wr_idx ON ont_acceptance_tests(work_order_id,started_at);

ALTER TABLE delivery_work_details ADD COLUMN IF NOT EXISTS one_required BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE delivery_work_details ADD COLUMN IF NOT EXISTS note_chiusura TEXT;
ALTER TABLE delivery_work_details ADD COLUMN IF NOT EXISTS extra_fields JSONB NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(extra_fields)='object');

CREATE TABLE IF NOT EXISTS delivery_closure_outbox (
    operation_id TEXT PRIMARY KEY REFERENCES control_plane_admin_operations(operation_id),
    work_order_id TEXT NOT NULL,
    external_reference TEXT NOT NULL,
    test_id TEXT NOT NULL REFERENCES ont_acceptance_tests(test_id),
    patch JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING' CHECK(status IN ('PENDING','DELIVERED','ERROR')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO schema_migrations(version) VALUES('015_one_acceptance_tests.sql') ON CONFLICT(version) DO NOTHING;