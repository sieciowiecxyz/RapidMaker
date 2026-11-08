ALTER TABLE sessions ADD COLUMN family_id text NOT NULL DEFAULT gen_random_uuid()::text;
CREATE INDEX sessions_family ON sessions(family_id);
