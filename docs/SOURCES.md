# Source discovery and local dictionary management

**Scope:** M06 LEX-060/061 implemented and verified by [CI 42875563900](https://github.com/sguzman/lexiva/actions/runs/42875563900). Real target-machine Hyprland GUI QA and M06 LEX-062/063 remain pending.

## Native interface

The collapsible **Dictionary sources — discover and import** section offers an explicit **Scan DICT files** action. It examines `/usr/share/dictd` and `/usr/local/share/dictd` and may include one additional user-entered directory. Discovery is nonrecursive and does not install packages, contact the network, or start dictd. A dictionary pair requires a `.index` file and a corresponding `.dict` or dictzip `.dict.dz` data file.

Each detected dictionary shows its title, recognized headword/definition language direction if known, and current indexed record count. **Import / reindex** launches a dedicated background worker, preserving an older published copy unless the new staged import finishes. Imported sources appear in the existing dictionary selector and become the selected search source only after successful publication.

The separate **Import Wiktextract JSONL / JSONL.gz** control accepts a local filesystem path, explicit **English** or **Spanish** Wiktionary edition (the definition language), and an optional `lang_code` headword filter (default `es`). An empty word filter means all languages and can be expensive on giant dumps; the selected edition is never guessed from the filename. No Kaikki files are bundled or automatically downloaded.

A live staged-entry count and cancel control are shown during work. Cancellation retains previously published entries. Imports are limited by the existing 128-entry staging batch, per-entry size checks and per-line JSONL checks. An abrupt process crash may leave a staging lease requiring explicit cleanup (not yet available through GUI).

The **Indexed dictionary sources** list includes the existing source record and entry count and a **Remove local index** action. This removes only derived SQLite source/entry records, with triggers updating the search index. **Installed dictionary files and input JSONL are not deleted or modified.** An indexed source with a missing path can be flagged while its published definitions remain available.

## Language controls

Search includes two independent, optional text fields: **Word language** and **Definition language**. Empty means no language restriction; `es` and `en` are typical examples. These restrictions are independent of the selected dictionary and passed into both ranked results and bounded spelling suggestions.

## Local storage and source identity

The catalog is at `$XDG_DATA_HOME/lexiva/catalog.sqlite` when that variable is an absolute directory, otherwise `~/.local/share/lexiva/catalog.sqlite`. Source IDs use a deterministic hash of the supplied path; for Wiktextract, edition and headword filter are also included, so different interpretations of the same file do not silently overwrite one another. Source provenance retains the original full path. SQLite is configured with a bounded busy timeout for the separate search and import connections.

## Not yet implemented or verified

- **LEX-062:** Persistently enabled/disabled sources and user-controlled priority ordering.
- **LEX-063:** Content-fingerprint change detection, package upgrade rescan prompts, missing-file recovery and explicit discard of abandoned staging from the interface.
- Actual Arch-installed FreeDict discovery/import and interactive UI behavior on the owner's Hyprland session (M01 LEX-011).
- Real multi-gigabyte Wiktextract end-to-end GUI resource and disk-full handling.

All imports are opt-in. No source content is implicitly licensed as MIT, copied into the repository, or downloaded automatically.
