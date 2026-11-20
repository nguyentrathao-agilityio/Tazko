CREATE TABLE card_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_card INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    to_card INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('relates', 'blocks', 'duplicates')),
    created_by INTEGER REFERENCES people (id) ON DELETE SET NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    CHECK (from_card != to_card)
);

CREATE UNIQUE INDEX card_links_pair ON card_links (min(from_card, to_card), max(from_card, to_card));
CREATE INDEX card_links_from ON card_links (from_card);
CREATE INDEX card_links_to ON card_links (to_card);
