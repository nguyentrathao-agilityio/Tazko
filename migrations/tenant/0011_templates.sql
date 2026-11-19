CREATE TABLE board_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    background TEXT NOT NULL,
    data TEXT NOT NULL,
    created_by INTEGER REFERENCES people (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

ALTER TABLE cards ADD COLUMN is_template INTEGER NOT NULL DEFAULT 0 CHECK (is_template IN (0, 1));

CREATE INDEX cards_templates ON cards (board_id) WHERE is_template = 1;
