ALTER TABLE comments ADD COLUMN author_name TEXT;

CREATE TABLE imports (
    id INTEGER PRIMARY KEY,
    board_id INTEGER REFERENCES boards (id) ON DELETE SET NULL,
    file_name TEXT NOT NULL,
    format TEXT NOT NULL,
    counts TEXT NOT NULL,
    people TEXT NOT NULL,
    created_by INTEGER REFERENCES people (id) ON DELETE SET NULL,
    finished_at INTEGER NOT NULL DEFAULT (unixepoch())
);
