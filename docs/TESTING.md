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

## DICT reader and source discovery checkpoint

[Run 42832705560](https://github.com/sguzman/lexiva/actions/runs/42832705560) passed **30 tests / 0 failures**, fmt/check/strict Clippy, including six new synthetic GNU DICT cases: offset/length alphabet and overflow, deterministic directory discovery, unicode headwords plus homographs, malformed and oversized indexes, and end-to-end source-scoped SQLite import of plain `.dict`. The tests **do not** prove compressed dictzip `.dict.dz` support or that an actual FreeDict package from the Arch repositories imports correctly; those remain M03 work.

## Dictzip version-1 verification

[Run 42835023624](https://github.com/sguzman/lexiva/actions/runs/42835023624) passed fmt, check, strict Clippy and **33 tests, zero failures**, including three new dictzip checks (chunk-crossing lookups, malformed RA/header rejection, and a fully staged compressed DICT → SQLite/FTS5 import). `tests/fixtures/synthetic.dict.dz` is original synthetic test data, valid as a gzip stream; no FreeDict package contents were used. Actual distributed FreeDict archives, malformed gzip footer CRC, and GUI QA require later independent verification.

## Real FreeDict metadata and package acceptance (M03 complete)

[CI run 42838302466](https://github.com/sguzman/lexiva/actions/runs/42838302466) passed Rust 1.95.0 formatting, check, strict Clippy, **36 normal tests / 0 failures**, and **1 explicit real-package test / 0 failures**. Ubuntu 24.04 packages `dict-freedict-spa-eng` and `dict-freedict-eng-spa` were installed in CI, with source files read under `/usr/share/dictd` and not bundled into Lexiva. Every article of the 4,507-entry Spanish→English (193,471 uncompressed bytes) and 5,912-entry English→Spanish (204,192 bytes) dictionaries was compared byte-for-byte with separate `gzip -dc` output, then imported into SQLite and retrieved via the source-scoped index. Additional metadata tests verify conservative language inference, administrative text retention, POS extraction and unknown-name handling. This is real Linux FreeDict packaging evidence, **not** native Arch/Hyprland GUI acceptance or proof of full TEI semantic parsing.

## Wiktextract streaming/edition check — M04 partial

[Run 42845015317](https://github.com/sguzman/lexiva/actions/runs/42845015317) passed fmt/check/strict Clippy, **45 regular tests (0 failures)** and **1 installed FreeDict integration test**. Nine new synthetic Wiktextract tests cover gzip/plain equivalence, English-Wiktionary Spanish words with English definitions, Spanish-Wiktionary Spanish words with Spanish definitions, ordered glosses, example translations, IPA/forms/raw unknown metadata, 400-record staged import with source filtering, invalid edition/source combinations, malformed-input rollback and 2 MiB line limits. Authentic Wiktextract dump records, large-dump memory/disk usage and GUI QA are still outstanding.
