ALTER TABLE email_queue ADD COLUMN unsubscribe_url TEXT;

CREATE TABLE email_digests (
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    org_id INTEGER NOT NULL REFERENCES orgs (id) ON DELETE CASCADE,
    last_sent_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, org_id)
) WITHOUT ROWID;
