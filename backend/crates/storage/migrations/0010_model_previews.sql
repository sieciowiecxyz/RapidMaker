ALTER TABLE private_models ADD COLUMN preview_bytes bigint NOT NULL DEFAULT 0
    CHECK (preview_bytes BETWEEN 0 AND 26214400);
