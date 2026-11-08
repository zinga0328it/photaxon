CREATE TABLE IF NOT EXISTS admin_photo_catalog (
    media_id TEXT PRIMARY KEY,
    work_order_id TEXT NOT NULL,
    asset_type TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    evidence_type TEXT NOT NULL,
    notes TEXT,
    current_version_id TEXT,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS admin_photo_versions (
    version_id TEXT PRIMARY KEY,
    media_id TEXT NOT NULL REFERENCES admin_photo_catalog(media_id),
    storage_key TEXT NOT NULL UNIQUE,
    content_type TEXT NOT NULL CHECK (content_type IN ('image/jpeg','image/png','image/webp')),
    byte_size BIGINT NOT NULL CHECK (byte_size BETWEEN 1 AND 7340032),
    sha256 TEXT NOT NULL CHECK (length(sha256)=64),
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(media_id,version_id)
);

DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid='admin_photo_catalog'::regclass AND conname='admin_photo_current_version_fk') THEN
        ALTER TABLE admin_photo_catalog ADD CONSTRAINT admin_photo_current_version_fk
            FOREIGN KEY(media_id,current_version_id) REFERENCES admin_photo_versions(media_id,version_id);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS admin_photo_wr_idx ON admin_photo_catalog(work_order_id,created_at);
CREATE INDEX IF NOT EXISTS admin_photo_asset_idx ON admin_photo_catalog(asset_type,asset_id);

INSERT INTO schema_migrations(version) VALUES ('012_admin_photo_catalog.sql') ON CONFLICT(version) DO NOTHING;