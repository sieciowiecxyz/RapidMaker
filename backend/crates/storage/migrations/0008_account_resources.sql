ALTER TABLE private_models ADD COLUMN user_id text REFERENCES users(id);
ALTER TABLE quotes ADD COLUMN user_id text REFERENCES users(id);
CREATE INDEX private_models_user ON private_models(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX quotes_user ON quotes(user_id) WHERE user_id IS NOT NULL;
