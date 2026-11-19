CREATE TABLE admin_templates (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    background TEXT NOT NULL,
    data TEXT NOT NULL,
    created_by INTEGER REFERENCES users (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
