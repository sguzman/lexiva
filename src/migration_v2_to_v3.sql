-- Preserve the v2 source identity, published entries, and pending staging.
BEGIN IMMEDIATE;
CREATE TABLE source_preferences (
    source_id TEXT PRIMARY KEY NOT NULL REFERENCES dictionary_sources(id) ON DELETE CASCADE,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    priority INTEGER NOT NULL
);
INSERT INTO source_preferences (source_id, enabled, priority)
SELECT id, 1, row_number() OVER (ORDER BY id) - 1
FROM dictionary_sources;
CREATE TRIGGER source_preferences_insert AFTER INSERT ON dictionary_sources BEGIN
    INSERT INTO source_preferences (source_id, enabled, priority)
    VALUES (new.id, 1, (SELECT coalesce(max(priority), -1) + 1 FROM source_preferences));
END;
PRAGMA user_version=3;
COMMIT;
