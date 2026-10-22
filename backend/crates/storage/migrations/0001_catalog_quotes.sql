CREATE TABLE catalog_materials (
    id text PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 64),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    density_g_cm3 double precision NOT NULL CHECK (density_g_cm3 > 0 AND density_g_cm3 < 10),
    cost_net_grosze_per_kg bigint CHECK (cost_net_grosze_per_kg BETWEEN 1 AND 100000000),
    production_enabled boolean NOT NULL DEFAULT false,
    source jsonb NOT NULL CHECK (jsonb_typeof(source) = 'object')
);

CREATE TABLE catalog_colors (
    material_id text NOT NULL REFERENCES catalog_materials(id),
    code text NOT NULL CHECK (length(code) BETWEEN 1 AND 64),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    rgba jsonb NOT NULL CHECK (jsonb_typeof(rgba) = 'array' AND jsonb_array_length(rgba) BETWEEN 1 AND 16),
    in_stock boolean NOT NULL DEFAULT false,
    PRIMARY KEY (material_id, code)
);

CREATE TABLE quotes (
    id text PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 128),
    request jsonb NOT NULL CHECK (jsonb_typeof(request) = 'object' AND octet_length(request::text) <= 1048576),
    result jsonb NOT NULL CHECK (jsonb_typeof(result) = 'object' AND octet_length(result::text) <= 1048576),
    created_at timestamptz NOT NULL DEFAULT now()
);
