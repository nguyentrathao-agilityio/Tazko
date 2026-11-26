ALTER TABLE invites ADD COLUMN expires_at INTEGER;

UPDATE invites SET expires_at = unixepoch() + 7 * 86400
WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE TRIGGER invites_expire_after_seven_days AFTER INSERT ON invites
WHEN NEW.expires_at IS NULL
BEGIN
    UPDATE invites SET expires_at = NEW.created_at + 7 * 86400 WHERE id = NEW.id;
END;
