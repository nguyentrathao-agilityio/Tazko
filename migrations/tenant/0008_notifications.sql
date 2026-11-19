CREATE TABLE notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('mention', 'assigned', 'due', 'card_added')),
    actor_id INTEGER REFERENCES people (id) ON DELETE SET NULL,
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    comment_id INTEGER REFERENCES comments (id) ON DELETE SET NULL,
    due_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    read_at INTEGER
);

CREATE INDEX notifications_user ON notifications (user_id, id);
CREATE INDEX notifications_unread ON notifications (user_id) WHERE read_at IS NULL;
CREATE UNIQUE INDEX notifications_due_once ON notifications (user_id, card_id, due_at) WHERE kind = 'due';

CREATE TABLE board_watchers (
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    PRIMARY KEY (board_id, user_id)
) WITHOUT ROWID;
