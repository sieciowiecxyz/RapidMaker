-- Pickup/cancel commands share the shipment lease, including recovery after an unknown HTTP result.
ALTER TABLE shipments DROP CONSTRAINT shipments_state_check;
ALTER TABLE shipments ADD CONSTRAINT shipments_state_check CHECK(state IN ('draft','queued','creating','ordering','booked','dispatched','delivered','manual_review','canceled'));
ALTER TABLE shipment_packages DROP CONSTRAINT shipment_packages_state_check;
ALTER TABLE shipment_packages ADD CONSTRAINT shipment_packages_state_check CHECK(state IN ('pending','creating','created','ordering','booked','dispatched','delivered','manual_review','canceled'));
CREATE TABLE shipment_actions (
 id text PRIMARY KEY,
 shipment_id text NOT NULL REFERENCES shipments(id),
 kind text NOT NULL CHECK(kind IN ('pickup','cancel')),
 request_id text NOT NULL UNIQUE,
 input jsonb NOT NULL CHECK(octet_length(input::text)<=32768),
 state text NOT NULL CHECK(state IN ('queued','submitting','pending','succeeded','partial','failed','manual_review')),
 result jsonb NOT NULL DEFAULT '{}' CHECK(octet_length(result::text)<=262144),
 actor_id text NOT NULL REFERENCES users(id),
 created_at bigint NOT NULL,
 updated_at bigint NOT NULL
);
CREATE UNIQUE INDEX shipment_actions_active ON shipment_actions(shipment_id) WHERE state IN ('queued','submitting','pending');
