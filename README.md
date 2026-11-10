# Lexiva

**Lexiva** is a native, offline-first dictionary reader for Linux built with Rust and [egui/eframe](https://github.com/emilk/egui). It aims to make installed Arch dictionaries and open lexical datasets readable in a fast, elegant interface.

> **Status: native shell plus tested lexical storage foundation.** The Rust data model, source-aware SQLite/FTS5 catalog, and eleven unit tests pass CI. Actual dictionary file import, reader/search GUI wiring, and native Hyprland QA are not yet finished.

## Planned features

- Read existing GNU DICT/FreeDict, Wiktionary/Wiktextract, StarDict and FreeDict TEI data without a server.
- Explicitly choose the headword language, definition language and which dictionary sources to query.
- Read numbered senses, examples, IPA and linguistic metadata when present in the source.
- Native keyboard navigation, persistent settings and System/Light/Dark themes.
- Safe, cancellable offline imports with accurate source attribution.

## Developer build

The source builds against Rust 1.95.0. To inspect the early native shell from a checked-out repository, run `cargo run --locked`. This launch path is **not yet tested on the target desktop** and the application does not currently return definitions. Release installation instructions will follow actual on-device acceptance.

## Project documentation

- [Inception and scope](docs/INCEPTION.md)
- [Roadmap and verifiable progress](docs/ROADMAP.md)
- [Architecture](docs/ARCHITECTURE.md)
- [SQLite schema and storage limitations](docs/STORAGE.md)
- [Dependencies, licensing and risks](docs/DEPENDENCIES.md)
- [Source formats](docs/FORMATS.md)
- [Testing and release acceptance](docs/TESTING.md)

## Visual identity

Lexiva uses a deterministic OKLCH identity derived from its project name, with light accent `#9C424B` and dark accent `#EA979B`. See [identity swatches](assets/identity.svg) and the [inception record](docs/INCEPTION.md).

![Lexiva color swatches](assets/identity.svg)

## License

[MIT](LICENSE) for the original software. Imported dictionaries and fonts retain their own independent licenses and are not automatically bundled.
