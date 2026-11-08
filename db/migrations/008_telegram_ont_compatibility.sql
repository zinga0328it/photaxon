-- Additive compatibility for the existing ONT inventory table.
-- Preserve serial_number and historical ONT rows; new field operations also record ROE and actor metadata.

ALTER TABLE onts ADD COLUMN IF NOT EXISTS roe_id TEXT REFERENCES roes(roe_id);
ALTER TABLE onts ADD COLUMN IF NOT EXISTS vendor TEXT;
ALTER TABLE onts ADD COLUMN IF NOT EXISTS model TEXT;
ALTER TABLE onts ADD COLUMN IF NOT EXISTS created_by_technician TEXT REFERENCES technicians(technician_id);

CREATE INDEX IF NOT EXISTS idx_onts_roe_status
    ON onts(roe_id, status);

INSERT INTO schema_migrations (version)
VALUES ('008_telegram_ont_compatibility.sql')
ON CONFLICT (version) DO NOTHING;