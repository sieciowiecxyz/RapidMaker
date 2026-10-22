CREATE TABLE pricing_config (
    id boolean PRIMARY KEY DEFAULT true CHECK (id),
    value jsonb NOT NULL CHECK (jsonb_typeof(value) = 'object' AND octet_length(value::text) <= 16384)
);
