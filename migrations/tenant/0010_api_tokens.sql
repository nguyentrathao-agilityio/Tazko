CREATE TABLE api_tokens (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    id_hash TEXT NOT NULL UNIQUE,
    scope TEXT NOT NULL CHECK (scope IN ('read', 'write', 'manage')),
    projects TEXT NOT NULL,
    created_by INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    expires_at INTEGER,
    last_used_at INTEGER,
    revoked_at INTEGER
);
