-- Version 3. Live entries remain separate from durable incomplete import batches.
-- Original package-managed dictionary files are always read-only.
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
-- Each source can have one pending staging session. A crash leaves only these
-- disposable rows; live entries stay unchanged until an explicit publish.
CREATE TABLE staging_imports (
    source_id TEXT PRIMARY KEY NOT NULL REFERENCES dictionary_sources(id) ON DELETE CASCADE
);
CREATE TABLE staging_entries (
    source_id TEXT NOT NULL REFERENCES staging_imports(source_id) ON DELETE CASCADE,
    source_entry_id TEXT NOT NULL,
    lemma TEXT NOT NULL,
    headword_language TEXT,
    definition_language TEXT,
    part_of_speech TEXT,
    payload TEXT NOT NULL,
    PRIMARY KEY (source_id, source_entry_id)
) WITHOUT ROWID;
-- Source visibility and priority are stored separately from immutable
-- dictionary metadata. Import/reindex must never reset user preferences.
CREATE TABLE source_preferences (
    source_id TEXT PRIMARY KEY NOT NULL REFERENCES dictionary_sources(id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    priority INTEGER NOT NULL
);
CREATE TRIGGER source_preferences_insert AFTER INSERT ON dictionary_sources BEGIN
    INSERT INTO source_preferences (source_id, enabled, priority)
    VALUES (new.id, 1, (SELECT coalesce(max(priority), -1) + 1 FROM source_preferences));
END;
PRAGMA user_version=3;
COMMIT;
