ALTER TABLE private_models ADD COLUMN deletion_started boolean NOT NULL DEFAULT false;
CREATE TABLE slicer_jobs (
    id text PRIMARY KEY,
    owner_hash text NOT NULL CHECK (owner_hash ~ '^[0-9a-f]{64}$'),
    model_id text NOT NULL,
    fingerprint text NOT NULL CHECK (length(fingerprint) BETWEEN 1 AND 256),
    payload jsonb NOT NULL CHECK (jsonb_typeof(payload)='object' AND octet_length(payload::text)<=65536),
    status text NOT NULL DEFAULT 'queued' CHECK (status IN ('queued','running','succeeded','failed')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 3),
    lease_token text,
    lease_until bigint,
    available_at bigint NOT NULL,
    result jsonb CHECK (jsonb_typeof(result)='object' AND octet_length(result::text)<=1048576),
    error text CHECK (octet_length(error)<=512),
    created_at bigint NOT NULL,
    expires_at bigint NOT NULL,
    CHECK ((status='running')=(lease_token IS NOT NULL AND lease_until IS NOT NULL))
);
CREATE INDEX slicer_jobs_ready ON slicer_jobs(available_at,created_at) WHERE status='queued';
CREATE INDEX slicer_jobs_owner ON slicer_jobs(owner_hash,fingerprint,expires_at);
CREATE INDEX slicer_jobs_models ON slicer_jobs(model_id) WHERE status IN ('queued','running');
