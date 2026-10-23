ALTER TABLE sessions ADD COLUMN admin_verified_at bigint;
CREATE TABLE passkeys (
    id text PRIMARY KEY,
    credential_id bytea NOT NULL UNIQUE,
    user_id text NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential jsonb NOT NULL CHECK (octet_length(credential::text)<=32768),
    counter bigint NOT NULL DEFAULT 0,
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 64),
    created_at bigint NOT NULL
);
CREATE INDEX passkeys_user ON passkeys(user_id);
CREATE TABLE passkey_challenges (
    id text PRIMARY KEY,
    session_hash bytea NOT NULL REFERENCES sessions(token_hash) ON DELETE CASCADE,
    user_id text NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    purpose text NOT NULL CHECK (purpose IN ('register','login')),
    state jsonb NOT NULL CHECK (octet_length(state::text)<=65536),
    bootstrap boolean NOT NULL DEFAULT false,
    created_at bigint NOT NULL,
    expires_at bigint NOT NULL,
    consumed_at bigint
);
CREATE INDEX passkey_challenges_expiry ON passkey_challenges(expires_at);
CREATE TABLE admin_bootstrap (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    user_id text NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_hash bytea NOT NULL REFERENCES sessions(token_hash) ON DELETE CASCADE,
    recovery boolean NOT NULL DEFAULT false,
    secret_hash bytea NOT NULL CHECK (octet_length(secret_hash)=32),
    expires_at bigint NOT NULL
);
