CREATE TABLE orders (
 id text PRIMARY KEY,
 owner_hash text NOT NULL CHECK(length(owner_hash)=64),
 user_id text REFERENCES users(id),
 contact jsonb NOT NULL CHECK(octet_length(contact::text)<=8192),
 lines jsonb NOT NULL CHECK(octet_length(lines::text)<=65536),
 amount_grosze bigint NOT NULL CHECK(amount_grosze BETWEEN 1 AND 99999999),
 provider_kind text NOT NULL CHECK(provider_kind IN ('intent','session')),
 provider_id text NOT NULL UNIQUE,
 status text NOT NULL DEFAULT 'awaiting_payment' CHECK(status IN ('awaiting_payment','paid','in_production','ready','shipped','completed')),
 guest_updates boolean NOT NULL,
 created_at bigint NOT NULL,
 payment_checked_at bigint NOT NULL DEFAULT 0,
 paid_at bigint
);
CREATE INDEX orders_owner ON orders(owner_hash,created_at DESC);
CREATE INDEX orders_user ON orders(user_id,created_at DESC);
CREATE TABLE order_messages (
 id text PRIMARY KEY,
 order_id text NOT NULL REFERENCES orders(id),
 author_id text REFERENCES users(id),
 operator boolean NOT NULL,
 body text NOT NULL CHECK(octet_length(body) BETWEEN 1 AND 4000),
 created_at bigint NOT NULL
);
CREATE INDEX order_messages_order ON order_messages(order_id,created_at);
