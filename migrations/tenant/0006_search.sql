ALTER TABLE cards ADD COLUMN description_text TEXT NOT NULL DEFAULT '';

CREATE VIRTUAL TABLE cards_fts USING fts5 (title, body, tokenize = 'unicode61 remove_diacritics 2');

INSERT INTO cards_fts (rowid, title, body) SELECT id, replace(replace(title, 'đ', 'd'), 'Đ', 'D'), replace(replace(description_text, 'đ', 'd'), 'Đ', 'D') FROM cards;

CREATE TRIGGER cards_fts_insert AFTER INSERT ON cards BEGIN
    INSERT INTO cards_fts (rowid, title, body) VALUES (
        new.id,
        replace(replace(new.title, 'đ', 'd'), 'Đ', 'D'),
        replace(replace(new.description_text, 'đ', 'd'), 'Đ', 'D')
    );
END;

CREATE TRIGGER cards_fts_update AFTER UPDATE OF title, description_text ON cards BEGIN
    DELETE FROM cards_fts WHERE rowid = old.id;
    INSERT INTO cards_fts (rowid, title, body) VALUES (
        new.id,
        replace(replace(new.title, 'đ', 'd'), 'Đ', 'D'),
        replace(replace(new.description_text, 'đ', 'd'), 'Đ', 'D')
    );
END;

CREATE TRIGGER cards_fts_delete AFTER DELETE ON cards BEGIN
    DELETE FROM cards_fts WHERE rowid = old.id;
END;
