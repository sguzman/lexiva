# GNU DICT / FreeDict adapter — verified file compatibility

**State:** M03 complete for file/discovery/metadata integration. App UI source management and native EndeavourOS/Hyprland runtime QA are separate, unfinished milestones.

The adapter lives in `src/dict.rs`, `src/dict/dictzip.rs` and `src/dict/metadata.rs`. It can stream both plain `.dict` and compressed `.dict.dz` records into `DictionaryStore::import_stream` without a dictd server.

## File reading

- Nonrecursive, read-only discovery in `/usr/share/dictd`, `/usr/local/share/dictd` and explicitly supplied directories.
- `.index` headword and checked DICT base-64 byte offsets/lengths, with an index-line size limit of 16 KiB.
- `.dict` exact range reads and dictzip v1 `.dict.dz` random-access raw-DEFLATE reads, with output bounds and one-chunk cache; article size limited to 4 MiB.
- Truncated entries, invalid offsets and malformed RA headers cannot be silently published to the search index.
- Administrative `00-database-*` records do not appear as normal searchable words. Their short description, long info and URL are captured as original texts where present.

## Language, metadata and formatting

- For recognized FreeDict filenames such as `freedict-spa-eng.index` and `freedict-eng-spa.index`, known ISO 639-3 source/destination codes map to normalized BCP-47 language labels `es` and `en`. The evidence source is retained in metadata. Unrecognized codes and arbitrary dictionary names remain **unknown** instead of guessing language direction.
- The original administrative text, source index/data paths, compression format, per-entry index position and data byte offset/length are retained as structured metadata.
- A known conventional part-of-speech abbreviation in an article heading may be labeled (for example `n` → noun), but articles are otherwise kept as **verbatim text** including indentation, punctuation and line breaks. Complex FreeDict TEI semantics, nested senses and example normalization will be addressed by the dedicated TEI adapter later.
- **No dictionary license is inferred or supplied automatically.** The source and redistributed package may have different terms from Lexiva's MIT code. Redistribution notices must be read from the actual dictionary's license metadata before bundling.

## Real package verification

GitHub Actions on Ubuntu 24.04 installs `dict-freedict-spa-eng`, `dict-freedict-eng-spa` and uses system `gzip -dc` as an independent reference implementation. An ignored-by-default Rust integration test is explicitly invoked after package installation, verifying **every lexical entry** against the independent decompressed byte range; both full dictionaries are also inserted into staged SQLite/FTS5 and queried.

| Package | Headword language | Definition language | Articles | Gzip-decoded bytes |
|---|---|---|---:|---:|
| `dict-freedict-spa-eng` | Spanish `es` | English `en` | 4,507 | 193,471 |
| `dict-freedict-eng-spa` | English `en` | Spanish `es` | 5,912 | 204,192 |

[CI run 42838302466](https://github.com/sguzman/lexiva/actions/runs/42838302466): formatting, compilation, Clippy, 36 normal tests and 1 real-package integration test passed. The packaged data is **not committed or bundled**. Authentic FreeDict content on Ubuntu is verified, but Arch package-specific installation layout and user-machine GUI behavior still require separate operational acceptance.

## Remaining restrictions

- Only dictzip v1 with an `RA` table is accepted; ordinary gzip without random-access metadata and alternative dictzip formats are unsupported.
- The gzip whole-member CRC is not recomputed by the random-access reader; the CI independent `gzip -dc` check covers the tested package inputs only.
- Dictionary licenses and optional proprietary formatting are not automatically known.
- Live GUI source controls, source rescan after package updates and dictionary-language filters belong to M05/M06.

Format references: [dictd file format](https://man.archlinux.org/man/dictd.8), [dictfmt(1)](https://manpages.ubuntu.com/manpages/noble/en/man1/dictfmt.1.html), [FreeDict source/rights documentation](https://freedict.org/documentation/).
