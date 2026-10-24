-- Old email-only sessions do not satisfy the new two-factor policy.
DELETE FROM sessions;
ALTER TABLE users ADD COLUMN password_hash text;
ALTER TABLE users ADD COLUMN totp_secret bytea CHECK (totp_secret IS NULL OR octet_length(totp_secret)=61);
ALTER TABLE users ADD COLUMN totp_pending bytea CHECK (totp_pending IS NULL OR octet_length(totp_pending)=61);
ALTER TABLE users ADD COLUMN totp_pending_until bigint;
ALTER TABLE users ADD COLUMN totp_last_step bigint NOT NULL DEFAULT -1;
ALTER TABLE users ADD COLUMN recovery_hashes jsonb NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(recovery_hashes)='array' AND jsonb_array_length(recovery_hashes)<=8);
ALTER TABLE sessions ADD COLUMN stage text NOT NULL DEFAULT 'setup' CHECK(stage IN ('setup','primary','second_factor','authenticated'));
ALTER TABLE sessions ADD COLUMN primary_verified_at bigint;
ALTER TABLE sessions ADD COLUMN factor_verified_at bigint;
ALTER TABLE sessions ADD COLUMN factor_verifier bytea CHECK(factor_verifier IS NULL OR octet_length(factor_verifier)=32);
ALTER TABLE sessions ADD COLUMN factor_sent_at bigint;
ALTER TABLE sessions ADD COLUMN factor_expires_at bigint;
ALTER TABLE sessions ADD COLUMN factor_attempts smallint NOT NULL DEFAULT 0 CHECK(factor_attempts BETWEEN 0 AND 5);
ALTER TABLE sessions ADD CONSTRAINT authenticated_factors CHECK(stage<>'authenticated' OR (primary_verified_at IS NOT NULL AND factor_verified_at IS NOT NULL));
