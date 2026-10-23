-- Holds exist before a payment provider is called; ambiguous attempts stay protected.
CREATE TABLE order_file_holds (
    order_id text PRIMARY KEY,
    owner_hash text NOT NULL CHECK (owner_hash ~ '^[0-9a-f]{64}$'),
    created_at bigint NOT NULL
);
CREATE INDEX order_file_holds_owner ON order_file_holds(owner_hash,created_at);
CREATE TABLE order_files (
    order_id text NOT NULL REFERENCES order_file_holds(order_id),
    quote_id text NOT NULL REFERENCES quotes(id),
    -- Keep the link and immutable quote after physical model cleanup.
    model_id text NOT NULL,
    filename text NOT NULL,
    retain_until bigint,
    PRIMARY KEY(order_id,quote_id)
);
CREATE INDEX order_files_model ON order_files(model_id,retain_until);
