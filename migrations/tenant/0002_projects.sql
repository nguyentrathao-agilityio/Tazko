ALTER TABLE people ADD COLUMN email TEXT NOT NULL DEFAULT '';

CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT 'c0',
    background TEXT NOT NULL DEFAULT 'dusk',
    members_create_boards INTEGER NOT NULL DEFAULT 1 CHECK (members_create_boards IN (0, 1)),
    members_invite INTEGER NOT NULL DEFAULT 0 CHECK (members_invite IN (0, 1)),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    archived_at INTEGER,
    deleted_at INTEGER
);

CREATE TABLE project_members (
    project_id INTEGER NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('owner', 'member', 'observer')),
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    last_active INTEGER,
    PRIMARY KEY (project_id, user_id)
) WITHOUT ROWID;

CREATE INDEX project_members_user ON project_members (user_id);

ALTER TABLE events ADD COLUMN project_id INTEGER;
ALTER TABLE events ADD COLUMN card_id INTEGER;

CREATE INDEX events_project ON events (project_id, id);
