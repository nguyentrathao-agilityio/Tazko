CREATE TABLE boards (
    id INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    background TEXT NOT NULL DEFAULT 'dusk',
    restricted INTEGER NOT NULL DEFAULT 0 CHECK (restricted IN (0, 1)),
    created_by INTEGER REFERENCES people (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    closed_at INTEGER
);

CREATE INDEX boards_project ON boards (project_id);

CREATE TABLE board_access (
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    PRIMARY KEY (board_id, user_id)
) WITHOUT ROWID;

CREATE TABLE board_stars (
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (user_id, board_id)
) WITHOUT ROWID;

CREATE TABLE lists (
    id INTEGER PRIMARY KEY,
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    position TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    archived_at INTEGER
);

CREATE INDEX lists_board ON lists (board_id, position);
