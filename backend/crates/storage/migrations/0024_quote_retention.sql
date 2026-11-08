-- Expiry stays in the immutable snapshot; JSON number comparison tolerates legacy values.
CREATE INDEX quotes_expiry ON quotes ((request->'expires_at'))
 WHERE jsonb_typeof(request->'expires_at')='number';
-- Each anti-reference guard has a matching index; order history survives file retention.
CREATE INDEX order_files_quote ON order_files(quote_id);
CREATE INDEX checkout_input_quotes ON checkout_drafts USING gin ((input->'parts') jsonb_path_ops);
CREATE INDEX checkout_result_quotes ON checkout_drafts USING gin ((quote->'quote_ids') jsonb_path_ops);
CREATE INDEX orders_line_quotes ON orders USING gin (lines jsonb_path_ops);
