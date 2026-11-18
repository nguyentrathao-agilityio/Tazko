ALTER TABLE cards ADD COLUMN description TEXT NOT NULL DEFAULT '';
ALTER TABLE cards ADD COLUMN start_at INTEGER;
ALTER TABLE cards ADD COLUMN due_at INTEGER;
ALTER TABLE cards ADD COLUMN done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1));
ALTER TABLE cards ADD COLUMN cover TEXT;

CREATE TABLE labels (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    board_id INTEGER NOT NULL REFERENCES boards (id) ON DELETE CASCADE,
    name TEXT NOT NULL DEFAULT '',
    color INTEGER NOT NULL CHECK (color BETWEEN 0 AND 7),
    position INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX labels_board ON labels (board_id, position);

CREATE TABLE card_labels (
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    label_id INTEGER NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, label_id)
) WITHOUT ROWID;

CREATE TABLE card_members (
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES people (id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, user_id)
) WITHOUT ROWID;

CREATE INDEX card_members_user ON card_members (user_id);

CREATE TABLE checklists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    position TEXT NOT NULL
);

CREATE INDEX checklists_card ON checklists (card_id, position);

CREATE TABLE checklist_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    checklist_id INTEGER NOT NULL REFERENCES checklists (id) ON DELETE CASCADE,
    text TEXT NOT NULL,
    done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
    position TEXT NOT NULL
);

CREATE INDEX checklist_items_list ON checklist_items (checklist_id, position);

CREATE TABLE comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_id INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    user_id INTEGER REFERENCES people (id) ON DELETE SET NULL,
    body TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    edited_at INTEGER
);

CREATE INDEX comments_card ON comments (card_id, id);

INSERT INTO labels (board_id, name, color, position)
SELECT b.id, d.name, d.color, d.color
FROM boards b
CROSS JOIN (
    SELECT 'Frontend' AS name, 0 AS color UNION ALL SELECT 'Backend', 1 UNION ALL SELECT 'Design', 2
    UNION ALL SELECT 'Bug', 3 UNION ALL SELECT 'Research', 4 UNION ALL SELECT 'Blocked', 5
    UNION ALL SELECT 'Content', 6 UNION ALL SELECT 'Urgent', 7
) d;
