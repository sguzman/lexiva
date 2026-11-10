# Lexiva indexed search and definition reader

**Status:** Implemented native egui search/results/reader shell and tested SQLite ranked-search backend. This is **not yet a complete first-run dictionary application** because the GUI source manager/import action is still M06.

## User-visible flow

1. Lexiva opens a derived SQLite catalog under the XDG data directory. On Linux this is `$XDG_DATA_HOME/lexiva/catalog.sqlite`, or `~/.local/share/lexiva/catalog.sqlite` if XDG_DATA_HOME is not an absolute path.
2. At startup the dictionary selector is deliberately **unselected**. No installed dictionary and no indexed source is searched implicitly; the user can choose a named indexed dictionary or explicitly opt into **All indexed dictionaries**.
3. Typing a word searches through a background SQLite worker. Results show the word, the dictionary title, and part of speech, preserving separate homographs. A visible error is displayed on index or worker errors.
4. Selecting a result displays a source-attributed definition with ordered senses and their glosses, usage examples and translations, IPA, etymology, forms, related terms, provenance and the original dictionary's license status. The Copy definition button puts a plain-text representation on the clipboard, preserving article line breaks.
5. Up/down arrows change the selected result. More complete focus/keyboard history and a real Hyprland interaction test remain pending.

## Indexed query details

`DictionaryStore::search_ranked` first queries FTS5 for exact quoted words and then for a quoted prefix, deduplicating by (source ID, entry ID). Literal exact spellings sort before longer prefixes. The FTS5 `unicode61 remove_diacritics 2` tokenizer supports accent-insensitive Latin word retrieval, while preserving the original lemma display. Optional source ID, headword-language and definition-language filters are implemented in the underlying SQLite search API. The GUI currently exposes only source selection; independent language UI controls are planned for M06. A typo-suggestion system is not yet implemented, so LEX-050 is still pending.

Search results are bounded to 100 records in the backend and 60 in the native worker. The worker coalesces queued keystrokes, performs SQLite work off the egui/UI thread, and rejects stale serial-tagged updates. It currently **does not interrupt an already running SQL statement**, so LEX-054 remains pending.

## Source ownership and limitations

The catalog is derived local data. It must not be mistaken for a dictionary source file, and a brand-new catalog has no indexed dictionary content. The GUI does not yet import GNU DICT/FreeDict or Wiktextract files itself. Source discovery/import, enabled-source persistence, rescan and language selectors are M06. Existing native shell compilation and unit/regression tests do **not** establish runtime behavior on the owner's Arch/Hyprland machine (LEX-011 is pending).

No dictionary content, fonts or non-MIT source licenses are implicitly shipped or granted.

## Evidence

- [CI 42861187298](https://github.com/sguzman/lexiva/actions/runs/42861187298): formatting, native build, Clippy, ranked search and definition copy tests, real Ubuntu FreeDict integration, existing 150k-record and Kaikki regression suite.
- [Storage contract](STORAGE.md), [FreeDict adapter](DICT.md), [Wiktextract adapter](WIKTEXTRACT.md) and [roadmap](ROADMAP.md).
