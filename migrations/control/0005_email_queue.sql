DROP INDEX IF EXISTS email_queue_pending;
DROP TABLE email_queue;

CREATE TABLE email_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    to_addr TEXT NOT NULL,
    subject TEXT NOT NULL,
    body_text TEXT NOT NULL,
    body_html TEXT NOT NULL,
    org_id INTEGER REFERENCES orgs (id) ON DELETE SET NULL,
    created_by INTEGER REFERENCES users (id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'sent', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    next_attempt_at INTEGER NOT NULL DEFAULT (unixepoch()),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    sent_at INTEGER
);

CREATE INDEX email_queue_due ON email_queue (next_attempt_at) WHERE status = 'pending';
CREATE INDEX email_queue_recent ON email_queue (to_addr, kind, created_at);
CREATE INDEX email_queue_org ON email_queue (org_id, kind, created_at);
