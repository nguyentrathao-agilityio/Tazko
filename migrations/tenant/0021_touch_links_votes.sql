CREATE TRIGGER touch_card_links_insert AFTER INSERT ON card_links
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id IN (NEW.from_card, NEW.to_card);
END;

CREATE TRIGGER touch_card_links_delete AFTER DELETE ON card_links
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id IN (OLD.from_card, OLD.to_card);
END;

CREATE TRIGGER touch_card_votes_insert AFTER INSERT ON card_votes
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = NEW.card_id;
END;

CREATE TRIGGER touch_card_votes_delete AFTER DELETE ON card_votes
BEGIN
    UPDATE cards SET updated_at = unixepoch() WHERE id = OLD.card_id;
END;
