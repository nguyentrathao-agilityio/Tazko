ALTER TABLE email_queue ADD COLUMN last_attempt_at INTEGER;

CREATE INDEX email_queue_finished ON email_queue (created_at) WHERE status != 'pending';
