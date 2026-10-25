ALTER TABLE orders ALTER COLUMN amount_grosze DROP NOT NULL;
ALTER TABLE orders DROP CONSTRAINT orders_status_check;
ALTER TABLE orders ADD CONSTRAINT orders_status_check CHECK(status IN ('awaiting_quote','quote_expired','awaiting_payment','paid','in_production','ready','shipped','completed'));
ALTER TABLE orders ADD CONSTRAINT orders_unquoted_amount CHECK(amount_grosze IS NOT NULL OR status IN ('awaiting_quote','quote_expired'));
CREATE TABLE manual_quotes (
 order_id text PRIMARY KEY REFERENCES orders(id),
 user_id text NOT NULL REFERENCES users(id),
 request_id text NOT NULL CHECK(length(request_id)=36),
 input jsonb NOT NULL CHECK(octet_length(input::text)<=65536),
 version bigint NOT NULL DEFAULT 1 CHECK(version>0),
 offer jsonb CHECK(octet_length(offer::text)<=8192),
 expires_at bigint NOT NULL,
 UNIQUE(user_id,request_id)
);
CREATE INDEX manual_quotes_expiry ON manual_quotes(expires_at);
CREATE VIEW manual_quote_views AS
 SELECT m.order_id,jsonb_build_object(
 'version',m.version,'valid_until',m.expires_at,'amount_grosze',o.amount_grosze,
 'payment_started',o.payment_attempt_id IS NOT NULL,'payment_state',p.state,
 'shipping_grosze',m.offer->'shipping_grosze','scope',m.offer->'scope','notes',m.input->'notes',
 'state',CASE WHEN o.paid_at IS NOT NULL THEN 'paid'
   WHEN o.status='quote_expired' OR p.state IN ('failed','expired') OR (o.payment_attempt_id IS NULL AND m.expires_at<=extract(epoch from now())::bigint) THEN 'expired'
   WHEN o.payment_attempt_id IS NOT NULL THEN 'payment_started'
   WHEN m.offer IS NOT NULL THEN 'offered' ELSE 'awaiting_quote' END) AS offer
 FROM manual_quotes m JOIN orders o ON o.id=m.order_id LEFT JOIN payment_attempts p ON p.id=o.payment_attempt_id;
ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_kind_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_kind_check CHECK(kind IN ('order_confirmed','payment_received','status_changed','chat_message','shipment_dispatched','manual_offer'));
