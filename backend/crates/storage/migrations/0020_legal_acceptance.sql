-- Existing accounts/orders have no inferred acceptance: NULL is intentionally retained.
ALTER TABLE auth_challenges ADD COLUMN legal_version text;
ALTER TABLE users ADD COLUMN legal_version text;
ALTER TABLE users ADD COLUMN legal_accepted_at bigint;
ALTER TABLE orders ADD COLUMN legal_version text;
ALTER TABLE orders ADD COLUMN legal_accepted_at bigint;
ALTER TABLE orders ADD COLUMN production_days integer CHECK(production_days > 0 AND production_days <= 365);
ALTER TABLE users ADD CONSTRAINT users_legal_pair CHECK((legal_version IS NULL) = (legal_accepted_at IS NULL));
ALTER TABLE orders ADD CONSTRAINT orders_legal_pair CHECK((legal_version IS NULL) = (legal_accepted_at IS NULL));
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_kind_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_kind_check CHECK(kind IN ('order_confirmed','payment_received','status_changed','chat_message','shipment_dispatched','manual_offer','legal_documents'));
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_payload_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_payload_check CHECK(octet_length(payload::text) <= CASE WHEN kind='legal_documents' THEN 131072 ELSE 16384 END);
