-- A persisted reservation precedes Stripe HTTP; uncertain results keep that reservation.
CREATE TABLE payment_refunds (
 id text PRIMARY KEY,
 order_id text NOT NULL REFERENCES orders(id),
 request_id text NOT NULL,
 actor_id text NOT NULL REFERENCES users(id),
 amount_grosze bigint NOT NULL CHECK(amount_grosze BETWEEN 1 AND 99999999),
 payment_amount_grosze bigint NOT NULL CHECK(payment_amount_grosze BETWEEN 1 AND 99999999),
 CHECK(amount_grosze<=payment_amount_grosze),
 state text NOT NULL DEFAULT 'queued' CHECK(state IN ('queued','unknown','pending','requires_action','succeeded','failed','canceled','manual_review')),
 provider_kind text NOT NULL CHECK(provider_kind IN ('intent','session')),
 original_provider_id text NOT NULL CHECK(octet_length(original_provider_id) BETWEEN 1 AND 128),
 correlation_id text NOT NULL,
 payment_intent text CHECK(payment_intent ~ '^pi_[A-Za-z0-9_]+$'),
 provider_id text UNIQUE CHECK(provider_id ~ '^re_[A-Za-z0-9_]+$'),
 idempotency_key text NOT NULL UNIQUE,
 lease_token text,
 lease_until bigint,
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 1000),
 next_attempt bigint NOT NULL,
 created_at bigint NOT NULL,
 updated_at bigint NOT NULL,
 UNIQUE(order_id,request_id)
);
CREATE INDEX refunds_ready ON payment_refunds(next_attempt) WHERE state IN ('queued','unknown','pending','requires_action');
CREATE INDEX refunds_actor ON payment_refunds(actor_id,created_at);
