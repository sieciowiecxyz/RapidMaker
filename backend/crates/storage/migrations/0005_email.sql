CREATE TABLE email_outbox (
    id text PRIMARY KEY CHECK (id ~ '^[A-Za-z0-9-]{1,64}$'),
    event_key text NOT NULL UNIQUE CHECK (octet_length(event_key) BETWEEN 1 AND 256),
    user_id text REFERENCES users(id),
    kind text NOT NULL CHECK (kind IN ('order_confirmed','payment_received','status_changed','chat_message')),
    recipient text NOT NULL CHECK (octet_length(recipient) BETWEEN 3 AND 254),
    payload jsonb NOT NULL CHECK (octet_length(payload::text) <= 8192),
    optional_enabled boolean NOT NULL,
    status text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','sending','sent','failed','skipped')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 5),
    available_at bigint NOT NULL,
    locked_until bigint,
    created_at bigint NOT NULL,
    sent_at bigint,
    finished_at bigint,
    last_error_code text CHECK (last_error_code ~ '^[a-z_]{1,40}$')
);
CREATE INDEX email_outbox_ready ON email_outbox (available_at) WHERE status='pending';
CREATE INDEX email_outbox_leases ON email_outbox (locked_until) WHERE status='sending';
CREATE INDEX email_outbox_retention ON email_outbox (finished_at) WHERE finished_at IS NOT NULL;
