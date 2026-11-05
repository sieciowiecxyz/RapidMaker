ALTER TABLE email_outbox DROP CONSTRAINT email_outbox_kind_check;
ALTER TABLE email_outbox ADD CONSTRAINT email_outbox_kind_check CHECK(kind IN ('order_confirmed','payment_received','status_changed','chat_message','shipment_dispatched','manual_offer','legal_documents','refund_succeeded'));

ALTER TABLE payment_refunds ADD COLUMN notification_queued_at bigint;
ALTER TABLE payment_refunds ADD COLUMN notification_retry_at bigint NOT NULL DEFAULT 0;
-- Existing successes predate this notification contract; do not send historical mail.
UPDATE payment_refunds SET notification_queued_at=updated_at WHERE state='succeeded';
CREATE INDEX payment_refunds_notification_pending ON payment_refunds(notification_retry_at,created_at,id)
    WHERE state='succeeded' AND notification_queued_at IS NULL;
