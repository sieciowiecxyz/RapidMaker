CREATE TABLE private_models (
    id text PRIMARY KEY CHECK (id ~ '^[0-9a-f-]{36}$'),
    owner_hash text NOT NULL CHECK (owner_hash ~ '^[0-9a-f]{64}$'),
    filename text NOT NULL CHECK (octet_length(filename) BETWEEN 1 AND 255),
    extension text NOT NULL CHECK (extension IN ('stl','step','stp','3mf')),
    source_bytes bigint NOT NULL CHECK (source_bytes BETWEEN 1 AND 33554432),
    mesh_bytes bigint NOT NULL CHECK (mesh_bytes BETWEEN 84 AND 52428800),
    sha256 text NOT NULL CHECK (sha256 ~ '^[0-9a-f]{64}$'),
    dimensions_mm double precision[] NOT NULL CHECK (array_length(dimensions_mm,1)=3 AND
        0 < ALL(dimensions_mm) AND 2000000 >= ALL(dimensions_mm)),
    expires_at bigint NOT NULL CHECK (expires_at > 0)
);
CREATE INDEX private_models_owner ON private_models(owner_hash);
CREATE INDEX private_models_expiry ON private_models(expires_at);
