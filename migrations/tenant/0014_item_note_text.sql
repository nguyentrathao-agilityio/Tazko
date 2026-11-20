ALTER TABLE notifications ADD COLUMN item_text TEXT;

CREATE INDEX notifications_item ON notifications (item_id) WHERE item_id IS NOT NULL;
