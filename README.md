# Lexiva

**Lexiva** is a native, offline-first dictionary reader for Linux built with Rust and [egui/eframe](https://github.com/emilk/egui). It aims to make installed Arch dictionaries and open lexical datasets readable in a fast, elegant interface.

> **Status: source management complete to automated CI scope; appearance/accessibility work in progress.** Lexiva offers explicit DICT/JSONL imports, indexed search, persistent source enable/disable and priority, source-change checks, a reader-size control, saved dictionary/language selection, event-driven Darkman integration where available, focus-safe lookup navigation, and saved interface zoom, reduced-motion and high-contrast settings. **Native Hyprland GUI QA, StarDict/TEI support, full-size corpus validation and release acceptance are unfinished.** A reported first-run screenshot revealed an unpainted root viewport (black clear background behind light controls) and clipped edge text; the root-panel/layout correction is committed but still requires native visual confirmation.

## Planned features

- Read existing GNU DICT/FreeDict, Wiktionary/Wiktextract, StarDict and FreeDict TEI data without a server.
- Explicitly choose the headword language, definition language and which dictionary sources to query.
- Read numbered senses, examples, IPA and linguistic metadata when present in the source.
- Native keyboard navigation that respects focused controls, persistent source/language/reader-size settings, an immediate System/Light/Dark selector, and optional interface zoom, reduced motion and high-contrast text. Darkman changes are received without polling where available.
- Safe, cancellable offline imports with accurate source attribution.

## Developer build

The source builds against Rust 1.95.0. The native egui shell has explicit first-use DICT and Wiktextract source import controls, wired to its XDG SQLite catalog. These are CI-tested but not verified through an actual Hyprland GUI session. The launch path has **not yet been verified on the target Hyprland desktop**. The repository's existing CI cannot substitute for native UI QA, and release installation instructions will follow that acceptance. Follow the workstation's established `mbx` coordination for local Cargo builds and use `cargo install --path` when installation is authorized.

## Project documentation

- [Inception and scope](docs/INCEPTION.md)
- [Roadmap and verifiable progress](docs/ROADMAP.md)
- [Architecture](docs/ARCHITECTURE.md)
- [SQLite schema and storage limitations](docs/STORAGE.md)
- [Dependencies, licensing and risks](docs/DEPENDENCIES.md)
- [Source formats](docs/FORMATS.md)
- [GNU DICT adapter status and limits](docs/DICT.md)
- [Wiktextract streaming, editions and limits](docs/WIKTEXTRACT.md)
- [Indexed search and native reader state](docs/SEARCH.md)
- [Source manager, import and removal contract](docs/SOURCES.md)
- [Testing and release acceptance](docs/TESTING.md)

## Visual identity

Lexiva uses a deterministic OKLCH identity derived from its project name, with light accent `#9C424B` and dark accent `#EA979B`. See [identity swatches](assets/identity.svg) and the [inception record](docs/INCEPTION.md).

![Lexiva color swatches](assets/identity.svg)

## License

[MIT](LICENSE) for the original software. Imported dictionaries and fonts retain their own independent licenses and are not automatically bundled.
