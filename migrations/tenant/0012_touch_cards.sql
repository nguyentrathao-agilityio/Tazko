CREATE TRIGGER touch_card_labels_insert AFTER INSERT ON card_labels
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = NEW.card_id;
END;

CREATE TRIGGER touch_card_labels_delete AFTER DELETE ON card_labels
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = OLD.card_id;
END;

CREATE TRIGGER touch_card_members_insert AFTER INSERT ON card_members
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = NEW.card_id;
END;

CREATE TRIGGER touch_card_members_delete AFTER DELETE ON card_members
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = OLD.card_id;
END;

CREATE TRIGGER touch_comments_insert AFTER INSERT ON comments
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = NEW.card_id;
END;

CREATE TRIGGER touch_comments_delete AFTER DELETE ON comments
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = OLD.card_id;
END;

CREATE TRIGGER touch_attachments_insert AFTER INSERT ON attachments
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = NEW.card_id;
END;

CREATE TRIGGER touch_attachments_delete AFTER DELETE ON attachments
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = OLD.card_id;
END;

CREATE TRIGGER touch_checklists_insert AFTER INSERT ON checklists
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = NEW.card_id;
END;

CREATE TRIGGER touch_checklists_delete AFTER DELETE ON checklists
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = OLD.card_id;
END;

CREATE TRIGGER touch_comments_update AFTER UPDATE OF body ON comments
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = NEW.card_id;
END;

CREATE TRIGGER touch_checklist_items_insert AFTER INSERT ON checklist_items
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = (SELECT card_id FROM checklists WHERE id = NEW.checklist_id);
END;

CREATE TRIGGER touch_checklist_items_delete AFTER DELETE ON checklist_items
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = (SELECT card_id FROM checklists WHERE id = OLD.checklist_id);
END;

CREATE TRIGGER touch_checklist_items_update AFTER UPDATE OF text, done ON checklist_items
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = (SELECT card_id FROM checklists WHERE id = NEW.checklist_id);
END;
