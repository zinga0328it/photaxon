-- Prevent two active ONTs from being assigned to the same GPON port.
-- Existing ONT assignments are preserved; the index applies only to assigned active ONTs.

CREATE UNIQUE INDEX IF NOT EXISTS idx_onts_one_active_per_gpon_port
    ON onts(current_port_id)
    WHERE status='ACTIVE' AND current_port_id IS NOT NULL;

INSERT INTO schema_migrations (version)
VALUES ('009_ont_gpon_port_safety.sql')
ON CONFLICT (version) DO NOTHING;