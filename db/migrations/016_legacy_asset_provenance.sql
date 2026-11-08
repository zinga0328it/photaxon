ALTER TABLE physical_asset_records ADD COLUMN IF NOT EXISTS metadata_status TEXT NOT NULL DEFAULT 'MEASURED'
    CHECK(metadata_status IN ('MEASURED','LEGACY_INCOMPLETE'));
ALTER TABLE physical_asset_records ADD COLUMN IF NOT EXISTS legacy_source_table TEXT;
ALTER TABLE physical_asset_records ADD COLUMN IF NOT EXISTS legacy_source_id TEXT;
ALTER TABLE physical_asset_records ADD COLUMN IF NOT EXISTS legacy_observed_at TIMESTAMPTZ;
CREATE UNIQUE INDEX IF NOT EXISTS physical_asset_legacy_source_unique
    ON physical_asset_records(legacy_source_table,legacy_source_id)
    WHERE legacy_source_table IS NOT NULL;
INSERT INTO schema_migrations(version) VALUES ('016_legacy_asset_provenance.sql') ON CONFLICT(version) DO NOTHING;