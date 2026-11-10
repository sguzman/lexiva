# Lexiva — Storage schema v3, preferences and import recovery

The implemented local data foundation is in `src/model.rs`, `src/storage.rs`, `src/schema.sql`, `src/storage/import.rs`, `src/migration_v1_to_v2.sql`, and `src/migration_v2_to_v3.sql`. The original v1 schema is preserved in `src/schema_v1.sql` for deterministic migration tests.

## Ownership and schema

- Original package-managed dictionary files are **read-only external inputs**. The SQLite catalog is derived data, not the authoritative source.
- `dictionary_sources` stores a stable ID and JSON record for title, origin, word/definition languages, license, attribution and format metadata.
- `lexical_entries` holds source-scoped entry IDs (allowing homographs), headwords, indexed language/POS fields and JSON preserving nested/ordered senses, examples and unknown structured source metadata.
- `lexical_fts` is an FTS5 external-content index on headwords with `unicode61 remove_diacritics 2` and triggers that keep live inserts, deletes and updates in sync.
- Schema v2 adds `staging_imports` and `staging_entries`. Pending entries are invisible to live searches until a successful atomic publish.
- Schema v3 adds `source_preferences` with stable source IDs, persisted enabled flags and explicit priority ordering, plus an after-insert trigger that sets safe defaults for new sources. Existing source metadata updates do **not** overwrite user preferences; source removal cascades to preference records.

## Streaming and publication

- `DictionaryStore::import_stream` consumes an iterator of already parsed lexical records. Callers supply a cancellation callback and can receive progress after each batch.
- It holds at most **128 entries** in its application-side pending batch; records must serialize to at most **4 MiB** each. The size is checked before adding an entry to the batch using a streaming byte counter, without making a second giant JSON buffer.
- Each stage batch commits separately. Live entries are untouched while the source is staged. `ImportSession::publish` rejects empty stages, verifies staged count and performs live delete/insert + staging cleanup in **one SQLite transaction**.
- Validation failures, duplicate IDs, cancellations and parser errors leave the previously published source intact. Normal `ImportSession` drop cleans up staging.
- A process crash can leave *disposable staging rows* in SQLite; they are **not automatically published or deleted on startup**. An explicit `discard_staged_import` is available after the caller ensures no other process is staging that source. Separate instances must not race a cleanup.

**Boundary:** This component receives already parsed records. Individual format adapters must later enforce bounds during parsing/decompression too. Staging uses disk space proportional to imported data; future import UX must report available disk capacity and show cancellation/progress in the GUI.

## Migration, integrity and recovery

- Versioned `PRAGMA user_version` is **3**. Existing v1 databases migrate through v2 and v3, while existing v2 databases migrate directly to v3. The new migration preserves source IDs, published lexical records, staging and index content. Unknown/future versions are refused.
- A full `PRAGMA quick_check` is run before altering v1 data; explicit `check_catalog_integrity` is available for later checks. Because this is O(database size), **normal v3 startup does not run it**.
- `check_fts_index` verifies FTS5 index consistency against live external-content rows. `rebuild_fts_index` reconstructs the *derived index* from preserved lexical records.
- A database that cannot be opened or fails validation is not automatically erased or overwritten; higher-level user-controlled recovery and clear diagnostics are required before release.
- Tests cover on-disk abandoned staging, explicit discard, SQLite v1→v2→v3 and v2→v3 migrations, metadata retention, enabled/priority persistence, FTS corruption/rebuild and invalid-file preservation.

## Unfinished integration and release checks

The GUI now has explicit DICT and Wiktextract local imports, search controls, persistent enabled/priority and source change checks. On-device Hyprland UI acceptance, multi-process instance policy, disk-space preflight/UX, end-to-end giant-dump performance and some broader adapter formats remain unverified.

**Evidence:** [GitHub Actions run 42751965002](https://github.com/sguzman/lexiva/actions/runs/42751965002) passed 24 unit/integration-style tests, formatting, compilation and Clippy.

**Latest evidence:** [CI 42880153881](https://github.com/sguzman/lexiva/actions/runs/42880153881): schema v3/migrations and persistent source settings passed unit tests plus the real FreeDict and Wiktextract regression suite.
