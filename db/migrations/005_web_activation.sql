-- FS-05.5 web activation storage for FiberSpider
-- Additive and idempotent.

CREATE TABLE IF NOT EXISTS activation_requests (
    activation_id TEXT PRIMARY KEY,
    technician_id TEXT NOT NULL REFERENCES technicians(technician_id),
    wr TEXT NOT NULL,
    ont_serial TEXT NOT NULL,
    roe TEXT,
    cabinet_id TEXT,
    splitter_id TEXT,
    notes TEXT,
    status TEXT NOT NULL,
    wr_validation TEXT NOT NULL DEFAULT 'NOT_IMPLEMENTED',
    simulation_result JSONB,
    otp_verified_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS activation_otp_challenges (
    challenge_id TEXT PRIMARY KEY,
    activation_id TEXT NOT NULL REFERENCES activation_requests(activation_id) ON DELETE CASCADE,
    otp_hash TEXT NOT NULL,
    otp_salt TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0 AND attempts <= 5),
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_activation_otp_challenges_activation_id ON activation_otp_challenges(activation_id);

INSERT INTO schema_migrations (version)
VALUES ('005_web_activation.sql')
ON CONFLICT (version) DO NOTHING;
