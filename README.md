# Lexiva

**Lexiva** is a native, offline-first dictionary reader for Linux built with Rust and [egui/eframe](https://github.com/emilk/egui). It aims to make installed Arch dictionaries and open lexical datasets readable in a fast, elegant interface.

> **Status: native search/reader wired to the verified dictionary index, with source setup still pending.** The egui search interface now displays attributed source results and ordered definitions from a pre-indexed SQLite catalog; the FreeDict reader/copy path and Wiktextract importer are covered in CI. **Fresh-install source indexing, typo suggestions, full keyboard/history, Hyprland runtime QA and multi-gigabyte dump acceptance are not yet complete.**

## Planned features

- Read existing GNU DICT/FreeDict, Wiktionary/Wiktextract, StarDict and FreeDict TEI data without a server.
- Explicitly choose the headword language, definition language and which dictionary sources to query.
- Read numbered senses, examples, IPA and linguistic metadata when present in the source.
- Native keyboard navigation, persistent settings and System/Light/Dark themes.
- Safe, cancellable offline imports with accurate source attribution.

## Developer build

The source builds against Rust 1.95.0. The native egui shell now searches and reads existing entries in its XDG SQLite catalog, but first-use discovery/import controls are not ready. The launch path has **not yet been verified on the target Hyprland desktop**. The repository's existing CI cannot substitute for native UI QA, and release installation instructions will follow that acceptance. Follow the workstation's established `mbx` coordination for local Cargo builds and use `cargo install --path` when installation is authorized.

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
- [Testing and release acceptance](docs/TESTING.md)

## Visual identity

Lexiva uses a deterministic OKLCH identity derived from its project name, with light accent `#9C424B` and dark accent `#EA979B`. See [identity swatches](assets/identity.svg) and the [inception record](docs/INCEPTION.md).

![Lexiva color swatches](assets/identity.svg)

## License

[MIT](LICENSE) for the original software. Imported dictionaries and fonts retain their own independent licenses and are not automatically bundled.
