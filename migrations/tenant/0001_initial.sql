CREATE TABLE people (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL DEFAULT '',
    avatar TEXT
);

CREATE TABLE events (
    id INTEGER PRIMARY KEY,
    type TEXT NOT NULL,
    actor_id INTEGER,
    board_id INTEGER,
    payload TEXT NOT NULL DEFAULT '{}',
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX events_board ON events (board_id, id);
