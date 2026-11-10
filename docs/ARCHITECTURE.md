# Lexiva — Architecture decisions

1. **Single native package:** Begin as a Cargo binary with internal modules. Add a reusable crate only after a real consumer emerges. Use eframe/egui, not a web wrapper.
2. **UI boundaries:** Screen/navigation, source manager, language filters, typography and reader consume a format-neutral search API, not files directly.
3. **Domain model:** Distinguish headword and definition language and allow unknowns. Keep ordered senses, POS, examples, forms, pronunciations, etymologies, references and original source metadata.
4. **Adapters:** DICT, Wiktextract, StarDict and TEI readers independently validate untrusted input, size and offsets. Reject unsupported variants without partial index publication.
5. **Storage:** Source IDs and provenance plus SQLite FTS5 indexes. Indexes derived/disposable; state and source manifests are valuable. Use XDG config/data/cache directories. Eframe native persistence now holds small appearance, zoom, selected-source/language and accessibility preferences plus root-window state, while SQLite under XDG data holds derived dictionary catalogs; review persistence paths during packaging.
6. **Imports:** Streaming, cancelable background work with bounded memory, progress/errors, transactional staging and atomic publication, retaining the previous usable index if an import fails.
7. **Source ownership:** Installed package files are read-only, never rewritten. Changes detected by fingerprint/rescan and safely reindexed. No dictd daemon mandatory.
8. **Search:** Exact then prefix, normalized accents/case and bounded fuzzy suggestions, with explicit language/source filtering and deterministic ordering. No silent merging across sources.
9. **Security:** No scripts or unsafe markup executed; mitigate XML entities, compression bombs, malformed offsets and path traversal. Fail with actionable errors.
10. **Appearance:** Immediate System/Light/Dark picker default System, with actual portal/Hyprland integration separately verified. Font fallback and keyboard accessibility required. Identity accents: #9C424B light / #EA979B dark from OKLCH hue 16.
11. **Validation:** Automated fmt/check/clippy/test on CI; actual target GUI QA cannot be replaced by a green compilation badge. Measure latency on optimized builds with real sizes.

Revisit decisions only when dependencies or actual requirements make a change necessary; do not scaffold unused crates or features.

## Implemented foundation (schema v2)

`src/model.rs` preserves source-aware lexical structure. `src/storage.rs` provides source-scoped SQLite/FTS5 index access; `src/storage/import.rs` implements bounded staged imports and single-transaction publication, and `src/migration_v1_to_v2.sql` handles existing v1 catalogs. Abandoned stage rows require explicit discard; a live full-text index can be rebuilt non-destructively. See [STORAGE.md](STORAGE.md) for validated behavior and limits, and [DEPENDENCIES.md](DEPENDENCIES.md) for crate/data-license decisions. FreeDict and scoped Wiktextract adapters are verified; native ranked search/reader integration is now compiled and tested, but live Hyprland GUI interaction and the M06 source-indexing controls remain pending.

## Wiktextract adapter

The read-only `src/wiktextract.rs` adapter streams original JSONL/gzip records into bounded structured `LexicalEntry` values for the established staged SQLite publisher. It stores original JSON objects as structured metadata for forward-compatibility and makes definition language an explicit choice derived from **Wiktionary edition**, independent of entry `lang_code`. Authentic per-word data passes real-record parsing, and a 150k generated corpus passes staged import/cancel/reimport and memory/disk measurements. Full multi-gigabyte raw dump compatibility, disk-space preflight, and native GUI interaction remain pending. See [WIKTEXTRACT.md](WIKTEXTRACT.md).

## Native indexed search and reader

`src/storage/search.rs` exposes source-attributed `SearchHit` records, stable source-entry identities, exact/prefix ranking and optional independent headword/definition language constraints. `src/search_worker.rs` hosts the SQLite connection on a dedicated worker thread, coalesces pending keystrokes, returns serial-tagged results and requests an egui repaint on completion; `src/app.rs` ignores stale responses. Default `SearchScope::Unselected` never searches everything; the user must choose a dictionary or explicitly opt into `All`. `src/reader.rs` displays typed lexical content as plain text, with attribution and copied definition output. Newer worker requests can interrupt older running SQLite statements, and stale replies/errors are suppressed. A bounded FTS5 candidate set powers conservative spelling suggestions; UI shortcuts and session-only history are implemented and tested at component level. First-run GUI source indexing and actual Hyprland keyboard/appearance QA are still incomplete.

## Explicit source manager (M06 partial)

`src/source_manager.rs` separates read-only DICT directory discovery and explicit per-source import/reindex from the egui paint loop via a background worker. It reuses the transactional staged `DictionaryStore::import_stream` and the edition-aware Wiktextract adapter; progress and cancellation travel over channels and a shared atomic flag. Deterministic source IDs preserve the path and edition/filter distinction; local-index removal is an SQLite transaction and does not delete source files. The search worker and import worker use separate SQLite connections with a bounded busy timeout. `src/app.rs` exposes two optional language inputs that are passed separately through `src/search_worker.rs` to `SearchFilters`. Persistent source enable/priority, file-change fingerprinting and native Hyprland acceptance are outstanding.

## Native system appearance (M07 in progress)

On Linux with Darkman, the GUI subscribes to the existing `$XDG_RUNTIME_DIR/darkman/control.sock` `watch` stream. The initial state and each subsequent light/dark notification trigger egui repaint, but are only applied while Appearance is set to System. An explicit Light or Dark choice is never overwritten. On machines without Darkman, normal eframe System behavior remains; native Hyprland verification and generic XDG portal integration remain outstanding. The watcher performs no idle polling and adds no crates or font redistribution.

## Accessibility and window-state behavior (M07 active)

The app stores a bounded reader-size preference and a separate interface zoom, retaining the framework's Ctrl+Plus/Minus/0 shortcuts. An Accessibility menu enables reduced-motion styles and high-contrast secondary and selection text while preserving Lexiva's accent colors. All appearance variants update both light and dark styles rather than only the active theme. Lookup history, arrow selection and Ctrl+Enter copying are gated by the focused control, preventing other text inputs from being hijacked. Eframe native root-window persistence and system font fallback are enabled explicitly; Wayland controls absolute positions. Unit-level tests verify focus gating and contrast ratios, but compositor behavior and graphical acceptance are still outstanding.

## Source preference persistence and file health (M06 complete to CI)

Schema v3 stores `source_preferences` separately from the source JSON payload, giving every indexed dictionary an enabled flag and stable ordered priority. An insert trigger provides defaults; existing v1/v2 catalogs migrate transactionally. Ranked search joins preferences **before** ranking, and the manager worker mutates enable/priority with SQLite transactions. File health is checked only on startup or an explicit user action by a bounded metadata+head/tail fingerprint of the original read-only DICT pair or JSONL file. Missing inputs do not remove published indexes; explicit reindex stages a replacement and only records a new signature after publication. Tests cover interrupted reindex metadata integrity. Hyprland runtime QA and comprehensive packaging remain pending.
