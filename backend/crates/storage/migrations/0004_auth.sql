CREATE TABLE users (
    id text PRIMARY KEY,
    email text NOT NULL UNIQUE CHECK (length(email) BETWEEN 3 AND 254 AND email = lower(btrim(email))),
    role text NOT NULL DEFAULT 'client' CHECK (role IN ('client','admin')),
    disabled_at bigint,
    created_at bigint NOT NULL,
    email_order_updates boolean NOT NULL DEFAULT true,
    email_chat_messages boolean NOT NULL DEFAULT true
);
CREATE TABLE auth_challenges (
    id text PRIMARY KEY,
    email text NOT NULL,
    browser_hash bytea NOT NULL CHECK (octet_length(browser_hash)=32),
    verifier bytea NOT NULL CHECK (octet_length(verifier)=32),
    attempts smallint NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 5),
    created_at bigint NOT NULL,
    expires_at bigint NOT NULL,
    consumed_at bigint
);
CREATE INDEX auth_challenges_email_created ON auth_challenges(email,created_at);
CREATE INDEX auth_challenges_expiry ON auth_challenges(expires_at);
CREATE TABLE sessions (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash)=32),
    user_id text NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at bigint NOT NULL,
    last_seen_at bigint NOT NULL,
    expires_at bigint NOT NULL
);
CREATE INDEX sessions_user ON sessions(user_id);
CREATE INDEX sessions_expiry ON sessions(expires_at);
CREATE TABLE auth_limits (
    scope_hash bytea PRIMARY KEY CHECK (octet_length(scope_hash)=32),
    window_start bigint NOT NULL,
    attempts bigint NOT NULL
);
CREATE INDEX auth_limits_expiry ON auth_limits(window_start);

-- Role/disabled changes revoke credentials; preference edits retain sessions.
CREATE FUNCTION revoke_changed_user_sessions() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.role IS DISTINCT FROM NEW.role OR OLD.disabled_at IS DISTINCT FROM NEW.disabled_at THEN
        DELETE FROM sessions WHERE user_id=NEW.id;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER users_revoke_sessions AFTER UPDATE OF role,disabled_at ON users
    FOR EACH ROW EXECUTE FUNCTION revoke_changed_user_sessions();
