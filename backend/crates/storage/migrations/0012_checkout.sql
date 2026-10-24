CREATE TABLE checkout_drafts (
 id text PRIMARY KEY,
 owner_hash text NOT NULL CHECK(owner_hash ~ '^[0-9a-f]{64}$'),
 user_id text REFERENCES users(id),
 version bigint NOT NULL DEFAULT 1 CHECK(version>0),
 input jsonb NOT NULL CHECK(octet_length(input::text)<=65536),
 quote jsonb CHECK(octet_length(quote::text)<=131072),
 quote_expires_at bigint,
 expires_at bigint NOT NULL,
 locked boolean NOT NULL DEFAULT false,
 created_at bigint NOT NULL
);
CREATE INDEX checkout_owner ON checkout_drafts(owner_hash,created_at);
CREATE TABLE payment_attempts (
 id text PRIMARY KEY,
 draft_id text NOT NULL REFERENCES checkout_drafts(id),
 owner_hash text NOT NULL,
 client_key_hash text NOT NULL CHECK(client_key_hash ~ '^[0-9a-f]{64}$'),
 fingerprint text NOT NULL CHECK(fingerprint ~ '^[0-9a-f]{64}$'),
 state text NOT NULL CHECK(state IN ('creating','unknown','open','processing','paid','failed','expired','manual_review')),
 provider_kind text NOT NULL CHECK(provider_kind IN ('session','intent')),
 provider_id text UNIQUE,
 amount_grosze bigint NOT NULL CHECK(amount_grosze BETWEEN 1 AND 99999999),
 currency text NOT NULL DEFAULT 'pln' CHECK(currency='pln'),
 idempotency_key text NOT NULL UNIQUE,
 provider_request jsonb NOT NULL CHECK(octet_length(provider_request::text)<=65536),
 provider_response jsonb CHECK(octet_length(provider_response::text)<=65536),
 lease_token text,
 lease_until bigint,
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 1000),
 next_attempt bigint NOT NULL,
 created_at bigint NOT NULL,
 updated_at bigint NOT NULL,
 UNIQUE(owner_hash,client_key_hash)
);
-- Closed attempts may be retried with a new key; an ambiguous attempt never releases this guard.
CREATE UNIQUE INDEX payment_one_active ON payment_attempts(draft_id)
 WHERE state IN ('creating','unknown','open','processing','paid','manual_review');
CREATE INDEX payment_ready ON payment_attempts(next_attempt)
 WHERE state IN ('creating','unknown','open','processing');
ALTER TABLE orders ALTER COLUMN provider_id DROP NOT NULL;
ALTER TABLE orders ADD COLUMN payment_attempt_id text UNIQUE REFERENCES payment_attempts(id);
CREATE TABLE stripe_events (
 id text PRIMARY KEY CHECK(octet_length(id)<=128),
 event_type text NOT NULL CHECK(octet_length(event_type)<=128),
 provider_kind text NOT NULL CHECK(provider_kind IN ('session','intent')),
 object_id text NOT NULL CHECK(octet_length(object_id)<=128),
 payload jsonb NOT NULL CHECK(octet_length(payload::text)<=262144),
 state text NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','processing','done','failed')),
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 20),
 next_attempt bigint NOT NULL,
 lease_token text,
 lease_until bigint,
 created_at bigint NOT NULL,
 finished_at bigint
);
CREATE INDEX stripe_event_ready ON stripe_events(next_attempt) WHERE state IN ('pending','processing');
