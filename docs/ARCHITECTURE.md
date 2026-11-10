# Lexiva — Architecture decisions

1. **Single native package:** Begin as a Cargo binary with internal modules. Add a reusable crate only after a real consumer emerges. Use eframe/egui, not a web wrapper.
2. **UI boundaries:** Screen/navigation, source manager, language filters, typography and reader consume a format-neutral search API, not files directly.
3. **Domain model:** Distinguish headword and definition language and allow unknowns. Keep ordered senses, POS, examples, forms, pronunciations, etymologies, references and original source metadata.
4. **Adapters:** DICT, Wiktextract, StarDict and TEI readers independently validate untrusted input, size and offsets. Reject unsupported variants without partial index publication.
5. **Storage:** Source IDs and provenance plus SQLite FTS5 indexes. Indexes derived/disposable; state and source manifests are valuable. Use XDG config/data/cache directories. The first shell may use eframe native persistence for theme only; move to deliberate XDG-owned state before release.
6. **Imports:** Streaming, cancelable background work with bounded memory, progress/errors, transactional staging and atomic publication, retaining the previous usable index if an import fails.
7. **Source ownership:** Installed package files are read-only, never rewritten. Changes detected by fingerprint/rescan and safely reindexed. No dictd daemon mandatory.
8. **Search:** Exact then prefix, normalized accents/case and bounded fuzzy suggestions, with explicit language/source filtering and deterministic ordering. No silent merging across sources.
9. **Security:** No scripts or unsafe markup executed; mitigate XML entities, compression bombs, malformed offsets and path traversal. Fail with actionable errors.
10. **Appearance:** Immediate System/Light/Dark picker default System, with actual portal/Hyprland integration separately verified. Font fallback and keyboard accessibility required. Identity accents: #9C424B light / #EA979B dark from OKLCH hue 16.
11. **Validation:** Automated fmt/check/clippy/test on CI; actual target GUI QA cannot be replaced by a green compilation badge. Measure latency on optimized builds with real sizes.

Revisit decisions only when dependencies or actual requirements make a change necessary; do not scaffold unused crates or features.

## Implemented foundation (schema v1)

`src/model.rs` contains source-aware lexical records preserving ordered nested senses, examples, languages and unrecognized structured metadata; `src/storage.rs` provides a bundled SQLite catalog, transactional finite-batch replacement and source-scoped FTS5 prefix lookup with update/delete triggers. See [STORAGE.md](STORAGE.md) for the schema contract and known limits; [DEPENDENCIES.md](DEPENDENCIES.md) records crate/data-license decisions. The backend is not connected to UI; large staged streaming imports, migrations and crash recovery are not yet implemented.
