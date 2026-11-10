# Lexiva — Dependency and dictionary-data risk record

Reviewed at the first backend addition (2026-11-09). This is an inception-level decision record, **not** a final transitive dependency audit or redistribution approval.

## Runtime crates

| Crate | Purpose | Published terms and toolchain | Decision / operational cost |
| --- | --- | --- | --- |
| [eframe 0.37.0](https://docs.rs/crate/eframe/0.37.0) | Native egui UI, input and persistence | MIT OR Apache-2.0; crate declares Rust 1.95 | Keep native Linux target and pinned version. Significant GPU/windowing dependency tree; actual Hyprland runtime validation remains required. |
| [rusqlite 0.41.1](https://docs.rs/crate/rusqlite/0.41.1) | SQLite transactions and FTS5 index | MIT; crate declares Rust 1.89. SQLite is public domain. | Use **bundled** SQLite for predictable FTS5 and no requirement for a system SQLite service; native C compilation is required. Do not use SQLCipher. |
| [serde 1.0](https://serde.rs/derive.html) | Typed model serialization | MIT OR Apache-2.0 | Keep original source metadata in explicitly typed fields plus a JSON value. |
| [serde_json 1.0](https://docs.rs/serde_json/latest/serde_json/) | Portable entry payload serialization | MIT OR Apache-2.0 | Treat arbitrary untrusted JSON as bounded import input. Typed round-tripping is not byte-for-byte preservation of original JSON syntax. |

Cargo.lock pins the actual transitive resolutions and is tracked for the application. Runtime source dependencies are not automatically equivalent to licensing of imported dictionary *data*. The MIT project license applies to original Lexiva code, not to external imported data.

## Format and redistribution risks

- **FreeDict / GNU DICT:** [FreeDict's licensing notes](https://freedict.org/documentation/) say that each individual dictionary has its own terms in its TEI header, with many under a GPL license. Read installed files as external sources; do not bundle dictionary payloads before verifying the actual source terms, notices and distribution requirements.
- **Wiktionary / Wiktextract:** [Wikimedia CC BY-SA 4.0](https://foundation.wikimedia.org/wiki/Legal:Text_of_the_Creative_Commons_Attribution-ShareAlike_4.0_International_License) requires attribution and share-alike when applicable. [Kaikki raw exports](https://kaikki.org/dictionary/rawdata.html) have edition/dump provenance and are enormous. Retain the exact edition and dump origin; don't assume every edition's word/definition language is the same.
- **StarDict:** the file format does not grant copyright rights to every dictionary offered online. Support user-supplied local resources without redistributing unlicensed commercial dictionary content.
- **FreeDict TEI:** rich structured entries may carry different language tags and granular license metadata. Parse safely and preserve source terms instead of normalizing them away.
- **Fonts:** personal machine installation does not establish permission to bundle font files. Use system fonts and fallbacks until each font's redistribution terms are reviewed.

## Importer, storage and GUI risks

- Source file parsing is a trust boundary: validate offsets, lengths, decompression ratios, file size, XML entities, malformed JSON and nested structures; no raw HTML execution.
- Very large Wiktextract imports must stream, bound memory, support cancel/progress and commit by atomic publication; the current initial batch replacement is **not** the finished streaming pipeline.
- SQLite schema upgrades must detect newer versions and refuse unexpected changes. Crash recovery, migration and corruption handling remain unverified.
- GUI runtime success, font selection, Wayland handling and live System theme updates cannot be inferred from passing CI compilation.
- A final software bill of materials, dependency security review and all redistribution notices remain release requirements (M10).

These decisions are to be revisited when introducing a new crate or distributing dictionary or font assets.
