# Lexiva — Storage schema v1 and current limits

The first database contract is implemented in `src/storage.rs`, `src/schema.sql` and the format-neutral `src/model.rs`.

## Ownership and identity

- The user's original installed/source dictionary files are **not modified**. Lexiva stores only derived catalog/index data.
- `dictionary_sources` has a stable `id` and a JSON payload preserving title, origin, edition/languages, license, attribution and adapter-specific metadata.
- `lexical_entries` stores a SQLite row ID, source ID, source-relative entry ID, headword, optional indexed languages/part of speech and JSON lexical payload. Unique source+entry ID allows homographs with separate identities.
- The typed record retains senses and child-senses in order, glosses, usage labels, examples, translations, IPA/audio links, forms, etymologies and extra JSON metadata.
- An empty/unknown language is represented with `None`, not silently labeled English.
- The payload is **structured** preservation, not a byte-identical backup of source files. The source files are the source of truth.

## Index consistency

- The `lexical_fts` FTS5 external-content index uses Unicode61 with diacritics removal and is updated by insert/delete/update triggers.
- `replace_source_entries` runs inside a single SQLite transaction: delete old rows, insert validated replacement rows and commit. A failure rolls back to the old state. Reindexing a source does not rewrite another source.
- Initial `search_prefix` is a bounded, escaped, source-scoped FTS lookup. Full ranking, independent language selectors and per-sense language filtering are still M05/M06 work.
- v1 is stored in SQLite `PRAGMA user_version`. A database with a newer version is rejected rather than implicitly rewritten.

## Explicit unfinished work

- Source-level replacements currently accept a slice of entries, so a complete huge corpus cannot be imported memory-safely through this API. M02 LEX-023 adds streaming staging/atomic publication.
- Automatic repair/migration, file location under XDG paths, source-file rescans and live GUI integration are not yet implemented.
- A release must verify real imported data, Unicode lookup semantics, filesystem interruption behavior and optimized performance.
