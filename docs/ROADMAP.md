# Lexiva — Verified engineering roadmap

Baseline: 2026-11-09. Full-release scope and stable effort weights agreed with owner; **weights are relative effort, not elapsed time**. See [inception](INCEPTION.md), [architecture](ARCHITECTURE.md), [formats](FORMATS.md), [testing](TESTING.md).

## Progress checkpoint

- Verified: **47 / 100 weighted points (47%)**. Previous checkpoint: 42%; this development round **+5 percentage points**: LEX-043 150k-record stress/reimport/cancel/disk/memory evidence (+3) and LEX-044 authentic English- and Spanish-edition per-word JSONL semantic checks (+2). M04 is verified complete to its defined acceptance scope; a full multi-gigabyte dump has NOT been benchmarked.
- Verified in [CI run 42855279348](https://github.com/sguzman/lexiva/actions/runs/42855279348): formatting, compilation, strict Clippy, **45 regular tests plus 4 targeted tests passed** (FreeDict, large synthetic corpus, two authentic Kaikki per-word records). The stress test generated 150,000 JSONL records, indexed 120,000 Spanish entries, cancelled/reimported atomically, verified FTS, and measured RSS and database growth. Full multi-gigabyte raw dump, native Hyprland GUI and p95 search remain unverified.
- Baseline commit: b0cb884657a22b3cf2ec91fa15f4765b94c3a605 on main before inception documentation.
- Environment limitation: tool execution container currently has no Rust toolchain; actual build requires GitHub CI and user-machine QA.
- Next: M05 user-facing lexical search and definition reader, followed by M06 explicit dictionary/source and independent headword/definition language selection. Native Hyprland startup smoke M01 LEX-011 remains pending; corpus-scale 24+ GiB benchmark should be revisited in M09 and acceptance.

## Milestones

| Phase | Weight | Verified | Status |
|---|---:|---:|---|
| M00 🧭 Inception & engineering contracts | 5 | 5 | verified complete |
| M01 🦀 Rust bootstrap | 6 | 5 | active |
| M02 🗃️ Lexical data & search foundation | 11 | 11 | verified complete |
| M03 📚 GNU DICT / Arch integration | 12 | 12 | verified complete |
| M04 🌍 Wiktionary integration | 14 | 14 | verified complete |
| M05 🔎 Search & reader | 13 | 0 | pending |
| M06 🧰 Dictionary & language management | 8 | 0 | pending |
| M07 🌗 Appearance and accessibility | 8 | 0 | pending |
| M08 🌟 StarDict & TEI | 10 | 0 | pending |
| M09 🛡️ Hardening/performance | 6 | 0 | pending |
| M10 📦 Documentation & distribution | 4 | 0 | pending |
| M11 ✅ Acceptance & release | 3 | 0 | pending |
| **Total** | **100** | **47** | **active** |

### M00 🧭 Inception & engineering contracts

Dependency: Repo reconnaissance; weight: 5 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-001 📝 | Inception purpose, target users, MVP/full-scope, non-goals | 1 | Decision and evidence published in appropriate inception/architecture documentation. | verified complete |
| LEX-002 🏗️ | Architecture, ownership, persistence and concurrency decisions | 1 | Decision and evidence published in appropriate inception/architecture documentation. | verified complete |
| LEX-003 🎨 | Deterministic OKLCH identity and typography/color contract | 1 | Decision and evidence published in appropriate inception/architecture documentation. | verified complete |
| LEX-004 ⚖️ | Risk and license review of all chosen dependencies and formats | 2 | Decision and evidence published in appropriate inception/architecture documentation. | verified complete |

### M01 🦀 Rust bootstrap

Dependency: M00; weight: 6 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-010 🔧 | Cargo 2024 package, MSRV and manifest | 2 | Actual Rust compile/tool/CI checks pass; GUI runtime must be tested separately. | verified complete |
| LEX-011 🧱 | Native eframe window and recoverable startup | 1 | Actual Rust compile/tool/CI checks pass; GUI runtime must be tested separately. | pending |
| LEX-012 🛠️ | fmt, Clippy, compilation and tests with mbx policy | 1 | Actual Rust compile/tool/CI checks pass; GUI runtime must be tested separately. | verified complete |
| LEX-013 🤖 | Linux CI and lockfile validation | 2 | Actual Rust compile/tool/CI checks pass; GUI runtime must be tested separately. | verified complete |

### M02 🗃️ Lexical data & search foundation

Dependency: M01; weight: 11 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-020 🧬 | Typed entries, sense structure, POS, language, IPA and etymology | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-021 🗄️ | Versioned SQLite storage and source-aware FTS5 indexes | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-022 🪪 | Source provenance and preservation of ordered/lossy metadata | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-023 🔄 | Transactional staged imports and atomic index publication | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-024 🩹 | Migration, corruption detection and safe cache recovery | 1 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |

### M03 📚 GNU DICT / Arch integration

Dependency: M02; weight: 12 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-030 📂 | Discover standard Arch dictionaries and user directories | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-031 📖 | Parse DICT index, article offsets, lengths and multiple records | 4 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-032 🗜️ | Random-access dictzip with error and size bounds | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-033 🏷️ | Language metadata, homographs and source formatting | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-034 🧪 | Real FreeDict/DICT package cross-check and edge fixtures | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |

### M04 🌍 Wiktionary integration

Dependency: M02; weight: 14 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-040 🌐 | Streaming JSONL and gzipped JSONL import | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-041 🗣️ | Map ordered senses, POS, examples, IPA, forms and relations | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-042 🇪🇸 | Separate headword/definition language for English/Spanish editions | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-043 💽 | Large import progress, cancellation, space and deduplication | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |
| LEX-044 🔬 | Real-record semantic checks and incomplete data cases | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | verified complete |

### M05 🔎 Search & reader

Dependency: M02 + real adapter; weight: 13 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-050 🔍 | Exact/prefix/Unicode/accent-aware search and typo suggestions | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-051 📑 | Result list with source, POS and stable ordering | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-052 📜 | Rich ordered definition reader, examples, IPA and navigation | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-053 ⌨️ | Keyboard-only search, selection, history and copy | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-054 🧵 | Cancelable background search and responsive error reporting | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |

### M06 🧰 Dictionary & language management

Dependency: M03–M05; weight: 8 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-060 🧭 | GUI source discovery, add/remove and status | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-061 🗺️ | Independent word and definition language selectors | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-062 🎚️ | Persistent source enabling and priority | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-063 🔃 | Rescan/reindex after package changes and missing files | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |

### M07 🌗 Appearance and accessibility

Dependency: M01 + M05; weight: 8 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-070 🌗 | Live System/Light/Dark selector and Hyprland detection | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-071 🖋️ | Fonts, fallbacks, scaling and readable definition typography | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-072 ♿ | Focus, contrast, zoom, keyboard and reduced-motion behavior | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-073 🧩 | Persistent UI configuration, window and theme | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |

### M08 🌟 StarDict & TEI

Dependency: M02–M06; weight: 10 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-080 ✨ | StarDict ifo/idx/dict/compressed data and synonyms | 4 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-081 📰 | Safe StarDict article decoding and markup rendering | 2 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-082 🏛️ | FreeDict TEI XML import, hierarchy and bounded XML | 3 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-083 🔌 | Per-variant compatibility matrix and fixtures | 1 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |

### M09 🛡️ Hardening/performance

Dependency: Real adapters + UI; weight: 6 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-090 🔐 | Adversarial file, decompression, XML and unsafe-markup tests | 2 | Repeatable safety/performance evidence with representative real and synthetic data. | pending |
| LEX-091 🚀 | Optimized startup/search/memory/render benchmarks | 2 | Repeatable safety/performance evidence with representative real and synthetic data. | pending |
| LEX-092 🧪 | Shutdown, cancellation, concurrency and recovery races | 1 | Repeatable safety/performance evidence with representative real and synthetic data. | pending |
| LEX-093 🧯 | Regression fixes and integrity review | 1 | Repeatable safety/performance evidence with representative real and synthetic data. | pending |

### M10 📦 Documentation & distribution

Dependency: M01–M09; weight: 4 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-100 📘 | Accurate README, guides, architecture, format/test docs | 1 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-101 🖥️ | Cargo installation, desktop integration and Arch packaging | 1 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-102 🧾 | License, font and source attribution inventory | 1 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |
| LEX-103 🏷️ | Version, changelog, release assets and checksums | 1 | Reproducible tests plus real-source or native GUI verification as appropriate; preserve data and source provenance. | pending |

### M11 ✅ Acceptance & release

Dependency: M00–M10; weight: 3 points.

| ID | Deliverable | Weight | Objective exit evidence | Status |
|---|---|---:|---|---|
| LEX-110 🏁 | End-to-end real Hyprland UI and dictionary matrix | 1 | Documented owner acceptance and exact reproducible test/release evidence. | pending |
| LEX-111 🧷 | Exact commit, CI, docs, packaging and blocker audit | 1 | Documented owner acceptance and exact reproducible test/release evidence. | pending |
| LEX-112 🚢 | Owner-accepted release; explicit publication/tag authorization | 1 | Documented owner acceptance and exact reproducible test/release evidence. | pending |

## Release acceptance matrix

- [ ] A01 — Clean Linux install and native Hyprland launch without a dictionary server
- [ ] A02 — DICT and compressed DICT/FreeDict lookup against actual Arch resource
- [ ] A03 — English Wiktionary Spanish entries give Spanish headwords with English senses
- [ ] A04 — Spanish Wiktionary provides Spanish definitions
- [ ] A05 — English-to-English lookup with source attribution
- [ ] A06 — Selected dictionaries and language pair exclude unintended sources
- [ ] A07 — Repeat import avoids duplicate entries; damaged or interrupted import keeps valid data
- [ ] A08 — Unicode and accents, ordered multiple senses and homographs
- [ ] A09 — Keyboard navigation, copy and readable focus
- [ ] A10 — System/Light/Dark manual and external theme switching on Hyprland
- [ ] A11 — Real StarDict dictionary including variants, and FreeDict TEI
- [ ] A12 — Performance and memory budgets met in an optimized measured build
- [ ] A13 — CI, documentation, licensing, packaging and actual install tests pass
- [ ] A14 — Owner approval and explicitly authorized release of exact accepted commit

## Performance budgets (unverified targets)

- Existing index startup <3 seconds in optimized build.
- Search p95 <150ms for ~100k headwords and <250ms for ~1m; record device, data and cache state.
- Large imports must use bounded RAM, remain cancellable, and not freeze GUI.
- No idle continuous indexing or loss of last-known-good index after failure.

## Scope guard

Excluded: online server/account, telemetry, web/mobile app, automatic OS package install, Anki, AI definitions, and universal proprietary formats. Hunspell spelling is an optional later extension; MDict/DSL/SLOB/ZIM require explicit new scope. Do not invent extra milestones on 'continue'. Rebaseline honestly only for discovered original-scope work.

## Checkpoints

- 2026-11-09: Repository inspected. Only LICENSE and .gitignore on main. Inception contracts accepted; documentation authored for M00 items 001–003; first bootstrap pending.

- 2026-11-09: Commits `caab949` (native eframe shell + initial CI), `0850416` (fmt repair), `8900eda` (initial Cargo.lock); CI run [42743338343](https://github.com/sguzman/lexiva/actions/runs/42743338343) passed fmt, check, Clippy and 2 unit tests. LEX-010/012/013 verified. GUI launch not tested.

- 2026-11-09 / 2026-11-10 UTC: Implemented `src/model.rs` (nested senses, language/provenance, metadata), `src/storage.rs` and `src/schema.sql` (SQLite v1 + FTS5 source-aware catalog). CI [42746299152](https://github.com/sguzman/lexiva/actions/runs/42746299152) passed fmt, check, Clippy and 11 tests (0 failures). Includes transactional batch rollback, homograph/source isolation, FTS replacement and future schema refusal; lockfile commit `a4ea52d`. LEX-020/021/022 verified; LEX-023/024 pending. Inception dependency/data license risks documented at [DEPENDENCIES](DEPENDENCIES.md) (LEX-004 verified).

- 2026-11-09 / 2026-11-10 UTC: Schema v2 staging/atomic publication and v1→v2 migration were implemented and tested; [CI 42751965002](https://github.com/sguzman/lexiva/actions/runs/42751965002) passed 24 tests, fmt/check/strict Clippy. Cancellation, duplicate rejection, source isolation, 128-record batching, 4 MiB per-record safeguards, on-disk abandoned staging, non-destructive corruption refusal and FTS rebuild tested. LEX-023 and LEX-024 verified; M02 completed. No real dictionary importer or GUI runtime validated yet.

- 2026-11-10: Implemented read-only Arch/system and user-directory GNU DICT discovery plus `.index`/plain `.dict` streaming reader with overflow/size guards, administrative entry exclusion and source-attributed output into staged SQLite imports. CI [42832705560](https://github.com/sguzman/lexiva/actions/runs/42832705560) passed 30 tests (6 new), fmt/check/Clippy. LEX-030/031 verified on synthetic fixtures. Compressed `.dict.dz`, real installed FreeDict QA and language/markup interpretation are not yet complete.

- 2026-11-10: Added bounded dictzip version-1 `RA` gzip-extra-header and random-access raw-DEFLATE decoder, retaining read-only source files. Added a generated, gzip-valid synthetic `.dict.dz` fixture with reads across chunks, invalid RA tests, and SQLite import integration. [CI 42835023624](https://github.com/sguzman/lexiva/actions/runs/42835023624) passed fmt/check/Clippy and 33 tests (0 failed). LEX-032 verified. Authentic FreeDict cross-check, content metadata and full gzip CRC checking remain pending.

- 2026-11-10: Added DICT `00-database-*` administrative record extraction, source/format provenance, conservative FreeDict ISO639-3 filename mapping (e.g. spa→es, eng→en), verbatim articles and cautiously recognized POS abbreviations. No guessed data license. Installed Ubuntu 24.04 `dict-freedict-spa-eng` and `dict-freedict-eng-spa` in CI and cross-checked **every indexed lexical article** against independent `gzip -dc` output, then imported both sources into SQLite/FTS5. [CI 42838302466](https://github.com/sguzman/lexiva/actions/runs/42838302466) passed fmt/check/Clippy, 36 unit tests and 1 real package test: spa-eng 4,507 records (193,471 uncompressed bytes), eng-spa 5,912 records (204,192 bytes). LEX-033/034 verified. M03 complete. This does not count as user-machine Arch or GUI QA.

- 2026-11-10: Implemented edition-explicit Wiktextract `.jsonl` and `.jsonl.gz` streaming adapter, BCP-47-shaped word language filter, ordered sense/gloss labels, examples and translations, pronunciation, inflection, etymology, relations, and full raw structured record provenance. Bounded 2 MiB input records integrate with existing 128-record staged SQLite publisher. [CI 42845015317](https://github.com/sguzman/lexiva/actions/runs/42845015317) passed fmt/check/Clippy, 45 regular unit tests and 1 FreeDict package test. LEX-040/041/042 verified on documented-shape synthetic inputs; LEX-043/044 remain pending for actual large Wiktionary dumps, semantic variants and measured resource usage.

- 2026-11-10: CI [42855279348](https://github.com/sguzman/lexiva/actions/runs/42855279348) passed the 150k streaming JSONL stress fixture: 120k selected Spanish entries, 2,070,248 compressed input bytes, 266,715,136 SQLite bytes after first import and 266,797,056 bytes after reimport, VmHWM 15,132 KiB, 23.37s first import and 24.13s reimport on GitHub-hosted Ubuntu runner. A cancelled replacement retained all 120k published entries with no stale staging; FTS and entry IDs revalidated. Separately, downloaded unbundled original Kaikki raw per-word JSONL: English Wiktionary `casás` (1 record, 606 bytes, English glosses) and Spanish Wiktionary `caso` (2 records, 9,056 bytes, Spanish glosses); independent parsed JSON matched retained raw metadata and SQLite searches. 45 normal tests + 4 targeted tests; all green. LEX-043/044 verified, M04 complete to scoped tests. No 24.8GB full dump, native GUI or production-scale profiling claim.
