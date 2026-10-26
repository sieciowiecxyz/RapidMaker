CREATE TABLE order_production (
 order_id text PRIMARY KEY REFERENCES orders(id),
 revision bigint NOT NULL CHECK(revision>0),
 value jsonb NOT NULL CHECK(jsonb_typeof(value)='object' AND octet_length(value::text)<=16384),
 updated_at bigint NOT NULL,
 updated_by text NOT NULL REFERENCES users(id)
);
CREATE TABLE invoice_drafts (
 order_id text PRIMARY KEY REFERENCES orders(id),
 revision bigint NOT NULL CHECK(revision>0),
 value jsonb NOT NULL CHECK(jsonb_typeof(value)='object' AND octet_length(value::text)<=16384),
 updated_at bigint NOT NULL,
 updated_by text NOT NULL REFERENCES users(id)
);
