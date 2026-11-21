ALTER TABLE device_codes ADD COLUMN client TEXT NOT NULL DEFAULT '';
ALTER TABLE device_codes ADD COLUMN denied_at INTEGER;
ALTER TABLE device_codes ADD COLUMN used_at INTEGER;
ALTER TABLE device_codes ADD COLUMN last_poll_at INTEGER;

CREATE INDEX device_codes_expiry ON device_codes (expires_at);
