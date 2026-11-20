CREATE TABLE webhooks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL,
    events TEXT NOT NULL,
    projects TEXT,
    salt TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_by INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE webhook_deliveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    webhook_id INTEGER NOT NULL REFERENCES webhooks (id) ON DELETE CASCADE,
    event TEXT NOT NULL,
    payload TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'delivered', 'failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    response_code INTEGER,
    error TEXT,
    duration_ms INTEGER,
    next_attempt_at INTEGER NOT NULL DEFAULT (unixepoch()),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    finished_at INTEGER
);

CREATE INDEX webhook_deliveries_hook ON webhook_deliveries (webhook_id, id);
CREATE INDEX webhook_deliveries_due ON webhook_deliveries (next_attempt_at) WHERE status = 'pending';

CREATE TABLE webhook_cursor (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_event INTEGER NOT NULL
);
