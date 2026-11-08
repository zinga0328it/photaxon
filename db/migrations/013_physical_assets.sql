CREATE TABLE IF NOT EXISTS physical_asset_records (
    asset_id TEXT PRIMARY KEY,
    asset_kind TEXT NOT NULL CHECK (asset_kind IN ('CABINET','SPLITTER','CLOSURE','CABLE','TUBE','FIBER','ROE','PTE','TERMINAL_BLOCK','PORT','RISER','FLOOR_DISTRIBUTION','TERMINATION','OTO','ONT','MODEM')),
    operational_code TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('PLANNED','ACTIVE','INACTIVE','FAULT','RETIRED')),
    attributes JSONB NOT NULL CHECK (jsonb_typeof(attributes)='object'),
    revision BIGINT NOT NULL DEFAULT 1 CHECK (revision>0),
    created_by TEXT NOT NULL,
    updated_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(asset_kind,operational_code),
    UNIQUE(asset_id,asset_kind)
);

CREATE TABLE IF NOT EXISTS physical_asset_dependencies (
    asset_id TEXT NOT NULL REFERENCES physical_asset_records(asset_id),
    target_id TEXT NOT NULL REFERENCES physical_asset_records(asset_id),
    PRIMARY KEY(asset_id,target_id),
    CHECK(asset_id<>target_id)
);

CREATE TABLE IF NOT EXISTS physical_resource_assignments (
    assignment_id TEXT PRIMARY KEY,
    resource_id TEXT NOT NULL REFERENCES physical_asset_records(asset_id),
    work_order_id TEXT NOT NULL,
    customer_id TEXT REFERENCES customers(customer_id),
    status TEXT NOT NULL CHECK(status IN ('RESERVED','OCCUPIED','RELEASED')),
    assigned_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    released_at TIMESTAMPTZ,
    CHECK((status='RELEASED')=(released_at IS NOT NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS physical_resource_one_assignment ON physical_resource_assignments(resource_id) WHERE status<>'RELEASED';
INSERT INTO schema_migrations(version) VALUES ('013_physical_assets.sql') ON CONFLICT(version) DO NOTHING;