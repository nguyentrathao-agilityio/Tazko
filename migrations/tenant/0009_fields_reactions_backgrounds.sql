CREATE TABLE custom_field_defs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('text', 'number', 'date', 'select', 'check')),
    options TEXT NOT NULL DEFAULT '[]',
    show_on_card INTEGER NOT NULL DEFAULT 0 CHECK (show_on_card IN (0, 1)),
    position INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX custom_field_defs_board ON custom_field_defs (board_id, position, id);

CREATE TABLE custom_field_values (
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    field_id INTEGER NOT NULL REFERENCES custom_field_defs (id) ON DELETE CASCADE,
    value TEXT NOT NULL,
    PRIMARY KEY (card_id, field_id)
) WITHOUT ROWID;

CREATE INDEX custom_field_values_field ON custom_field_values (field_id);

CREATE TABLE comment_reactions (
    comment_id INTEGER NOT NULL REFERENCES comments (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (comment_id, user_id, emoji)
) WITHOUT ROWID;

CREATE TABLE board_images (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    hash TEXT NOT NULL CHECK (length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*'),
    mime TEXT NOT NULL,
    size INTEGER NOT NULL,
    name TEXT NOT NULL DEFAULT '',
    created_by INTEGER REFERENCES people (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX board_images_board ON board_images (board_id);
CREATE INDEX board_images_hash ON board_images (hash);
