# Lexiva — Storage schema v2 and import recovery

The implemented local data foundation is in `src/model.rs`, `src/storage.rs`, `src/schema.sql`, `src/storage/import.rs`, and `src/migration_v1_to_v2.sql`. The original v1 schema is preserved in `src/schema_v1.sql` for deterministic migration tests.

## Ownership and schema

- Original package-managed dictionary files are **read-only external inputs**. The SQLite catalog is derived data, not the authoritative source.
- `dictionary_sources` stores a stable ID and JSON record for title, origin, word/definition languages, license, attribution and format metadata.
- `lexical_entries` holds source-scoped entry IDs (allowing homographs), headwords, indexed language/POS fields and JSON preserving nested/ordered senses, examples and unknown structured source metadata.
- `lexical_fts` is an FTS5 external-content index on headwords with `unicode61 remove_diacritics 2` and triggers that keep live inserts, deletes and updates in sync.
- Schema v2 adds `staging_imports` and `staging_entries`. Pending entries are invisible to live searches until a successful atomic publish.

## Streaming and publication

- `DictionaryStore::import_stream` consumes an iterator of already parsed lexical records. Callers supply a cancellation callback and can receive progress after each batch.
- It holds at most **128 entries** in its application-side pending batch; records must serialize to at most **4 MiB** each. The size is checked before adding an entry to the batch using a streaming byte counter, without making a second giant JSON buffer.
- Each stage batch commits separately. Live entries are untouched while the source is staged. `ImportSession::publish` rejects empty stages, verifies staged count and performs live delete/insert + staging cleanup in **one SQLite transaction**.
- Validation failures, duplicate IDs, cancellations and parser errors leave the previously published source intact. Normal `ImportSession` drop cleans up staging.
- A process crash can leave *disposable staging rows* in SQLite; they are **not automatically published or deleted on startup**. An explicit `discard_staged_import` is available after the caller ensures no other process is staging that source. Separate instances must not race a cleanup.

**Boundary:** This component receives already parsed records. Individual format adapters must later enforce bounds during parsing/decompression too. Staging uses disk space proportional to imported data; future import UX must report available disk capacity and show cancellation/progress in the GUI.

## Migration, integrity and recovery

- Versioned `PRAGMA user_version` is 2. Existing schema-v1 databases are migrated transactionally, preserving source records and live entries. Unknown/future versions are refused.
- A full `PRAGMA quick_check` is run before altering v1 data; explicit `check_catalog_integrity` is available for later checks. Because this is O(database size), **normal v2 startup does not run it**.
- `check_fts_index` verifies FTS5 index consistency against live external-content rows. `rebuild_fts_index` reconstructs the *derived index* from preserved lexical records.
- A database that cannot be opened or fails validation is not automatically erased or overwritten; higher-level user-controlled recovery and clear diagnostics are required before release.
- Tests cover on-disk abandoned staging, explicit discard, SQLite v1→v2 migration, metadata retention, FTS corruption/rebuild and invalid-file preservation.

## Unfinished integration and release checks

The GUI has no dictionary source/import wiring yet. Real FreeDict/Wiktionary files, multi-process instance policy, import disk-space UX, adapter parsing limits, large-scale performance and Hyprland runtime QA remain unverified.

**Evidence:** [GitHub Actions run 42751965002](https://github.com/sguzman/lexiva/actions/runs/42751965002) passed 24 unit/integration-style tests, formatting, compilation and Clippy.
