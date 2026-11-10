# GNU DICT adapter — current implementation

The adapter lives in `src/dict.rs` and is **not yet wired into the GUI**. The implementation can be called by a Rust integration consumer to stream records directly into `DictionaryStore::import_stream`.

## Authoritative format notes

The [dictd database-format manual](https://man.archlinux.org/man/dictd.8) describes a tab-delimited `.index` record with the headword, base-64 encoded byte offset and base-64 byte length. These numbers refer to the uncompressed flat data stream; they are **not** line numbers, UTF-8 character offsets, nor RFC 4648 whole-byte encoded values. The [ArchWiki dictd guide](https://wiki.archlinux.org/title/Dictd) uses paths such as `/usr/share/dictd/eng-spa.index` and `eng-spa.dict.dz`.

## Implemented

- **Non-recursive read-only discovery** of paired `.index`/(`.dict` or `.dict.dz`) resources in `/usr/share/dictd`, `/usr/local/share/dictd` and explicitly selected paths. Absent directories do not cause failure, while unreadable directories produce an error. Discovery order is deterministic.
- **Uncompressed** `.dict` reading with tab-delimited `.index` parsing, checked base-64 integers, byte-accurate seeks, exact bounded reads, 16 KiB index-line limit and 4 MiB article limit.
- Plain-text body preservation as a single sense, attached to the caller-supplied dictionary source, headword language and definition language. Language or grammatical metadata is **never inferred from the filename**. The original index line and byte offset/length are retained.
- Skip `00-database-*` administrative records from headword results. Domain-specific metadata extraction and rich article markup decoding remain unfinished.
- Iterator emits an error and stops on malformed/invalid indexes; a caller using the transactional import API retains the last published dictionary.
- Verified synthetic tests for Unicode words, homographs, index corruption, overflow, discovered files and source-scoped SQLite insertion.

## Limitations and next steps

- Most FreeDict packages provide **dictzip** `.dict.dz` files. The reader currently detects these but refuses to parse them; bounded random-access dictzip implementation is task **LEX-032**.
- Preserve detailed FreeDict editorial metadata, language direction and article formatting under **LEX-033**.
- Run actual Arch-installed FreeDict/DICT sources through source discovery and the import/search pipeline and compare representative definitions with the input files under **LEX-034**.
- The GUI source manager, filters and definition reader are independent upcoming tasks M05/M06.

**Verification:** [CI run 42832705560](https://github.com/sguzman/lexiva/actions/runs/42832705560): 30 tests, 0 failures, successful formatting, compile and Clippy. This is synthetic adapter coverage, not real-package QA.
