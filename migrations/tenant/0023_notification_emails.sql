ALTER TABLE notifications ADD COLUMN emailed_at INTEGER;

UPDATE notifications SET emailed_at = created_at;

CREATE INDEX notifications_unemailed ON notifications (id) WHERE emailed_at IS NULL;
