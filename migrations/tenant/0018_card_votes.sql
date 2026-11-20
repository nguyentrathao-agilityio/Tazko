ALTER TABLE projects ADD COLUMN voting INTEGER NOT NULL DEFAULT 1 CHECK (voting IN (0, 1));

CREATE TABLE card_votes (
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    PRIMARY KEY (card_id, user_id)
) WITHOUT ROWID;

CREATE INDEX card_votes_user ON card_votes (user_id);
