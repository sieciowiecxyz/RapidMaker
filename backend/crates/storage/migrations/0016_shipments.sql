-- Provider writes are preceded by durable local states; uncertain creation is never retried blindly.
CREATE TABLE shipments (
 id text PRIMARY KEY,
 order_id text NOT NULL UNIQUE REFERENCES orders(id),
 version bigint NOT NULL CHECK(version>0),
 last_error text CHECK(last_error ~ '^[a-z_]{1,80}$'),
 state text NOT NULL CHECK(state IN ('draft','queued','creating','ordering','booked','dispatched','delivered','manual_review')),
 input jsonb NOT NULL CHECK(octet_length(input::text)<=131072),
 quote jsonb CHECK(octet_length(quote::text)<=262144),
 request_id text UNIQUE,
 command_uuid text NOT NULL UNIQUE,
 actor_id text NOT NULL REFERENCES users(id),
 lease_token text,
 lease_until bigint,
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 1000),
 next_attempt bigint NOT NULL,
 created_at bigint NOT NULL,
 updated_at bigint NOT NULL
);
CREATE TABLE shipment_packages (
 id text PRIMARY KEY,
 shipment_id text NOT NULL REFERENCES shipments(id),
 reference text NOT NULL UNIQUE CHECK(length(reference)<=27),
 provider_id text UNIQUE,
 state text NOT NULL CHECK(state IN ('pending','creating','created','ordering','booked','dispatched','delivered','manual_review')),
 payload jsonb NOT NULL CHECK(octet_length(payload::text)<=32768),
 tracking jsonb NOT NULL DEFAULT '{}' CHECK(octet_length(tracking::text)<=32768),
 updated_at bigint NOT NULL
);
CREATE INDEX shipments_work ON shipments(next_attempt) WHERE state<>'draft' AND state<>'delivered';

ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_kind_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_kind_check CHECK(kind IN ('order_confirmed','payment_received','status_changed','chat_message','shipment_dispatched'));
