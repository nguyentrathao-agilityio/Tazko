CREATE TABLE cards (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    list_id INTEGER NOT NULL REFERENCES lists (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    position TEXT NOT NULL,
    created_by INTEGER REFERENCES people (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    archived_at INTEGER
);

CREATE INDEX cards_list ON cards (list_id, position);
CREATE INDEX cards_board ON cards (board_id);
