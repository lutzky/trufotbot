-- Although 'patients' has a telegram_group_id, this might have changed since
-- the dose was given.
ALTER TABLE doses ADD telegram_group_id INTEGER;
ALTER TABLE doses ADD telegram_message_id INTEGER;
