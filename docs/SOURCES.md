# Source discovery and local dictionary management

**Scope:** All M06 tasks LEX-060–063 implemented and verified to automated CI scope in [run 42880153881](https://github.com/sguzman/lexiva/actions/runs/42880153881). Real target-machine Hyprland GUI QA remains pending.

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

- Background source watching, cryptographic full-file content hash and automatic package-update reimports are not implemented.
- GUI action to discard abandoned staging after process termination is not yet implemented; only the safe storage API supports it.
- Actual Arch-installed FreeDict discovery/import and interactive UI behavior on the owner's Hyprland session (M01 LEX-011).
- Real multi-gigabyte Wiktextract end-to-end GUI resource and disk-full handling.

All imports are opt-in. No source content is implicitly licensed as MIT, copied into the repository, or downloaded automatically.

## Persisted enable/disable and priority (LEX-062)

SQLite schema v3 stores dictionary user preferences in `source_preferences`, separate from original dictionary source metadata. All preexisting sources migrate enabled by default, in deterministic initial order; future imports and metadata updates preserve these preferences.

The **Indexed dictionary sources** list has a persistent **Enabled** checkbox and **↑/↓** controls for explicit priority ordering. Disabled sources disappear from the search dropdown and are filtered out **inside SQL** for both ranked headword searches and spelling suggestions. They retain their indexed entries and can be re-enabled without reimporting. Priority is respected for both source selection presentation and combined search results. A disabled source cannot be found by explicitly passing its source ID to the ranked search API.

The migration preserves existing v1/v2 source IDs, published entries and incomplete staging. Preferences are retained when the database is reopened and when a source is upserted after reindex.

## Explicit source change checks and reindex (LEX-063)

Use **Check source files** to refresh file health without importing anything. The background manager compares the last successful import signature with current source files. DICT fingerprints cover **both** the `.index` and `.dict`/`.dict.dz` files; Wiktextract fingerprints cover the selected local JSONL input. Each signature includes file size, modification time and a bounded FNV-1a sample of the **first and last 8 KiB**, so checking large corpora does not reread them completely. This is an economical change detector, **not a collision-resistant full-file checksum**; an interior edit with identical size and modification time may not be detected.

The indexed-source row shows **Up to date**, **Source changed — reindex**, **Source file missing — cached index retained**, or **Not yet verified — reindex**. An explicit **Reindex** rebuilds the selected source through the existing bounded, atomic importer; errors, missing files and cancellation retain the last published entries and source metadata. No automatic reimport runs after package updates. If a file is restored to the same path, **Check source files** enables reindexing again.

**Remaining limitations:** no background filesystem watcher or package-manager hook, no automatic source relinking after a file has moved to a new path, no cryptographic full-dump hash, no disk-full preflight for enormous JSONL, and no GUI action yet to discard crashed/abandoned staging leases. These are future hardening/UX concerns rather than evidence of M06's narrower automated acceptance.

[CI evidence](https://github.com/sguzman/lexiva/actions/runs/42880153881): 63 regular tests passed, with 4 additional FreeDict/Wiktextract/stress test cases; v2 migration, preference survival across reopen, disabled-source exclusion, priority rank, same-size source data edit, explicit reindex, missing-source preservation and cancelled reindex are included.
