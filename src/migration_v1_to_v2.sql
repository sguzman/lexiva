-- Upgrade a committed v1 catalog without rewriting dictionary records.
-- If a statement fails, SQLite rolls this transaction back on connection close.
BEGIN IMMEDIATE;
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
PRAGMA user_version=2;
COMMIT;
