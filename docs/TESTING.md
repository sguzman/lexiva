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
