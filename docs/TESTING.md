# Lexiva — Engineering test plan

## On Rust changes

- `cargo fmt --all -- --check`
- `cargo check --locked --all-targets`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo test --locked --all-targets`
- On the owner's PC, use the established mbx coordination around cargo work. CI outcomes do not prove local GUI success.

## Unit/integration targets

Unicode, accent/case normalization, language uncertainty, ordered senses, homographs, source-specific metadata, source selection, deterministic search ordering, malformed JSONL, decompression/offset boundaries, TEI XML security, duplicate imports, cancellation, migrations and last-good-index preservation.

## Native UI acceptance

On EndeavourOS/Hyprland: clean launch, search by keyboard, select/copy, source manager, session persistence, live System/Light/Dark on external day/night switch, fallback fonts, long definitions, keyboard focus and error reporting. Compare imported real DICT, Wiktextract, StarDict and TEI entries with their respective input data.

## Quantified targets, not achieved claims

Existing-index startup under 3 seconds; search p95 <150ms at ~100k headwords and <250ms at ~1m, with hardware and cold/warm conditions specified. Large imports bounded in memory with live cancellation. No needless idle polling.

## Release discipline

All applicable automated and on-device tests pass, packaging and installation path are verified, source/font license notices recorded and exact candidate commit is accepted by owner. Release/tag is an independently authorized action.

## CI checkpoint: typed lexical storage

[Run 42746299152](https://github.com/sguzman/lexiva/actions/runs/42746299152) succeeded on Rust 1.95.0, with 11 passing tests across theme persistence, model-language validation/round-trip, source provenance, source-scoped search, SQLite FTS updates, transactional replacement rollback, query escaping and schema-forward incompatibility. These are synthetic tests; no real FreeDict, Wiktionary, StarDict or TEI files have yet been tested, and source import/GUI performance remain unverified.

## Schema v2 and staged-import checkpoint

[Run 42751965002](https://github.com/sguzman/lexiva/actions/runs/42751965002) verified formatting, Rust compilation, strict Clippy and **24 tests passing**. The tests cover transactional staged publishing, cancellation, empty and duplicate imports, on-disk abandoned staging across reopen, deliberate stage discard, v1→v2 migration, FTS rebuild, old-data preservation, invalid database file refusal and a 4 MiB input-record guard. These use synthetic fixtures; **real dictionary file ingestion and native GUI runtime validation remain pending**.
