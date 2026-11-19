DROP TABLE personal_tokens;

CREATE TABLE personal_tokens (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    id_hash TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL CHECK (scope IN ('read', 'write', 'manage')),
    projects TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at INTEGER,
    last_used_at INTEGER,
    revoked_at INTEGER
);

CREATE INDEX personal_tokens_user ON personal_tokens (user_id);
