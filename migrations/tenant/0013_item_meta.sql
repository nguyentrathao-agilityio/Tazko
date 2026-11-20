ALTER TABLE checklist_items ADD COLUMN assignee_id INTEGER REFERENCES people (id) ON DELETE SET NULL;
ALTER TABLE checklist_items ADD COLUMN due_at INTEGER;

CREATE INDEX checklist_items_assignee ON checklist_items (assignee_id) WHERE assignee_id IS NOT NULL;

CREATE TRIGGER touch_checklist_items_meta AFTER UPDATE OF assignee_id, due_at ON checklist_items
BEGIN
    UPDATE cards SET updated_at = unixepoch()
    WHERE id = (SELECT card_id FROM checklists WHERE id = NEW.checklist_id);
END;

ALTER TABLE notifications ADD COLUMN item_id INTEGER REFERENCES checklist_items (id) ON DELETE SET NULL;
