-- Version 1. The catalog is derived data; source files remain read-only.
BEGIN IMMEDIATE;
CREATE TABLE dictionary_sources (
    id TEXT PRIMARY KEY NOT NULL,
    payload TEXT NOT NULL
);
CREATE TABLE lexical_entries (
    id INTEGER PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL REFERENCES dictionary_sources(id) ON DELETE CASCADE,
    source_entry_id TEXT NOT NULL,
    lemma TEXT NOT NULL,
    headword_language TEXT,
    definition_language TEXT,
    part_of_speech TEXT,
    payload TEXT NOT NULL,
    UNIQUE (source_id, source_entry_id)
);
CREATE INDEX entries_by_source_and_lemma
    ON lexical_entries (source_id, lemma);
CREATE VIRTUAL TABLE lexical_fts USING fts5(
    lemma,
    content='lexical_entries',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER entries_insert AFTER INSERT ON lexical_entries BEGIN
    INSERT INTO lexical_fts(rowid, lemma) VALUES (new.id, new.lemma);
END;
CREATE TRIGGER entries_delete AFTER DELETE ON lexical_entries BEGIN
    INSERT INTO lexical_fts(lexical_fts, rowid, lemma)
    VALUES ('delete', old.id, old.lemma);
END;
CREATE TRIGGER entries_update AFTER UPDATE ON lexical_entries BEGIN
    INSERT INTO lexical_fts(lexical_fts, rowid, lemma)
    VALUES ('delete', old.id, old.lemma);
    INSERT INTO lexical_fts(rowid, lemma) VALUES (new.id, new.lemma);
END;
PRAGMA user_version=1;
COMMIT;
