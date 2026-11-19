CREATE TABLE attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('file', 'link')),
    name TEXT NOT NULL,
    hash TEXT CHECK (hash IS NULL OR (length(hash) = 64 AND hash NOT GLOB '*[^0-9a-f]*')),
    size INTEGER NOT NULL DEFAULT 0,
    mime TEXT NOT NULL DEFAULT '',
    url TEXT,
    created_by INTEGER REFERENCES people (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    CHECK ((kind = 'file' AND hash IS NOT NULL AND url IS NULL) OR (kind = 'link' AND hash IS NULL AND url IS NOT NULL))
);

CREATE INDEX attachments_card ON attachments (card_id, id);
CREATE INDEX attachments_hash ON attachments (hash) WHERE hash IS NOT NULL;

ALTER TABLE cards ADD COLUMN cover_attachment INTEGER REFERENCES attachments (id) ON DELETE SET NULL;
