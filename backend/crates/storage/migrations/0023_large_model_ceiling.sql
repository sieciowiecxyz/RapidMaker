-- Absolute storage ceilings; application profiles retain their own smaller caps.
ALTER TABLE private_models DROP CONSTRAINT private_models_source_bytes_check;
ALTER TABLE private_models ADD CONSTRAINT private_models_source_bytes_check
    CHECK (source_bytes BETWEEN 1 AND 100663296);
ALTER TABLE private_models DROP CONSTRAINT private_models_mesh_bytes_check;
ALTER TABLE private_models ADD CONSTRAINT private_models_mesh_bytes_check
    CHECK (mesh_bytes BETWEEN 84 AND 209715200);
